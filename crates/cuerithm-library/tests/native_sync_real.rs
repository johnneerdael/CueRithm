//! End-to-end synchronization against a copy of a real rekordbox 7 library.
//!
//! Skipped unless `CUERITHM_REKORDBOX_COPY` names a copied `master.db` (with its WAL/SHM beside it). The copy
//! is snapshotted first, and the snapshot stands in for the live library, so no original is opened for
//! writing. One track's audio file is read to hash it for the import; nothing from the library is printed.
use cuerithm_library::*;
use cuerithm_media::CancellationToken;
use cuerithm_rekordbox::*;
use cuerithm_smartlists::PlaylistKind;
use std::{collections::BTreeMap, path::PathBuf};

fn key() -> DatabaseKey {
    let cancel = CancellationToken::default();
    let paths = DiscoveryPaths::current().unwrap();
    let found = discover_targets(&paths, &cancel).unwrap();
    let target = found
        .installations
        .first()
        .expect("a rekordbox installation");
    let source = NativeKeySource::for_installation(target, paths.platform).unwrap();
    resolve_database_key(
        &source,
        &paths.agent_options_file().unwrap(),
        Default::default(),
        &cancel,
    )
    .unwrap()
}
fn read(path: &std::path::Path, key: &DatabaseKey) -> RekordboxCollection {
    ReadOnlyDatabase::open(path, key)
        .unwrap()
        .read_collection(Default::default(), &CancellationToken::default())
        .unwrap()
}

