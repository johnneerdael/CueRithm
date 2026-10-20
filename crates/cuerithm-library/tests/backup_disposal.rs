use cuerithm_library::{ExtensionMode, Library, UnusedRemovalResult, UnusedScanRequest};
use cuerithm_media::CancellationToken;
use std::path::{Path, PathBuf};
struct Fixture {
    dir: tempfile::TempDir,
    lib: Library,
    db: PathBuf,
    music: PathBuf,
    removed: UnusedRemovalResult,
}
fn files(root: &Path) -> Vec<PathBuf> {
    let mut out = vec![];
    if root.is_dir() {
        for entry in std::fs::read_dir(root).unwrap() {
            let entry = entry.unwrap();
            if entry.file_type().unwrap().is_dir() {
                out.extend(files(&entry.path()));
            } else {
                out.push(entry.path());
            }
        }
    }
    out
}
impl Fixture {
    fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let music = dir.path().join("music");
        std::fs::create_dir(&music).unwrap();
        std::fs::write(music.join("a.flac"), b"first").unwrap();
        std::fs::write(music.join("b.flac"), b"second").unwrap();
        std::fs::write(music.join("keep.txt"), b"keep").unwrap();
        let db = dir.path().join("app/db");
        let lib = Library::open(&db).unwrap();
        let token = CancellationToken::default();
        let scan = lib
            .scan_unused_files(
                UnusedScanRequest {
                    root: music.clone(),
                    extension_mode: ExtensionMode::Include,
                    extensions: vec!["flac".into()],
                },
                &token,
            )
            .unwrap();
        let plan = lib
            .preview_unused_removal(
                &scan.id,
                &scan.files.iter().map(|f| f.id.clone()).collect::<Vec<_>>(),
                &token,
            )
            .unwrap();
        let removed = lib.apply_unused_removal(&plan.id, &token).unwrap();
        Self {
            dir,
            lib,
            db,
            music,
            removed,
        }
    }
    fn backups(&self) -> Vec<PathBuf> {
        files(&self.lib.data_dir().join("maintenance/removal"))
    }
}
#[test]
fn reviewed_disposal_releases_only_owned_backups_and_explicitly_removes_restore_availability() {
    let f = Fixture::new();
    let token = CancellationToken::default();
    let initial = f.backups();
    assert_eq!(initial.len(), 2);
    let preview = f
        .lib
        .preview_unused_backup_disposal(std::slice::from_ref(&f.removed.id), &token)
        .unwrap();
    assert_eq!(preview.size_bytes, 11);
    assert_eq!(preview.files.len(), 2);
    assert_eq!(preview.restores_lost, vec![f.removed.id.clone()]);
    assert_eq!(f.backups(), initial);
    std::fs::write(f.music.join("a.flac"), b"newer user file").unwrap();
    let result = f
        .lib
        .apply_unused_backup_disposal(&preview.id, &token)
        .unwrap();
    assert_eq!(result.state, "committed");
    assert!(!result.cleanup_pending);
    assert_eq!(result.size_bytes, 11);
    assert!(f.backups().is_empty());
    assert_eq!(
        std::fs::read(f.music.join("a.flac")).unwrap(),
        b"newer user file"
    );
    assert_eq!(std::fs::read(f.music.join("keep.txt")).unwrap(), b"keep");
    assert!(f.lib.restore_unused_removal(&f.removed.id, &token).is_err());
    let history = f.lib.unused_removal_history().unwrap();
    let entry = history
        .iter()
        .find(|h| h.result.id == f.removed.id)
        .unwrap();
    assert!(!entry.restore_available);
    assert!(entry.backups_disposed);
    assert!(
        f.lib
            .apply_unused_backup_disposal(&preview.id, &token)
            .is_err()
    );
    assert!(f.dir.path().exists());
}
#[test]
fn restored_operations_share_one_backup_set_and_disposal_does_not_change_restored_files() {
    let mut f = Fixture::new();
    let token = CancellationToken::default();
    f.lib.restore_unused_removal(&f.removed.id, &token).unwrap();
    // A restored removal keeps its recovery copies until they are disposed of, and says so in the history.
    let restored = f.lib.unused_removal_history().unwrap();
    let entry = restored
        .iter()
        .find(|h| h.result.id == f.removed.id)
        .unwrap();
    assert_eq!(entry.state, "undone");
    assert!(!entry.backups_disposed && !entry.cleanup_pending);
    assert_eq!(f.backups().len(), 2);
    let preview = f
        .lib
        .preview_unused_backup_disposal(std::slice::from_ref(&f.removed.id), &token)
        .unwrap();
    assert!(preview.restores_lost.is_empty());
    assert_eq!(preview.size_bytes, 11);
    let disposed = f
        .lib
        .apply_unused_backup_disposal(&preview.id, &token)
        .unwrap();
    assert!(f.backups().is_empty());
    assert!(
        f.lib
            .unused_removal_history()
            .unwrap()
            .iter()
            .find(|h| h.result.id == f.removed.id)
            .unwrap()
            .backups_disposed
    );
    drop(f.lib);
    f.lib = Library::open(&f.db).unwrap();
    assert!(
        f.lib
            .unused_backup_disposal_history()
            .unwrap()
            .iter()
            .any(|r| r.id == disposed.id && r.state == "committed")
    );
    assert_eq!(std::fs::read(f.music.join("a.flac")).unwrap(), b"first");
    assert_eq!(std::fs::read(f.music.join("b.flac")).unwrap(), b"second");
}
#[test]
fn a_restore_after_preview_invalidates_backup_disposal_without_losing_backups() {
    let f = Fixture::new();
    let token = CancellationToken::default();
    let preview = f
        .lib
        .preview_unused_backup_disposal(std::slice::from_ref(&f.removed.id), &token)
        .unwrap();
    f.lib.restore_unused_removal(&f.removed.id, &token).unwrap();
    assert!(
        f.lib
            .apply_unused_backup_disposal(&preview.id, &token)
            .is_err()
    );
    assert_eq!(f.backups().len(), 2);
    let fresh = f
        .lib
        .preview_unused_backup_disposal(std::slice::from_ref(&f.removed.id), &token)
        .unwrap();
    f.lib
        .apply_unused_backup_disposal(&fresh.id, &token)
        .unwrap();
    assert!(f.backups().is_empty());
}
#[test]
fn sql_failure_returns_all_backups_and_keeps_restore_available_for_retry() {
    let f = Fixture::new();
    let token = CancellationToken::default();
    let preview = f
        .lib
        .preview_unused_backup_disposal(std::slice::from_ref(&f.removed.id), &token)
        .unwrap();
    let conn = rusqlite::Connection::open(&f.db).unwrap();
    conn.execute_batch("CREATE TRIGGER fail_dispose BEFORE UPDATE ON maintenance_scans WHEN NEW.kind='unused_backup_disposal' AND json_extract(NEW.payload_json,'$.result.state')='committed' BEGIN SELECT RAISE(ABORT,'injected disposal commit failure'); END;").unwrap();
    assert!(
        f.lib
            .apply_unused_backup_disposal(&preview.id, &token)
            .is_err()
    );
    assert_eq!(f.backups().len(), 2);
    assert!(
        f.lib
            .unused_removal_history()
            .unwrap()
            .iter()
            .find(|h| h.result.id == f.removed.id)
            .unwrap()
            .restore_available
    );
    conn.execute_batch("DROP TRIGGER fail_dispose;").unwrap();
    f.lib
        .apply_unused_backup_disposal(&preview.id, &token)
        .unwrap();
    assert!(f.backups().is_empty());
}
#[test]
fn changed_backups_invalid_selections_and_cancellation_cannot_dispose_files() {
    let f = Fixture::new();
    let token = CancellationToken::default();
    assert!(f.lib.preview_unused_backup_disposal(&[], &token).is_err());
    assert!(
        f.lib
            .preview_unused_backup_disposal(&["unknown".into()], &token)
            .is_err()
    );
    let preview = f
        .lib
        .preview_unused_backup_disposal(std::slice::from_ref(&f.removed.id), &token)
        .unwrap();
    let backup = f.backups()[0].clone();
    std::fs::write(&backup, b"new backup contents").unwrap();
    assert!(
        f.lib
            .apply_unused_backup_disposal(&preview.id, &token)
            .is_err()
    );
    assert_eq!(std::fs::read(&backup).unwrap(), b"new backup contents");
    assert_eq!(f.backups().len(), 2);
    let cancelled = CancellationToken::default();
    cancelled.cancel();
    assert!(
        f.lib
            .apply_unused_backup_disposal(&preview.id, &cancelled)
            .is_err()
    );
}
