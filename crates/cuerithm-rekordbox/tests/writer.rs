use cuerithm_media::{CancellationToken, file_sha256};
use cuerithm_rekordbox::{
    DatabaseKey, NativeRowMutation, NativeWritePlan, ReadOnlyDatabase, TargetValue,
    stage_native_write,
};
use sha2::{Digest, Sha256};
use std::{collections::BTreeMap, path::Path};

const KEY: &[u8] = b"authored native staging key";
fn open(path: &Path) -> rusqlite::Connection {
    let conn = rusqlite::Connection::open(path).unwrap();
    assert_eq!(
        unsafe { rusqlite::ffi::sqlite3_key(conn.handle(), KEY.as_ptr().cast(), KEY.len() as i32) },
        rusqlite::ffi::SQLITE_OK
    );
    conn.pragma_update(None, "cipher_compatibility", 4).unwrap();
    conn
}
fn values() -> BTreeMap<String, TargetValue> {
    BTreeMap::from([
        ("ID".into(), TargetValue::Text("1".into())),
        ("Title".into(), TargetValue::Text("Before".into())),
        ("Opaque".into(), TargetValue::Blob(vec![0, 255, 7])),
    ])
}
fn generated(path: &Path) {
    let conn = open(path);
    conn.execute_batch("CREATE TABLE djmdContent(ID TEXT PRIMARY KEY,Title TEXT NOT NULL,Opaque BLOB); INSERT INTO djmdContent VALUES('1','Before',X'00FF07');").unwrap();
}
fn plan(path: &Path) -> NativeWritePlan {
    let key = DatabaseKey::new(KEY).unwrap();
    let schema = ReadOnlyDatabase::open(path, &key)
        .unwrap()
        .schema()
        .unwrap();
    NativeWritePlan {
        source: path.into(),
        schema_fingerprint: schema.fingerprint,
        rows: vec![NativeRowMutation {
            table: "djmdContent".into(),
            id: "1".into(),
            expected_fingerprint: Some(format!(
                "{:x}",
                Sha256::digest(serde_json::to_vec(&values()).unwrap())
            )),
            values: BTreeMap::from([("Title".into(), TargetValue::Text("After".into()))]),
        }],
    }
}

#[test]
fn staging_updates_only_an_owned_encrypted_copy_and_retains_unknown_columns() {
    let dir = tempfile::tempdir().unwrap();
    let source = dir.path().join("source.db");
    generated(&source);
    let before = file_sha256(&source).unwrap();
    let output = stage_native_write(
        &plan(&source),
        &dir.path().join("staging"),
        &DatabaseKey::new(KEY).unwrap(),
        &CancellationToken::default(),
    )
    .unwrap();
    assert_eq!(output.changed_rows, 1);
    assert!(!output.write_capability.can_write());
    let copy = open(&output.snapshot.path);
    assert_eq!(
        copy.query_row("SELECT Title FROM djmdContent WHERE ID='1'", [], |r| r
            .get::<_, String>(
            0
        ))
        .unwrap(),
        "After"
    );
    assert_eq!(
        copy.query_row("SELECT Opaque FROM djmdContent WHERE ID='1'", [], |r| r
            .get::<_, Vec<u8>>(0))
            .unwrap(),
        vec![0, 255, 7]
    );
    assert_eq!(file_sha256(&source).unwrap(), before);
    assert_eq!(
        output.snapshot.sha256,
        file_sha256(&output.snapshot.path).unwrap()
    );
    assert_ne!(
        &std::fs::read(&output.snapshot.path).unwrap()[..16],
        b"SQLite format 3\0"
    );
}

#[test]
fn staging_rejects_stale_rows_duplicate_mutations_and_unknown_tables_without_publication() {
    let dir = tempfile::tempdir().unwrap();
    let source = dir.path().join("source.db");
    generated(&source);
    let before = file_sha256(&source).unwrap();
    for kind in 0..3 {
        let mut request = plan(&source);
        match kind {
            0 => request.rows[0].expected_fingerprint = Some("0".repeat(64)),
            1 => request.rows.push(request.rows[0].clone()),
            _ => request.rows[0].table = "djmdProperty".into(),
        }
        assert!(
            stage_native_write(
                &request,
                &dir.path().join("staging"),
                &DatabaseKey::new(KEY).unwrap(),
                &CancellationToken::default()
            )
            .is_err()
        );
        assert_eq!(file_sha256(&source).unwrap(), before);
    }
}

#[test]
fn unchanged_row_updates_are_noops_and_cancellation_does_not_change_source() {
    let dir = tempfile::tempdir().unwrap();
    let source = dir.path().join("source.db");
    generated(&source);
    let mut request = plan(&source);
    request.rows[0]
        .values
        .insert("Title".into(), TargetValue::Text("Before".into()));
    let output = stage_native_write(
        &request,
        &dir.path().join("staging"),
        &DatabaseKey::new(KEY).unwrap(),
        &CancellationToken::default(),
    )
    .unwrap();
    assert_eq!(output.changed_rows, 0);
    let before = file_sha256(&source).unwrap();
    let cancel = CancellationToken::default();
    cancel.cancel();
    assert!(
        stage_native_write(
            &request,
            &dir.path().join("cancelled"),
            &DatabaseKey::new(KEY).unwrap(),
            &cancel
        )
        .is_err()
    );
    assert!(!dir.path().join("cancelled").exists());
    assert_eq!(file_sha256(&source).unwrap(), before);
}
