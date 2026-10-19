use cuerithm_library::{
    DuplicateMatchMethod, DuplicateScanRequest, DuplicateScanScope, Library, PlaylistEdit,
    PlaylistKind,
};
use cuerithm_media::{CancellationToken, MediaRuntime};
use std::path::{Path, PathBuf};

struct Fixture {
    dir: tempfile::TempDir,
    db: PathBuf,
    lib: Library,
    runtime: MediaRuntime,
    ids: Vec<String>,
    paths: Vec<PathBuf>,
}
impl Fixture {
    fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let db = dir.path().join("app/library.sqlite");
        let lib = Library::open(&db).unwrap();
        let root = std::env::var_os("CUERITHM_MEDIA_RUNTIME")
            .map(PathBuf::from)
            .unwrap_or_else(|| {
                Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools/media-runtime/bin")
            });
        let runtime = MediaRuntime::from_bundle(&root).unwrap();
        let mut ids = vec![];
        let mut paths = vec![];
        for i in 0..4 {
            let path = dir.path().join(format!("track-{i}.wav"));
            wav(&path, if i == 2 { 43 } else { 37 });
            if i == 1 {
                let mut bytes = std::fs::read(&path).unwrap();
                bytes.extend_from_slice(b"JUNK\x04\0\0\0tag!");
                let size = (bytes.len() - 8) as u32;
                bytes[4..8].copy_from_slice(&size.to_le_bytes());
                std::fs::write(&path, bytes).unwrap();
            }
            let job = lib
                .enqueue(
                    std::slice::from_ref(&path),
                    lib.settings().unwrap().revision,
                )
                .unwrap()[0]
                .clone();
            ids.push(
                lib.jobs()
                    .unwrap()
                    .iter()
                    .find(|r| r.id == job)
                    .unwrap()
                    .track_id
                    .clone(),
            );
            paths.push(path);
        }
        let conn = rusqlite::Connection::open(&db).unwrap();
        conn.execute("UPDATE tracks SET title='Signal',artist='North',duration=20,state='ready',analysis_json='{\"untouched\":true}',metadata_json='{\"codec\":\"pcm_s16le\",\"sample_rate\":8000,\"channels\":1,\"tags\":{},\"properties\":{\"audio.bit_rate\":\"128000\"}}'",[]).unwrap();
        conn.execute("UPDATE tracks SET title='Other' WHERE id=?1", [&ids[2]])
            .unwrap();
        drop(conn);
        lib.set_archived(&ids[3], true).unwrap();
        Self {
            dir,
            db,
            lib,
            runtime,
            ids,
            paths,
        }
    }
    fn playlist(&self, id: &str, tracks: Vec<String>) {
        self.lib
            .apply_playlist_edit(
                PlaylistEdit::Create {
                    id: Some(id.into()),
                    parent_id: None,
                    kind: PlaylistKind::Playlist,
                    name: id.into(),
                    definition: None,
                },
                self.lib.playlist_state().unwrap().revision,
            )
            .unwrap();
        self.lib
            .apply_playlist_edit(
                PlaylistEdit::AddMembers {
                    id: id.into(),
                    track_ids: tracks,
                },
                self.lib.playlist_state().unwrap().revision,
            )
            .unwrap();
    }
    fn content(&self) -> Vec<String> {
        let conn = rusqlite::Connection::open(&self.db).unwrap();
        ["SELECT json_group_array(json_array(id,hex(path),title,artist,archived,source_hash,state,analysis_json,metadata_json,current_job)) FROM (SELECT * FROM tracks ORDER BY id)","SELECT json_group_array(json_array(id,state,stage,attempts,settings_revision)) FROM (SELECT * FROM jobs ORDER BY id)","SELECT json_group_array(json_array(id,track_id,job_id,json,created)) FROM (SELECT * FROM cue_revisions ORDER BY id)"].iter().map(|sql|conn.query_row(sql,[],|r|r.get(0)).unwrap()).collect()
    }
}
fn wav(path: &Path, seed: i32) {
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: 8000,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut out = hound::WavWriter::create(path, spec).unwrap();
    for i in 0..160000 {
        out.write_sample::<i16>(((i * seed) % 20001 - 10000) as i16)
            .unwrap();
    }
    out.finalize().unwrap();
}
fn sets(report: &cuerithm_library::DuplicateScanReport) -> Vec<std::collections::BTreeSet<String>> {
    report
        .groups
        .iter()
        .map(|g| g.track_ids.iter().cloned().collect())
        .collect()
}

#[test]
fn exact_audio_precedence_persistent_cache_and_read_only_scan_survive_restart() {
    let f = Fixture::new();
    let before = f.content();
    let hashes: Vec<_> = f
        .paths
        .iter()
        .map(|p| cuerithm_media::file_sha256(p).unwrap())
        .collect();
    let result = f
        .lib
        .scan_duplicates(
            DuplicateScanRequest::default(),
            &f.runtime,
            &CancellationToken::default(),
        )
        .unwrap();
    assert!(!result.stale);
    assert_eq!(result.tracks.len(), 3);
    assert_eq!(result.fingerprints_computed, 3);
    assert_eq!(result.cache_hits, 0);
    assert_eq!(
        sets(&result),
        vec![
            [f.ids[0].clone(), f.ids[1].clone()]
                .into_iter()
                .collect::<std::collections::BTreeSet<_>>()
        ]
    );
    assert!(matches!(
        result.groups[0].method,
        DuplicateMatchMethod::Fingerprint
    ));
    assert_eq!(f.content(), before);
    assert_eq!(
        hashes,
        f.paths
            .iter()
            .map(|p| cuerithm_media::file_sha256(p).unwrap())
            .collect::<Vec<_>>()
    );
    assert_eq!(f.lib.duplicate_scan(&result.id).unwrap().id, result.id);
    let Fixture {
        dir,
        db,
        lib,
        runtime,
        ids,
        paths,
    } = f;
    drop(lib);
    let lib = Library::open(&db).unwrap();
    let second = lib
        .scan_duplicates(
            DuplicateScanRequest::default(),
            &runtime,
            &CancellationToken::default(),
        )
        .unwrap();
    assert_eq!(second.cache_hits, 3);
    assert_eq!(second.fingerprints_computed, 0);
    assert_eq!(
        sets(&second),
        vec![
            [ids[0].clone(), ids[1].clone()]
                .into_iter()
                .collect::<std::collections::BTreeSet<_>>()
        ]
    );
    assert!(!second.stale);
    assert!(paths.iter().all(|p| p.exists()));
    drop(dir);
}

