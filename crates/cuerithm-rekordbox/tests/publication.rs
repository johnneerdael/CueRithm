use cuerithm_media::CancellationToken;
use cuerithm_rekordbox::{
    DatabaseKey, NativeDatabaseLease, NativeRowMutation, NativeWritePlan, TargetValue,
};
use rusqlite::{Connection, ErrorCode, config::DbConfig};
use sha2::{Digest, Sha256};
use std::{collections::BTreeMap, path::Path, process::Command};

const KEY: &[u8] = b"authored exclusive publication key";
fn key() -> DatabaseKey {
    DatabaseKey::new(KEY).unwrap()
}
fn open(path: &Path) -> Connection {
    let connection = Connection::open(path).unwrap();
    assert_eq!(
        unsafe {
            rusqlite::ffi::sqlite3_key(connection.handle(), KEY.as_ptr().cast(), KEY.len() as i32)
        },
        rusqlite::ffi::SQLITE_OK
    );
    connection
        .pragma_update(None, "cipher_compatibility", 4)
        .unwrap();
    connection.busy_timeout(std::time::Duration::ZERO).unwrap();
    connection
}
fn fixture(path: &Path, wal: bool) {
    let connection = open(path);
    connection.execute_batch("PRAGMA foreign_keys=ON;
        CREATE TABLE djmdArtist(ID TEXT PRIMARY KEY,Name TEXT);
        INSERT INTO djmdArtist VALUES('artist','Known');
        CREATE TABLE djmdContent(ID TEXT PRIMARY KEY,Title TEXT NOT NULL,ArtistID TEXT REFERENCES djmdArtist(ID),Opaque BLOB);
        INSERT INTO djmdContent VALUES('1','Before','artist',X'00FF');
        CREATE TABLE unknownRows(ID TEXT PRIMARY KEY, payload);
        INSERT INTO unknownRows VALUES('private',X'0102');").unwrap();
    if wal {
        connection
            .pragma_update(None, "journal_mode", "WAL")
            .unwrap();
        connection
            .set_db_config(DbConfig::SQLITE_DBCONFIG_NO_CKPT_ON_CLOSE, true)
            .unwrap();
        connection
            .pragma_update(None, "wal_autocheckpoint", 0)
            .unwrap();
        connection
            .execute("UPDATE unknownRows SET payload=X'FEED'", [])
            .unwrap();
    }
}
fn request(path: &Path, schema: &str, title: &str) -> NativeWritePlan {
    let values: BTreeMap<String, TargetValue> = BTreeMap::from([
        ("ID".into(), TargetValue::Text("1".into())),
        ("Title".into(), TargetValue::Text("Before".into())),
        ("ArtistID".into(), TargetValue::Text("artist".into())),
        ("Opaque".into(), TargetValue::Blob(vec![0, 255])),
    ]);
    NativeWritePlan {
        source: path.into(),
        schema_fingerprint: schema.into(),
        rows: vec![NativeRowMutation {
            table: "djmdContent".into(),
            id: "1".into(),
            expected_fingerprint: Some(format!(
                "{:x}",
                Sha256::digest(serde_json::to_vec(&values).unwrap())
            )),
            values: BTreeMap::from([("Title".into(), TargetValue::Text(title.into()))]),
        }],
    }
}
fn assert_busy(path: &Path) {
    let connection = open(path);
    for sql in [
        "SELECT Title FROM djmdContent",
        "UPDATE djmdContent SET Title='Intruder'",
    ] {
        let error = connection
            .prepare(sql)
            .and_then(|mut statement| statement.raw_query().next().map(|_| ()))
            .unwrap_err();
        assert_eq!(
            error.sqlite_error_code(),
            Some(ErrorCode::DatabaseBusy),
            "{error}"
        );
    }
}
fn child_busy(path: &Path) {
    let output = Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "publication_probe", "--ignored", "--nocapture"])
        .env("CUERITHM_AUTHORED_PUBLICATION_PROBE", path)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
#[ignore = "child-process probe used by the exclusive lease tests"]
fn publication_probe() {
    assert_busy(Path::new(
        &std::env::var_os("CUERITHM_AUTHORED_PUBLICATION_PROBE").unwrap(),
    ));
}

#[test]
fn lease_blocks_readers_and_writers_between_transactions_and_during_commit_callback() {
    for wal in [false, true] {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("master.db");
        fixture(&path, wal);
        let cancel = CancellationToken::default();
        let mut lease = NativeDatabaseLease::acquire(&path, &key(), &cancel).unwrap();
        let before = lease.state(&cancel).unwrap();
        assert_busy(&path);
        child_busy(&path);
        let changed = lease
            .apply_rows_checked(
                &request(&path, &before.schema_fingerprint, "After"),
                &before,
                &cancel,
                || {
                    child_busy(&path);
                    Ok(())
                },
            )
            .unwrap();
        assert_eq!(changed.changed_rows, 1);
        assert_eq!(changed.before_state, before);
        assert_ne!(changed.after_state.logical_sha256, before.logical_sha256);
        assert_busy(&path);
        child_busy(&path);
        drop(lease);
        assert_eq!(
            open(&path)
                .query_row("SELECT Title FROM djmdContent", [], |row| row
                    .get::<_, String>(0))
                .unwrap(),
            "After"
        );
    }
}

