//! End-to-end synchronization against a copy of a real rekordbox 7 library.
//!
//! Skipped unless `CUERITHM_REKORDBOX_COPY` names a copied `master.db` (with its WAL/SHM beside it). The copy
//! is snapshotted first, and the snapshot stands in for the live library, so no original is opened for
//! writing. One track's audio file is read to hash it for the import; nothing from the library is printed.
use cuerithm_library::*;
use cuerithm_media::CancellationToken;
use cuerithm_rekordbox::*;
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
    let owners: BTreeMap<_, _> = [
        "title", "artist", "album", "genre", "label", "comment", "key", "bpm", "rating", "cues",
    ]
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
        .native_sync_prepare(&live.path, &key, work.path(), &cancel)
        .unwrap();
    assert!(idle.plan.tracks.is_empty(), "{:?}", idle.plan);
    // Edit the track in Cuerithm.
    lib.set_track_text_for_test(&local_id, "Cuerithm edited title", "Cuerithm edited genre")
        .unwrap();
    let prepared = lib
        .native_sync_prepare(&live.path, &key, work.path(), &cancel)
        .unwrap();
    let planned = &prepared.plan.tracks[0];
    assert_eq!(planned.content_id, content_id);
    assert!(
        planned.fields.contains(&"title".to_owned())
            && planned.fields.contains(&"genre".to_owned())
    );
    // Apply.
    let run = lib
        .native_sync_apply(&live.path, &key, work.path(), backups.path(), &cancel)
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
        .native_sync_apply(&live.path, &key, work.path(), backups.path(), &cancel)
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
        .native_sync_apply(&live.path, &key, work.path(), backups.path(), &cancel)
        .unwrap();
    assert_eq!(conflicted.state, "nothing_to_write");
    assert!(conflicted.skipped >= 1);
    let after_conflict = lib
        .native_sync_prepare(&live.path, &key, work.path(), &cancel)
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