#[test]
fn selected_scope_keeps_explicit_archived_members_while_full_filter_keeps_outside_copies() {
    let f = Fixture::new();
    f.playlist("chosen", vec![f.ids[1].clone(), f.ids[3].clone()]);
    let mut request = DuplicateScanRequest {
        scope: DuplicateScanScope::SelectedPlaylists,
        playlist_ids: vec!["chosen".into()],
        ..Default::default()
    };
    let selected = f
        .lib
        .scan_duplicates(request.clone(), &f.runtime, &CancellationToken::default())
        .unwrap();
    assert_eq!(selected.tracks.len(), 2);
    assert_eq!(
        sets(&selected),
        vec![
            [f.ids[1].clone(), f.ids[3].clone()]
                .into_iter()
                .collect::<std::collections::BTreeSet<_>>()
        ]
    );
    request.scope = DuplicateScanScope::FullLibraryFiltered;
    let filtered = f
        .lib
        .scan_duplicates(request, &f.runtime, &CancellationToken::default())
        .unwrap();
    assert_eq!(
        sets(&filtered),
        vec![
            [f.ids[0].clone(), f.ids[1].clone()]
                .into_iter()
                .collect::<std::collections::BTreeSet<_>>()
        ]
    );
}

#[test]
fn source_edits_invalidate_cached_audio_even_at_the_same_size_and_timestamp() {
    let f = Fixture::new();
    f.lib
        .scan_duplicates(
            DuplicateScanRequest::default(),
            &f.runtime,
            &CancellationToken::default(),
        )
        .unwrap();
    let file = std::fs::File::options()
        .read(true)
        .write(true)
        .open(&f.paths[0])
        .unwrap();
    let modified = file.metadata().unwrap().modified().unwrap();
    let length = file.metadata().unwrap().len();
    wav(&f.paths[0], 53);
    file.set_times(std::fs::FileTimes::new().set_modified(modified))
        .unwrap();
    assert_eq!(std::fs::metadata(&f.paths[0]).unwrap().len(), length);
    let current = f
        .lib
        .scan_duplicates(
            DuplicateScanRequest::default(),
            &f.runtime,
            &CancellationToken::default(),
        )
        .unwrap();
    assert_eq!(current.cache_hits, 2);
    assert_eq!(current.fingerprints_computed, 1);
    assert!(
        !current
            .groups
            .iter()
            .any(|g| matches!(g.method, DuplicateMatchMethod::Fingerprint))
    );
}

#[test]
fn cancellation_never_saves_a_complete_report_and_library_edits_mark_results_stale() {
    let f = Fixture::new();
    let cancel = CancellationToken::default();
    cancel.cancel();
    assert!(
        f.lib
            .scan_duplicates(DuplicateScanRequest::default(), &f.runtime, &cancel)
            .is_err()
    );
    let conn = rusqlite::Connection::open(&f.db).unwrap();
    assert_eq!(
        conn.query_row("SELECT COUNT(*) FROM maintenance_scans", [], |r| r
            .get::<_, u64>(0))
            .unwrap(),
        0
    );
    drop(conn);
    let mut changed = false;
    let report = f
        .lib
        .scan_duplicates_with_progress(
            DuplicateScanRequest::default(),
            &f.runtime,
            &CancellationToken::default(),
            |progress| {
                if progress.completed == 1 && !changed {
                    f.lib.set_archived(&f.ids[0], true).unwrap();
                    changed = true;
                }
            },
        )
        .unwrap();
    assert!(changed);
    assert!(report.stale);
    assert!(f.lib.duplicate_scan(&report.id).unwrap().stale);
}

#[test]
fn unanalysed_tracks_missing_parents_and_missing_files_are_distinct() {
    let f = Fixture::new();
    let conn = rusqlite::Connection::open(&f.db).unwrap();
    conn.execute("UPDATE tracks SET duration=NULL WHERE id=?1", [&f.ids[0]])
        .unwrap();
    std::fs::remove_file(&f.paths[1]).unwrap();
    std::fs::create_dir(f.dir.path().join("offline")).unwrap();
    let vanished = f.dir.path().join("offline/music.wav");
    wav(&vanished, 31);
    let job = f
        .lib
        .enqueue(
            std::slice::from_ref(&vanished),
            f.lib.settings().unwrap().revision,
        )
        .unwrap()[0]
        .clone();
    let offline_id = f
        .lib
        .jobs()
        .unwrap()
        .into_iter()
        .find(|j| j.id == job)
        .unwrap()
        .track_id;
    // A parent disappearing cannot prove whether the storage is disconnected.
    std::fs::remove_file(&vanished).unwrap();
    std::fs::remove_dir(f.dir.path().join("offline")).unwrap();
    let r = f
        .lib
        .scan_duplicates(
            DuplicateScanRequest::default(),
            &f.runtime,
            &CancellationToken::default(),
        )
        .unwrap();
    let state = |id: &str| r.tracks.iter().find(|t| t.id == id).unwrap().state;
    assert_eq!(
        state(&f.ids[0]),
        cuerithm_library::MaintenanceFileState::DurationSkipped
    );
    assert_eq!(
        state(&f.ids[1]),
        cuerithm_library::MaintenanceFileState::Missing
    );
    assert_eq!(
        state(&offline_id),
        cuerithm_library::MaintenanceFileState::Unavailable
    );
}