#[test]
fn a_local_edit_is_previewed_written_published_acknowledged_and_not_repeated() {
    let Some(copy) = std::env::var_os("CUERITHM_REKORDBOX_COPY").map(PathBuf::from) else {
        eprintln!("skipped: CUERITHM_REKORDBOX_COPY is not set");
        return;
    };
    let key = key();
    let cancel = CancellationToken::default();
    let scratch = tempfile::tempdir().unwrap();
    let live = ReadOnlyDatabase::open(&copy, &key)
        .unwrap()
        .capture(scratch.path(), &key, &cancel)
        .unwrap();
    let before = read(&live.path, &key);
    let track = before
        .tracks
        .iter()
        .find(|t| {
            t.importable_local()
                && t.source
                    .resolved_path
                    .as_ref()
                    .is_some_and(|p| std::path::Path::new(p).is_file())
        })
        .expect("a track whose audio file exists");
    let content_id = track.identity.content_id.clone();
    let library_dir = tempfile::tempdir().unwrap();
    let lib = Library::open(&library_dir.path().join("library")).unwrap();
    let owners: BTreeMap<_, _> = native_writable_fields()
        .into_iter()
        .map(|f| (f.to_owned(), NativeFieldOwner::Local))
        .collect();
    let report = lib
        .import_rekordbox_collection(
            &before,
            NativeImportOptions {
                settings_revision: lib.settings().unwrap().revision,
                content_ids: Some([content_id.clone()].into()),
                initial_owners: owners,
            },
            &cancel,
        )
        .unwrap();
    let link = lib
        .native_track_link(&report.target_id, &content_id)
        .unwrap();
    let local_id = link.local_track_id.clone().expect("a linked local track");
    // Nothing differs yet, so there is nothing to write.
    let work = tempfile::tempdir().unwrap();
    let backups = tempfile::tempdir().unwrap();
    let idle = lib
        .native_sync_prepare(
            &live.path,
            &key,
            work.path(),
            NativeSyncOptions::default(),
            &cancel,
        )
        .unwrap();
    assert!(idle.plan.tracks.is_empty(), "{:?}", idle.plan);
    // Edit the track in Cuerithm.
    lib.set_track_text_for_test(&local_id, "Cuerithm edited title", "Cuerithm edited genre")
        .unwrap();
    lib.add_track_tag_for_test(&local_id, "Cuerithm Acceptance", "Written")
        .unwrap();
    let prepared = lib
        .native_sync_prepare(
            &live.path,
            &key,
            work.path(),
            NativeSyncOptions::default(),
            &cancel,
        )
        .unwrap();
    let planned = &prepared.plan.tracks[0];
    assert_eq!(planned.content_id, content_id);
    assert!(
        planned.fields.contains(&"title".to_owned())
            && planned.fields.contains(&"genre".to_owned())
    );
    // Apply.
    let run = lib
        .native_sync_apply(
            &live.path,
            &key,
            work.path(),
            backups.path(),
            NativeSyncOptions::default(),
            &cancel,
        )
        .unwrap();
    assert_eq!(run.state, "committed");
    assert_eq!(run.usn_after, Some(run.usn_before.unwrap() + 1));
    let after = read(&live.path, &key);
    let written = after
        .tracks
        .iter()
        .find(|t| t.identity.content_id == content_id)
        .unwrap();
    assert_eq!(written.metadata.title, "Cuerithm edited title");
    assert_eq!(written.metadata.genre, "Cuerithm edited genre");
    assert_eq!(written.metadata.artist, track.metadata.artist);
    // The new MyTag is on the track and the track's existing rekordbox tags were kept.
    let names = |c: &RekordboxCollection, t: &ImportedTrack| {
        let mut out: Vec<(String, String)> = t
            .tag_ids
            .iter()
            .filter_map(|id| c.tags.iter().find(|x| x.id == *id))
            .map(|tag| {
                let category = c
                    .tags
                    .iter()
                    .find(|x| Some(&x.id) == tag.parent_id.as_ref())
                    .map(|x| x.name.clone())
                    .unwrap_or_default();
                (category, tag.name.clone())
            })
            .collect();
        out.sort();
        out
    };
    let mut expected = names(&before, track);
    expected.push(("Cuerithm Acceptance".into(), "Written".into()));
    expected.sort();
    assert_eq!(names(&after, written), expected);
    for other in before
        .tracks
        .iter()
        .filter(|t| t.identity.content_id != content_id)
    {
        let now = after
            .tracks
            .iter()
            .find(|t| t.identity.content_id == other.identity.content_id)
            .unwrap();
        assert_eq!(now.raw_hash, other.raw_hash, "unrelated track changed");
    }
    // The written fields are now an agreed baseline, so the next sync has nothing to do.
    let link = lib
        .native_track_link(&report.target_id, &content_id)
        .unwrap();
    assert!(link.fields["title"].acknowledged && link.fields["genre"].acknowledged);
    assert!(!link.fields["label"].acknowledged);
    let again = lib
        .native_sync_apply(
            &live.path,
            &key,
            work.path(),
            backups.path(),
            NativeSyncOptions::default(),
            &cancel,
        )
        .unwrap();
    assert_eq!(again.state, "nothing_to_write");
    assert_eq!(
        read(&live.path, &key).identity.local_update_count,
        after.identity.local_update_count
    );
    // A later edit in rekordbox to a field Cuerithm also edited is a conflict that is reported, not written.
    lib.set_track_text_for_test(
        &local_id,
        "Edited again in Cuerithm",
        "Cuerithm edited genre",
    )
    .unwrap();
    {
        let current = read_row_fingerprint(&live.path, &key, &content_id);
        let mut writer = WriteDatabase::open(&live.path, &key).unwrap();
        writer
            .apply(
                &WritePlan {
                    tracks: vec![TrackWrite {
                        content_id: content_id.clone(),
                        expected_fingerprint: current,
                        fields: ContentFields {
                            title: Some("Edited in rekordbox".into()),
                            ..Default::default()
                        },
                        cues: None,
                        tags: None,
                    }],
                    ..Default::default()
                },
                &cancel,
            )
            .unwrap();
    }
    let conflicted = lib
        .native_sync_apply(
            &live.path,
            &key,
            work.path(),
            backups.path(),
            NativeSyncOptions::default(),
            &cancel,
        )
        .unwrap();
    assert_eq!(conflicted.state, "nothing_to_write");
    assert!(conflicted.skipped >= 1);
    let after_conflict = lib
        .native_sync_prepare(
            &live.path,
            &key,
            work.path(),
            NativeSyncOptions::default(),
            &cancel,
        )
        .unwrap();
    assert!(
        after_conflict
            .plan
            .skipped
            .iter()
            .any(|s| s.field == "title" && s.decision == FieldDecision::Conflict),
        "{:?}",
        after_conflict.plan.skipped
    );
    let kept = read(&live.path, &key);
    assert_eq!(
        kept.tracks
            .iter()
            .find(|t| t.identity.content_id == content_id)
            .unwrap()
            .metadata
            .title,
        "Edited in rekordbox"
    );
    let history = lib.native_sync_history().unwrap();
    assert_eq!(history.iter().filter(|r| r.state == "committed").count(), 1);
    assert_eq!(history.len(), 3);
}

