use super::*;
use std::path::PathBuf;

const RULES: &str = r#"{"spaces":["Serato Library"],"conjunction":"AND","live_updates":true,"rules":[{"attribute":"artist","operation":"IS","type":"STRING","version":1,"value":"Synthetic Artist"}]}"#;
const FOREIGN_RULES: &str = r#"{"spaces":["Serato Library"],"conjunction":"AND","live_updates":true,"rules":[{"attribute":"genre","operation":"IS","type":"STRING","version":1,"value":"House"}]}"#;

fn database() -> (tempfile::TempDir, PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("library.sqlite");
    let conn = Connection::open(&path).unwrap();
    conn.execute_batch("PRAGMA user_version=12;
        CREATE TABLE master(revision INTEGER); INSERT INTO master VALUES(20);
        CREATE TABLE space(id INTEGER PRIMARY KEY,name TEXT); INSERT INTO space VALUES(1,'Serato Library');
        CREATE TABLE asset(id INTEGER PRIMARY KEY,portable_id TEXT,name TEXT,external_id INTEGER,location_id INTEGER);
        CREATE TABLE space_asset(id INTEGER PRIMARY KEY,asset_id INTEGER,space_id INTEGER);
        CREATE TABLE container(id INTEGER PRIMARY KEY,revision INTEGER,parent_id INTEGER,name TEXT,type INTEGER,list_order INTEGER,space_id INTEGER,time_added INTEGER,expanded INTEGER,portable_id TEXT,color INTEGER);
        INSERT INTO container VALUES(0,0,NULL,'root',0,0,NULL,0,0,'',NULL),(1,1,0,'Serato Library root',0,1,1,0,0,'',NULL);
        CREATE TABLE location_container(id INTEGER PRIMARY KEY,container_id INTEGER,location_id INTEGER,external_container_id INTEGER);
        CREATE TABLE container_asset(id INTEGER PRIMARY KEY,container_id INTEGER,space_asset_id INTEGER,list_order INTEGER,revision INTEGER,asset_id INTEGER,location_container_id INTEGER,external_container_asset_id INTEGER);
        CREATE TABLE connection(location_id INTEGER,database_uri TEXT);
        CREATE TABLE smart_crate_rules(container_id INTEGER PRIMARY KEY,revision INTEGER NOT NULL,version INTEGER NOT NULL,rules TEXT NOT NULL);").unwrap();
    (dir, path)
}
fn native_source(revision: i64) -> (tempfile::TempDir, PathBuf) {
    let (dir, path) = database();
    let conn = Connection::open(&path).unwrap();
    conn.execute_batch("INSERT INTO asset(id,portable_id,name) VALUES(1,'synthetic.wav','Synthetic');
        INSERT INTO space_asset VALUES(1,1,1);
        INSERT INTO container VALUES(2,2,1,'Folder',1,1,1,0,0,'',NULL),(3,3,2,'Crate􅺊shared',2,1,1,0,0,'',NULL);").unwrap();
    conn.execute(
        "INSERT INTO smart_crate_rules VALUES(3,?,1,?)",
        params![revision, RULES],
    )
    .unwrap();
    (dir, path)
}
fn register(master: &Path, source: &Path, location: i64) {
    Connection::open(master)
        .unwrap()
        .execute(
            "INSERT INTO connection VALUES(?,?)",
            params![location, source.to_str().unwrap()],
        )
        .unwrap();
}
fn mapped(conn: &Connection, location: i64, source_container: i64) -> i64 {
    conn.query_row("SELECT container_id FROM location_container WHERE location_id=? AND external_container_id=?",params![location,source_container],|r|r.get(0)).unwrap()
}
fn rules(conn: &Connection, container: i64) -> Option<(i64, i64, String)> {
    conn.query_row(
        "SELECT revision,version,rules FROM smart_crate_rules WHERE container_id=?",
        [container],
        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
    )
    .optional()
    .unwrap()
}
fn materialize(source: &Path) {
    Connection::open(source).unwrap().execute_batch("UPDATE container SET name='Crate',type=1,revision=50 WHERE id=3;
        DELETE FROM smart_crate_rules WHERE container_id=3;
        INSERT INTO container_asset(container_id,space_asset_id,list_order,revision) VALUES(3,1,1,51);").unwrap();
}

#[test]
fn native_rules_roundtrip_updates_and_materialized_crates_keep_their_mapping() {
    let (_srcdir, source) = native_source(31);
    let (_masterdir, master) = database();
    register(&master, &source, 42);
    refresh_master(&master, &source, &source).unwrap();
    let conn = Connection::open(&master).unwrap();
    let id = mapped(&conn, 42, 3);
    assert_eq!(rules(&conn, id), Some((31, 1, RULES.into())));
    Connection::open(&source)
        .unwrap()
        .execute(
            "UPDATE smart_crate_rules SET revision=32,version=2,rules=? WHERE container_id=3",
            [FOREIGN_RULES],
        )
        .unwrap();
    refresh_master(&master, &source, &source).unwrap();
    assert_eq!(mapped(&conn, 42, 3), id);
    assert_eq!(rules(&conn, id), Some((32, 2, FOREIGN_RULES.into())));
    materialize(&source);
    refresh_master(&master, &source, &source).unwrap();
    assert_eq!(
        mapped(&conn, 42, 3),
        id,
        "The native-to-ordinary transition must retain its existing master ID"
    );
    assert_eq!(
        conn.query_row("SELECT name,type FROM container WHERE id=?", [id], |r| Ok(
            (r.get::<_, String>(0)?, r.get::<_, i64>(1)?)
        ))
        .unwrap(),
        ("Crate".into(), 1)
    );
    assert_eq!(rules(&conn, id), None);
    assert_eq!(conn.query_row("SELECT count(*) FROM container_asset WHERE location_container_id IN(SELECT id FROM location_container WHERE location_id=42 AND container_id=?)",[id],|r|r.get::<_,i64>(0)).unwrap(),1);
    let source_connection = Connection::open(&source).unwrap();
    source_connection
        .execute_batch(
            "UPDATE container SET name='Crate􅺊new-suffix',type=2,revision=60 WHERE id=3;
        DELETE FROM container_asset WHERE container_id=3;",
        )
        .unwrap();
    source_connection
        .execute("INSERT INTO smart_crate_rules VALUES(3,61,1,?)", [RULES])
        .unwrap();
    refresh_master(&master, &source, &source).unwrap();
    assert_eq!(mapped(&conn, 42, 3), id);
    assert_eq!(rules(&conn, id), Some((61, 1, RULES.into())));
    assert_eq!(conn.query_row("SELECT count(*) FROM container_asset WHERE location_container_id IN(SELECT id FROM location_container WHERE location_id=42 AND container_id=?)",[id],|r|r.get::<_,i64>(0)).unwrap(),0,"Returning to a native smart crate must remove captured ordinary membership");
}

#[test]
fn shared_smart_conversion_preserves_foreign_rules_then_coalesces_ordinary_crates() {
    let (_adir, a) = native_source(31);
    let (_bdir, b) = native_source(41);
    let (_mdir, master) = database();
    register(&master, &a, 42);
    register(&master, &b, 43);
    let other = Connection::open(&b).unwrap();
    other
        .execute(
            "INSERT INTO container VALUES(4,4,2,'Other􅺊foreign',2,2,1,0,0,'',NULL)",
            [],
        )
        .unwrap();
    other
        .execute(
            "INSERT INTO smart_crate_rules VALUES(4,77,1,?)",
            [FOREIGN_RULES],
        )
        .unwrap();
    refresh_master(&master, &a, &a).unwrap();
    refresh_master(&master, &b, &b).unwrap();
    let conn = Connection::open(&master).unwrap();
    let shared = mapped(&conn, 42, 3);
    assert_eq!(mapped(&conn, 43, 3), shared);
    let foreign = mapped(&conn, 43, 4);
    assert_eq!(rules(&conn, shared), Some((41, 1, RULES.into())));
    materialize(&a);
    refresh_master(&master, &a, &a).unwrap();
    let ordinary = mapped(&conn, 42, 3);
    assert_ne!(ordinary, shared);
    assert_eq!(mapped(&conn, 43, 3), shared);
    assert_eq!(rules(&conn, ordinary), None);
    assert_eq!(rules(&conn, shared), Some((41, 1, RULES.into())));
    assert_eq!(rules(&conn, foreign), Some((77, 1, FOREIGN_RULES.into())));
    materialize(&b);
    refresh_master(&master, &b, &b).unwrap();
    refresh_master(&master, &a, &a).unwrap();
    assert_eq!(mapped(&conn, 43, 3), ordinary);
    assert_eq!(rules(&conn, ordinary), None);
    assert_eq!(rules(&conn, shared), None);
    assert_eq!(rules(&conn, foreign), Some((77, 1, FOREIGN_RULES.into())));
    assert_eq!(
        conn.query_row(
            "SELECT count(*) FROM container WHERE name='Crate'",
            [],
            |r| r.get::<_, i64>(0)
        )
        .unwrap(),
        1
    );
    assert_eq!(conn.query_row("SELECT count(*) FROM container_asset WHERE location_container_id IN(SELECT id FROM location_container WHERE container_id=?)",[ordinary],|r|r.get::<_,i64>(0)).unwrap(),2);
}

#[test]
fn missing_native_rules_roll_back_the_aggregate_and_preserve_foreign_records() {
    let (_sdir, source) = native_source(31);
    let (_mdir, master) = database();
    register(&master, &source, 42);
    let src = Connection::open(&source).unwrap();
    src.execute("DELETE FROM smart_crate_rules WHERE container_id=3", [])
        .unwrap();
    drop(src);
    let conn = Connection::open(&master).unwrap();
    conn.execute(
        "INSERT INTO container VALUES(9,9,1,'Foreign􅺊owned',2,1,1,0,0,'',NULL)",
        [],
    )
    .unwrap();
    conn.execute("INSERT INTO location_container(container_id,location_id,external_container_id) VALUES(9,99,9)",[]).unwrap();
    conn.execute(
        "INSERT INTO smart_crate_rules VALUES(9,77,1,?)",
        [FOREIGN_RULES],
    )
    .unwrap();
    let original = std::fs::read(&source).unwrap();
    assert!(
        refresh_master(&master, &source, &source)
            .unwrap_err()
            .to_string()
            .contains("no rules record")
    );
    assert_eq!(rules(&conn, 9), Some((77, 1, FOREIGN_RULES.into())));
    assert_eq!(
        conn.query_row("SELECT count(*) FROM asset", [], |r| r.get::<_, i64>(0))
            .unwrap(),
        0
    );
    assert_eq!(
        conn.query_row("SELECT count(*) FROM container", [], |r| r.get::<_, i64>(0))
            .unwrap(),
        3
    );
    assert_eq!(
        conn.query_row(
            "SELECT count(*) FROM location_container WHERE location_id=42",
            [],
            |r| r.get::<_, i64>(0)
        )
        .unwrap(),
        0
    );
    assert_eq!(std::fs::read(&source).unwrap(), original);
}

fn identify(path: &Path, byte: u8) {
    let hex = format!("{byte:02x}").repeat(16);
    Connection::open(path)
        .unwrap()
        .execute_batch(&format!(
            "CREATE VIEW serato_db AS SELECT X'{hex}' AS uuid;"
        ))
        .unwrap();
}
fn local_report(source: &Path, removed: bool) -> crate::ownership::PlaylistOwnershipReport {
    use crate::ownership::*;
    let mut report = PlaylistOwnershipReport::default();
    report.current.identity =
        crate::serato4::owned::identity(&Connection::open(source).unwrap()).unwrap();
    report.current.bindings.push(PlaylistBinding {
        source_id: "folder".into(),
        locator: r#"{"space":1,"container":2}"#.into(),
        parent_source_id: None,
        created: true,
        fingerprint: String::new(),
    });
    if removed {
        report.deleted.insert("crate".into());
    } else {
        report.current.bindings.push(PlaylistBinding {
            source_id: "crate".into(),
            locator: r#"{"space":1,"container":3}"#.into(),
            parent_source_id: Some("folder".into()),
            created: true,
            fingerprint: String::new(),
        });
    }
    report
}
fn aggregate_request(
    report: &crate::ownership::PlaylistOwnershipReport,
) -> crate::ownership::PlaylistOwnershipRequest {
    crate::ownership::PlaylistOwnershipRequest {
        previous: report.current.clone(),
        ..Default::default()
    }
}
#[test]
fn owned_master_deletion_prunes_one_location_then_shared_container_with_creation_proof() {
    let (_ad, a) = native_source(10);
    let (_bd, b) = native_source(11);
    let (_md, master) = database();
    identify(&master, 1);
    identify(&a, 11);
    identify(&b, 12);
    materialize(&a);
    materialize(&b);
    register(&master, &a, 42);
    register(&master, &b, 43);
    let first = refresh_master_owned(
        &master,
        &a,
        &a,
        &Default::default(),
        &local_report(&a, false),
    )
    .unwrap();
    let second = refresh_master_owned(
        &master,
        &b,
        &b,
        &aggregate_request(&first),
        &local_report(&b, false),
    )
    .unwrap();
    let conn = Connection::open(&master).unwrap();
    let target = mapped(&conn, 42, 3);
    assert_eq!(target, mapped(&conn, 43, 3));
    // A native crate not claimed by either source remains foreign.
    conn.execute_batch("INSERT INTO container VALUES(90,1,1,'Foreign',1,99,1,0,0,'',NULL);INSERT INTO location_container VALUES(900,90,99,90);").unwrap();
    Connection::open(&a)
        .unwrap()
        .execute_batch(
            "DELETE FROM container_asset WHERE container_id=3;DELETE FROM container WHERE id=3;",
        )
        .unwrap();
    let third = refresh_master_owned(
        &master,
        &a,
        &a,
        &aggregate_request(&second),
        &local_report(&a, true),
    )
    .unwrap();
    assert_eq!(mapped(&conn, 43, 3), target);
    assert_eq!(conn.query_row("SELECT COUNT(*) FROM location_container WHERE location_id=42 AND external_container_id=3",[],|r|r.get::<_,i64>(0)).unwrap(),0);
    Connection::open(&b)
        .unwrap()
        .execute_batch(
            "DELETE FROM container_asset WHERE container_id=3;DELETE FROM container WHERE id=3;",
        )
        .unwrap();
    let fourth = refresh_master_owned(
        &master,
        &b,
        &b,
        &aggregate_request(&third),
        &local_report(&b, true),
    )
    .unwrap();
    assert!(fourth.deleted.contains("crate"));
    assert_eq!(
        conn.query_row("SELECT COUNT(*) FROM container WHERE id=?", [target], |r| r
            .get::<_, i64>(0))
            .unwrap(),
        0
    );
    assert_eq!(
        conn.query_row(
            "SELECT COUNT(*) FROM location_container WHERE location_id=99",
            [],
            |r| r.get::<_, i64>(0)
        )
        .unwrap(),
        1
    );
    assert_eq!(
        conn.query_row("SELECT COUNT(*) FROM asset", [], |r| r.get::<_, i64>(0))
            .unwrap(),
        2
    );
}
#[test]
fn owned_master_matched_container_is_not_promoted_to_created_and_native_edits_conflict() {
    let (_sd, source) = native_source(10);
    materialize(&source);
    let (_md, master) = database();
    identify(&master, 2);
    identify(&source, 13);
    register(&master, &source, 42);
    let conn = Connection::open(&master).unwrap();
    conn.execute_batch("INSERT INTO container VALUES(80,1,1,'Folder',1,1,1,0,0,'',NULL),(81,1,80,'Crate',1,1,1,0,0,'',NULL);").unwrap();
    let first = refresh_master_owned(
        &master,
        &source,
        &source,
        &Default::default(),
        &local_report(&source, false),
    )
    .unwrap();
    assert!(first.current.bindings.iter().all(|b| !b.created));
    conn.execute("UPDATE container SET name='Native edit' WHERE id=81", [])
        .unwrap();
    assert!(
        refresh_master_owned(
            &master,
            &source,
            &source,
            &aggregate_request(&first),
            &local_report(&source, false)
        )
        .is_err()
    );
    conn.execute("UPDATE container SET name='Crate' WHERE id=81", [])
        .unwrap();
    Connection::open(&source)
        .unwrap()
        .execute_batch(
            "DELETE FROM container_asset WHERE container_id=3;DELETE FROM container WHERE id=3;",
        )
        .unwrap();
    let next = refresh_master_owned(
        &master,
        &source,
        &source,
        &aggregate_request(&first),
        &local_report(&source, true),
    )
    .unwrap();
    assert!(next.retired.contains("crate"));
    assert_eq!(
        conn.query_row("SELECT name FROM container WHERE id=81", [], |r| r
            .get::<_, String>(0))
            .unwrap(),
        "Crate"
    );
}
#[test]
fn owned_master_rename_swap_uses_mapped_ids_with_native_unique_sibling_names() {
    let (_sd, source) = native_source(10);
    materialize(&source);
    let src = Connection::open(&source).unwrap();
    src.execute_batch("INSERT INTO container VALUES(4,1,2,'Other',1,2,1,0,0,'',NULL);")
        .unwrap();
    let (_md, master) = database();
    identify(&master, 3);
    identify(&source, 14);
    register(&master, &source, 42);
    let mut report = local_report(&source, false);
    report
        .current
        .bindings
        .push(crate::ownership::PlaylistBinding {
            source_id: "other".into(),
            locator: r#"{"space":1,"container":4}"#.into(),
            parent_source_id: Some("folder".into()),
            created: true,
            fingerprint: String::new(),
        });
    let first =
        refresh_master_owned(&master, &source, &source, &Default::default(), &report).unwrap();
    let conn = Connection::open(&master).unwrap();
    let a = mapped(&conn, 42, 3);
    let b = mapped(&conn, 42, 4);
    conn.execute_batch(
        "CREATE UNIQUE INDEX sibling_names ON container(parent_id,name COLLATE NOCASE);",
    )
    .unwrap();
    src.execute_batch("UPDATE container SET name='temporary' WHERE id=3;UPDATE container SET name='Crate' WHERE id=4;UPDATE container SET name='Other' WHERE id=3;").unwrap();
    refresh_master_owned(
        &master,
        &source,
        &source,
        &aggregate_request(&first),
        &report,
    )
    .unwrap();
    assert_eq!(mapped(&conn, 42, 3), a);
    assert_eq!(mapped(&conn, 42, 4), b);
    assert_eq!(
        conn.query_row("SELECT name FROM container WHERE id=?", [a], |r| r
            .get::<_, String>(0))
            .unwrap(),
        "Other"
    );
    assert_eq!(
        conn.query_row("SELECT name FROM container WHERE id=?", [b], |r| r
            .get::<_, String>(0))
            .unwrap(),
        "Crate"
    );
}
#[test]
fn owned_shared_locations_coalesce_after_folder_and_crate_rename_without_duplicates() {
    let (_ad, a) = native_source(10);
    let (_bd, b) = native_source(11);
    materialize(&a);
    materialize(&b);
    let (_md, master) = database();
    identify(&master, 4);
    identify(&a, 15);
    identify(&b, 16);
    register(&master, &a, 42);
    register(&master, &b, 43);
    let first = refresh_master_owned(
        &master,
        &a,
        &a,
        &Default::default(),
        &local_report(&a, false),
    )
    .unwrap();
    let second = refresh_master_owned(
        &master,
        &b,
        &b,
        &aggregate_request(&first),
        &local_report(&b, false),
    )
    .unwrap();
    let conn = Connection::open(&master).unwrap();
    let old = mapped(&conn, 42, 3);
    conn.execute_batch(
        "CREATE UNIQUE INDEX sibling_names ON container(parent_id,name COLLATE NOCASE)",
    )
    .unwrap();
    Connection::open(&a).unwrap().execute_batch("UPDATE container SET name='Renamed folder' WHERE id=2;UPDATE container SET name='Renamed crate' WHERE id=3;").unwrap();
    let third = refresh_master_owned(
        &master,
        &a,
        &a,
        &aggregate_request(&second),
        &local_report(&a, false),
    )
    .unwrap();
    let new = mapped(&conn, 42, 3);
    assert_ne!(new, old);
    assert_eq!(mapped(&conn, 43, 3), old);
    Connection::open(&b).unwrap().execute_batch("UPDATE container SET name='Renamed folder' WHERE id=2;UPDATE container SET name='Renamed crate' WHERE id=3;").unwrap();
    refresh_master_owned(
        &master,
        &b,
        &b,
        &aggregate_request(&third),
        &local_report(&b, false),
    )
    .unwrap();
    assert_eq!(mapped(&conn, 42, 3), new);
    assert_eq!(mapped(&conn, 43, 3), new);
    assert_eq!(
        conn.query_row("SELECT COUNT(*) FROM container WHERE type<>0", [], |r| r
            .get::<_, i64>(0))
            .unwrap(),
        2
    );
}
