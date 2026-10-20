use super::*;
fn fixture() -> (tempfile::TempDir, PathBuf, SyncLibrary) {
    let (dir, path, lib) = super::super::tests::fixture();
    Connection::open(&path).unwrap().execute_batch("CREATE VIEW serato_db AS SELECT X'00112233445566778899aabbccddeeff' AS uuid; CREATE TABLE smart_crate_rules(container_id INTEGER PRIMARY KEY,revision INTEGER,version INTEGER,rules TEXT);
 CREATE TABLE alias_container(id INTEGER PRIMARY KEY,container_id INTEGER,list_order INTEGER);
 CREATE TABLE container_keep_alive_list_tbl(container_id INTEGER,location_id INTEGER);").unwrap();
    (dir, path, lib)
}
fn request(report: &PlaylistOwnershipReport, removed: &[&str]) -> PlaylistOwnershipRequest {
    PlaylistOwnershipRequest {
        previous: report.current.clone(),
        removed: removed.iter().map(|s| s.to_string()).collect(),
    }
}
fn id(report: &PlaylistOwnershipReport, source: &str) -> i64 {
    serde_json::from_str::<Locator>(
        &report
            .current
            .bindings
            .iter()
            .find(|b| b.source_id == source)
            .unwrap()
            .locator,
    )
    .unwrap()
    .container
}
#[test]
fn rename_move_and_name_swap_keep_owned_native_ids() {
    let (_d, path, mut lib) = fixture();
    lib.playlists.push(SyncPlaylist {
        id: "other".into(),
        name: "Other".into(),
        ..Default::default()
    });
    let first = write_database_owned(
        &path,
        None,
        &lib,
        &SyncOptions::default(),
        &Default::default(),
    )
    .unwrap();
    assert!(first.current.bindings.iter().all(|b| b.created));
    Connection::open(&path)
        .unwrap()
        .execute_batch(
            "CREATE UNIQUE INDEX sibling_names ON container(parent_id,name COLLATE NOCASE)",
        )
        .unwrap();
    let child = id(&first, "child");
    let other = id(&first, "other");
    lib.playlists[0].name = "Other".into();
    lib.playlists[0].parent_id = None;
    lib.playlists[2].name = "Crate".into();
    let next = write_database_owned(
        &path,
        None,
        &lib,
        &SyncOptions::default(),
        &request(&first, &[]),
    )
    .unwrap();
    assert_eq!(id(&next, "child"), child);
    assert_eq!(id(&next, "other"), other);
    let conn = Connection::open(&path).unwrap();
    assert_eq!(
        conn.query_row("SELECT name FROM container WHERE id=?", [child], |r| r
            .get::<_, String>(
            0
        ))
        .unwrap(),
        "Other"
    );
    let third = write_database_owned(
        &path,
        None,
        &SyncLibrary::default(),
        &SyncOptions::default(),
        &request(&next, &["child", "parent", "other"]),
    )
    .unwrap();
    assert_eq!(third.deleted.len(), 3);
    assert!(third.current.bindings.is_empty());
    assert_eq!(
        conn.query_row("SELECT COUNT(*) FROM container WHERE type<>0", [], |r| r
            .get::<_, i64>(0))
            .unwrap(),
        0
    );
    assert_eq!(
        conn.query_row("SELECT COUNT(*) FROM asset", [], |r| r.get::<_, i64>(0))
            .unwrap(),
        1
    );
}
#[test]
fn external_match_is_retired_and_foreign_children_or_aliases_block_owned_deletion() {
    let (_d, path, mut lib) = fixture();
    lib.playlists = vec![SyncPlaylist {
        id: "one".into(),
        name: "Existing".into(),
        ..Default::default()
    }];
    let conn = Connection::open(&path).unwrap();
    conn.execute_batch("INSERT INTO container VALUES(1,1,0,'Serato Library root',0,1,1,0,0,'',NULL),(9,1,1,'Existing',1,1,1,0,0,'',NULL);").unwrap();
    let first = write_database_owned(
        &path,
        None,
        &lib,
        &SyncOptions::default(),
        &Default::default(),
    )
    .unwrap();
    assert!(!first.current.bindings[0].created);
    let retired = write_database_owned(
        &path,
        None,
        &SyncLibrary::default(),
        &SyncOptions::default(),
        &request(&first, &["one"]),
    )
    .unwrap();
    assert!(retired.retired.contains("one"));
    assert_eq!(
        conn.query_row("SELECT COUNT(*) FROM container WHERE id=9", [], |r| r
            .get::<_, i64>(0))
            .unwrap(),
        1
    );
    lib.playlists[0].name = "Owned".into();
    let owned = write_database_owned(
        &path,
        None,
        &lib,
        &SyncOptions::default(),
        &Default::default(),
    )
    .unwrap();
    let ownedid = id(&owned, "one");
    conn.execute(
        "INSERT INTO container VALUES(90,1,?,'Foreign child',1,1,1,0,0,'',NULL)",
        [ownedid],
    )
    .unwrap();
    assert!(
        write_database_owned(
            &path,
            None,
            &SyncLibrary::default(),
            &SyncOptions::default(),
            &request(&owned, &["one"])
        )
        .is_err()
    );
    conn.execute("DELETE FROM container WHERE id=90", [])
        .unwrap();
    conn.execute("INSERT INTO alias_container VALUES(1,?,1)", [ownedid])
        .unwrap();
    assert!(
        write_database_owned(
            &path,
            None,
            &SyncLibrary::default(),
            &SyncOptions::default(),
            &request(&owned, &["one"])
        )
        .is_err()
    );
    assert_eq!(
        conn.query_row("SELECT name FROM container WHERE id=?", [ownedid], |r| {
            r.get::<_, String>(0)
        })
        .unwrap(),
        "Owned"
    );
}
#[test]
fn native_edit_and_database_replacement_conflict_without_mutation() {
    let (_d, path, mut lib) = fixture();
    let first = write_database_owned(
        &path,
        None,
        &lib,
        &SyncOptions::default(),
        &Default::default(),
    )
    .unwrap();
    let child = id(&first, "child");
    let conn = Connection::open(&path).unwrap();
    conn.execute(
        "UPDATE container SET name='Native edit' WHERE id=?",
        [child],
    )
    .unwrap();
    lib.playlists[0].name = "Source rename".into();
    assert!(
        write_database_owned(
            &path,
            None,
            &lib,
            &SyncOptions::default(),
            &request(&first, &[])
        )
        .is_err()
    );
    assert_eq!(
        conn.query_row("SELECT name FROM container WHERE id=?", [child], |r| r
            .get::<_, String>(
            0
        ))
        .unwrap(),
        "Native edit"
    );
    conn.execute_batch("DROP VIEW serato_db;CREATE VIEW serato_db AS SELECT X'ffeeddccbbaa99887766554433221100' AS uuid;").unwrap();
    assert!(
        write_database_owned(
            &path,
            None,
            &SyncLibrary::default(),
            &SyncOptions::default(),
            &request(&first, &["child"])
        )
        .is_err()
    );
}
#[test]
fn partial_sync_preserves_omitted_bindings_and_detects_edits_later() {
    let (_d, path, lib) = fixture();
    let first = write_database_owned(
        &path,
        None,
        &lib,
        &SyncOptions::default(),
        &Default::default(),
    )
    .unwrap();
    let conn = Connection::open(&path).unwrap();
    conn.execute(
        "UPDATE container SET name='External' WHERE id=?",
        [id(&first, "child")],
    )
    .unwrap();
    let next = write_database_owned(
        &path,
        None,
        &SyncLibrary::default(),
        &SyncOptions::default(),
        &request(&first, &[]),
    )
    .unwrap();
    assert_eq!(next.current.bindings, first.current.bindings);
    assert!(
        write_database_owned(
            &path,
            None,
            &lib,
            &SyncOptions::default(),
            &request(&next, &[])
        )
        .is_err()
    );
}
#[test]
fn saved_native_smart_metadata_is_persisted_in_native_units_and_foreign_fields_survive() {
    use cuerithm_smartlists::FieldValue;
    let (_d, path, mut lib) = fixture();
    let t = &mut lib.tracks[0];
    t.attributes
        .insert("composer".into(), FieldValue::Text("Composer".into()));
    t.attributes
        .insert("remixer".into(), FieldValue::Text("Remixer".into()));
    t.attributes
        .insert("grouping".into(), FieldValue::Text("Grouping".into()));
    t.attributes
        .insert("dateAdded".into(), FieldValue::Date(1_700_000_000_987));
    write_database(&path, &lib, &Default::default()).unwrap();
    let conn = Connection::open(&path).unwrap();
    assert_eq!(
        conn.query_row(
            "SELECT composer,remixer,grouping,time_added FROM asset",
            [],
            |r| Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, i64>(3)?
            ))
        )
        .unwrap(),
        (
            "Composer".into(),
            "Remixer".into(),
            "Grouping".into(),
            1_700_000_000
        )
    );
    conn.execute_batch("UPDATE asset SET dj_play_count=42,emoji='foreign';")
        .unwrap();
    lib.tracks[0].attributes.clear();
    write_database(&path, &lib, &Default::default()).unwrap();
    assert_eq!(
        conn.query_row(
            "SELECT composer,dj_play_count,emoji,time_added FROM asset",
            [],
            |r| Ok((
                r.get::<_, String>(0)?,
                r.get::<_, i64>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, i64>(3)?
            ))
        )
        .unwrap(),
        ("Composer".into(), 42, "foreign".into(), 1_700_000_000)
    );
}
#[test]
fn unfamiliar_native_inbound_references_block_cascading_deletion() {
    let (_d, path, lib) = fixture();
    let first =
        write_database_owned(&path, None, &lib, &Default::default(), &Default::default()).unwrap();
    let target = id(&first, "child");
    let conn = Connection::open(&path).unwrap();
    conn.execute_batch("CREATE TABLE foreign_reference(container INTEGER REFERENCES container(id) ON DELETE CASCADE,note TEXT)").unwrap();
    conn.execute(
        "INSERT INTO foreign_reference VALUES(?,'foreign')",
        [target],
    )
    .unwrap();
    assert!(
        write_database_owned(
            &path,
            None,
            &SyncLibrary::default(),
            &Default::default(),
            &request(&first, &["child", "parent"])
        )
        .is_err()
    );
    assert_eq!(
        conn.query_row("SELECT count(*) FROM foreign_reference", [], |r| r
            .get::<_, i64>(0))
            .unwrap(),
        1
    );
    assert_eq!(
        conn.query_row("SELECT name FROM container WHERE id=?", [target], |r| {
            r.get::<_, String>(0)
        })
        .unwrap(),
        "Crate"
    );
}
#[test]
fn selected_sibling_reordering_preserves_foreign_crate_and_native_ids() {
    let (_d, path, mut lib) = fixture();
    lib.playlists.push(SyncPlaylist {
        id: "other".into(),
        name: "Other".into(),
        folder: true,
        ..Default::default()
    });
    let first =
        write_database_owned(&path, None, &lib, &Default::default(), &Default::default()).unwrap();
    let conn = Connection::open(&path).unwrap();
    conn.execute_batch("CREATE UNIQUE INDEX sibling_orders ON container(parent_id,list_order)")
        .unwrap();
    let parent = id(&first, "parent");
    let root: i64 = conn
        .query_row(
            "SELECT parent_id FROM container WHERE id=?",
            [parent],
            |r| r.get(0),
        )
        .unwrap();
    conn.execute(
        "INSERT INTO container VALUES(90,1,?,'Foreign',1,50,1,0,0,'',NULL)",
        [root],
    )
    .unwrap();
    lib.playlists.reverse();
    let second = write_database_owned(
        &path,
        None,
        &lib,
        &Default::default(),
        &request(&first, &[]),
    )
    .unwrap();
    assert_eq!(id(&second, "parent"), parent);
    let ordered = conn
        .prepare("SELECT name,list_order FROM container WHERE parent_id=? ORDER BY list_order,id")
        .unwrap()
        .query_map([root], |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?))
        })
        .unwrap()
        .collect::<rusqlite::Result<Vec<_>>>()
        .unwrap();
    assert_eq!(
        ordered,
        vec![
            ("Other".into(), 1),
            ("Folder".into(), 2),
            ("Foreign".into(), 50)
        ]
    );
}
