use super::*;
use crate::{SyncCue, SyncPlaylist};

fn database() -> (tempfile::TempDir, PathBuf) {
    let folder = tempfile::tempdir().unwrap();
    let path = folder.path().join("MediaLibrary.db");
    let conn = Connection::open(&path).unwrap();
    conn.execute_batch("CREATE TABLE database2(rowid INTEGER PRIMARY KEY,collection TEXT NOT NULL,key TEXT NOT NULL,data BLOB,metadata BLOB,UNIQUE(collection,key)); CREATE TABLE yap2(extension TEXT,key TEXT,data);") .unwrap();
    for extension in INDEXES {
        conn.execute("INSERT INTO yap2 VALUES(?1,'versionTag',7)", [extension])
            .unwrap();
    }
    (folder, path)
}
fn library() -> SyncLibrary {
    SyncLibrary {
        tracks: vec![SyncTrack {
            export_timing: None,
            native_timeline: None,
            id: "one".into(),
            path: "/music/café & mix.mp3".into(),
            title: "Café".into(),
            artist: "Artist".into(),
            album: "Album".into(),
            genre: "House".into(),
            label: "Label".into(),
            comment: "Comment".into(),
            key: "8A".into(),
            bpm: 120.0,
            duration_seconds: 300.0,
            sample_rate: 48000,
            artwork: None,
            attributes: Default::default(),
            rating: 4,
            year: Some(2026),
            color: Some(0xcc0000),
            cues: vec![
                SyncCue {
                    index: 0,
                    name: "Intro".into(),
                    start_seconds: 1.5,
                    end_seconds: None,
                    color: Some(0x00cc00),
                },
                SyncCue {
                    index: 1,
                    name: "Loop".into(),
                    start_seconds: 20.0,
                    end_seconds: Some(24.0),
                    color: None,
                },
            ],
            beatgrid: vec![SyncBeat {
                seconds: 0.25,
                bpm: 120.0,
                beat: 1,
            }],
        }],
        playlists: vec![
            SyncPlaylist {
                smart: None,
                id: "folder".into(),
                parent_id: None,
                name: "Sets".into(),
                folder: true,
                track_ids: vec![],
            },
            SyncPlaylist {
                smart: None,
                id: "set".into(),
                parent_id: Some("folder".into()),
                name: "Tonight".into(),
                folder: false,
                track_ids: vec!["one".into(), "one".into()],
            },
        ],
    }
}
fn get(path: &Path, collection: &str, id: &str) -> Object {
    let conn = Connection::open(path).unwrap();
    let bytes: Vec<u8> = conn
        .query_row(
            "SELECT data FROM database2 WHERE collection=?1 AND key=?2",
            params![collection, id],
            |row| row.get(0),
        )
        .unwrap();
    decode(&bytes).unwrap()
}
#[test]
fn local_location_filename_index_tracks_native_rows_without_resetting_its_registry() {
    let (_folder, path) = database();
    let conn = Connection::open(&path).unwrap();
    conn.execute_batch("CREATE TABLE secondaryIndex_mediaItemLocationIndex(rowid INTEGER PRIMARY KEY,fileName TEXT); INSERT INTO secondaryIndex_mediaItemLocationIndex VALUES(999,'foreign.mp3'); INSERT INTO yap2 VALUES('mediaItemLocationIndex','versionTag',23);").unwrap();
    let mut input = library();
    write(&path, &input, &SyncOptions::default()).unwrap();
    let id = stable_id("track", "one", false);
    let location_row: i64 = conn
        .query_row(
            "SELECT rowid FROM database2 WHERE collection='localMediaItemLocations' AND key=?1",
            [&id],
            |row| row.get(0),
        )
        .unwrap();
    let filename = || {
        conn.query_row(
            "SELECT fileName FROM secondaryIndex_mediaItemLocationIndex WHERE rowid=?1",
            [location_row],
            |row| row.get::<_, String>(0),
        )
        .unwrap()
    };
    assert_eq!(filename(), "café & mix.mp3");
    conn.execute(
        "UPDATE secondaryIndex_mediaItemLocationIndex SET fileName='stale.mp3' WHERE rowid=?1",
        [location_row],
    )
    .unwrap();
    input.tracks[0].path = "C:\\Music\\new café & mix.mp3".into();
    write(&path, &input, &SyncOptions::default()).unwrap();
    assert_eq!(filename(), "new café & mix.mp3");
    assert_eq!(
        conn.query_row(
            "SELECT fileName FROM secondaryIndex_mediaItemLocationIndex WHERE rowid=999",
            [],
            |row| row.get::<_, String>(0)
        )
        .unwrap(),
        "foreign.mp3"
    );
    assert_eq!(
        conn.query_row(
            "SELECT data FROM yap2 WHERE extension='mediaItemLocationIndex' AND key='versionTag'",
            [],
            |row| row.get::<_, i64>(0)
        )
        .unwrap(),
        23
    );
    assert_eq!(
        conn.query_row(
            "SELECT COUNT(*) FROM secondaryIndex_mediaItemLocationIndex",
            [],
            |row| row.get::<_, i64>(0)
        )
        .unwrap(),
        2
    );
}
#[test]
fn native_metadata_cues_loops_grid_and_playlist_roundtrip() {
    let (_folder, path) = database();
    let library = library();
    write(&path, &library, &SyncOptions::default()).unwrap();
    let id = stable_id("track", "one", false);
    let item = get(&path, "mediaItems", &id);
    assert_eq!(item.get("comments"), Some(&text("Comment")));
    assert_eq!(item.get("year"), Some(&Value::U16(2026)));
    assert!(item.get("artistUUIDs").is_some());
    let user = get(&path, "mediaItemUserData", &id);
    assert_eq!(user.get("manualKeySignatureIndex2"), Some(&Value::U16(1)));
    let Some(Value::Array(cues)) = user.get("cuePoints") else {
        panic!()
    };
    let Value::Class(cue) = &cues[1] else {
        panic!()
    };
    assert_eq!(cue.get("number"), Some(&Value::Double(2.0)));
    assert_eq!(cue.get("endTime"), Some(&Value::Double(24.0)));
    let Some(Value::Class(grid)) = user.get(GRID) else {
        panic!()
    };
    assert_eq!(
        grid.get("firstDownbeatPosition"),
        Some(&Value::Double(0.25))
    );
    let location = get(&path, "localMediaItemLocations", &id);
    assert_eq!(
        location.get("sourceURIs"),
        Some(&Value::Array(vec![Value::Uri(
            "file:///music/caf%C3%A9%20&%20mix.mp3".into()
        )]))
    );
    let playlist = get(&path, PLAYLISTS, &stable_id("playlist", "set", true));
    let Some(Value::Array(items)) = playlist.get("itemUUIDs") else {
        panic!()
    };
    assert_eq!(items.len(), 2);
    assert_ne!(items[0], items[1]);
    assert_eq!(
        playlist.get("parentUUID"),
        Some(&text(&stable_id("playlist", "folder", true)))
    );
    let conn = Connection::open(&path).unwrap();
    assert_eq!(
        conn.query_row("SELECT COUNT(*) FROM yap2 WHERE data!=0", [], |row| row
            .get::<_, i64>(0))
            .unwrap(),
        0
    );
    // A repeated sync replaces owned rows while preserving their row IDs.
    let before: i64 = conn
        .query_row(
            "SELECT rowid FROM database2 WHERE collection='mediaItems'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    write(path.parent().unwrap(), &library, &SyncOptions::default()).unwrap();
    let after: i64 = conn
        .query_row(
            "SELECT rowid FROM database2 WHERE collection='mediaItems'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(before, after);
    assert_eq!(
        conn.query_row(
            "SELECT COUNT(*) FROM database2 WHERE collection=?1",
            [LINKS],
            |row| row.get::<_, i64>(0)
        )
        .unwrap(),
        2
    );
}
#[test]
fn existing_source_identity_bookmarks_and_skipped_grid_are_preserved() {
    let (_folder, path) = database();
    let mut conn = Connection::open(&path).unwrap();
    let tx = conn.transaction().unwrap();
    let mut old = Object::new("ADCMediaItemLocation");
    old.set("uuid", text("existing"));
    old.set(
        "sourceURIs",
        Value::Array(vec![Value::Uri(
            file_uri(&library().tracks[0].path).unwrap(),
        )]),
    );
    old.set("urlBookmarkData", Value::Binary(vec![1, 7, 19, 0, 4]));
    upsert(&tx, "localMediaItemLocations", "existing", &old).unwrap();
    let mut user = Object::new("ADCMediaItemUserData");
    let mut grid = Object::new("ADCBeatGridEdits");
    grid.set("fractionalBeatShift", Value::Float(0.375));
    user.set(GRID, Value::Class(grid.clone()));
    user.set("manualBPM", Value::Double(123.45));
    user.set("manualGain", Value::Float(-2.5));
    upsert(&tx, "mediaItemUserData", "existing", &user).unwrap();
    tx.commit().unwrap();
    write(
        &path,
        &library(),
        &SyncOptions {
            skip_beatgrids: true,
            ..Default::default()
        },
    )
    .unwrap();
    let user = get(&path, "mediaItemUserData", "existing");
    assert_eq!(user.get(GRID), Some(&Value::Class(grid)));
    assert_eq!(user.get("manualBPM"), Some(&Value::Double(123.45)));
    assert_eq!(user.get("manualGain"), Some(&Value::Float(-2.5)));
    assert_eq!(
        get(&path, "localMediaItemLocations", "existing").get("urlBookmarkData"),
        Some(&Value::Binary(vec![1, 7, 19, 0, 4]))
    );
    assert_eq!(
        conn.query_row(
            "SELECT COUNT(*) FROM database2 WHERE collection='mediaItems'",
            [],
            |row| row.get::<_, i64>(0)
        )
        .unwrap(),
        1
    );
}
#[test]
fn failure_rolls_back_every_collection_and_index_change() {
    let (_folder, path) = database();
    let mut library = library();
    library.tracks[0].beatgrid = vec![
        SyncBeat {
            seconds: 1.0,
            bpm: 120.0,
            beat: 1,
        },
        SyncBeat {
            seconds: 10.0,
            bpm: 127.0,
            beat: 1,
        },
    ];
    assert!(write(&path, &library, &SyncOptions::default()).is_err());
    let conn = Connection::open(path).unwrap();
    assert_eq!(
        conn.query_row("SELECT COUNT(*) FROM database2", [], |row| row
            .get::<_, i64>(0))
            .unwrap(),
        0
    );
    assert_eq!(
        conn.query_row("SELECT COUNT(*) FROM yap2 WHERE data!=7", [], |row| row
            .get::<_, i64>(0))
            .unwrap(),
        0
    );
}
#[test]
fn malformed_and_legacy_objects_fail_instead_of_being_overwritten() {
    for bytes in [
        b"bplist00".as_slice(),
        b"TSAF\x03\x00\x03\x00".as_slice(),
        &[0; 100],
    ] {
        assert!(decode(bytes).is_err());
    }
    let (_folder, path) = database();
    let conn = Connection::open(&path).unwrap();
    conn.execute(
        "INSERT INTO database2(collection,key,data) VALUES('localMediaItemLocations','old',?1)",
        [b"bplist00".as_slice()],
    )
    .unwrap();
    assert!(write(&path, &library(), &SyncOptions::default()).is_err());
    assert_eq!(
        conn.query_row("SELECT COUNT(*) FROM database2", [], |row| row
            .get::<_, i64>(0))
            .unwrap(),
        1
    );
}
#[test]
fn key_conventions_and_file_uris_match_native_indices() {
    for (key, index) in [
        ("8A", 1),
        ("8B", 0),
        ("1B", 22),
        ("Am", 1),
        ("C", 0),
        ("F# minor", 19),
        ("1m", 1),
        ("8d", 2),
    ] {
        assert_eq!(key_index(key), Some(index), "{key}");
    }
    assert_eq!(key_index("unknown"), None);
    assert_eq!(
        file_uri(Path::new("C:\\Music\\a #1.mp3")).unwrap(),
        "file:///C:/Music/a%20%231.mp3"
    );
}
#[test]
fn variable_grid_retains_native_bar_markers() {
    let markers = [
        SyncBeat {
            seconds: 0.5,
            bpm: 120.0,
            beat: 1,
        },
        SyncBeat {
            seconds: 8.5,
            bpm: 128.0,
            beat: 1,
        },
        SyncBeat {
            seconds: 16.0,
            bpm: 120.0,
            beat: 1,
        },
    ];
    let (_, grid) = beatgrid(&markers).unwrap();
    let Some(Value::Array(values)) = grid.get("downbeatMarkers") else {
        panic!()
    };
    assert_eq!(
        values[1],
        Value::Fields(vec![
            ("position".into(), Value::Float(8.5)),
            ("numberOfBars".into(), Value::Small(4))
        ])
    );
    let mut root = Object::new("Test");
    root.set(GRID, Value::Class(grid));
    assert_eq!(decode(&encode(&root).unwrap()).unwrap(), root);
}
#[test]
fn playlist_cycle_and_slot_collisions_reject_without_writes() {
    let (_folder, path) = database();
    let mut library = library();
    library.playlists[0].parent_id = Some("folder".into());
    assert!(write(&path, &library, &SyncOptions::default()).is_err());
    library.playlists[0].parent_id = None;
    library.tracks[0].cues[1].index = 0;
    assert!(write(&path, &library, &SyncOptions::default()).is_err());
}

#[test]
fn reads_synthetic_reference_writer_with_string_pointers() {
    // Generated from synthetic metadata by the locally observed native-format
    // reference serializer. Contains no music, user paths, or application code.
    let bytes = include_bytes!("fixtures/native-user-data.bin");
    let object = decode(bytes).unwrap();
    assert_eq!(object.class, "ADCMediaItemUserData");
    assert_eq!(
        object.get("uuid"),
        Some(&text("synthetic-native-reference"))
    );
    assert_eq!(object.get("manualGain"), Some(&Value::Float(-2.5)));
    let Some(Value::Array(title_ids)) = object.get("titleIDs") else {
        panic!()
    };
    let Value::Class(title) = &title_ids[0] else {
        panic!()
    };
    assert_eq!(title.get("title"), Some(&text("Café")));
    assert_eq!(title.get("uuid"), Some(&text("synthetic-native-reference")));
    let Some(Value::Array(cues)) = object.get("cuePoints") else {
        panic!()
    };
    let Value::Class(cue) = &cues[1] else {
        panic!()
    };
    assert_eq!(cue.get("endTime"), Some(&Value::Double(24.0)));
    assert_eq!(decode(&encode(&object).unwrap()).unwrap(), object);
}

#[test]
fn partial_playlist_scope_preserves_siblings_and_cleans_the_old_parent_on_move() {
    let (_folder, path) = database();
    let folder = SyncPlaylist {
        id: "folder".into(),
        name: "Folder".into(),
        folder: true,
        ..Default::default()
    };
    let first = SyncPlaylist {
        id: "first".into(),
        name: "First".into(),
        parent_id: Some("folder".into()),
        ..Default::default()
    };
    let second = SyncPlaylist {
        id: "second".into(),
        name: "Second".into(),
        parent_id: Some("folder".into()),
        ..Default::default()
    };
    let all = SyncLibrary {
        tracks: vec![],
        playlists: vec![folder.clone(), first.clone(), second],
    };
    write(&path, &all, &SyncOptions::default()).unwrap();
    let partial = SyncLibrary {
        tracks: vec![],
        playlists: vec![folder, first.clone()],
    };
    write(&path, &partial, &SyncOptions::default()).unwrap();
    let old_parent = stable_id("playlist", "folder", true);
    let Some(Value::Array(children)) = get(&path, PLAYLISTS, &old_parent)
        .get("childUUIDs")
        .cloned()
    else {
        panic!()
    };
    assert_eq!(children.len(), 2);
    let moved = SyncLibrary {
        tracks: vec![],
        playlists: vec![SyncPlaylist {
            parent_id: None,
            ..first
        }],
    };
    write(&path, &moved, &SyncOptions::default()).unwrap();
    let Some(Value::Array(children)) = get(&path, PLAYLISTS, &old_parent)
        .get("childUUIDs")
        .cloned()
    else {
        panic!()
    };
    assert_eq!(children, vec![text(&stable_id("playlist", "second", true))]);
    let Some(Value::Array(children)) = get(&path, PLAYLISTS, ROOT).get("childUUIDs").cloned()
    else {
        panic!()
    };
    assert!(children.contains(&text(&old_parent)));
    assert!(children.contains(&text(&stable_id("playlist", "first", true))));
}

#[test]
fn empty_projection_leaves_native_database_bytes_and_index_versions_untouched() {
    let (_folder, path) = database();
    let before = std::fs::read(&path).unwrap();
    write(&path, &SyncLibrary::default(), &SyncOptions::default()).unwrap();
    assert_eq!(std::fs::read(&path).unwrap(), before);
    assert_eq!(cue_color(0x1058de, false), Some(2));
    assert_eq!(cue_color(0xffe194, true), Some(12));
    assert_eq!(cue_color(0xffe194, false), None);
}

#[cfg(not(windows))]
#[test]
fn native_smart_filter_set_toggles_to_membership_without_duplicate_identity() {
    use cuerithm_smartlists::{Operator, RuleValue, SmartDefinition, SmartRule};
    let (_dir, path) = database();
    let mut input = library();
    input.playlists[1].smart = Some(SmartDefinition {
        match_all: true,
        rules: vec![SmartRule {
            id: "artist".into(),
            field: "artist".into(),
            operator: Operator::StringContains,
            value: RuleValue::Text("Artist".into()),
            or: false,
        }],
    });
    let id = stable_id("playlist", "set", true);
    write(&path, &input, &SyncOptions::default()).unwrap();
    let object = get(&path, PLAYLISTS, &id);
    assert_eq!(object.get("type"), Some(&Value::Small(2)));
    assert!(object.get("itemUUIDs").is_none());
    let Some(Value::Class(set)) = object.get("filterSet") else {
        panic!()
    };
    let Some(Value::Array(filters)) = set.get("filters") else {
        panic!()
    };
    let Value::Class(filter) = &filters[0] else {
        panic!()
    };
    assert_eq!(filter.class, "ADCMediaItemStringFilter");
    assert_eq!(filter.get("keyPath"), Some(&text("artist")));
    assert_eq!(filter.get("operatorType"), Some(&Value::U16(0)));
    write(
        &path,
        &input,
        &SyncOptions {
            materialize_smartlists: true,
            ..Default::default()
        },
    )
    .unwrap();
    let object = get(&path, PLAYLISTS, &id);
    assert_eq!(object.get("type"), Some(&Value::Small(0)));
    assert!(object.get("filterSet").is_none());
    let Some(Value::Array(items)) = object.get("itemUUIDs") else {
        panic!()
    };
    assert_eq!(items.len(), 2);
    write(&path, &input, &SyncOptions::default()).unwrap();
    let conn = Connection::open(&path).unwrap();
    assert_eq!(
        conn.query_row(
            "SELECT COUNT(*) FROM database2 WHERE collection=?1",
            [LINKS],
            |row| row.get::<_, i64>(0)
        )
        .unwrap(),
        0
    );
    assert_eq!(
        conn.query_row(
            "SELECT COUNT(*) FROM database2 WHERE collection=?1 AND key=?2",
            params![PLAYLISTS, id],
            |row| row.get::<_, i64>(0)
        )
        .unwrap(),
        1
    );
}

#[test]
fn prepared_composer_grouping_and_added_date_are_native_metadata() {
    use cuerithm_smartlists::FieldValue;
    let (_dir, path) = database();
    let mut input = library();
    input.tracks[0].attributes = std::collections::BTreeMap::from([
        (
            "composer".into(),
            FieldValue::Text("Fixture composer".into()),
        ),
        ("grouping".into(), FieldValue::Text("Fixture group".into())),
        ("dateAdded".into(), FieldValue::Date(978307323456)),
    ]);
    write(&path, &input, &SyncOptions::default()).unwrap();
    let object = get(&path, "mediaItems", &stable_id("track", "one", false));
    assert_eq!(object.get("composer"), Some(&text("Fixture composer")));
    assert_eq!(object.get("grouping"), Some(&text("Fixture group")));
    let Some(Value::Date(date)) = object.get("addedDate") else {
        panic!()
    };
    assert!((*date - 123.456).abs() < 0.000001);
}

fn owned_request(
    report: &crate::ownership::PlaylistOwnershipReport,
    removed: &[&str],
) -> crate::ownership::PlaylistOwnershipRequest {
    crate::ownership::PlaylistOwnershipRequest {
        previous: report.current.clone(),
        removed: removed.iter().map(|id| (*id).to_owned()).collect(),
    }
}
#[test]
fn acknowledged_playlist_uuid_survives_rename_move_and_name_swap() {
    let (_dir, path) = database();
    let mut input = library();
    input.playlists.push(SyncPlaylist {
        id: "other".into(),
        name: "Other".into(),
        folder: true,
        ..Default::default()
    });
    input.playlists.push(SyncPlaylist {
        id: "sibling".into(),
        name: "Sibling".into(),
        parent_id: Some("folder".into()),
        track_ids: vec!["one".into()],
        ..Default::default()
    });
    let first = write_owned(&path, &input, &SyncOptions::default(), &Default::default()).unwrap();
    assert!(first.current.bindings.iter().all(|binding| binding.created));
    let id = stable_id("playlist", "set", true);
    let sibling = stable_id("playlist", "sibling", true);
    {
        let mut conn = Connection::open(&path).unwrap();
        let tx = conn.transaction().unwrap();
        let mut object = existing(&tx, PLAYLISTS, &id, "ADCMediaItemPlaylist").unwrap();
        object.set("futureProperty", text("preserve"));
        upsert(&tx, PLAYLISTS, &id, &object).unwrap();
        tx.execute(
            "UPDATE database2 SET metadata=?1 WHERE collection=?2 AND key=?3",
            params![vec![1u8, 7, 4], PLAYLISTS, id],
        )
        .unwrap();
        tx.commit().unwrap();
    }
    input.playlists[1].name = "Sibling".into();
    input.playlists[1].parent_id = Some("other".into());
    input.playlists[3].name = "Tonight".into();
    let next = write_owned(
        &path,
        &input,
        &SyncOptions::default(),
        &owned_request(&first, &[]),
    )
    .unwrap();
    assert_eq!(
        next.current
            .bindings
            .iter()
            .find(|binding| binding.source_id == "set")
            .unwrap()
            .locator,
        id
    );
    assert_eq!(
        get(&path, PLAYLISTS, &id).get("name"),
        Some(&text("Sibling"))
    );
    assert_eq!(
        get(&path, PLAYLISTS, &sibling).get("name"),
        Some(&text("Tonight"))
    );
    assert_eq!(
        get(&path, PLAYLISTS, &id).get("futureProperty"),
        Some(&text("preserve"))
    );
    assert_eq!(
        get(&path, PLAYLISTS, &id).get("parentUUID"),
        Some(&text(&stable_id("playlist", "other", true)))
    );
    let conn = Connection::open(&path).unwrap();
    assert_eq!(
        conn.query_row(
            "SELECT metadata FROM database2 WHERE collection=?1 AND key=?2",
            params![PLAYLISTS, id],
            |row| row.get::<_, Vec<u8>>(0)
        )
        .unwrap(),
        [1, 7, 4]
    );
}
#[test]
fn owned_tombstone_only_batch_removes_graph_and_links_without_deleting_media() {
    let (_dir, path) = database();
    let first = write_owned(
        &path,
        &library(),
        &SyncOptions::default(),
        &Default::default(),
    )
    .unwrap();
    let report = write_owned(
        &path,
        &SyncLibrary::default(),
        &SyncOptions::default(),
        &owned_request(&first, &["set", "folder"]),
    )
    .unwrap();
    assert_eq!(
        report.deleted,
        std::collections::BTreeSet::from(["set".into(), "folder".into()])
    );
    assert!(report.current.bindings.is_empty());
    let conn = Connection::open(&path).unwrap();
    assert_eq!(
        conn.query_row(
            "SELECT COUNT(*) FROM database2 WHERE collection=?1",
            [LINKS],
            |row| row.get::<_, i64>(0)
        )
        .unwrap(),
        0
    );
    assert_eq!(
        conn.query_row(
            "SELECT COUNT(*) FROM database2 WHERE collection=?1",
            [PLAYLISTS],
            |row| row.get::<_, i64>(0)
        )
        .unwrap(),
        1
    );
    for collection in [
        "mediaItems",
        "mediaItemUserData",
        "mediaItemAnalyzedData",
        "localMediaItemLocations",
    ] {
        assert_eq!(
            conn.query_row(
                "SELECT COUNT(*) FROM database2 WHERE collection=?1",
                [collection],
                |row| row.get::<_, i64>(0)
            )
            .unwrap(),
            1
        );
    }
    assert_eq!(
        get(&path, PLAYLISTS, ROOT).get("childUUIDs"),
        Some(&Value::Array(vec![]))
    );
}
#[test]
fn matched_external_tombstones_retire_bindings_without_touching_native_bytes() {
    let (_dir, path) = database();
    write(&path, &library(), &SyncOptions::default()).unwrap();
    let adopted = write_owned(
        &path,
        &library(),
        &SyncOptions::default(),
        &Default::default(),
    )
    .unwrap();
    assert!(
        adopted
            .current
            .bindings
            .iter()
            .all(|binding| !binding.created)
    );
    let before = std::fs::read(&path).unwrap();
    let report = write_owned(
        &path,
        &SyncLibrary::default(),
        &SyncOptions::default(),
        &owned_request(&adopted, &["set", "folder"]),
    )
    .unwrap();
    assert_eq!(
        report.retired,
        std::collections::BTreeSet::from(["set".into(), "folder".into()])
    );
    assert!(report.deleted.is_empty());
    assert_eq!(std::fs::read(&path).unwrap(), before);
}
#[test]
fn stale_bound_structure_blocks_before_any_track_or_playlist_write() {
    let (_dir, path) = database();
    let mut input = library();
    let first = write_owned(&path, &input, &SyncOptions::default(), &Default::default()).unwrap();
    let id = stable_id("playlist", "set", true);
    {
        let mut conn = Connection::open(&path).unwrap();
        let tx = conn.transaction().unwrap();
        let mut object = existing(&tx, PLAYLISTS, &id, "ADCMediaItemPlaylist").unwrap();
        object.set("name", text("Changed in djay"));
        upsert(&tx, PLAYLISTS, &id, &object).unwrap();
        tx.commit().unwrap();
    }
    let before = std::fs::read(&path).unwrap();
    input.tracks[0].title = "Would overwrite media metadata".into();
    assert!(
        write_owned(
            &path,
            &input,
            &SyncOptions::default(),
            &owned_request(&first, &[])
        )
        .unwrap_err()
        .to_string()
        .contains("changed")
    );
    assert_eq!(std::fs::read(&path).unwrap(), before);
}
#[test]
fn even_a_fingerprinted_owned_folder_cannot_delete_an_unowned_child() {
    let (_dir, path) = database();
    let folder = stable_id("playlist", "folder", true);
    {
        let mut conn = Connection::open(&path).unwrap();
        let tx = conn.transaction().unwrap();
        let mut foreign = Object::new("ADCMediaItemPlaylist");
        foreign.set("uuid", text("foreign-child"));
        foreign.set("name", text("Foreign"));
        foreign.set("type", Value::Small(0));
        foreign.set("parentUUID", text(&folder));
        foreign.set("itemUUIDs", Value::Array(vec![]));
        upsert(&tx, PLAYLISTS, "foreign-child", &foreign).unwrap();
        tx.commit().unwrap();
    }
    let first = write_owned(
        &path,
        &library(),
        &SyncOptions::default(),
        &Default::default(),
    )
    .unwrap();
    assert!(
        first
            .current
            .bindings
            .iter()
            .find(|binding| binding.source_id == "folder")
            .unwrap()
            .created
    );
    let before = std::fs::read(&path).unwrap();
    let error = write_owned(
        &path,
        &SyncLibrary::default(),
        &SyncOptions::default(),
        &owned_request(&first, &["set", "folder"]),
    )
    .unwrap_err();
    assert!(error.to_string().contains("unowned"));
    assert_eq!(std::fs::read(&path).unwrap(), before);
}
#[test]
fn shared_or_foreign_link_references_are_protected_and_unrelated_edits_survive() {
    let (_dir, path) = database();
    let input = library();
    let first = write_owned(&path, &input, &SyncOptions::default(), &Default::default()).unwrap();
    let link = stable_id("playlist-item", "set:0", true);
    {
        let mut conn = Connection::open(&path).unwrap();
        let tx = conn.transaction().unwrap();
        let mut foreign = Object::new("ADCMediaItemPlaylist");
        foreign.set("uuid", text("foreign-list"));
        foreign.set("name", text("Foreign"));
        foreign.set("type", Value::Small(0));
        foreign.set("parentUUID", text(ROOT));
        foreign.set("itemUUIDs", Value::Array(vec![text(&link)]));
        upsert(&tx, PLAYLISTS, "foreign-list", &foreign).unwrap();
        tx.commit().unwrap();
    }
    let before = std::fs::read(&path).unwrap();
    assert!(
        write_owned(
            &path,
            &SyncLibrary::default(),
            &SyncOptions::default(),
            &owned_request(&first, &["set", "folder"])
        )
        .unwrap_err()
        .to_string()
        .contains("referenced")
    );
    assert_eq!(std::fs::read(&path).unwrap(), before);
    {
        let mut conn = Connection::open(&path).unwrap();
        let tx = conn.transaction().unwrap();
        let mut foreign = existing(&tx, PLAYLISTS, "foreign-list", "ADCMediaItemPlaylist").unwrap();
        foreign.set("itemUUIDs", Value::Array(vec![]));
        foreign.set("name", text("Unrelated edited name"));
        upsert(&tx, PLAYLISTS, "foreign-list", &foreign).unwrap();
        tx.commit().unwrap();
    }
    write_owned(
        &path,
        &input,
        &SyncOptions::default(),
        &owned_request(&first, &[]),
    )
    .unwrap();
    assert_eq!(
        get(&path, PLAYLISTS, "foreign-list").get("name"),
        Some(&text("Unrelated edited name"))
    );
}
#[test]
fn missing_owned_uuid_is_verified_absent_but_a_reused_uuid_is_not_deleted() {
    let (_dir, path) = database();
    let first = write_owned(
        &path,
        &library(),
        &SyncOptions::default(),
        &Default::default(),
    )
    .unwrap();
    let id = stable_id("playlist", "set", true);
    {
        let mut conn = Connection::open(&path).unwrap();
        let tx = conn.transaction().unwrap();
        let mut object = existing(&tx, PLAYLISTS, &id, "ADCMediaItemPlaylist").unwrap();
        object.set("name", text("Replacement at reused ID"));
        upsert(&tx, PLAYLISTS, &id, &object).unwrap();
        tx.commit().unwrap();
    }
    assert!(
        write_owned(
            &path,
            &SyncLibrary::default(),
            &SyncOptions::default(),
            &owned_request(&first, &["set"])
        )
        .is_err()
    );
    let (_other_dir, other_path) = database();
    let other = write_owned(
        &other_path,
        &SyncLibrary {
            playlists: vec![SyncPlaylist {
                id: "empty".into(),
                name: "Empty".into(),
                ..Default::default()
            }],
            ..Default::default()
        },
        &SyncOptions::default(),
        &Default::default(),
    )
    .unwrap();
    let id = &other.current.bindings[0].locator;
    Connection::open(&other_path)
        .unwrap()
        .execute(
            "DELETE FROM database2 WHERE collection=?1 AND key=?2",
            params![PLAYLISTS, id],
        )
        .unwrap();
    let report = write_owned(
        &other_path,
        &SyncLibrary::default(),
        &SyncOptions::default(),
        &owned_request(&other, &["empty"]),
    )
    .unwrap();
    assert!(report.deleted.contains("empty"));
}
#[test]
fn encoded_date_filter_fields_use_native_date_and_relative_value_tags() {
    let absolute = crate::smart_rules::DjayFilter {
        field: "addedDate",
        name: "Added",
        multiple: false,
        operator: 6,
        value: crate::smart_rules::DjayValue::Date {
            start: Some(123.125),
            end: Some(456.875),
            relative: None,
        },
    };
    let relative = crate::smart_rules::DjayFilter {
        field: "addedDate",
        name: "Added",
        multiple: false,
        operator: 4,
        value: crate::smart_rules::DjayValue::Date {
            start: None,
            end: None,
            relative: Some((2, 1)),
        },
    };
    let mut object = Object::new("ADCMediaItemFilterSet");
    object.set(
        "filters",
        Value::Array(vec![native_filter(absolute), native_filter(relative)]),
    );
    let parsed = decode(&encode(&object).unwrap()).unwrap();
    let Some(Value::Array(filters)) = parsed.get("filters") else {
        panic!()
    };
    let Value::Class(absolute) = &filters[0] else {
        panic!()
    };
    assert_eq!(absolute.class, "ADCMediaItemDateFilter");
    assert_eq!(absolute.get("startDate"), Some(&Value::Date(123.125)));
    assert_eq!(absolute.get("endDate"), Some(&Value::Date(456.875)));
    assert!(absolute.get("startTime").is_none());
    let Value::Class(relative) = &filters[1] else {
        panic!()
    };
    assert_eq!(relative.get("periodValue"), Some(&Value::Double(2.0)));
    assert_eq!(relative.get("period"), Some(&Value::Small(1)));
}
