#[path = "import_fixture_support/mod.rs"]
mod support;
use cuerithm_dj_sync::import::*;
use cuerithm_media::CancellationToken;
use rusqlite::Connection;
use support::*;
fn database(path: &std::path::Path, optional: bool) -> Connection {
    let db = Connection::open(path.join("master.sqlite")).unwrap();
    db.execute_batch(r#"PRAGMA user_version=12;
CREATE TABLE space(id INTEGER PRIMARY KEY,name TEXT);INSERT INTO space VALUES(1,'Serato Library');
CREATE TABLE space_asset(id INTEGER PRIMARY KEY,space_id INTEGER,asset_id INTEGER);INSERT INTO space_asset VALUES(1,1,10),(2,1,11);
CREATE TABLE connection(location_id INTEGER,database_uri TEXT);INSERT INTO connection VALUES(1,'C:/Users/Invented/Serato/Library/root.sqlite');
CREATE TABLE asset(id INTEGER PRIMARY KEY,location_id INTEGER,portable_id TEXT,name TEXT,artist TEXT,bpm REAL,key TEXT,length_ms INTEGER);INSERT INTO asset VALUES(10,1,'Music/one.flac','Database title','Artist',121,'8A',1250),(11,99,'Music/missing.flac','Missing volume',NULL,0,'',0);
CREATE TABLE container(id INTEGER PRIMARY KEY,parent_id INTEGER,name TEXT,type INTEGER,list_order INTEGER,space_id INTEGER);INSERT INTO container VALUES(0,NULL,'root',0,0,NULL),(1,0,'Serato Library root',0,0,1),(2,1,'Set',1,1,1),(3,1,'Smart',2,2,1),(4,1,'Apple Music',0,3,1);
CREATE TABLE location_container(id INTEGER PRIMARY KEY,container_id INTEGER,location_id INTEGER);INSERT INTO location_container VALUES(20,2,1),(30,3,1);
CREATE TABLE container_asset(id INTEGER PRIMARY KEY,asset_id INTEGER,location_container_id INTEGER,list_order INTEGER);INSERT INTO container_asset VALUES(100,10,20,1),(101,11,20,2),(102,10,20,3);
CREATE TABLE smart_crate_rules(container_id INTEGER,version INTEGER,rules TEXT);INSERT INTO smart_crate_rules VALUES(3,1,'{"unknown_operator":true}');"#).unwrap();
    if optional {
        db.execute_batch("ALTER TABLE asset ADD COLUMN rating REAL;ALTER TABLE asset ADD COLUMN emoji TEXT;ALTER TABLE asset ADD COLUMN color INTEGER;UPDATE asset SET rating=0.8,color=16711680;CREATE TABLE container_keep_alive_list_tbl(container_id INTEGER,location_id INTEGER);INSERT INTO container_keep_alive_list_tbl VALUES(2,1),(3,1);").unwrap();
    }
    db
}
fn capture(root: &std::path::Path) -> ImportSnapshot {
    let request = SourceCaptureRequest::from_selected_root(
        SourceProvider::Serato4,
        root,
        vec![ArtifactRequest::sqlite("master.sqlite")],
    )
    .unwrap();
    capture_source(
        &request,
        root,
        &ImportLimits::default(),
        &CancellationToken::default(),
    )
    .unwrap()
}
#[test]
fn serato4_optional_tables_do_not_drop_crates() {
    for optional in [false, true] {
        let root = tempfile::tempdir().unwrap();
        drop(database(root.path(), optional));
        let snapshot = capture(root.path());
        let (_, rows) = read(SourceProvider::Serato4, &snapshot);
        let tracks = rows.tracks();
        assert_eq!(tracks.len(), 2);
        let first = tracks
            .iter()
            .find(|t| t.extensions["native"]["id"] == 10)
            .unwrap();
        assert_eq!(
            first.metadata["title"],
            Presence::Value(serde_json::json!("Database title"))
        );
        assert_eq!(first.extensions["duration_seconds"], 1.25);
        assert_eq!(first.bpm.as_ref().unwrap().value, Some(121.));
        assert_eq!(
            first.locations.original.as_deref(),
            Some("C:/Music/one.flac")
        );
        assert_eq!(first.metadata.contains_key("rating"), optional);
        assert!(tracks.iter().all(|t| {
            t.issues
                .iter()
                .any(|i| i.code == "serato_performance_unavailable")
        }));
        let collections = rows.collections();
        assert!(collections.iter().any(|c| c.name == "Set"));
        assert!(!collections.iter().any(|c| c.name == "Apple Music"));
        let smart = collections.iter().find(|c| c.name == "Smart").unwrap();
        assert_eq!(smart.kind, CollectionKind::Smart);
        assert!(smart.extensions.contains_key("smart_rules"));
        let members = rows.members();
        assert_eq!(members.len(), 3);
        assert_eq!(
            members[0].1.track_external_id,
            members[2].1.track_external_id
        );
        assert_ne!(
            members[0].1.external_occurrence_id,
            members[2].1.external_occurrence_id
        );
    }
}
#[test]
fn serato4_unknown_profiles_and_cycles_reject_without_repairing_source() {
    for change in [
        "PRAGMA user_version=99",
        "UPDATE container SET parent_id=3 WHERE id=3",
    ] {
        let root = tempfile::tempdir().unwrap();
        let db = database(root.path(), false);
        db.execute_batch(change).unwrap();
        drop(db);
        let snapshot = capture(root.path());
        assert!(
            adapter(SourceProvider::Serato4)
                .read(
                    &snapshot,
                    &ReadOptions::default(),
                    &mut Records::default(),
                    &CancellationToken::default()
                )
                .is_err()
        );
        snapshot
            .validate_source(&CancellationToken::default())
            .unwrap();
    }
}

#[test]
fn serato4_dangling_and_null_membership_references_remain_visible() {
    let root = tempfile::tempdir().unwrap();
    let db = database(root.path(), false);
    db.execute_batch("INSERT INTO container_asset VALUES(103,999,20,3),(104,NULL,20,4);")
        .unwrap();
    drop(db);
    let snapshot = capture(root.path());
    let (_, rows) = read(SourceProvider::Serato4, &snapshot);
    assert_eq!(rows.tracks().len(), 4);
    assert_eq!(rows.members().len(), 5);
    assert!(
        rows.tracks()
            .iter()
            .filter(|t| t
                .issues
                .iter()
                .any(|i| i.code == "missing_or_unselected_serato_asset"))
            .count()
            == 2
    );
    assert!(rows.rows.iter().any(
        |r| matches!(r,ObservationRecord::Issue(i)if i.code=="duplicate_serato_membership_position")
    ));
}