#[test]
fn saved_reports_recheck_source_content_and_cancelled_scans_do_not_save_reports() {
    let f = Fixture::new();
    let r = f
        .lib
        .scan_duplicates(
            DuplicateScanRequest::default(),
            &f.runtime,
            &CancellationToken::default(),
        )
        .unwrap();
    wav(&f.paths[0], 71);
    assert!(f.lib.duplicate_scan(&r.id).unwrap().stale);
    let cancel = CancellationToken::default();
    assert!(
        f.lib
            .scan_duplicates_with_progress(
                DuplicateScanRequest::default(),
                &f.runtime,
                &cancel,
                |p| {
                    if p.completed == 1 {
                        cancel.cancel();
                    }
                }
            )
            .is_err()
    );
    let conn = rusqlite::Connection::open(&f.db).unwrap();
    assert_eq!(
        conn.query_row("SELECT COUNT(*) FROM maintenance_scans", [], |r| r
            .get::<_, u32>(0))
            .unwrap(),
        1
    );
}

#[test]
fn selected_smart_scope_evaluates_fresh_membership_without_publishing_members() {
    use cuerithm_library::SmartDefinition;
    use cuerithm_smartlists::{Operator, RuleValue, SmartRule};
    let f = Fixture::new();
    f.lib
        .apply_playlist_edit(
            PlaylistEdit::Create {
                id: Some("dynamic".into()),
                parent_id: None,
                kind: PlaylistKind::Smartlist,
                name: "Dynamic".into(),
                definition: Some(SmartDefinition {
                    match_all: true,
                    rules: vec![SmartRule {
                        id: "r".into(),
                        field: "title".into(),
                        operator: Operator::StringEquals,
                        value: RuleValue::Text("Signal".into()),
                        or: false,
                    }],
                }),
            },
            f.lib.playlist_state().unwrap().revision,
        )
        .unwrap();
    let request = DuplicateScanRequest {
        scope: DuplicateScanScope::SelectedPlaylists,
        playlist_ids: vec!["dynamic".into()],
        ..Default::default()
    };
    let r = f
        .lib
        .scan_duplicates(request.clone(), &f.runtime, &CancellationToken::default())
        .unwrap();
    assert_eq!(r.tracks.len(), 2);
    assert_eq!(r.groups.len(), 1);
    let conn = rusqlite::Connection::open(&f.db).unwrap();
    assert_eq!(
        conn.query_row("SELECT COUNT(*) FROM smart_playlist_members", [], |r| r
            .get::<_, u32>(0))
            .unwrap(),
        0
    );
    conn.execute("UPDATE tracks SET title='Other' WHERE id=?1", [&f.ids[0]])
        .unwrap();
    drop(conn);
    assert!(f.lib.duplicate_scan(&r.id).unwrap().stale);
    let next = f
        .lib
        .scan_duplicates(request, &f.runtime, &CancellationToken::default())
        .unwrap();
    assert_eq!(next.tracks.len(), 1);
    assert!(next.groups.is_empty());
}

#[test]
fn ignores_are_component_scoped_and_persisted_ignores_invalidate_reports() {
    let f = Fixture::new();
    let r = f
        .lib
        .scan_duplicates(
            DuplicateScanRequest {
                ignored_paths: vec![f.dir.path().join("track-")],
                ..Default::default()
            },
            &f.runtime,
            &CancellationToken::default(),
        )
        .unwrap();
    assert_eq!(r.tracks.len(), 3);
    let conn = rusqlite::Connection::open(&f.db).unwrap();
    conn.execute(
        "INSERT INTO maintenance_ignored_tracks VALUES(?1)",
        [&f.ids[0]],
    )
    .unwrap();
    drop(conn);
    assert!(f.lib.duplicate_scan(&r.id).unwrap().stale);
    let next = f
        .lib
        .scan_duplicates(
            DuplicateScanRequest::default(),
            &f.runtime,
            &CancellationToken::default(),
        )
        .unwrap();
    assert!(next.groups.is_empty());
    let excluded = f
        .lib
        .scan_duplicates(
            DuplicateScanRequest {
                ignored_paths: vec![f.dir.path().to_owned()],
                ..Default::default()
            },
            &f.runtime,
            &CancellationToken::default(),
        )
        .unwrap();
    assert!(excluded.tracks.is_empty());
    assert!(excluded.groups.is_empty());
    assert!(!excluded.stale);
}

#[test]
fn edited_last_file_and_cancelled_final_callback_cannot_publish_a_fresh_report() {
    let f = Fixture::new();
    let report = f
        .lib
        .scan_duplicates_with_progress(
            DuplicateScanRequest::default(),
            &f.runtime,
            &CancellationToken::default(),
            |p| {
                if p.completed == p.total {
                    wav(&f.paths[0], 79);
                }
            },
        )
        .unwrap();
    assert!(report.stale);
    let cancel = CancellationToken::default();
    assert!(
        f.lib
            .scan_duplicates_with_progress(
                DuplicateScanRequest::default(),
                &f.runtime,
                &cancel,
                |p| {
                    if p.completed == p.total {
                        cancel.cancel();
                    }
                }
            )
            .is_err()
    );
    let conn = rusqlite::Connection::open(&f.db).unwrap();
    assert_eq!(
        conn.query_row("SELECT COUNT(*) FROM maintenance_scans", [], |r| r
            .get::<_, u32>(0))
            .unwrap(),
        1
    );
}

