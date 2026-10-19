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