fn read_row_fingerprint(path: &std::path::Path, key: &DatabaseKey, content_id: &str) -> String {
    read(path, key)
        .tracks
        .iter()
        .find(|t| t.identity.content_id == content_id)
        .unwrap()
        .raw_hash
        .clone()
}

fn playlist_edit(lib: &Library, edit: PlaylistEdit) -> PlaylistChange {
    let revision = lib.playlist_state().unwrap().revision;
    lib.apply_playlist_edit(edit, revision).unwrap()
}
fn create(kind: PlaylistKind, name: &str, parent: Option<&str>) -> PlaylistEdit {
    PlaylistEdit::Create {
        id: None,
        parent_id: parent.map(str::to_owned),
        kind,
        name: name.into(),
        definition: None,
    }
}

#[test]
fn playlists_are_mirrored_by_path_managed_by_cuerithm_and_never_taken_over() {
    let Some(copy) = std::env::var_os("CUERITHM_REKORDBOX_COPY").map(PathBuf::from) else {
        eprintln!("skipped: CUERITHM_REKORDBOX_COPY is not set");
        return;
    };
    let key = key();
    let cancel = CancellationToken::default();
    let scratch = tempfile::tempdir().unwrap();
    let live = ReadOnlyDatabase::open(&copy, &key)
        .unwrap()
        .capture(scratch.path(), &key, &cancel)
        .unwrap();
    let before = read(&live.path, &key);
    let track = before
        .tracks
        .iter()
        .find(|t| {
            t.importable_local()
                && t.source
                    .resolved_path
                    .as_ref()
                    .is_some_and(|p| std::path::Path::new(p).is_file())
        })
        .expect("a track whose audio file exists");
    let content_id = track.identity.content_id.clone();
    let library_dir = tempfile::tempdir().unwrap();
    let lib = Library::open(&library_dir.path().join("library")).unwrap();
    let report = lib
        .import_rekordbox_collection(
            &before,
            NativeImportOptions {
                settings_revision: lib.settings().unwrap().revision,
                content_ids: Some([content_id.clone()].into()),
                initial_owners: Default::default(),
            },
            &cancel,
        )
        .unwrap();
    let local_track = lib
        .native_track_link(&report.target_id, &content_id)
        .unwrap()
        .local_track_id
        .unwrap();
    let (work, backups) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let sync = || {
        lib.native_sync_apply(
            &live.path,
            &key,
            work.path(),
            backups.path(),
            NativeSyncOptions::default(),
            &cancel,
        )
        .unwrap()
    };
    // A folder with a playlist holding the linked track, plus a playlist whose name already exists in rekordbox.
    let existing_name = before
        .playlists
        .iter()
        .find(|p| {
            p.deleted == Some(false) && p.attribute == 0 && p.parent_id.as_deref() == Some("root")
        })
        .map(|p| p.name.clone());
    let folder = playlist_edit(
        &lib,
        create(PlaylistKind::Folder, "Cuerithm Acceptance", None),
    )
    .created_ids[0]
        .clone();
    let playlist = playlist_edit(
        &lib,
        create(PlaylistKind::Playlist, "Linked", Some(&folder)),
    )
    .created_ids[0]
        .clone();
    playlist_edit(
        &lib,
        PlaylistEdit::AddMembers {
            id: playlist.clone(),
            track_ids: vec![local_track.clone()],
        },
    );
    let prepared = lib
        .native_sync_prepare(
            &live.path,
            &key,
            work.path(),
            NativeSyncOptions::default(),
            &cancel,
        )
        .unwrap();
    assert_eq!(prepared.playlists.writes(), 2, "{:?}", prepared.playlists);
    let run = sync();
    assert_eq!(run.state, "committed");
    let after = read(&live.path, &key);
    let rb_folder = after
        .playlists
        .iter()
        .find(|p| p.name == "Cuerithm Acceptance" && p.attribute == 1)
        .unwrap();
    let rb_playlist = after
        .playlists
        .iter()
        .find(|p| p.name == "Linked" && p.attribute == 0)
        .unwrap();
    assert_eq!(
        rb_playlist.parent_id.as_deref(),
        Some(rb_folder.id.as_str())
    );
    assert_eq!(
        after.playlist_local_track_ids(&rb_playlist.id),
        vec![content_id.clone()]
    );
    assert_eq!(after.playlists.len(), before.playlists.len() + 2);
    // Repeating changes nothing.
    assert_eq!(sync().state, "nothing_to_write");
    // A playlist CueRithm wrote is removed from rekordbox when it is removed here.
    let temp = playlist_edit(
        &lib,
        create(PlaylistKind::Playlist, "Temporary", Some(&folder)),
    )
    .created_ids[0]
        .clone();
    playlist_edit(
        &lib,
        PlaylistEdit::AddMembers {
            id: temp.clone(),
            track_ids: vec![local_track.clone()],
        },
    );
    assert_eq!(sync().state, "committed");
    let rb_temp = |c: &RekordboxCollection| {
        c.playlists
            .iter()
            .find(|p| p.name == "Temporary" && p.deleted == Some(false))
            .map(|p| p.id.clone())
    };
    assert!(rb_temp(&read(&live.path, &key)).is_some());
    playlist_edit(&lib, PlaylistEdit::Remove { id: temp });
    let removal = lib
        .native_sync_prepare(
            &live.path,
            &key,
            work.path(),
            NativeSyncOptions::default(),
            &cancel,
        )
        .unwrap();
    assert!(
        removal
            .playlists
            .items
            .iter()
            .any(|i| i.action == PlaylistAction::Remove
                && i.path.last().map(String::as_str) == Some("Temporary"))
    );
    assert_eq!(sync().state, "committed");
    assert!(rb_temp(&read(&live.path, &key)).is_none());
    assert!(
        read(&live.path, &key)
            .playlists
            .iter()
            .any(|p| p.name == "Linked" && p.deleted == Some(false))
    );
    // A playlist edited in rekordbox is reported as changed, not overwritten.
    {
        let mut writer = WriteDatabase::open(&live.path, &key).unwrap();
        writer
            .apply(
                &WritePlan {
                    playlists: vec![PlaylistWrite {
                        path: vec!["Cuerithm Acceptance".into(), "Linked".into()],
                        kind: cuerithm_rekordbox::PlaylistKind::Playlist,
                        position: 1,
                        tracks: vec![],
                        expected_members: None,
                        rules: None,
                    }],
                    ..Default::default()
                },
                &cancel,
            )
            .unwrap();
    }
    let changed = lib
        .native_sync_prepare(
            &live.path,
            &key,
            work.path(),
            NativeSyncOptions::default(),
            &cancel,
        )
        .unwrap();
    assert!(
        changed
            .playlists
            .items
            .iter()
            .any(|i| i.path.last().map(String::as_str) == Some("Linked")
                && i.action == PlaylistAction::SkipChanged)
    );
    assert_eq!(sync().state, "nothing_to_write");
    assert!(
        read(&live.path, &key)
            .playlist_local_track_ids(&rb_playlist.id)
            .is_empty()
    );
    // A playlist that already exists in rekordbox under the same name is not taken over.
    if let Some(name) = existing_name {
        let clash =
            playlist_edit(&lib, create(PlaylistKind::Playlist, &name, None)).created_ids[0].clone();
        playlist_edit(
            &lib,
            PlaylistEdit::AddMembers {
                id: clash,
                track_ids: vec![local_track.clone()],
            },
        );
        let prepared = lib
            .native_sync_prepare(
                &live.path,
                &key,
                work.path(),
                NativeSyncOptions::default(),
                &cancel,
            )
            .unwrap();
        assert!(
            prepared
                .playlists
                .items
                .iter()
                .any(|i| i.path == vec![name.clone()] && i.action == PlaylistAction::SkipExisting)
        );
        sync();
        let kept_members = |c: &RekordboxCollection| {
            let p = c
                .playlists
                .iter()
                .find(|p| {
                    p.name == name && p.attribute == 0 && p.parent_id.as_deref() == Some("root")
                })
                .unwrap();
            c.playlist_local_track_ids(&p.id)
        };
        assert_eq!(kept_members(&read(&live.path, &key)), kept_members(&before));
    }
}

