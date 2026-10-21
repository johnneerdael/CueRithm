use cuerithm_media::{CancellationToken, file_sha256};
use cuerithm_rekordbox::{DatabaseKey, ReadOnlyDatabase};
use std::path::Path;
fn key() -> DatabaseKey {
    DatabaseKey::new(b"authored test key with '\0 bytes").unwrap()
}
fn apply(conn: &rusqlite::Connection, key: &[u8]) {
    let rc =
        unsafe { rusqlite::ffi::sqlite3_key(conn.handle(), key.as_ptr().cast(), key.len() as i32) };
    assert_eq!(rc, rusqlite::ffi::SQLITE_OK);
    conn.pragma_update(None, "cipher_compatibility", 4).unwrap();
}
fn generated(path: &Path, wal: bool) -> rusqlite::Connection {
    let conn = rusqlite::Connection::open(path).unwrap();
    apply(&conn, b"authored test key with '\0 bytes");
    conn.execute_batch("CREATE TABLE authored(id INTEGER PRIMARY KEY, title TEXT NOT NULL); PRAGMA user_version=7; PRAGMA application_id=34;").unwrap();
    if wal {
        conn.pragma_update(None, "journal_mode", "WAL").unwrap();
        conn.pragma_update(None, "wal_autocheckpoint", 0).unwrap();
    }
    conn.execute(
        "INSERT INTO authored VALUES(1,'Authored confidential sentinel')",
        [],
    )
    .unwrap();
    conn
}
#[test]
fn native_reader_opens_real_encryption_without_exposing_keys_or_accepting_plain_sqlite() {
    let d = tempfile::tempdir().unwrap();
    let path = d.path().join("master.db");
    drop(generated(&path, false));
    let before = file_sha256(&path).unwrap();
    let bytes = std::fs::read(&path).unwrap();
    assert_ne!(&bytes[..16], b"SQLite format 3\0");
    assert!(
        !bytes
            .windows(30)
            .any(|b| b == b"Authored confidential sentinel")
    );
    let reader = ReadOnlyDatabase::open(&path, &key()).unwrap();
    let schema = reader.schema().unwrap();
    assert!(!reader.write_capability().unwrap().can_write());
    assert_eq!(schema.user_version, 7);
    assert_eq!(schema.application_id, 34);
    assert!(schema.objects.iter().any(|o| o.name == "authored"));
    assert!(reader.checks().unwrap().integrity_ok);
    let wrong = DatabaseKey::new(b"wrong secret which must never be logged").unwrap();
    let error = ReadOnlyDatabase::open(&path, &wrong)
        .err()
        .expect("Wrong key rejected")
        .to_string();
    assert!(!error.contains("wrong secret"));
    assert!(!format!("{:?}", key()).contains("authored test"));
    let plain = d.path().join("plain.db");
    let conn = rusqlite::Connection::open(&plain).unwrap();
    conn.execute_batch("CREATE TABLE plain(x);").unwrap();
    drop(conn);
    assert!(ReadOnlyDatabase::open(&plain, &key()).is_err());
    assert_eq!(file_sha256(&path).unwrap(), before);
}
#[test]
fn encrypted_backup_includes_wal_commits_and_is_standalone_coherent_and_owned() {
    let d = tempfile::tempdir().unwrap();
    let path = d.path().join("master.db");
    let writer = generated(&path, true);
    let wal = path.with_file_name("master.db-wal");
    assert!(wal.exists());
    let before = (file_sha256(&path).unwrap(), file_sha256(&wal).unwrap());
    let reader = ReadOnlyDatabase::open(&path, &key()).unwrap();
    let snapshot = reader
        .capture(
            &d.path().join("cache"),
            &key(),
            &CancellationToken::default(),
        )
        .unwrap();
    assert!(snapshot.checks.integrity_ok);
    assert_eq!(snapshot.checks.foreign_key_issues, 0);
    assert_eq!(
        snapshot.schema.fingerprint,
        reader.schema().unwrap().fingerprint
    );
    assert_ne!(
        &std::fs::read(&snapshot.path).unwrap()[..16],
        b"SQLite format 3\0"
    );
    let copy = rusqlite::Connection::open_with_flags(
        &snapshot.path,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
    )
    .unwrap();
    apply(&copy, b"authored test key with '\0 bytes");
    assert_eq!(
        copy.query_row("SELECT title FROM authored WHERE id=1", [], |r| r
            .get::<_, String>(0))
            .unwrap(),
        "Authored confidential sentinel"
    );
    drop(copy);
    assert_eq!(
        (file_sha256(&path).unwrap(), file_sha256(&wal).unwrap()),
        before
    );
    drop(writer);
    let owned = snapshot.path.clone();
    drop(snapshot);
    assert!(!owned.exists());
}
#[test]
fn cancellation_does_not_allocate_a_snapshot_and_missing_inputs_are_never_created() {
    let d = tempfile::tempdir().unwrap();
    let path = d.path().join("master.db");
    drop(generated(&path, false));
    let reader = ReadOnlyDatabase::open(&path, &key()).unwrap();
    let cancel = CancellationToken::default();
    cancel.cancel();
    let cache = d.path().join("not-created");
    assert!(reader.capture(&cache, &key(), &cancel).is_err());
    assert!(!cache.exists());
    let absent = d.path().join("missing.db");
    assert!(ReadOnlyDatabase::open(&absent, &key()).is_err());
    assert!(!absent.exists());
    assert!(DatabaseKey::new([]).is_err());
}
#[test]
fn schema_identity_is_independent_of_row_values_and_detects_structural_changes() {
    let d = tempfile::tempdir().unwrap();
    let path = d.path().join("master.db");
    let conn = generated(&path, false);
    let first = ReadOnlyDatabase::open(&path, &key())
        .unwrap()
        .schema()
        .unwrap()
        .fingerprint;
    conn.execute("UPDATE authored SET title='Different authored value'", [])
        .unwrap();
    let same = ReadOnlyDatabase::open(&path, &key())
        .unwrap()
        .schema()
        .unwrap()
        .fingerprint;
    assert_eq!(first, same);
    conn.execute_batch("CREATE INDEX authored_title ON authored(title);")
        .unwrap();
    let changed = ReadOnlyDatabase::open(&path, &key())
        .unwrap()
        .schema()
        .unwrap()
        .fingerprint;
    assert_ne!(first, changed);
}