#[test]
fn migration_from_v6_preserves_tracks_and_initializes_empty_maintenance() {
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("library.sqlite");
    let conn = rusqlite::Connection::open(&db).unwrap();
    for text in [
        include_str!("../migrations/001_initial.sql"),
        include_str!("../migrations/002_tag_writes.sql"),
        include_str!("../migrations/003_preparation_cache.sql"),
        include_str!("../migrations/004_cleanup.sql"),
        include_str!("../migrations/005_cleanup_preparation.sql"),
        include_str!("../migrations/006_playlists.sql"),
    ] {
        conn.execute_batch(text).unwrap();
    }
    conn.execute("INSERT INTO tracks(id,path,title,source_hash,state,analysis_json,created,updated) VALUES('preserve',?1,'Legacy','source','ready','{}',1,1)",[b"unix:/preserve.flac".to_vec()]).unwrap();
    let before: String = conn
        .query_row(
            "SELECT json_group_array(json_array(id,hex(path),analysis_json)) FROM tracks",
            [],
            |r| r.get(0),
        )
        .unwrap();
    drop(conn);
    let lib = Library::open(&db).unwrap();
    let conn = rusqlite::Connection::open(&db).unwrap();
    assert_eq!(
        conn.query_row("pragma user_version", [], |r| r.get::<_, u32>(0))
            .unwrap(),
        7
    );
    assert_eq!(
        conn.query_row(
            "SELECT json_group_array(json_array(id,hex(path),analysis_json)) FROM tracks",
            [],
            |r| r.get::<_, String>(0)
        )
        .unwrap(),
        before
    );
    assert_eq!(
        conn.query_row("SELECT COUNT(*) FROM maintenance_scans", [], |r| r
            .get::<_, u32>(0))
            .unwrap(),
        0
    );
    drop(lib);
}

#[test]
fn duplicate_candidate_order_follows_import_order_even_after_title_sort_changes() {
    let f = Fixture::new();
    let conn = rusqlite::Connection::open(&f.db).unwrap();
    conn.execute(
        "UPDATE tracks SET sort_title='zzz' WHERE id=?1",
        [&f.ids[0]],
    )
    .unwrap();
    conn.execute(
        "UPDATE tracks SET sort_title='aaa' WHERE id=?1",
        [&f.ids[1]],
    )
    .unwrap();
    drop(conn);
    let r = f
        .lib
        .scan_duplicates(
            DuplicateScanRequest::default(),
            &f.runtime,
            &CancellationToken::default(),
        )
        .unwrap();
    assert_eq!(
        r.tracks.iter().map(|t| t.id.clone()).collect::<Vec<_>>(),
        f.ids[..3]
    );
}

#[test]
fn duplicate_review_survives_restart_and_guards_revision_and_ignores() {
    use cuerithm_library::{DuplicateReviewEdit, GroupEdit};
    let mut f = Fixture::new();
    let before = f.content();
    let token = CancellationToken::default();
    let scan = f
        .lib
        .scan_duplicates(DuplicateScanRequest::default(), &f.runtime, &token)
        .unwrap();
    let review = f.lib.begin_duplicate_review(&scan.id, &token).unwrap();
    assert_eq!(review.revision, 0);
    assert_eq!(review.groups.len(), 1);
    let group = review.groups[0].id.clone();
    let chosen = f.ids[1].clone();
    let edit = |edit| DuplicateReviewEdit::Group {
        group_id: group.clone(),
        edit,
    };
    let selected = f
        .lib
        .edit_duplicate_review(
            &review.id,
            0,
            edit(GroupEdit::Select {
                track_id: chosen.clone(),
            }),
            &token,
        )
        .unwrap();
    assert_eq!(selected.groups[0].keeper.as_deref(), Some(chosen.as_str()));
    assert!(
        f.lib
            .edit_duplicate_review(&review.id, 0, edit(GroupEdit::Lock), &token)
            .is_err()
    );
    drop(f.lib);
    f.lib = Library::open(&f.db).unwrap();
    let reopened = f.lib.clone();
    let loaded = reopened.duplicate_review(&review.id, &token).unwrap();
    assert!(!loaded.stale);
    assert_eq!(loaded.revision, 1);
    assert_eq!(loaded.groups[0].keeper, selected.groups[0].keeper);
    let ignored = reopened
        .edit_duplicate_review(
            &review.id,
            1,
            edit(GroupEdit::ToggleIgnore {
                track_id: chosen.clone(),
            }),
            &token,
        )
        .unwrap();
    assert!(!ignored.stale);
    assert!(!ignored.groups[0].ready());
    assert!(f.lib.duplicate_scan(&scan.id).unwrap().stale);
    assert_eq!(f.content(), before);
    let fresh = f
        .lib
        .scan_duplicates(DuplicateScanRequest::default(), &f.runtime, &token)
        .unwrap();
    assert!(fresh.groups.is_empty());
    let restored = reopened
        .edit_duplicate_review(
            &review.id,
            2,
            edit(GroupEdit::ToggleIgnore { track_id: chosen }),
            &token,
        )
        .unwrap();
    assert!(restored.groups[0].ready());
    assert!(!restored.stale);
    assert!(
        !f.lib
            .scan_duplicates(DuplicateScanRequest::default(), &f.runtime, &token)
            .unwrap()
            .groups
            .is_empty()
    );
}

#[test]
fn duplicate_reviews_reject_source_library_and_concurrent_review_changes() {
    use cuerithm_library::{DuplicateReviewEdit, GroupEdit};
    let token = CancellationToken::default();
    let f = Fixture::new();
    let scan = f
        .lib
        .scan_duplicates(DuplicateScanRequest::default(), &f.runtime, &token)
        .unwrap();
    let one = f.lib.begin_duplicate_review(&scan.id, &token).unwrap();
    let two = f.lib.begin_duplicate_review(&scan.id, &token).unwrap();
    let edit = DuplicateReviewEdit::Group {
        group_id: one.groups[0].id.clone(),
        edit: GroupEdit::ToggleIgnore {
            track_id: f.ids[1].clone(),
        },
    };
    f.lib
        .edit_duplicate_review(&one.id, 0, edit, &token)
        .unwrap();
    assert!(f.lib.duplicate_review(&two.id, &token).unwrap().stale);
    let bad = DuplicateReviewEdit::Group {
        group_id: one.groups[0].id.clone(),
        edit: GroupEdit::Select {
            track_id: f.ids[2].clone(),
        },
    };
    assert!(
        f.lib
            .edit_duplicate_review(&one.id, 1, bad, &token)
            .is_err()
    );
    assert_eq!(f.lib.duplicate_review(&one.id, &token).unwrap().revision, 1);
    wav(&f.paths[0], 59);
    assert!(f.lib.duplicate_review(&one.id, &token).unwrap().stale);
    assert!(
        f.lib
            .edit_duplicate_review(
                &one.id,
                1,
                DuplicateReviewEdit::Group {
                    group_id: one.groups[0].id.clone(),
                    edit: GroupEdit::Lock
                },
                &token
            )
            .is_err()
    );
    let f = Fixture::new();
    let scan = f
        .lib
        .scan_duplicates(DuplicateScanRequest::default(), &f.runtime, &token)
        .unwrap();
    let review = f.lib.begin_duplicate_review(&scan.id, &token).unwrap();
    f.lib.set_archived(&f.ids[0], true).unwrap();
    assert!(f.lib.duplicate_review(&review.id, &token).unwrap().stale);
    assert!(f.lib.begin_duplicate_review(&scan.id, &token).is_err());
}