#[test]
fn smart_lists_are_native_when_every_rule_maps_and_materialized_otherwise() {
    use cuerithm_smartlists::{Operator, RuleValue, SmartDefinition, SmartRule};
    let Some(copy) = std::env::var_os("CUERITHM_REKORDBOX_COPY").map(PathBuf::from) else {
        eprintln!("skipped: CUERITHM_REKORDBOX_COPY is not set");
        return;
    };
    let key = key();
    let cancel = CancellationToken::default();
    let scratch = tempfile::tempdir().unwrap();
    let live = ReadOnlyDatabase::open(&copy, &key)
        .unwrap()
        .capture(scratch.path(), &key, &cancel)
        .unwrap();
    let before = read(&live.path, &key);
    let track = before
        .tracks
        .iter()
        .find(|t| {
            t.importable_local()
                && t.source
                    .resolved_path
                    .as_ref()
                    .is_some_and(|p| std::path::Path::new(p).is_file())
        })
        .expect("a track whose audio file exists");
    let content_id = track.identity.content_id.clone();
    let library_dir = tempfile::tempdir().unwrap();
    let lib = Library::open(&library_dir.path().join("library")).unwrap();
    let report = lib
        .import_rekordbox_collection(
            &before,
            NativeImportOptions {
                settings_revision: lib.settings().unwrap().revision,
                content_ids: Some([content_id.clone()].into()),
                initial_owners: Default::default(),
            },
            &cancel,
        )
        .unwrap();
    assert!(
        lib.native_track_link(&report.target_id, &content_id)
            .unwrap()
            .local_track_id
            .is_some()
    );
    let counter = std::cell::Cell::new(0);
    let rule = |field: &str, operator, value| {
        counter.set(counter.get() + 1);
        SmartRule {
            id: format!("rule-{}", counter.get()),
            field: field.into(),
            operator,
            value,
            or: false,
        }
    };
    let native = SmartDefinition {
        match_all: true,
        rules: vec![
            rule(
                "artist",
                Operator::StringContains,
                RuleValue::Text("Cuerithm acceptance".into()),
            ),
            rule(
                "bpm",
                Operator::NumberBetween,
                RuleValue::NumberRange {
                    start: 120.,
                    end: 130.,
                },
            ),
        ],
    };
    let regex = SmartDefinition {
        match_all: true,
        rules: vec![rule(
            "title",
            Operator::StringRegExpMatch,
            RuleValue::Text("^A".into()),
        )],
    };
    for (name, definition) in [("Native rules", native), ("Regex rules", regex)] {
        playlist_edit(
            &lib,
            PlaylistEdit::Create {
                id: None,
                parent_id: None,
                kind: PlaylistKind::Smartlist,
                name: name.into(),
                definition: Some(definition),
            },
        );
    }
    let (work, backups) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let prepared = lib
        .native_sync_prepare(
            &live.path,
            &key,
            work.path(),
            NativeSyncOptions::default(),
            &cancel,
        )
        .unwrap();
    let kind_of = |name: &str| {
        prepared
            .playlists
            .items
            .iter()
            .find(|i| i.path == vec![name.to_owned()])
            .map(|i| (i.kind, i.materialized))
    };
    assert_eq!(
        kind_of("Native rules"),
        Some((PlaylistType::Smartlist, false))
    );
    assert_eq!(kind_of("Regex rules"), Some((PlaylistType::Playlist, true)));
    let run = lib
        .native_sync_apply(
            &live.path,
            &key,
            work.path(),
            backups.path(),
            NativeSyncOptions::default(),
            &cancel,
        )
        .unwrap();
    assert_eq!(run.state, "committed");
    let after = read(&live.path, &key);
    let native = after
        .playlists
        .iter()
        .find(|p| p.name == "Native rules" && p.deleted == Some(false))
        .unwrap();
    assert_eq!(native.attribute, 4);
    let xml = native.smart_rule.clone().unwrap();
    assert!(
        xml.starts_with(&format!(
            "<NODE Id=\"{}\" AutomaticUpdate=\"0\" LogicalOperator=\"1\">",
            native.id
        )),
        "{xml}"
    );
    assert!(
        xml.contains("PropertyName=\"artist\"")
            && xml.contains("ValueLeft=\"12000\" ValueRight=\"13000\" ValueUnit=\"0\"")
    );
    let materialized = after
        .playlists
        .iter()
        .find(|p| p.name == "Regex rules" && p.deleted == Some(false))
        .unwrap();
    assert_eq!(materialized.attribute, 0);
    // The existing smart lists in the library were not touched, and repeating writes nothing.
    let originals: Vec<_> = before
        .playlists
        .iter()
        .filter(|p| p.attribute == 4 && p.deleted == Some(false))
        .map(|p| (p.id.clone(), p.smart_rule.clone()))
        .collect();
    for (id, rule) in &originals {
        assert_eq!(
            after
                .playlists
                .iter()
                .find(|p| p.id == *id)
                .unwrap()
                .smart_rule,
            *rule
        );
    }
    assert_eq!(
        lib.native_sync_apply(
            &live.path,
            &key,
            work.path(),
            backups.path(),
            NativeSyncOptions::default(),
            &cancel
        )
        .unwrap()
        .state,
        "nothing_to_write"
    );
}

