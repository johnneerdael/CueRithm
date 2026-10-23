use cuerithm_dj_sync::import::*;
use cuerithm_media::CancellationToken;
use rusqlite::Connection;
use std::{fs, path::Path};
fn hash(path: &Path) -> String {
    use sha2::{Digest, Sha256};
    format!("{:x}", Sha256::digest(fs::read(path).unwrap()))
}
#[test]
fn snapshot_reads_committed_wal_without_source_writes() {
    let source = tempfile::tempdir().unwrap();
    let workspace = tempfile::tempdir().unwrap();
    let db_path = source.path().join("mixxxdb.sqlite");
    let db = Connection::open(&db_path).unwrap();
    db.execute_batch("PRAGMA journal_mode=WAL; PRAGMA wal_autocheckpoint=0; CREATE TABLE songs(id INTEGER); INSERT INTO songs VALUES(42);").unwrap();
    let wal = source.path().join("mixxxdb.sqlite-wal");
    let before = (hash(&db_path), hash(&wal));
    let request = SourceCaptureRequest::from_selected_root(
        SourceProvider::Mixxx,
        source.path(),
        vec![ArtifactRequest::sqlite("mixxxdb.sqlite")],
    )
    .unwrap();
    let snapshot = capture_source(
        &request,
        workspace.path(),
        &ImportLimits::default(),
        &CancellationToken::default(),
    )
    .unwrap();
    let copied = Connection::open_with_flags(
        snapshot.artifact("mixxxdb.sqlite").unwrap(),
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
    )
    .unwrap();
    assert_eq!(
        copied
            .query_row("SELECT id FROM songs", [], |r| r.get::<_, i64>(0))
            .unwrap(),
        42
    );
    assert_eq!(before, (hash(&db_path), hash(&wal)));
    snapshot
        .validate_source(&CancellationToken::default())
        .unwrap();
    db.execute("INSERT INTO songs VALUES(43)", []).unwrap();
    assert!(
        snapshot
            .validate_source(&CancellationToken::default())
            .is_err()
    );
}
#[test]
fn snapshot_rejects_escape_symlinks_limits_and_cancellation() {
    let source = tempfile::tempdir().unwrap();
    let workspace = tempfile::tempdir().unwrap();
    assert!(
        SourceCaptureRequest::from_selected_root(
            SourceProvider::M3u,
            source.path(),
            vec![ArtifactRequest::stable("../outside")]
        )
        .is_err()
    );
    fs::write(source.path().join("list.m3u8"), b"track.flac\n").unwrap();
    let request = SourceCaptureRequest::from_selected_root(
        SourceProvider::M3u,
        source.path(),
        vec![ArtifactRequest::stable("list.m3u8")],
    )
    .unwrap();
    let cancel = CancellationToken::default();
    cancel.cancel();
    assert!(
        capture_source(
            &request,
            workspace.path(),
            &ImportLimits::default(),
            &cancel
        )
        .is_err()
    );
    let limits = ImportLimits {
        snapshot_bytes: 3,
        ..Default::default()
    };
    assert!(
        capture_source(
            &request,
            workspace.path(),
            &limits,
            &CancellationToken::default()
        )
        .is_err()
    );
    assert_eq!(fs::read_dir(workspace.path()).unwrap().count(), 0);
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(workspace.path(), source.path().join("escape")).unwrap();
        assert!(
            SourceCaptureRequest::from_selected_root(
                SourceProvider::M3u,
                source.path(),
                vec![ArtifactRequest::stable("escape/file")]
            )
            .is_err()
        );
    }
}

#[test]
fn changing_one_artifact_invalidates_the_entire_captured_set() {
    let source = tempfile::tempdir().unwrap();
    let workspace = tempfile::tempdir().unwrap();
    fs::write(source.path().join("one.json"), b"one").unwrap();
    fs::write(source.path().join("two.json"), b"two").unwrap();
    let request = SourceCaptureRequest::from_selected_root(
        SourceProvider::DjStudio,
        source.path(),
        vec![
            ArtifactRequest::stable("one.json"),
            ArtifactRequest::stable("two.json"),
        ],
    )
    .unwrap();
    let snapshot = capture_source(
        &request,
        workspace.path(),
        &ImportLimits::default(),
        &CancellationToken::default(),
    )
    .unwrap();
    fs::write(source.path().join("two.json"), b"NEW").unwrap();
    assert!(
        snapshot
            .validate_source(&CancellationToken::default())
            .is_err()
    );
    assert_eq!(
        fs::read(snapshot.artifact("two.json").unwrap()).unwrap(),
        b"two"
    );
}

#[test]
fn byte_identical_source_replacement_with_restored_timestamp_invalidates_snapshot() {
    let source = tempfile::tempdir().unwrap();
    let workspace = tempfile::tempdir().unwrap();
    let path = source.path().join("source.bin");
    fs::write(&path, b"same bytes").unwrap();
    let modified = path.metadata().unwrap().modified().unwrap();
    let request = SourceCaptureRequest::from_selected_root(
        SourceProvider::M3u,
        source.path(),
        vec![ArtifactRequest::stable("source.bin")],
    )
    .unwrap();
    let snapshot = capture_source(
        &request,
        workspace.path(),
        &ImportLimits::default(),
        &CancellationToken::default(),
    )
    .unwrap();
    fs::rename(&path, source.path().join("parked.bin")).unwrap();
    fs::write(&path, b"same bytes").unwrap();
    fs::File::options()
        .write(true)
        .open(&path)
        .unwrap()
        .set_times(fs::FileTimes::new().set_modified(modified))
        .unwrap();
    assert!(
        snapshot
            .validate_source(&CancellationToken::default())
            .is_err(),
        "replacement must invalidate captured source authority even if bytes and modified time match"
    );
}

#[test]
fn explicitly_selected_additional_roots_keep_original_file_guards() {
    let main = tempfile::tempdir().unwrap();
    let external = tempfile::tempdir().unwrap();
    let workspace = tempfile::tempdir().unwrap();
    std::fs::write(main.path().join("database.xml"), b"main invented library").unwrap();
    std::fs::write(
        external.path().join("database.xml"),
        b"external invented library",
    )
    .unwrap();
    let request = SourceCaptureRequest::from_selected_root(
        SourceProvider::VirtualDj,
        main.path(),
        vec![
            ArtifactRequest::stable("database.xml"),
            ArtifactRequest::stable("database.xml")
                .with_selected_root("external-volume", external.path()),
        ],
    )
    .unwrap();
    let snapshot = capture_source(
        &request,
        workspace.path(),
        &ImportLimits::default(),
        &CancellationToken::default(),
    )
    .unwrap();
    assert_eq!(snapshot.source_root(), main.path());
    assert_eq!(
        snapshot.original_artifact_path("external-volume").unwrap(),
        external.path().join("database.xml")
    );
    assert_eq!(
        std::fs::read(snapshot.artifact("external-volume").unwrap()).unwrap(),
        b"external invented library"
    );
    std::fs::write(
        external.path().join("database.xml"),
        b"later external library edit",
    )
    .unwrap();
    assert!(
        snapshot
            .validate_source(&CancellationToken::default())
            .is_err()
    );
    assert_eq!(
        std::fs::read(main.path().join("database.xml")).unwrap(),
        b"main invented library"
    );
}