#[test]
fn duplicate_review_preferences_locks_copy_only_and_cancellation_are_read_only() {
    use cuerithm_library::{DuplicateReviewEdit, GroupEdit, Preference, ReviewState};
    let f = Fixture::new();
    let token = CancellationToken::default();
    let scan = f
        .lib
        .scan_duplicates(DuplicateScanRequest::default(), &f.runtime, &token)
        .unwrap();
    let review = f.lib.begin_duplicate_review(&scan.id, &token).unwrap();
    let before = f.content();
    let edit = |edit| DuplicateReviewEdit::Group {
        group_id: review.groups[0].id.clone(),
        edit,
    };
    let review = f
        .lib
        .edit_duplicate_review(
            &review.id,
            0,
            edit(GroupEdit::UseTags {
                track_id: Some(f.ids[1].clone()),
            }),
            &token,
        )
        .unwrap();
    let review = f
        .lib
        .edit_duplicate_review(&review.id, 1, edit(GroupEdit::Lock), &token)
        .unwrap();
    let review = f
        .lib
        .edit_duplicate_review(
            &review.id,
            2,
            edit(GroupEdit::Unlock { force: false }),
            &token,
        )
        .unwrap();
    assert_eq!(review.groups[0].state, ReviewState::CopyCues);
    let review = f
        .lib
        .edit_duplicate_review(
            &review.id,
            3,
            DuplicateReviewEdit::Prefer {
                preference: Preference::TitleContains {
                    value: "Signal".into(),
                },
                override_locked: false,
                use_best_cues: true,
            },
            &token,
        )
        .unwrap();
    assert_eq!(review.revision, 4);
    assert_eq!(f.content(), before);
    let cancelled = CancellationToken::default();
    cancelled.cancel();
    assert!(
        f.lib
            .edit_duplicate_review(&review.id, 4, edit(GroupEdit::Lock), &cancelled)
            .is_err()
    );
    assert_eq!(
        f.lib.duplicate_review(&review.id, &token).unwrap().revision,
        4
    );
}

#[test]
fn persistent_duplicate_ignores_can_be_removed_after_the_original_review_is_stale() {
    use cuerithm_library::{DuplicateReviewEdit, GroupEdit};
    let f = Fixture::new();
    let token = CancellationToken::default();
    let scan = f
        .lib
        .scan_duplicates(DuplicateScanRequest::default(), &f.runtime, &token)
        .unwrap();
    let review = f.lib.begin_duplicate_review(&scan.id, &token).unwrap();
    let ignored_id = f.ids[1].clone();
    f.lib
        .edit_duplicate_review(
            &review.id,
            0,
            DuplicateReviewEdit::Group {
                group_id: review.groups[0].id.clone(),
                edit: GroupEdit::ToggleIgnore {
                    track_id: ignored_id.clone(),
                },
            },
            &token,
        )
        .unwrap();
    f.lib.set_archived(&f.ids[2], true).unwrap();
    assert!(f.lib.duplicate_review(&review.id, &token).unwrap().stale);
    let ignored = f.lib.duplicate_ignores().unwrap();
    assert_eq!(ignored.tracks.len(), 1);
    assert_eq!(ignored.tracks[0].id, ignored_id);
    let before = f.content();
    assert!(
        f.lib
            .set_duplicate_ignores(
                vec![ignored_id.clone(), "outside".into()],
                false,
                ignored.stamp.clone()
            )
            .is_err()
    );
    assert_eq!(f.lib.duplicate_ignores().unwrap().tracks.len(), 1);
    let cleared = f
        .lib
        .set_duplicate_ignores(vec![ignored_id.clone()], false, ignored.stamp.clone())
        .unwrap();
    assert!(cleared.tracks.is_empty());
    assert!(
        f.lib
            .set_duplicate_ignores(vec![ignored_id], true, ignored.stamp)
            .is_err()
    );
    assert_eq!(f.content(), before);
    assert!(
        !f.lib
            .scan_duplicates(DuplicateScanRequest::default(), &f.runtime, &token)
            .unwrap()
            .groups
            .is_empty()
    );
}

#[test]
fn cancelling_saved_report_reload_and_export_uses_the_requested_token() {
    use cuerithm_library::{BrokenScanRequest, ExtensionMode, UnusedScanRequest};
    let f = Fixture::new();
    let token = CancellationToken::default();
    let duplicate = f
        .lib
        .scan_duplicates(DuplicateScanRequest::default(), &f.runtime, &token)
        .unwrap();
    let unused = f
        .lib
        .scan_unused_files(
            UnusedScanRequest {
                root: f.dir.path().to_path_buf(),
                extension_mode: ExtensionMode::All,
                extensions: vec![],
            },
            &token,
        )
        .unwrap();
    let broken = f
        .lib
        .scan_broken_tracks(BrokenScanRequest::default(), &f.runtime, &token)
        .unwrap();
    token.cancel();
    let errors = [
        f.lib
            .duplicate_scan_cancellable(&duplicate.id, &token)
            .unwrap_err(),
        f.lib
            .unused_scan_cancellable(&unused.id, &token)
            .unwrap_err(),
        f.lib
            .broken_scan_cancellable(&broken.id, &token)
            .unwrap_err(),
        f.lib
            .unused_scan_export_cancellable(&unused.id, &token)
            .unwrap_err(),
    ];
    assert!(errors.iter().all(|e| e.to_string().contains("cancelled")));
}