/// A minimal PCM WAV file: `seconds` of silence at 44.1 kHz stereo.
fn silent_wav(path: &std::path::Path, seconds: u32) {
    let data = 44_100 * 4 * seconds;
    let mut out = b"RIFF".to_vec();
    out.extend((36 + data).to_le_bytes());
    out.extend(b"WAVEfmt ");
    out.extend(16u32.to_le_bytes());
    out.extend([1, 0, 2, 0]);
    out.extend(44_100u32.to_le_bytes());
    out.extend((44_100u32 * 4).to_le_bytes());
    out.extend([4, 0, 16, 0]);
    out.extend(b"data");
    out.extend(data.to_le_bytes());
    out.extend(vec![0u8; data as usize]);
    std::fs::write(path, out).unwrap();
}

#[test]
fn a_cuerithm_track_rekordbox_lacks_is_added_with_its_grid_analysis_file_and_playlist_place() {
    let Some(copy) = std::env::var_os("CUERITHM_REKORDBOX_COPY").map(PathBuf::from) else {
        eprintln!("skipped: CUERITHM_REKORDBOX_COPY is not set");
        return;
    };
    let key = key();
    let cancel = CancellationToken::default();
    let scratch = tempfile::tempdir().unwrap();
    let live = ReadOnlyDatabase::open(&copy, &key)
        .unwrap()
        .capture(scratch.path(), &key, &cancel)
        .unwrap();
    let before = read(&live.path, &key);
    let linked = before
        .tracks
        .iter()
        .find(|t| {
            t.importable_local()
                && t.source
                    .resolved_path
                    .as_ref()
                    .is_some_and(|p| std::path::Path::new(p).is_file())
        })
        .expect("a track whose audio file exists")
        .identity
        .content_id
        .clone();
    let library_dir = tempfile::tempdir().unwrap();
    let lib = Library::open(&library_dir.path().join("library")).unwrap();
    let report = lib
        .import_rekordbox_collection(
            &before,
            NativeImportOptions {
                settings_revision: lib.settings().unwrap().revision,
                content_ids: Some([linked.clone()].into()),
                initial_owners: Default::default(),
            },
            &cancel,
        )
        .unwrap();
    let music = tempfile::tempdir().unwrap();
    let file = music.path().join("cuerithm acceptance new.wav");
    silent_wav(&file, 4);
    let file = file.canonicalize().unwrap();
    let new_id = lib
        .add_ready_track_for_test(
            &file,
            "Cuerithm New Track",
            "Cuerithm Artist",
            124.,
            4.,
            0.25,
        )
        .unwrap();
    let folder = playlist_edit(
        &lib,
        create(PlaylistKind::Folder, "Cuerithm New Acceptance", None),
    )
    .created_ids[0]
        .clone();
    let playlist = playlist_edit(&lib, create(PlaylistKind::Playlist, "Fresh", Some(&folder)))
        .created_ids[0]
        .clone();
    playlist_edit(
        &lib,
        PlaylistEdit::AddMembers {
            id: playlist,
            track_ids: vec![new_id.clone()],
        },
    );
    let (work, backups) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    // Offered, but not added unless asked for.
    let offered = lib
        .native_sync_prepare(
            &live.path,
            &key,
            work.path(),
            NativeSyncOptions::default(),
            &cancel,
        )
        .unwrap();
    assert_eq!(offered.new_tracks.items.len(), 1);
    assert_eq!(offered.new_tracks.items[0].local_track_id, new_id);
    let declined = lib
        .native_sync_apply(
            &live.path,
            &key,
            work.path(),
            backups.path(),
            NativeSyncOptions::default(),
            &cancel,
        )
        .unwrap();
    assert_eq!(declined.tracks_added, 0);
    assert!(
        !read(&live.path, &key)
            .tracks
            .iter()
            .any(|t| t.metadata.title == "Cuerithm New Track")
    );
    // Added when asked for, with the analysis file published beside the database change.
    let options = NativeSyncOptions {
        add_new_tracks: true,
    };
    let run = lib
        .native_sync_apply(
            &live.path,
            &key,
            work.path(),
            backups.path(),
            options,
            &cancel,
        )
        .unwrap();
    assert_eq!(run.state, "committed");
    assert_eq!(run.tracks_added, 1);
    let after = read(&live.path, &key);
    let added = after
        .tracks
        .iter()
        .find(|t| t.metadata.title == "Cuerithm New Track")
        .expect("the new track");
    assert_eq!(added.metadata.artist, "Cuerithm Artist");
    let share = live.path.parent().unwrap().join("share");
    let analysis_files = walk(&share);
    assert!(
        analysis_files.iter().any(|p| p.ends_with("ANLZ0000.DAT")),
        "{analysis_files:?}"
    );
    // The playlist holds it, and it is linked so that nothing is added twice.
    let fresh = after
        .playlists
        .iter()
        .find(|p| p.name == "Fresh" && p.deleted == Some(false))
        .expect("the playlist");
    assert!(
        after
            .playlist_links
            .iter()
            .any(|m| m.playlist_id == fresh.id
                && m.deleted == Some(false)
                && m.content_id == added.identity.content_id)
    );
    assert!(
        lib.native_linked_content_ids(&report.target_id)
            .unwrap()
            .contains(&added.identity.content_id)
    );
    let again = lib
        .native_sync_prepare(&live.path, &key, work.path(), options, &cancel)
        .unwrap();
    assert!(again.new_tracks.items.is_empty(), "{:?}", again.new_tracks);
    assert!(again.plan.tracks.is_empty(), "{:?}", again.plan);
    let repeat = lib
        .native_sync_apply(
            &live.path,
            &key,
            work.path(),
            backups.path(),
            options,
            &cancel,
        )
        .unwrap();
    assert_eq!(repeat.state, "nothing_to_write");
}

fn walk(dir: &std::path::Path) -> Vec<PathBuf> {
    let mut out = vec![];
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                out.extend(walk(&path));
            } else {
                out.push(path);
            }
        }
    }
    out
}
