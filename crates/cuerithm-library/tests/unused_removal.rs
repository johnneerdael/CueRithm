use cuerithm_library::{ExtensionMode, Library, UnusedRemovalRole, UnusedScanRequest};
use cuerithm_media::CancellationToken;
use std::path::{Path, PathBuf};
struct Fixture {
    dir: tempfile::TempDir,
    lib: Library,
    db: PathBuf,
    music: PathBuf,
}
impl Fixture {
    fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let music = dir.path().join("music");
        std::fs::create_dir(&music).unwrap();
        let db = dir.path().join("app/db");
        let lib = Library::open(&db).unwrap();
        Self {
            dir,
            lib,
            db,
            music,
        }
    }
    fn file(&self, name: &str, bytes: &[u8]) -> PathBuf {
        let path = self.music.join(name);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, bytes).unwrap();
        path
    }
    fn scan(&self) -> cuerithm_library::UnusedScanReport {
        self.lib
            .scan_unused_files(
                UnusedScanRequest {
                    root: self.music.clone(),
                    extension_mode: ExtensionMode::Include,
                    extensions: vec!["flac".into()],
                },
                &CancellationToken::default(),
            )
            .unwrap()
    }
    fn add(&self, path: &Path) {
        self.lib
            .enqueue(&[path.to_owned()], self.lib.settings().unwrap().revision)
            .unwrap();
    }
    fn state(&self) -> serde_json::Value {
        let conn = rusqlite::Connection::open(&self.db).unwrap();
        let mut result = serde_json::Map::new();
        for table in ["tracks", "jobs", "cue_revisions", "playlist_members"] {
            let mut q = conn
                .prepare(&format!("SELECT * FROM {table} ORDER BY rowid"))
                .unwrap();
            let columns = q.column_count();
            let rows = q
                .query_map([], |row| {
                    Ok((0..columns)
                        .map(|i| format!("{:?}", row.get_ref(i).unwrap()))
                        .collect::<Vec<_>>())
                })
                .unwrap()
                .collect::<rusqlite::Result<Vec<_>>>()
                .unwrap();
            result.insert(table.into(), serde_json::json!(rows));
        }
        result.into()
    }
}
#[test]
fn preview_names_selected_files_and_only_the_first_lexicon_sibling_without_removing_anything() {
    let f = Fixture::new();
    let source = f.file("nested/loose.flac", b"authored source");
    let serato = f.file("nested/loose.1.2.serato-stems", b"first sibling");
    let vdj = f.file("nested/loose.flac.vdjstems", b"second sibling");
    let kept = f.file("kept.flac", b"referenced");
    f.add(&kept);
    let before = f.state();
    let scan = f.scan();
    assert_eq!(scan.files.len(), 1);
    let preview = f
        .lib
        .preview_unused_removal(
            &scan.id,
            &[scan.files[0].id.clone()],
            &CancellationToken::default(),
        )
        .unwrap();
    assert_eq!(preview.files.len(), 2);
    assert_eq!(preview.files[0].role, UnusedRemovalRole::Selected);
    assert_eq!(preview.files[1].role, UnusedRemovalRole::Sibling);
    assert!(
        preview.files[1]
            .display_path
            .ends_with("loose.1.2.serato-stems")
    );
    assert_eq!(preview.size_bytes, 28);
    assert!(source.exists() && serato.exists() && vdj.exists() && kept.exists());
    assert_eq!(f.state(), before);
}
#[test]
fn reviewed_removal_and_restart_discovered_restore_preserve_library_state_and_exact_file_bytes() {
    let mut f = Fixture::new();
    let source = f.file("nested/loose.flac", b"source bytes");
    let sibling = f.file("nested/loose.flac.vdjstems", b"stem bytes");
    let kept = f.file("kept.flac", b"live");
    f.add(&kept);
    let before = f.state();
    let scan = f.scan();
    let token = CancellationToken::default();
    let preview = f
        .lib
        .preview_unused_removal(&scan.id, &[scan.files[0].id.clone()], &token)
        .unwrap();
    let applied = f.lib.apply_unused_removal(&preview.id, &token).unwrap();
    assert!(!source.exists() && !sibling.exists());
    assert_eq!(std::fs::read(&kept).unwrap(), b"live");
    assert_eq!(f.state(), before);
    assert!(f.lib.apply_unused_removal(&preview.id, &token).is_err());
    drop(f.lib);
    f.lib = Library::open(&f.db).unwrap();
    let history = f.lib.unused_removal_history().unwrap();
    assert!(
        history
            .iter()
            .any(|h| h.result.id == applied.id && h.state == "committed")
    );
    let restored = f.lib.restore_unused_removal(&applied.id, &token).unwrap();
    assert!(restored.restored);
    assert_eq!(std::fs::read(&source).unwrap(), b"source bytes");
    assert_eq!(std::fs::read(&sibling).unwrap(), b"stem bytes");
    assert_eq!(f.state(), before);
    assert!(f.lib.restore_unused_removal(&applied.id, &token).is_err());
    assert!(f.dir.path().exists());
}

