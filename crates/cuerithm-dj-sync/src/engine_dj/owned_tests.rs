use super::*;
use crate::{
    SyncPlaylist,
    ownership::{PlaylistOwnershipReport, PlaylistOwnershipRequest},
};
use std::collections::BTreeSet;
#[test]
fn saved_dates_and_container_filetypes_are_exported_coherently() {
    let (dir, path, mut library) = fixture();
    library.tracks[0].path = dir.path().join("source.m4a");
    library.tracks[0].attributes.insert(
        "dateAdded".into(),
        cuerithm_smartlists::FieldValue::Date(1_640_995_200_123),
    );
    library.tracks[0].attributes.insert(
        "fileType".into(),
        cuerithm_smartlists::FieldValue::Text("alac".into()),
    );
    let first = run(
        &path,
        &library,
        &Default::default(),
        &[],
        &Default::default(),
    )
    .unwrap();
    let db = Connection::open(&path).unwrap();
    let metadata: (i64, String) = db
        .query_row(
            "SELECT dateAdded,fileType FROM Track WHERE title='Synthetic'",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    assert_eq!(metadata, (1_640_995_200, "m4a".into()));
    library.tracks[0].attributes.remove("dateAdded");
    run(&path, &library, &first, &[], &Default::default()).unwrap();
    assert_eq!(
        db.query_row(
            "SELECT dateAdded FROM Track WHERE title='Synthetic'",
            [],
            |r| r.get::<_, i64>(0)
        )
        .unwrap(),
        0
    );
}
#[test]
fn native_bpm_metadata_keeps_source_precision_for_smart_comparisons() {
    use cuerithm_smartlists::{Operator, RuleValue, SmartDefinition, SmartRule};
    let (_dir, path, mut library) = fixture();
    library.playlists.clear();
    let db = Connection::open(&path).unwrap();
    for bpm in [120.0049, 120.0, 128.12345] {
        library.tracks[0].bpm = bpm;
        run(
            &path,
            &library,
            &Default::default(),
            &[],
            &Default::default(),
        )
        .unwrap();
        let stored: (f64, f64) = db
            .query_row(
                "SELECT bpm,bpmAnalyzed FROM Track WHERE title='Synthetic'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        assert_eq!(stored, (bpm, bpm));
        let comparison = cuerithm_smartlists::comparison_bpm(bpm).unwrap();
        let playlist = SyncPlaylist {
            smart: Some(SmartDefinition {
                match_all: true,
                rules: vec![SmartRule {
                    id: "bpm".into(),
                    field: "bpm".into(),
                    operator: Operator::NumberEquals,
                    value: RuleValue::Number(comparison),
                    or: false,
                }],
            }),
            ..Default::default()
        };
        let Some(crate::smart_rules::NativeSmart::EngineDj(rules)) = crate::smart_rules::compile(
            &playlist,
            &library,
            &Default::default(),
            crate::DjSoftware::EngineDj,
        )
        .unwrap() else {
            panic!("expected native BPM rule")
        };
        let rules: serde_json::Value = serde_json::from_str(&rules).unwrap();
        let column = rules["rules"][0]["col"].as_str().unwrap();
        assert!(
            db.query_row(
                &format!("SELECT {column}=?1 FROM Track WHERE title='Synthetic'"),
                [comparison],
                |row| row.get::<_, bool>(0)
            )
            .unwrap()
        );
    }
}
fn fixture() -> (tempfile::TempDir, PathBuf, SyncLibrary) {
    let (dir, path) = super::tests::fixture();
    let db = Connection::open(&path).unwrap();
    db.execute_batch("CREATE TABLE Smartlist(listUuid TEXT PRIMARY KEY,title TEXT,parentPlaylistPath TEXT,nextPlaylistPath TEXT,nextListUuid TEXT,rules TEXT,lastEditTime TEXT,UNIQUE(title,parentPlaylistPath),UNIQUE(parentPlaylistPath,nextPlaylistPath,nextListUuid));
        CREATE TRIGGER native_delete_playlist AFTER DELETE ON Playlist BEGIN
        UPDATE Playlist SET nextListId=OLD.nextListId WHERE nextListId=OLD.id;
        DELETE FROM Playlist WHERE parentListId=OLD.id; END;").unwrap();
    let library = SyncLibrary {
        tracks: vec![super::tests::track(dir.path().join("source.wav"))],
        playlists: vec![
            SyncPlaylist {
                id: "a".into(),
                name: "A".into(),
                folder: true,
                ..Default::default()
            },
            SyncPlaylist {
                id: "b".into(),
                name: "B".into(),
                folder: true,
                ..Default::default()
            },
            SyncPlaylist {
                id: "set".into(),
                parent_id: Some("a".into()),
                name: "Set".into(),
                track_ids: vec!["source".into()],
                ..Default::default()
            },
        ],
    };
    (dir, path, library)
}
fn run(
    path: &Path,
    library: &SyncLibrary,
    previous: &PlaylistOwnershipReport,
    removed: &[&str],
    options: &SyncOptions,
) -> Result<PlaylistOwnershipReport> {
    write_staged_owned(
        path,
        path.parent().unwrap().parent().unwrap(),
        library,
        options,
        &PlaylistOwnershipRequest {
            previous: previous.current.clone(),
            removed: removed.iter().map(|id| (*id).into()).collect(),
        },
    )
}
fn locator(report: &PlaylistOwnershipReport, id: &str) -> String {
    report
        .current
        .bindings
        .iter()
        .find(|b| b.source_id == id)
        .unwrap()
        .locator
        .clone()
}
fn native(report: &PlaylistOwnershipReport, id: &str) -> i64 {
    locator(report, id)
        .strip_prefix("playlist:")
        .unwrap()
        .parse()
        .unwrap()
}
#[test]
fn move_swap_and_deletion_only_keep_native_ids_and_repair_rollups() {
    let (_dir, path, mut library) = fixture();
    let db = Connection::open(&path).unwrap();
    db.execute(
        "UPDATE Playlist SET isPersisted=0,lastEditTime='untouched' WHERE title='Keep'",
        [],
    )
    .unwrap();
    let first = run(
        &path,
        &library,
        &Default::default(),
        &[],
        &Default::default(),
    )
    .unwrap();
    let set = native(&first, "set");
    let a = native(&first, "a");
    let b = native(&first, "b");
    library.playlists[0].name = "B".into();
    library.playlists[1].name = "A".into();
    library.playlists[2].parent_id = Some("b".into());
    library.playlists[2].name = "Moved".into();
    let moved = run(&path, &library, &first, &[], &Default::default()).unwrap();
    assert_eq!(native(&moved, "set"), set);
    assert_eq!(native(&moved, "a"), a);
    assert_eq!(native(&moved, "b"), b);
    assert_eq!(
        db.query_row(
            "SELECT parentListId FROM Playlist WHERE id=?1",
            [set],
            |r| r.get::<_, i64>(0)
        )
        .unwrap(),
        b
    );
    assert_eq!(
        db.query_row(
            "SELECT COUNT(*) FROM PlaylistEntity WHERE listId=?1",
            [a],
            |r| r.get::<_, i64>(0)
        )
        .unwrap(),
        0
    );
    assert_eq!(
        db.query_row(
            "SELECT membershipReference FROM PlaylistEntity WHERE listId=?1",
            [b],
            |r| r.get::<_, i64>(0)
        )
        .unwrap(),
        1
    );
    let foreign: (i64, String) = db
        .query_row(
            "SELECT isPersisted,lastEditTime FROM Playlist WHERE title='Keep'",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    assert_eq!(foreign, (0, "untouched".into()));
    let deleted = run(
        &path,
        &SyncLibrary::default(),
        &moved,
        &["set"],
        &Default::default(),
    )
    .unwrap();
    assert_eq!(deleted.deleted, BTreeSet::from(["set".into()]));
    assert!(
        deleted
            .current
            .bindings
            .iter()
            .all(|b| b.source_id != "set")
    );
    assert_eq!(
        db.query_row("SELECT COUNT(*) FROM Playlist WHERE id=?1", [set], |r| r
            .get::<_, i64>(0))
            .unwrap(),
        0
    );
    assert_eq!(
        db.query_row(
            "SELECT COUNT(*) FROM PlaylistEntity WHERE listId=?1",
            [b],
            |r| r.get::<_, i64>(0)
        )
        .unwrap(),
        0
    );
    assert_eq!(
        db.query_row("SELECT COUNT(*) FROM Track", [], |r| r.get::<_, i64>(0))
            .unwrap(),
        2
    );
}
#[test]
fn target_edits_and_foreign_children_refuse_owned_deletion_without_cascading() {
    let (_dir, path, library) = fixture();
    let first = run(
        &path,
        &library,
        &Default::default(),
        &[],
        &Default::default(),
    )
    .unwrap();
    let db = Connection::open(&path).unwrap();
    let a = native(&first, "a");
    let set = native(&first, "set");
    db.execute("UPDATE Playlist SET title='Native edit' WHERE id=?1", [set])
        .unwrap();
    assert!(run(&path, &library, &first, &[], &Default::default()).is_err());
    assert_eq!(
        db.query_row("SELECT title FROM Playlist WHERE id=?1", [set], |r| r
            .get::<_, String>(0))
            .unwrap(),
        "Native edit"
    );
    db.execute("UPDATE Playlist SET title='Set' WHERE id=?1", [set])
        .unwrap();
    let foreign: i64 = db
        .query_row("SELECT MAX(id)+1 FROM Playlist", [], |r| r.get(0))
        .unwrap();
    db.execute(
        "UPDATE Playlist SET nextListId=?1 WHERE id=?2",
        params![foreign, set],
    )
    .unwrap();
    db.execute(
        "INSERT INTO Playlist(id,title,parentListId,nextListId) VALUES(?1,'Foreign',?2,0)",
        params![foreign, a],
    )
    .unwrap();
    assert!(
        run(
            &path,
            &SyncLibrary::default(),
            &first,
            &["a", "set"],
            &Default::default()
        )
        .is_err()
    );
    assert_eq!(
        db.query_row(
            "SELECT COUNT(*) FROM Playlist WHERE id IN(?1,?2,?3)",
            params![a, set, foreign],
            |r| r.get::<_, i64>(0)
        )
        .unwrap(),
        3
    );
}
#[test]
fn external_binding_is_retired_and_native_database_identity_is_guarded() {
    let (_dir, path, _) = fixture();
    let input = SyncLibrary {
        playlists: vec![SyncPlaylist {
            id: "external".into(),
            name: "Keep".into(),
            ..Default::default()
        }],
        ..Default::default()
    };
    let first = run(&path, &input, &Default::default(), &[], &Default::default()).unwrap();
    assert!(!first.current.bindings[0].created);
    let retired = run(
        &path,
        &SyncLibrary::default(),
        &first,
        &["external"],
        &Default::default(),
    )
    .unwrap();
    assert!(retired.retired.contains("external"));
    assert!(retired.deleted.is_empty());
    let db = Connection::open(&path).unwrap();
    assert_eq!(
        db.query_row(
            "SELECT COUNT(*) FROM Playlist WHERE title='Keep'",
            [],
            |r| r.get::<_, i64>(0)
        )
        .unwrap(),
        1
    );
    db.execute("UPDATE Information SET uuid='replacement'", [])
        .unwrap();
    assert!(
        run(&path, &input, &first, &[], &Default::default())
            .unwrap_err()
            .to_string()
            .contains("identity changed")
    );
}
fn smart(id: &str, name: &str, parent: &str) -> SyncPlaylist {
    use cuerithm_smartlists::{Operator, RuleValue, SmartDefinition, SmartRule};
    SyncPlaylist {
        id: id.into(),
        name: name.into(),
        parent_id: Some(parent.into()),
        track_ids: vec!["source".into()],
        smart: Some(SmartDefinition {
            match_all: true,
            rules: vec![SmartRule {
                id: "bpm".into(),
                field: "bpm".into(),
                operator: Operator::NumberEquals,
                value: RuleValue::Number(120.0),
                or: false,
            }],
        }),
        ..Default::default()
    }
}
#[test]
fn smart_uuid_paths_successors_and_unselected_smart_rollups_survive_changes() {
    let (_dir, path, mut library) = fixture();
    library.playlists.insert(2, smart("s", "Smart", "a"));
    library.playlists.push(smart("t", "Tail", "a"));
    let first = run(
        &path,
        &library,
        &Default::default(),
        &[],
        &Default::default(),
    )
    .unwrap();
    let db = Connection::open(&path).unwrap();
    let a = native(&first, "a");
    let sid = locator(&first, "s");
    let tid = engine_smart_uuid("t");
    assert_eq!(
        db.query_row(
            "SELECT membershipReference FROM PlaylistEntity WHERE listId=?1",
            [a],
            |r| r.get::<_, i64>(0)
        )
        .unwrap(),
        3
    );
    let deleted = run(
        &path,
        &SyncLibrary::default(),
        &first,
        &["set"],
        &Default::default(),
    )
    .unwrap();
    assert_eq!(
        db.query_row(
            "SELECT nextListUuid FROM Smartlist WHERE listUuid=?1",
            [engine_smart_uuid("s")],
            |r| r.get::<_, String>(0)
        )
        .unwrap(),
        tid
    );
    assert_eq!(
        db.query_row(
            "SELECT membershipReference FROM PlaylistEntity WHERE listId=?1",
            [a],
            |r| r.get::<_, i64>(0)
        )
        .unwrap(),
        2
    );
    let mut partial = SyncLibrary::default();
    partial.playlists.push(SyncPlaylist {
        id: "a".into(),
        name: "Renamed".into(),
        folder: true,
        ..Default::default()
    });
    let renamed = run(&path, &partial, &deleted, &[], &Default::default()).unwrap();
    assert_eq!(locator(&renamed, "s"), sid);
    assert_eq!(
        db.query_row(
            "SELECT parentPlaylistPath FROM Smartlist WHERE listUuid=?1",
            [engine_smart_uuid("s")],
            |r| r.get::<_, String>(0)
        )
        .unwrap(),
        "Renamed;"
    );
    let removed = run(
        &path,
        &SyncLibrary::default(),
        &renamed,
        &["s", "t", "a"],
        &Default::default(),
    )
    .unwrap();
    assert!(removed.deleted.contains("a"));
    assert_eq!(
        db.query_row("SELECT COUNT(*) FROM Smartlist", [], |r| r.get::<_, i64>(0))
            .unwrap(),
        0
    );
}
#[test]
fn smart_move_and_partial_representation_conversion_retain_binding_and_slot() {
    let (_dir, path, mut library) = fixture();
    library.playlists.insert(2, smart("s", "Smart", "a"));
    let first = run(
        &path,
        &library,
        &Default::default(),
        &[],
        &Default::default(),
    )
    .unwrap();
    let sid = locator(&first, "s");
    library.playlists[2].name = "Moved smart".into();
    library.playlists[2].parent_id = Some("b".into());
    let moved = run(&path, &library, &first, &[], &Default::default()).unwrap();
    assert_eq!(locator(&moved, "s"), sid);
    let mut partial = SyncLibrary {
        tracks: library.tracks.clone(),
        playlists: vec![library.playlists[1].clone(), library.playlists[2].clone()],
    };
    let materialized = run(
        &path,
        &partial,
        &moved,
        &[],
        &SyncOptions {
            materialize_smartlists: true,
            ..Default::default()
        },
    )
    .unwrap();
    assert!(locator(&materialized, "s").starts_with("playlist:"));
    assert!(
        materialized
            .current
            .bindings
            .iter()
            .find(|b| b.source_id == "s")
            .unwrap()
            .created
    );
    partial.playlists[1].name = "Back to smart".into();
    let native = run(&path, &partial, &materialized, &[], &Default::default()).unwrap();
    assert_eq!(locator(&native, "s"), sid);
}