#[test]
fn cancellation_after_a_native_page_step_keeps_inputs_and_other_cache_files() {
    let d = tempfile::tempdir().unwrap();
    let path = d.path().join("master.db");
    let writer = generated(&path, false);
    writer
        .execute_batch("CREATE TABLE bulk(value BLOB); INSERT INTO bulk VALUES(zeroblob(4194304));")
        .unwrap();
    drop(writer);
    let before = file_sha256(&path).unwrap();
    let reader = ReadOnlyDatabase::open(&path, &key()).unwrap();
    let cancel = CancellationToken::default();
    let cache = d.path().join("cache");
    std::fs::create_dir(&cache).unwrap();
    std::fs::write(cache.join("unrelated"), b"retained").unwrap();
    let mut progress = 0;
    let result = reader.capture_with_progress(&cache, &key(), &cancel, |state| {
        progress += 1;
        if state.remaining_pages > 0 {
            cancel.cancel();
        }
    });
    assert!(
        result.is_err(),
        "Cancellation between page batches must abort the owned snapshot"
    );
    assert!(progress > 0);
    assert_eq!(std::fs::read(cache.join("unrelated")).unwrap(), b"retained");
    assert_eq!(std::fs::read_dir(&cache).unwrap().count(), 1);
    assert_eq!(file_sha256(&path).unwrap(), before);
}
#[cfg(unix)]
#[test]
fn symbolic_database_inputs_are_rejected_without_mutating_their_targets() {
    use std::os::unix::fs::symlink;
    let d = tempfile::tempdir().unwrap();
    let path = d.path().join("master.db");
    drop(generated(&path, false));
    let before = file_sha256(&path).unwrap();
    let link = d.path().join("alias.db");
    symlink(&path, &link).unwrap();
    assert!(ReadOnlyDatabase::open(&link, &key()).is_err());
    assert_eq!(file_sha256(&path).unwrap(), before);
}

#[test]
fn integrity_and_foreign_key_reports_do_not_hide_existing_orphans() {
    let d = tempfile::tempdir().unwrap();
    let path = d.path().join("master.db");
    let conn = generated(&path, false);
    conn.pragma_update(None, "foreign_keys", false).unwrap();
    conn.execute_batch("CREATE TABLE parent(id INTEGER PRIMARY KEY); CREATE TABLE child(parent_id INTEGER REFERENCES parent(id)); INSERT INTO child VALUES(99);").unwrap();
    drop(conn);
    let reader = ReadOnlyDatabase::open(&path, &key()).unwrap();
    let checks = reader.checks().unwrap();
    assert!(checks.integrity_ok);
    assert_eq!(checks.foreign_key_issues, 1);
    let snapshot = reader
        .capture(
            &d.path().join("cache"),
            &key(),
            &CancellationToken::default(),
        )
        .unwrap();
    assert_eq!(snapshot.checks.foreign_key_issues, 1);
}
