use cuerithm_rekordbox::{
    DatabaseSchema, SchemaObject, WriteCapability, WriteRestriction, validate_write_profile,
};
use sha2::{Digest, Sha256};

fn authored_schema() -> DatabaseSchema {
    let objects = vec![SchemaObject {
        kind: "table".into(),
        name: "authored_content".into(),
        table: "authored_content".into(),
        sql: "CREATE TABLE authored_content(id TEXT PRIMARY KEY, title TEXT)".into(),
    }];
    DatabaseSchema {
        user_version: 0,
        application_id: 0,
        fingerprint: format!(
            "{:x}",
            Sha256::digest(serde_json::to_vec(&objects).unwrap())
        ),
        objects,
    }
}

#[test]
fn a_readable_schema_is_not_a_verified_write_profile() {
    let result = validate_write_profile(&authored_schema()).unwrap();
    assert_eq!(
        result,
        WriteCapability::ReadOnly {
            restriction: WriteRestriction::UnrecognizedSchema,
        }
    );
    assert!(!result.can_write());
}

#[test]
fn a_claimed_fingerprint_cannot_hide_changed_columns_or_types() {
    let mut schema = authored_schema();
    schema.objects[0].sql = "CREATE TABLE authored_content(id TEXT PRIMARY KEY, title BLOB)".into();
    assert!(validate_write_profile(&schema).is_err());
}

#[test]
fn copying_the_observed_native_fingerprint_cannot_enable_writes() {
    let mut schema = authored_schema();
    schema.fingerprint = "6bd02ac0a3f9cadfd3ae9b4c95e1ca0663fd3e7717a74e89a4c153834611d2eb".into();
    assert!(validate_write_profile(&schema).is_err());
}

#[test]
fn unknown_version_markers_do_not_enable_an_authored_schema() {
    let mut schema = authored_schema();
    schema.user_version = 999;
    schema.application_id = 999;
    assert!(!validate_write_profile(&schema).unwrap().can_write());
}