#[test]
fn snapshot_preserves_wal_unknown_rows_and_restores_through_same_inode() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("master.db");
    fixture(&path, true);
    assert!(path.with_file_name("master.db-wal").exists());
    #[cfg(unix)]
    let inode = {
        use std::os::unix::fs::MetadataExt;
        std::fs::metadata(&path).unwrap().ino()
    };
    let cancel = CancellationToken::default();
    let mut lease = NativeDatabaseLease::acquire(&path, &key(), &cancel).unwrap();
    let before = lease.state(&cancel).unwrap();
    let snapshot = lease.snapshot(directory.path(), &key(), &cancel).unwrap();
    assert_ne!(
        &std::fs::read(&snapshot.path).unwrap()[..16],
        b"SQLite format 3\0"
    );
    assert_eq!(
        open(&snapshot.path)
            .query_row("SELECT payload FROM unknownRows", [], |row| row
                .get::<_, Vec<u8>>(0))
            .unwrap(),
        vec![254, 237]
    );
    let after = lease
        .apply_rows(
            &request(&path, &before.schema_fingerprint, "After"),
            &before,
            &cancel,
        )
        .unwrap()
        .after_state;
    let restored = lease
        .restore_snapshot_checked(&snapshot.path, &key(), &before, &after, &cancel)
        .unwrap();
    assert_eq!(restored, before);
    assert_busy(&path);
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        assert_eq!(std::fs::metadata(&path).unwrap().ino(), inode);
    }
}

#[test]
fn restore_rejects_changed_recovery_image_before_target_mutation() {
    for wal in [false, true] {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("master.db");
        fixture(&path, false);
        let cancel = CancellationToken::default();
        let mut lease = NativeDatabaseLease::acquire(&path, &key(), &cancel).unwrap();
        let before = lease.state(&cancel).unwrap();
        let snapshot = lease.snapshot(directory.path(), &key(), &cancel).unwrap();
        let after = lease
            .apply_rows(
                &request(&path, &before.schema_fingerprint, "After"),
                &before,
                &cancel,
            )
            .unwrap()
            .after_state;
        let altered = open(&snapshot.path);
        if wal {
            altered.pragma_update(None, "journal_mode", "WAL").unwrap();
            altered
                .pragma_update(None, "wal_autocheckpoint", 0)
                .unwrap();
        }
        altered
            .execute("UPDATE unknownRows SET payload=X'BAD0'", [])
            .unwrap();
        if wal {
            assert!(snapshot.path.with_extension("db-wal").exists());
        }
        let result =
            lease.restore_snapshot_checked(&snapshot.path, &key(), &before, &after, &cancel);
        assert!(
            result.is_err(),
            "a coherent but unfrozen recovery image must be refused"
        );
        assert_eq!(
            lease.state(&cancel).unwrap(),
            after,
            "target must remain at the published post-image"
        );
    }
}

#[test]
fn stale_whole_database_state_and_later_external_edits_refuse_restore() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("master.db");
    fixture(&path, false);
    let cancel = CancellationToken::default();
    let lease = NativeDatabaseLease::acquire(&path, &key(), &cancel).unwrap();
    let before = lease.state(&cancel).unwrap();
    let snapshot = lease.snapshot(directory.path(), &key(), &cancel).unwrap();
    drop(lease);
    let external = open(&path);
    external.pragma_update(None, "journal_mode", "WAL").unwrap();
    external
        .set_db_config(DbConfig::SQLITE_DBCONFIG_NO_CKPT_ON_CLOSE, true)
        .unwrap();
    external
        .pragma_update(None, "wal_autocheckpoint", 0)
        .unwrap();
    let unchanged_main_file = cuerithm_media::file_sha256(&path).unwrap();
    external
        .execute("UPDATE unknownRows SET payload='external'", [])
        .unwrap();
    drop(external);
    assert_eq!(
        cuerithm_media::file_sha256(&path).unwrap(),
        unchanged_main_file
    );
    let mut lease = NativeDatabaseLease::acquire(&path, &key(), &cancel).unwrap();
    assert_ne!(lease.state(&cancel).unwrap(), before);
    assert!(
        lease
            .apply_rows(
                &request(&path, &before.schema_fingerprint, "After"),
                &before,
                &cancel
            )
            .is_err()
    );
    assert!(
        lease
            .restore_snapshot(&snapshot.path, &key(), &before, &cancel)
            .is_err()
    );
    drop(lease);
    assert_eq!(
        open(&path)
            .query_row("SELECT payload FROM unknownRows", [], |row| row
                .get::<_, String>(0))
            .unwrap(),
        "external"
    );
}