#[test]
fn stale_and_partial_reports_unknown_ids_and_referenced_siblings_cannot_be_removed() {
    let f = Fixture::new();
    let a = f.file("loose.flac", b"source");
    let sibling = f.file("loose.flac.vdjstems", b"sidecar");
    f.add(&sibling);
    let token = CancellationToken::default();
    let scan = f.scan();
    assert!(
        f.lib
            .preview_unused_removal(&scan.id, &[scan.files[0].id.clone()], &token)
            .is_err()
    );
    assert!(
        f.lib
            .preview_unused_removal(&scan.id, &["unknown".into()], &token)
            .is_err()
    );
    assert!(f.lib.preview_unused_removal(&scan.id, &[], &token).is_err());
    assert!(a.exists() && sibling.exists());
    let f = Fixture::new();
    let path = f.file("loose.flac", b"source");
    let scan = f.scan();
    let preview = f
        .lib
        .preview_unused_removal(&scan.id, &[scan.files[0].id.clone()], &token)
        .unwrap();
    std::fs::write(&path, b"later!").unwrap();
    assert!(f.lib.apply_unused_removal(&preview.id, &token).is_err());
    assert_eq!(std::fs::read(&path).unwrap(), b"later!");
    let conn = rusqlite::Connection::open(&f.db).unwrap();
    conn.execute("UPDATE maintenance_scans SET payload_json=json_set(payload_json,'$.report.complete',json('false')) WHERE id=?1",[&scan.id]).unwrap();
    assert!(
        f.lib
            .preview_unused_removal(&scan.id, &[scan.files[0].id.clone()], &token)
            .is_err()
    );
}

#[test]
fn later_library_references_and_same_content_replacements_invalidate_the_plan() {
    for replace in [false, true] {
        let f = Fixture::new();
        let path = f.file("loose.flac", b"source");
        let token = CancellationToken::default();
        let scan = f.scan();
        let preview = f
            .lib
            .preview_unused_removal(&scan.id, &[scan.files[0].id.clone()], &token)
            .unwrap();
        if replace {
            let newer = f.file("replacement", b"source");
            std::fs::remove_file(&path).unwrap();
            std::fs::rename(newer, &path).unwrap();
        } else {
            f.add(&path);
        }
        assert!(f.lib.apply_unused_removal(&preview.id, &token).is_err());
        assert_eq!(std::fs::read(&path).unwrap(), b"source");
    }
}

#[test]
fn sql_failure_and_cancellation_leave_all_sources_and_library_state_intact() {
    let f = Fixture::new();
    let a = f.file("a.flac", b"first");
    let b = f.file("b.flac", b"second");
    let token = CancellationToken::default();
    let scan = f.scan();
    let selected = scan.files.iter().map(|f| f.id.clone()).collect::<Vec<_>>();
    let preview = f
        .lib
        .preview_unused_removal(&scan.id, &selected, &token)
        .unwrap();
    let before = f.state();
    let conn = rusqlite::Connection::open(&f.db).unwrap();
    conn.execute_batch("CREATE TRIGGER fail_unused_commit BEFORE UPDATE ON maintenance_scans WHEN NEW.kind='unused_removal_operation' AND json_extract(NEW.payload_json,'$.state')='committed' BEGIN SELECT RAISE(ABORT,'injected unused commit failure'); END;").unwrap();
    assert!(f.lib.apply_unused_removal(&preview.id, &token).is_err());
    assert_eq!(std::fs::read(&a).unwrap(), b"first");
    assert_eq!(std::fs::read(&b).unwrap(), b"second");
    assert_eq!(f.state(), before);
    conn.execute_batch("DROP TRIGGER fail_unused_commit;")
        .unwrap();
    let cancelled = CancellationToken::default();
    cancelled.cancel();
    assert!(f.lib.apply_unused_removal(&preview.id, &cancelled).is_err());
    assert!(a.exists() && b.exists());
    let applied = f.lib.apply_unused_removal(&preview.id, &token).unwrap();
    f.lib.restore_unused_removal(&applied.id, &token).unwrap();
}

#[test]
fn restore_refuses_new_files_at_original_paths_and_remains_retryable() {
    let f = Fixture::new();
    let path = f.file("loose.flac", b"source");
    let token = CancellationToken::default();
    let scan = f.scan();
    let preview = f
        .lib
        .preview_unused_removal(&scan.id, &[scan.files[0].id.clone()], &token)
        .unwrap();
    let applied = f.lib.apply_unused_removal(&preview.id, &token).unwrap();
    std::fs::write(&path, b"new user file").unwrap();
    assert!(f.lib.restore_unused_removal(&applied.id, &token).is_err());
    assert_eq!(std::fs::read(&path).unwrap(), b"new user file");
    std::fs::remove_file(&path).unwrap();
    f.lib.restore_unused_removal(&applied.id, &token).unwrap();
    assert_eq!(std::fs::read(&path).unwrap(), b"source");
}

#[cfg(unix)]
#[test]
fn symlinked_parents_and_referenced_hardlink_aliases_are_never_removed() {
    use std::os::unix::fs::symlink;
    let f = Fixture::new();
    let path = f.file("nested/loose.flac", b"source");
    let scan = f.scan();
    let token = CancellationToken::default();
    let preview = f
        .lib
        .preview_unused_removal(&scan.id, &[scan.files[0].id.clone()], &token)
        .unwrap();
    let outside = f.dir.path().join("outside");
    std::fs::rename(f.music.join("nested"), &outside).unwrap();
    symlink(&outside, f.music.join("nested")).unwrap();
    assert!(f.lib.apply_unused_removal(&preview.id, &token).is_err());
    assert_eq!(std::fs::read(&path).unwrap(), b"source");
    let f = Fixture::new();
    let path = f.file("loose.flac", b"same");
    let reference = f.file("kept.dat", b"same");
    f.add(&reference);
    let scan = f.scan();
    let preview = f
        .lib
        .preview_unused_removal(&scan.id, &[scan.files[0].id.clone()], &token)
        .unwrap();
    std::fs::remove_file(&path).unwrap();
    std::fs::hard_link(&reference, &path).unwrap();
    assert!(f.lib.apply_unused_removal(&preview.id, &token).is_err());
    assert!(path.exists() && reference.exists());
}