#[test]
fn duplicate_merge_replaces_playlists_archives_records_and_undo_survives_restart() {
    let mut f = Fixture::new();
    f.playlist(
        "affected",
        vec![f.ids[0].clone(), f.ids[2].clone(), f.ids[1].clone()],
    );
    f.playlist("unaffected", vec![f.ids[2].clone(), f.ids[0].clone()]);
    let token = CancellationToken::default();
    let before = f.content();
    let hashes = f
        .paths
        .iter()
        .map(|p| cuerithm_media::file_sha256(p).unwrap())
        .collect::<Vec<_>>();
    let scan = f
        .lib
        .scan_duplicates(DuplicateScanRequest::default(), &f.runtime, &token)
        .unwrap();
    let review = f.lib.begin_duplicate_review(&scan.id, &token).unwrap();
    let preview = f
        .lib
        .preview_duplicate_merge(
            &review.id,
            review.revision,
            vec![review.groups[0].id.clone()],
            &token,
        )
        .unwrap();
    assert_eq!(preview.playlists.len(), 1);
    assert_eq!(
        preview.playlists[0].after,
        vec![f.ids[2].clone(), f.ids[0].clone()]
    );
    let applied = f.lib.apply_duplicate_merge(&preview.id, &token).unwrap();
    assert_eq!(applied.archived_ids, vec![f.ids[1].clone()]);
    assert_eq!(
        f.lib.duplicate_merge_history().unwrap()[0].result.id,
        applied.id
    );
    assert_eq!(
        f.lib.manual_playlist_members("affected").unwrap(),
        vec![f.ids[2].clone(), f.ids[0].clone()]
    );
    assert_eq!(
        f.lib.manual_playlist_members("unaffected").unwrap(),
        vec![f.ids[2].clone(), f.ids[0].clone()]
    );
    assert!(f.lib.apply_duplicate_merge(&preview.id, &token).is_err());
    drop(f.lib);
    f.lib = Library::open(&f.db).unwrap();
    let undone = f.lib.undo_duplicate_merge(&applied.id, &token).unwrap();
    assert_eq!(undone.undo_of.as_deref(), Some(applied.id.as_str()));
    assert_eq!(
        f.lib.duplicate_merge_history().unwrap()[0]
            .undone_by
            .as_deref(),
        Some(undone.id.as_str())
    );
    assert_eq!(f.content(), before);
    assert_eq!(
        f.lib.manual_playlist_members("affected").unwrap(),
        vec![f.ids[0].clone(), f.ids[2].clone(), f.ids[1].clone()]
    );
    assert!(f.lib.undo_duplicate_merge(&applied.id, &token).is_err());
    assert_eq!(
        hashes,
        f.paths
            .iter()
            .map(|p| cuerithm_media::file_sha256(p).unwrap())
            .collect::<Vec<_>>()
    );
}

#[test]
fn duplicate_merge_preview_and_undo_reject_later_review_playlist_source_and_aba_changes() {
    use cuerithm_library::{DuplicateReviewEdit, GroupEdit};
    let f = Fixture::new();
    let token = CancellationToken::default();
    let scan = f
        .lib
        .scan_duplicates(DuplicateScanRequest::default(), &f.runtime, &token)
        .unwrap();
    let review = f.lib.begin_duplicate_review(&scan.id, &token).unwrap();
    let preview = f
        .lib
        .preview_duplicate_merge(&review.id, 0, vec![review.groups[0].id.clone()], &token)
        .unwrap();
    f.lib
        .edit_duplicate_review(
            &review.id,
            0,
            DuplicateReviewEdit::Group {
                group_id: review.groups[0].id.clone(),
                edit: GroupEdit::Lock,
            },
            &token,
        )
        .unwrap();
    assert!(f.lib.apply_duplicate_merge(&preview.id, &token).is_err());
    assert!(
        !rusqlite::Connection::open(&f.db)
            .unwrap()
            .query_row(
                "SELECT archived FROM tracks WHERE id=?1",
                [&f.ids[1]],
                |r| r.get::<_, bool>(0)
            )
            .unwrap()
    );
    let f = Fixture::new();
    let scan = f
        .lib
        .scan_duplicates(DuplicateScanRequest::default(), &f.runtime, &token)
        .unwrap();
    let review = f.lib.begin_duplicate_review(&scan.id, &token).unwrap();
    let preview = f
        .lib
        .preview_duplicate_merge(&review.id, 0, vec![review.groups[0].id.clone()], &token)
        .unwrap();
    wav(&f.paths[1], 79);
    assert!(f.lib.apply_duplicate_merge(&preview.id, &token).is_err());
    let f = Fixture::new();
    let scan = f
        .lib
        .scan_duplicates(DuplicateScanRequest::default(), &f.runtime, &token)
        .unwrap();
    let review = f.lib.begin_duplicate_review(&scan.id, &token).unwrap();
    let preview = f
        .lib
        .preview_duplicate_merge(&review.id, 0, vec![review.groups[0].id.clone()], &token)
        .unwrap();
    let applied = f.lib.apply_duplicate_merge(&preview.id, &token).unwrap();
    f.lib.set_archived(&f.ids[0], true).unwrap();
    f.lib.set_archived(&f.ids[0], false).unwrap();
    assert!(f.lib.undo_duplicate_merge(&applied.id, &token).is_err());
}