#[test]
fn acquisition_refuses_an_existing_native_reader() {
    for wal in [false, true] {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("master.db");
        fixture(&path, wal);
        let native = open(&path);
        native
            .execute_batch("BEGIN; SELECT * FROM djmdContent;")
            .unwrap();
        assert!(
            NativeDatabaseLease::acquire(&path, &key(), &CancellationToken::default()).is_err()
        );
        native.execute_batch("ROLLBACK").unwrap();
    }
}

#[test]
fn noops_and_failed_fk_or_monitor_checks_preserve_all_rows() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("master.db");
    fixture(&path, false);
    let cancel = CancellationToken::default();
    let mut lease = NativeDatabaseLease::acquire(&path, &key(), &cancel).unwrap();
    let before = lease.state(&cancel).unwrap();
    let noop = lease
        .apply_rows(
            &request(&path, &before.schema_fingerprint, "Before"),
            &before,
            &cancel,
        )
        .unwrap();
    assert_eq!(noop.changed_rows, 0);
    assert_eq!(noop.after_state, before);
    assert!(
        lease
            .apply_rows_checked(
                &request(&path, &before.schema_fingerprint, "After"),
                &before,
                &cancel,
                || anyhow::bail!("Authored monitor rejected publication")
            )
            .is_err()
    );
    assert_eq!(lease.state(&cancel).unwrap(), before);
    let mut broken = request(&path, &before.schema_fingerprint, "After");
    broken.rows[0]
        .values
        .insert("ArtistID".into(), TargetValue::Text("missing".into()));
    assert!(lease.apply_rows(&broken, &before, &cancel).is_err());
    assert_eq!(lease.state(&cancel).unwrap(), before);
    cancel.cancel();
    assert!(lease.apply_rows(&broken, &before, &cancel).is_err());
}

#[cfg(unix)]
#[test]
fn replacing_the_path_during_final_guard_refuses_commit() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("master.db");
    let moved = directory.path().join("moved.db");
    let replacement = directory.path().join("replacement.db");
    fixture(&path, false);
    fixture(&replacement, false);
    let cancel = CancellationToken::default();
    let mut lease = NativeDatabaseLease::acquire(&path, &key(), &cancel).unwrap();
    let before = lease.state(&cancel).unwrap();
    assert!(
        lease
            .apply_rows_checked(
                &request(&path, &before.schema_fingerprint, "After"),
                &before,
                &cancel,
                || {
                    std::fs::rename(&path, &moved)?;
                    std::fs::rename(&replacement, &path)?;
                    Ok(())
                }
            )
            .is_err()
    );
    drop(lease);
    assert_eq!(
        open(&moved)
            .query_row("SELECT Title FROM djmdContent", [], |row| row
                .get::<_, String>(0))
            .unwrap(),
        "Before"
    );
}

#[test]
fn frozen_after_state_is_checked_before_commit_and_unplanned_trigger_output_rolls_back() {
    for nondeterministic in [false, true] {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("master.db");
        fixture(&path, false);
        if nondeterministic {
            open(&path).execute_batch("CREATE TRIGGER authored_nondeterministic AFTER UPDATE ON djmdContent BEGIN UPDATE unknownRows SET payload=randomblob(32); END;").unwrap();
        }
        let schema = cuerithm_rekordbox::ReadOnlyDatabase::open(&path, &key())
            .unwrap()
            .schema()
            .unwrap();
        let plan = request(&path, &schema.fingerprint, "After");
        let cancel = CancellationToken::default();
        let staged = cuerithm_rekordbox::stage_native_write(
            &plan,
            &directory.path().join("stage"),
            &key(),
            &cancel,
        )
        .unwrap();
        let expected_after = {
            let lease =
                NativeDatabaseLease::acquire(&staged.snapshot.path, &key(), &cancel).unwrap();
            lease.state(&cancel).unwrap()
        };
        let mut lease = NativeDatabaseLease::acquire(&path, &key(), &cancel).unwrap();
        let before = lease.state(&cancel).unwrap();
        let called = std::cell::Cell::new(false);
        let result =
            lease.apply_rows_frozen_checked(&plan, &before, &expected_after, &cancel, || {
                called.set(true);
                Ok(())
            });
        if nondeterministic {
            assert!(
                result
                    .unwrap_err()
                    .to_string()
                    .contains("frozen staged post-image")
            );
            assert!(!called.get());
            assert_eq!(lease.state(&cancel).unwrap(), before);
        } else {
            assert_eq!(result.unwrap().after_state, expected_after);
            assert!(called.get());
        }
        drop(lease);
        assert_eq!(
            open(&path)
                .query_row("SELECT Title FROM djmdContent", [], |row| row
                    .get::<_, String>(0))
                .unwrap(),
            if nondeterministic { "Before" } else { "After" }
        );
    }
}