#[test]
fn duplicate_merge_cancellation_and_pending_file_transfers_never_partially_archive() {
    use cuerithm_library::{DuplicateReviewEdit, GroupEdit};
    let f = Fixture::new();
    let token = CancellationToken::default();
    let scan = f
        .lib
        .scan_duplicates(DuplicateScanRequest::default(), &f.runtime, &token)
        .unwrap();
    let review = f.lib.begin_duplicate_review(&scan.id, &token).unwrap();
    let review = f
        .lib
        .edit_duplicate_review(
            &review.id,
            0,
            DuplicateReviewEdit::Group {
                group_id: review.groups[0].id.clone(),
                edit: GroupEdit::UseTags {
                    track_id: Some(f.ids[1].clone()),
                },
            },
            &token,
        )
        .unwrap();
    let before = f.content();
    let preview = f
        .lib
        .preview_duplicate_merge(
            &review.id,
            review.revision,
            vec![review.groups[0].id.clone()],
            &token,
        )
        .unwrap();
    assert_eq!(preview.plan.groups[0].copies.len(), 1);
    assert!(
        f.lib
            .apply_duplicate_merge(&preview.id, &token)
            .unwrap_err()
            .to_string()
            .contains("file publication")
    );
    let cancel = CancellationToken::default();
    cancel.cancel();
    assert!(f.lib.apply_duplicate_merge(&preview.id, &cancel).is_err());
    assert_eq!(f.content(), before);
}

#[test]
fn duplicate_merge_and_undo_transaction_failures_leave_no_partial_changes_or_consumed_token() {
    let f = Fixture::new();
    let token = CancellationToken::default();
    f.playlist("affected", vec![f.ids[0].clone(), f.ids[1].clone()]);
    let scan = f
        .lib
        .scan_duplicates(DuplicateScanRequest::default(), &f.runtime, &token)
        .unwrap();
    let review = f.lib.begin_duplicate_review(&scan.id, &token).unwrap();
    let preview = f
        .lib
        .preview_duplicate_merge(&review.id, 0, vec![review.groups[0].id.clone()], &token)
        .unwrap();
    let before = f.content();
    let stamp = f.lib.duplicate_ignores().unwrap().stamp;
    let conn = rusqlite::Connection::open(&f.db).unwrap();
    conn.execute_batch("CREATE TRIGGER fail_merge_insert BEFORE INSERT ON playlist_members BEGIN SELECT RAISE(ABORT,'injected membership failure'); END;").unwrap();
    assert!(f.lib.apply_duplicate_merge(&preview.id, &token).is_err());
    assert_eq!(f.content(), before);
    assert_eq!(f.lib.duplicate_ignores().unwrap().stamp, stamp);
    assert_eq!(
        f.lib.manual_playlist_members("affected").unwrap(),
        vec![f.ids[0].clone(), f.ids[1].clone()]
    );
    conn.execute_batch("DROP TRIGGER fail_merge_insert;")
        .unwrap();
    let applied = f.lib.apply_duplicate_merge(&preview.id, &token).unwrap();
    let after = f.content();
    let after_stamp = f.lib.duplicate_ignores().unwrap().stamp;
    conn.execute_batch("CREATE TRIGGER fail_undo_delete BEFORE DELETE ON playlist_members BEGIN SELECT RAISE(ABORT,'injected undo failure'); END;").unwrap();
    assert!(f.lib.undo_duplicate_merge(&applied.id, &token).is_err());
    assert_eq!(f.content(), after);
    assert_eq!(f.lib.duplicate_ignores().unwrap().stamp, after_stamp);
    conn.execute_batch("DROP TRIGGER fail_undo_delete;")
        .unwrap();
    f.lib.undo_duplicate_merge(&applied.id, &token).unwrap();
    assert_eq!(f.content(), before);
}

#[test]
fn duplicate_merge_can_keep_an_archived_playlist_member_without_archiving_ignored_members() {
    use cuerithm_library::{DuplicateReviewEdit, GroupEdit};
    let f = Fixture::new();
    let token = CancellationToken::default();
    f.playlist(
        "picked",
        vec![f.ids[3].clone(), f.ids[0].clone(), f.ids[1].clone()],
    );
    let request = DuplicateScanRequest {
        scope: DuplicateScanScope::SelectedPlaylists,
        playlist_ids: vec!["picked".into()],
        ..Default::default()
    };
    let scan = f.lib.scan_duplicates(request, &f.runtime, &token).unwrap();
    let review = f.lib.begin_duplicate_review(&scan.id, &token).unwrap();
    let edit = |edit| DuplicateReviewEdit::Group {
        group_id: review.groups[0].id.clone(),
        edit,
    };
    let review = f
        .lib
        .edit_duplicate_review(
            &review.id,
            0,
            edit(GroupEdit::Select {
                track_id: f.ids[3].clone(),
            }),
            &token,
        )
        .unwrap();
    let review = f
        .lib
        .edit_duplicate_review(
            &review.id,
            1,
            edit(GroupEdit::ToggleIgnore {
                track_id: f.ids[1].clone(),
            }),
            &token,
        )
        .unwrap();
    let before = f.content();
    let preview = f
        .lib
        .preview_duplicate_merge(
            &review.id,
            review.revision,
            vec![review.groups[0].id.clone()],
            &token,
        )
        .unwrap();
    let applied = f.lib.apply_duplicate_merge(&preview.id, &token).unwrap();
    assert_eq!(applied.archived_ids, vec![f.ids[0].clone()]);
    assert_eq!(applied.restored_ids, vec![f.ids[3].clone()]);
    assert_eq!(
        f.lib.manual_playlist_members("picked").unwrap(),
        vec![f.ids[3].clone(), f.ids[1].clone()]
    );
    f.lib.undo_duplicate_merge(&applied.id, &token).unwrap();
    assert_eq!(f.content(), before);
}

#[test]
fn duplicate_merge_preserves_smart_definitions_and_rebuilds_members_after_apply_and_undo() {
    use cuerithm_library::SmartDefinition;
    use cuerithm_smartlists::{Operator, RuleValue, SmartRule};
    let f = Fixture::new();
    let token = CancellationToken::default();
    f.lib
        .apply_playlist_edit(
            PlaylistEdit::Create {
                id: Some("smart".into()),
                parent_id: None,
                kind: PlaylistKind::Smartlist,
                name: "Smart".into(),
                definition: Some(SmartDefinition {
                    match_all: true,
                    rules: vec![SmartRule {
                        id: "r".into(),
                        field: "title".into(),
                        operator: Operator::StringEquals,
                        value: RuleValue::Text("Signal".into()),
                        or: false,
                    }],
                }),
            },
            f.lib.playlist_state().unwrap().revision,
        )
        .unwrap();
    let now = || {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as i64
    };
    f.lib.refresh_smart_lists(now()).unwrap();
    assert_eq!(f.lib.playlist_members("smart").unwrap().len(), 2);
    let tree = f.lib.playlist_state().unwrap().nodes;
    let scan = f
        .lib
        .scan_duplicates(DuplicateScanRequest::default(), &f.runtime, &token)
        .unwrap();
    let review = f.lib.begin_duplicate_review(&scan.id, &token).unwrap();
    let preview = f
        .lib
        .preview_duplicate_merge(&review.id, 0, vec![review.groups[0].id.clone()], &token)
        .unwrap();
    assert!(preview.playlists.is_empty());
    let applied = f.lib.apply_duplicate_merge(&preview.id, &token).unwrap();
    assert_eq!(f.lib.playlist_state().unwrap().nodes, tree);
    assert_eq!(
        rusqlite::Connection::open(&f.db)
            .unwrap()
            .query_row(
                "SELECT state FROM smart_playlist_status WHERE playlist_id='smart'",
                [],
                |r| r.get::<_, String>(0)
            )
            .unwrap(),
        "stale"
    );
    f.lib.refresh_smart_lists(now()).unwrap();
    assert_eq!(
        f.lib.playlist_members("smart").unwrap(),
        vec![f.ids[0].clone()]
    );
    f.lib.undo_duplicate_merge(&applied.id, &token).unwrap();
    f.lib.refresh_smart_lists(now()).unwrap();
    assert_eq!(f.lib.playlist_members("smart").unwrap().len(), 2);
    assert_eq!(f.lib.playlist_state().unwrap().nodes, tree);
}

#[test]
fn duplicate_merge_undo_uses_a_supported_journal_version_across_implementation_updates() {
    let f = Fixture::new();
    let token = CancellationToken::default();
    let scan = f
        .lib
        .scan_duplicates(DuplicateScanRequest::default(), &f.runtime, &token)
        .unwrap();
    let review = f.lib.begin_duplicate_review(&scan.id, &token).unwrap();
    let preview = f
        .lib
        .preview_duplicate_merge(&review.id, 0, vec![review.groups[0].id.clone()], &token)
        .unwrap();
    let before = f.content();
    let result = f.lib.apply_duplicate_merge(&preview.id, &token).unwrap();
    let conn = rusqlite::Connection::open(&f.db).unwrap();
    conn.execute("UPDATE maintenance_scans SET payload_json=json_set(payload_json,'$.schema','future-v2') WHERE id=?1",[&result.id]).unwrap();
    assert!(
        f.lib
            .undo_duplicate_merge(&result.id, &token)
            .unwrap_err()
            .to_string()
            .contains("Unsupported")
    );
    conn.execute("UPDATE maintenance_scans SET payload_json=json_set(payload_json,'$.schema','duplicate-library-merge-v1','$.logic','older-compatible-build') WHERE id=?1",[&result.id]).unwrap();
    f.lib.undo_duplicate_merge(&result.id, &token).unwrap();
    assert_eq!(f.content(), before);
}

#[test]
fn duplicate_merge_batches_disjoint_groups_with_interleaved_playlist_references() {
    let mut f = Fixture::new();
    let token = CancellationToken::default();
    for index in 4..6 {
        let path = f.dir.path().join(format!("track-{index}.wav"));
        wav(&path, 71);
        let job = f
            .lib
            .enqueue(
                std::slice::from_ref(&path),
                f.lib.settings().unwrap().revision,
            )
            .unwrap()[0]
            .clone();
        let id = f
            .lib
            .jobs()
            .unwrap()
            .into_iter()
            .find(|j| j.id == job)
            .unwrap()
            .track_id;
        rusqlite::Connection::open(&f.db).unwrap().execute("UPDATE tracks SET title='Parallel',artist='South',duration=20,state='ready' WHERE id=?1",[&id]).unwrap();
        f.ids.push(id);
        f.paths.push(path);
    }
    f.playlist(
        "interleaved",
        vec![
            f.ids[0].clone(),
            f.ids[4].clone(),
            f.ids[2].clone(),
            f.ids[1].clone(),
            f.ids[5].clone(),
        ],
    );
    let before = f.content();
    let scan = f
        .lib
        .scan_duplicates(DuplicateScanRequest::default(), &f.runtime, &token)
        .unwrap();
    assert_eq!(scan.groups.len(), 2);
    let review = f.lib.begin_duplicate_review(&scan.id, &token).unwrap();
    let preview = f
        .lib
        .preview_duplicate_merge(
            &review.id,
            0,
            review.groups.iter().rev().map(|g| g.id.clone()).collect(),
            &token,
        )
        .unwrap();
    assert_eq!(
        preview.playlists[0].after,
        vec![f.ids[2].clone(), f.ids[0].clone(), f.ids[4].clone()]
    );
    let result = f.lib.apply_duplicate_merge(&preview.id, &token).unwrap();
    assert_eq!(
        result
            .archived_ids
            .into_iter()
            .collect::<std::collections::BTreeSet<_>>(),
        [f.ids[1].clone(), f.ids[5].clone()]
            .into_iter()
            .collect::<std::collections::BTreeSet<_>>()
    );
    assert_eq!(
        f.lib.manual_playlist_members("interleaved").unwrap(),
        preview.playlists[0].after
    );
    f.lib.undo_duplicate_merge(&result.id, &token).unwrap();
    assert_eq!(f.content(), before);
}
