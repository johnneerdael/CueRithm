#[path = "import_fixture_support/mod.rs"]
mod support;
use base64::{Engine, engine::general_purpose::STANDARD};
use cuerithm_dj_sync::import::*;
use cuerithm_media::CancellationToken;
use rusqlite::{Connection, params, params_from_iter, types::Value as SqlValue};
use serde_json::Value;
use support::*;
fn fixture() -> Value {
    serde_json::from_str(include_str!(
        "../../../docs/research/dj-library-import-handover/fixtures/mik-synthetic-import.json"
    ))
    .unwrap()
}
fn schema() -> Value {
    serde_json::from_str(include_str!(
        "../../../docs/research/dj-library-import-handover/fixtures/mik-coredata-11-schema.json"
    ))
    .unwrap()
}
fn dictionary(pairs: Vec<(&str, plist::Value)>) -> plist::Value {
    plist::Value::Dictionary(pairs.into_iter().map(|(k, v)| (k.to_owned(), v)).collect())
}
fn metadata(model: &str) -> Vec<u8> {
    let hashes = dictionary(
        [
            "Collection",
            "CuePoint",
            "EnergySegment",
            "KeySegment",
            "Song",
            "Waveform",
        ]
        .into_iter()
        .map(|name| (name, plist::Value::Data(vec![name.len() as u8; 32])))
        .collect(),
    );
    let value = dictionary(vec![
        (
            "NSStoreUUID",
            plist::Value::String("synthetic-store".into()),
        ),
        ("NSStoreType", plist::Value::String("SQLite".into())),
        (
            "NSStoreModelVersionIdentifiers",
            plist::Value::Array(vec![plist::Value::String(model.into())]),
        ),
        ("NSStoreModelVersionHashes", hashes),
    ]);
    let mut bytes = Vec::new();
    value.to_writer_binary(&mut bytes).unwrap();
    bytes
}
fn database(root: &std::path::Path) -> Connection {
    let db = Connection::open(root.join("Collection11.mikdb")).unwrap();
    let schema = schema();
    for (table, profile) in schema["tables"].as_object().unwrap() {
        let cols = profile["columns"].as_array().unwrap();
        let mut definitions = cols
            .iter()
            .map(|c| {
                format!(
                    "\"{}\" {}",
                    c["name"].as_str().unwrap(),
                    c["sqliteDeclaredType"].as_str().unwrap()
                )
            })
            .collect::<Vec<_>>();
        let mut keys = cols
            .iter()
            .filter(|c| c["primaryKeyOrdinal"].as_u64().unwrap() > 0)
            .collect::<Vec<_>>();
        keys.sort_by_key(|c| c["primaryKeyOrdinal"].as_u64().unwrap());
        if !keys.is_empty() {
            definitions.push(format!(
                "PRIMARY KEY({})",
                keys.iter()
                    .map(|c| format!("\"{}\"", c["name"].as_str().unwrap()))
                    .collect::<Vec<_>>()
                    .join(",")
            ));
        }
        db.execute_batch(&format!(
            "CREATE TABLE \"{table}\"({});",
            definitions.join(",")
        ))
        .unwrap();
    }
    db.execute(
        "INSERT INTO Z_METADATA(Z_VERSION,Z_UUID,Z_PLIST) VALUES(1,'synthetic-store',?1)",
        [metadata("DataModel 11")],
    )
    .unwrap();
    for (id, name) in [
        (1, "Collection"),
        (2, "CuePoint"),
        (3, "EnergySegment"),
        (4, "KeySegment"),
        (5, "Song"),
        (6, "Waveform"),
    ] {
        db.execute(
            "INSERT INTO Z_PRIMARYKEY VALUES(?1,?2,0,1000)",
            params![id, name],
        )
        .unwrap();
    }
    let fixture = fixture();
    for (group, table, entity) in [
        ("collections", "ZCOLLECTION", 1),
        ("songs", "ZSONG", 5),
        ("memberships", "Z_1SONGS", 0),
        ("cuePoints", "ZCUEPOINT", 2),
    ] {
        for row in fixture[group].as_array().unwrap() {
            let mut names = Vec::new();
            let mut values = Vec::new();
            for (name, value) in row.as_object().unwrap() {
                let name = name.strip_suffix("_base64").unwrap_or(name);
                names.push(format!("\"{name}\""));
                values.push(if row.get(format!("{name}_base64")).is_some() {
                    SqlValue::Blob(STANDARD.decode(value.as_str().unwrap()).unwrap())
                } else {
                    match value {
                        Value::Null => SqlValue::Null,
                        Value::String(v) => SqlValue::Text(v.clone()),
                        Value::Number(v) if v.is_i64() => SqlValue::Integer(v.as_i64().unwrap()),
                        Value::Number(v) => SqlValue::Real(v.as_f64().unwrap()),
                        _ => panic!("unexpected fixture"),
                    }
                });
            }
            if entity > 0 {
                names.push("Z_ENT".into());
                values.push(SqlValue::Integer(entity));
            }
            db.execute(
                &format!(
                    "INSERT INTO {table}({}) VALUES({})",
                    names.join(","),
                    vec!["?"; values.len()].join(",")
                ),
                params_from_iter(values),
            )
            .unwrap();
        }
    }
    db
}
fn capture(root: &std::path::Path) -> ImportSnapshot {
    let request = SourceCaptureRequest::from_selected_root(
        SourceProvider::MixedInKey11,
        root,
        vec![ArtifactRequest::sqlite("Collection11.mikdb")],
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
fn mik_fixture_preserves_analysis_cache_markers_and_beats() {
    let root = tempfile::tempdir().unwrap();
    let db = database(root.path());
    db.execute_batch("INSERT INTO ZWAVEFORM(Z_PK,Z_ENT,ZSONG,ZOBJECT) VALUES(1,6,101,X'00');INSERT INTO ZENERGYSEGMENT(Z_PK,Z_ENT,ZSONG,ZSTARTTIME,ZLENGTH,ZENERGY) VALUES(1,3,101,0,1,7);").unwrap();
    drop(db);
    let snapshot = capture(root.path());
    let (header, rows) = read(SourceProvider::MixedInKey11, &snapshot);
    assert_eq!(header.source.library_instance_id, "synthetic-store");
    let tracks = rows.tracks();
    assert_eq!(tracks.len(), 1);
    let one = tracks[0];
    assert_eq!(one.key.as_ref().unwrap().camelot.as_deref(), Some("8A"));
    assert_eq!(
        one.cached_tags["key"],
        Presence::Value(serde_json::json!("Am"))
    );
    assert_eq!(
        one.cached_tags["bpm"],
        Presence::Value(serde_json::json!(119))
    );
    assert_eq!(one.bpm.as_ref().unwrap().value, Some(120.));
    assert_eq!(
        one.locations.original.as_deref(),
        Some("/DemoMusic/Example.flac")
    );
    assert_eq!(one.cues.len(), 2);
    assert_eq!(one.cues[0].native_start_seconds, Some(-0.005));
    assert_eq!(one.cues[1].native_start_seconds, Some(32.125));
    assert!(one.cues.iter().all(|c| c.family == CueFamily::Marker
        && c.source_slot.is_none()
        && c.canonical_start_seconds.is_none()));
    assert!(one.cues[0].name.is_none());
    let grid = one.grid.as_ref().unwrap();
    assert_eq!(
        grid.points
            .iter()
            .map(|p| p.native_seconds)
            .collect::<Vec<_>>(),
        [0., 0.5, 1.]
    );
    assert_eq!(grid.bpm, Some(120.));
    assert!(!grid.downbeat_known);
    assert!(grid.points.iter().all(|p| p.beat_index.is_none()));
    assert_eq!(rows.members()[0].1.position, Some(2048));
    assert!(one.extensions.contains_key("source_energy_segments"));
}

fn read_options(
    snapshot: &ImportSnapshot,
    options: &ReadOptions,
) -> anyhow::Result<(ObservationHeader, Records)> {
    let mut rows = Records::default();
    let header = adapter(SourceProvider::MixedInKey11).read(
        snapshot,
        options,
        &mut rows,
        &CancellationToken::default(),
    )?;
    validate_observation(&header, &mut rows.rows.clone().into_iter(), &options.limits)?;
    Ok((header, rows))
}
fn bookmark() -> Vec<u8> {
    STANDARD
        .decode(
            fixture()["songs"][0]["ZBOOKMARKDATA_base64"]
                .as_str()
                .unwrap(),
        )
        .unwrap()
}
fn beats(change: impl FnOnce(&mut plist::Value)) -> Vec<u8> {
    let bytes = STANDARD
        .decode(fixture()["songs"][0]["ZBEATS_base64"].as_str().unwrap())
        .unwrap();
    let mut value = plist::Value::from_reader(std::io::Cursor::new(bytes)).unwrap();
    change(&mut value);
    let mut output = Vec::new();
    value.to_writer_binary(&mut output).unwrap();
    output
}
fn update_blob(db: &Connection, column: &str, bytes: &[u8]) {
    db.execute(
        &format!("UPDATE ZSONG SET {column}=?1 WHERE Z_PK=101"),
        [bytes],
    )
    .unwrap();
}
#[test]
fn mac_bookmark_history_requires_explicit_mapping_on_foreign_host() {
    let root = tempfile::tempdir().unwrap();
    drop(database(root.path()));
    let (_, rows) = read(SourceProvider::MixedInKey11, &capture(root.path()));
    if std::env::consts::OS != "macos" {
        assert!(
            rows.tracks()[0].locations.resolved.is_none(),
            "a historical Mac path is not host authority"
        );
    }
    assert!(rows.tracks()[0].locations.native_path.is_none());
}
#[test]
fn mac_origin_snapshot_can_be_mapped_without_claiming_native_windows_storage() {
    let root = tempfile::tempdir().unwrap();
    drop(database(root.path()));
    let mapped = tempfile::tempdir().unwrap();
    std::fs::write(mapped.path().join("Example.flac"), b"not decoded").unwrap();
    let snapshot = capture(root.path());
    let options = ReadOptions {
        path_mappings: vec![PathMapping {
            source_prefix: "/DemoMusic".into(),
            local_root: mapped.path().into(),
            root_id: "approved-folder".into(),
        }],
        ..Default::default()
    };
    let (_, rows) = read_options(&snapshot, &options).unwrap();
    assert_eq!(rows.tracks()[0].locations.availability, "available");
    assert_eq!(
        rows.tracks()[0].locations.platform.as_deref(),
        Some("macos")
    );
    assert_eq!(
        rows.tracks()[0].locations.resolved.as_deref(),
        mapped
            .path()
            .canonicalize()
            .unwrap()
            .join("Example.flac")
            .to_str()
    );
    assert!(
        adapter(SourceProvider::MixedInKey11)
            .inspect(&snapshot)
            .unwrap()
            .origin["model_qualification"]
            .contains("Windows storage unsupported")
    );
    assert_eq!(
        std::fs::read(mapped.path().join("Example.flac")).unwrap(),
        b"not decoded"
    );
}
#[test]
fn zeros_nulls_unknown_keys_and_cache_values_remain_distinct() {
    let root = tempfile::tempdir().unwrap();
    let db = database(root.path());
    db.execute_batch("UPDATE ZSONG SET ZKEY='0',ZTEMPO=0,ZENERGY=0,ZNAME='',ZANALYSISDATE=0;INSERT INTO ZSONG(Z_PK,Z_ENT,ZKEY,ZTEMPO,ZENERGY,ZTAGKEY) VALUES(102,5,NULL,NULL,NULL,NULL),(103,5,'unknown key',123,7,'08A');INSERT INTO ZKEYSEGMENT(Z_PK,Z_ENT,ZSONG,ZKEY,ZSTARTTIME,ZLENGTH) VALUES(1,4,101,'7A',0,1);").unwrap();
    drop(db);
    let (_, rows) = read(SourceProvider::MixedInKey11, &capture(root.path()));
    let tracks = rows.tracks();
    assert_eq!(tracks.len(), 3);
    let one = tracks.iter().find(|t| t.external_id == "song:101").unwrap();
    assert_eq!(one.key.as_ref().unwrap().raw.as_deref(), Some("0"));
    assert!(one.key.as_ref().unwrap().camelot.is_none());
    assert!(one.bpm.as_ref().unwrap().value.is_none());
    assert_eq!(one.energy, Presence::Absent);
    assert_eq!(
        one.metadata["title"],
        Presence::Empty(serde_json::json!(""))
    );
    assert_eq!(
        one.extensions["ZANALYSISDATE"]["utc"],
        "2001-01-01T00:00:00Z"
    );
    assert_eq!(one.extensions["source_row"]["ZENERGY"], 0.);
    assert_eq!(
        one.cached_tags["energy"],
        Presence::Value(serde_json::json!(5))
    );
    assert_eq!(
        one.extensions["source_key_segments"]["rows"][0]["ZKEY"],
        "7A"
    );
    let null = tracks.iter().find(|t| t.external_id == "song:102").unwrap();
    assert_eq!(null.key.as_ref().unwrap().presence, FieldPresence::Null);
    assert_eq!(null.energy, Presence::Null);
    assert_eq!(null.cached_tags["key"], Presence::Null);
    let unknown = tracks.iter().find(|t| t.external_id == "song:103").unwrap();
    assert_eq!(
        unknown.key.as_ref().unwrap().raw.as_deref(),
        Some("unknown key")
    );
    assert!(unknown.key.as_ref().unwrap().camelot.is_none());
}
#[test]
fn unknown_model_entity_mapping_and_column_shapes_fail_before_rows() {
    for variant in 0..3 {
        let root = tempfile::tempdir().unwrap();
        let db = database(root.path());
        match variant {
            0 => {
                db.execute(
                    "UPDATE Z_METADATA SET Z_PLIST=?1",
                    [metadata("DataModel 12")],
                )
                .unwrap();
            }
            1 => {
                db.execute_batch("UPDATE Z_PRIMARYKEY SET Z_NAME='FutureSong' WHERE Z_ENT=5")
                    .unwrap();
            }
            _ => {
                db.execute_batch("ALTER TABLE ZSONG ADD COLUMN ZFUTURE VARCHAR")
                    .unwrap();
            }
        }
        drop(db);
        let mut records = Records::default();
        assert!(
            adapter(SourceProvider::MixedInKey11)
                .read(
                    &capture(root.path()),
                    &ReadOptions::default(),
                    &mut records,
                    &CancellationToken::default()
                )
                .is_err()
        );
        assert!(records.rows.is_empty());
    }
}
#[test]
fn broken_bookmarks_retain_song_metadata_and_raw_blob() {
    for variant in 0..4 {
        let root = tempfile::tempdir().unwrap();
        let db = database(root.path());
        let mut bytes = bookmark();
        match variant {
            0 => bytes[64..68].copy_from_slice(&u32::MAX.to_le_bytes()),
            1 => bytes[148..152].copy_from_slice(&72u32.to_le_bytes()), // TOC links to itself
            2 => bytes[108..112].copy_from_slice(&7u32.to_le_bytes()),  // path array payload length
            _ => {
                bytes.truncate(20);
            }
        }
        update_blob(&db, "ZBOOKMARKDATA", &bytes);
        drop(db);
        let (_, rows) = read(SourceProvider::MixedInKey11, &capture(root.path()));
        let track = rows.tracks()[0];
        assert_eq!(
            track.metadata["title"],
            Presence::Value(serde_json::json!("Example track"))
        );
        assert!(track.locations.original.is_none());
        assert!(
            track
                .issues
                .iter()
                .any(|i| i.code == "unresolved_mik_bookmark")
        );
        assert!(rows.blobs.values().any(|raw| raw == &bytes));
    }
}
#[test]
fn bookmark_decoder_bounds_components_toc_cycles_and_external_volumes() {
    use cuerithm_dj_sync::import::readers::bookmark::decode_embedded_bookmark_path;
    let bytes = bookmark();
    let limits = ImportLimits::default();
    assert_eq!(
        decode_embedded_bookmark_path(&bytes, &limits)
            .unwrap()
            .embedded_path
            .as_deref(),
        Some("/DemoMusic/Example.flac")
    );
    for end in 0..bytes.len() {
        assert!(decode_embedded_bookmark_path(&bytes[..end], &limits).is_err());
    }
    let mut unsafe_component = bytes.clone();
    unsafe_component[76] = b'/';
    assert!(decode_embedded_bookmark_path(&unsafe_component, &limits).is_err());
    let mut external = bytes.clone();
    external[132] = b'X';
    let evidence = decode_embedded_bookmark_path(&external, &limits).unwrap();
    assert!(evidence.embedded_path.is_none());
    assert!(
        evidence
            .issues
            .contains(&"unqualified_bookmark_volume".to_owned())
    );
}
#[test]
fn bad_beat_uids_cycles_and_classes_withhold_grid_without_losing_cues() {
    for variant in 0..3 {
        let root = tempfile::tempdir().unwrap();
        let db = database(root.path());
        let bytes = beats(|p| {
            let objects = p
                .as_dictionary_mut()
                .unwrap()
                .get_mut("$objects")
                .unwrap()
                .as_array_mut()
                .unwrap();
            if variant == 0 {
                objects[1].as_dictionary_mut().unwrap().insert(
                    "MIKBeats_beatTimes".into(),
                    plist::Value::Uid(plist::Uid::new(9999)),
                );
            } else if variant == 1 {
                objects[3] = plist::Value::Uid(plist::Uid::new(3));
            } else {
                objects[7].as_dictionary_mut().unwrap().insert(
                    "$classname".into(),
                    plist::Value::String("UnqualifiedBeats".into()),
                );
            }
        });
        update_blob(&db, "ZBEATS", &bytes);
        drop(db);
        let (_, rows) = read(SourceProvider::MixedInKey11, &capture(root.path()));
        let track = rows.tracks()[0];
        assert!(track.grid.is_none());
        assert_eq!(track.cues.len(), 2);
        assert!(
            track
                .issues
                .iter()
                .any(|i| i.code == "unsupported_mik_beats_archive")
        );
        assert!(rows.blobs.values().any(|raw| raw == &bytes));
    }
}
#[test]
fn direct_numeric_beat_objects_keep_archive_tempo_separate_from_song_tempo() {
    let root = tempfile::tempdir().unwrap();
    let db = database(root.path());
    let bytes = beats(|p| {
        let objects = p
            .as_dictionary_mut()
            .unwrap()
            .get_mut("$objects")
            .unwrap()
            .as_array_mut()
            .unwrap();
        objects[2].as_dictionary_mut().unwrap().insert(
            "NS.objects".into(),
            plist::Value::Array(vec![
                plist::Value::Real(-0.1),
                plist::Value::Real(0.6),
                plist::Value::Real(1.2),
            ]),
        );
    });
    update_blob(&db, "ZBEATS", &bytes);
    db.execute_batch("UPDATE ZSONG SET ZTEMPO=130,ZKEY='08A'")
        .unwrap();
    drop(db);
    let (_, rows) = read(SourceProvider::MixedInKey11, &capture(root.path()));
    let track = rows.tracks()[0];
    let grid = track.grid.as_ref().unwrap();
    assert_eq!(
        grid.points
            .iter()
            .map(|p| p.native_seconds)
            .collect::<Vec<_>>(),
        [-0.1, 0.6, 1.2]
    );
    assert_eq!(grid.bpm, Some(120.));
    assert_eq!(track.bpm.as_ref().unwrap().value, Some(130.));
    assert_eq!(track.key.as_ref().unwrap().camelot.as_deref(), Some("8A"));
}
#[test]
fn duplicate_marker_times_and_orphan_children_keep_native_identities() {
    let root = tempfile::tempdir().unwrap();
    let db = database(root.path());
    db.execute_batch("INSERT INTO ZCUEPOINT(Z_PK,Z_ENT,ZSONG,ZTIME,ZNAME) VALUES(203,2,101,32.125,'second'),(204,2,999,-0.1,'orphan'),(205,2,NULL,1,'unlinked');").unwrap();
    drop(db);
    let (header, rows) = read(SourceProvider::MixedInKey11, &capture(root.path()));
    assert_eq!(header.completeness, Completeness::PartialWithErrors);
    assert_eq!(header.accounting.unrepresented_records, 1);
    let tracks = rows.tracks();
    let track = tracks.iter().find(|t| t.external_id == "song:101").unwrap();
    assert_eq!(track.cues.len(), 3);
    assert_ne!(track.cues[1].external_id, track.cues[2].external_id);
    assert!(
        track
            .issues
            .iter()
            .any(|i| i.code == "mik_duplicate_cue_time")
    );
    let orphan = tracks.iter().find(|t| t.external_id == "song:999").unwrap();
    assert_eq!(orphan.cues.len(), 1);
    assert!(orphan.issues.iter().any(|i| i.code == "missing_mik_song"));
}
#[test]
fn cyclic_and_missing_collection_parents_preserve_all_rows_and_memberships() {
    let root = tempfile::tempdir().unwrap();
    let db = database(root.path());
    db.execute_batch("UPDATE ZCOLLECTION SET ZPARENTCOLLECTION=2 WHERE Z_PK=1;INSERT INTO ZCOLLECTION(Z_PK,Z_ENT,ZNAME,ZPARENTCOLLECTION,ZINDEX) VALUES(3,1,'orphan',999,-5),(4,1,'library',NULL,1);UPDATE ZCOLLECTION SET ZISLIBRARY=1 WHERE Z_PK=4;").unwrap();
    drop(db);
    let (_, rows) = read(SourceProvider::MixedInKey11, &capture(root.path()));
    assert_eq!(rows.collections().len(), 5);
    assert_eq!(rows.members().len(), 1);
    assert!(
        rows.rows
            .iter()
            .any(|r| matches!(r,ObservationRecord::Issue(i) if i.code=="mik_collection_cycle"))
    );
    let collections = rows.collections();
    let library = collections
        .iter()
        .find(|c| c.external_id == "collection:4")
        .unwrap();
    assert_eq!(library.kind, CollectionKind::Folder);
    assert_eq!(library.extensions["source_kind"], "library");
    assert_eq!(
        collections
            .iter()
            .find(|c| c.external_id == "collection:3")
            .unwrap()
            .parent_id
            .as_deref(),
        Some("collection:999")
    );
}
#[test]
fn cancelled_mik_read_emits_nothing() {
    let root = tempfile::tempdir().unwrap();
    drop(database(root.path()));
    let snapshot = capture(root.path());
    let cancel = CancellationToken::default();
    cancel.cancel();
    let mut rows = Records::default();
    assert!(
        adapter(SourceProvider::MixedInKey11)
            .read(&snapshot, &ReadOptions::default(), &mut rows, &cancel)
            .is_err()
    );
    assert!(rows.rows.is_empty());
}

#[test]
fn cached_comment_archives_preserve_order_flags_and_tag_versions() {
    use plist::{Uid, Value as P};
    let root = tempfile::tempdir().unwrap();
    let db = database(root.path());
    let entry = |text: &str, primary: bool, version: i64| {
        dictionary(vec![
            ("$class", P::Uid(Uid::new(5))),
            (
                "NS.keys",
                P::Array(vec![
                    P::String("comment".into()),
                    P::String("isPrimary".into()),
                    P::String("tagVersion".into()),
                ]),
            ),
            (
                "NS.objects",
                P::Array(vec![
                    P::String(text.into()),
                    P::Boolean(primary),
                    P::Integer(version.into()),
                ]),
            ),
        ])
    };
    let archive = dictionary(vec![
        ("$archiver", P::String("NSKeyedArchiver".into())),
        ("$version", P::Integer(100000.into())),
        ("$top", dictionary(vec![("root", P::Uid(Uid::new(1)))])),
        (
            "$objects",
            P::Array(vec![
                P::String("$null".into()),
                dictionary(vec![
                    ("$class", P::Uid(Uid::new(4))),
                    (
                        "NS.objects",
                        P::Array(vec![P::Uid(Uid::new(2)), P::Uid(Uid::new(3))]),
                    ),
                ]),
                entry("first", true, 2),
                entry("second", false, 1),
                dictionary(vec![("$classname", P::String("NSArray".into()))]),
                dictionary(vec![("$classname", P::String("NSDictionary".into()))]),
            ]),
        ),
    ]);
    let mut bytes = Vec::new();
    archive.to_writer_binary(&mut bytes).unwrap();
    update_blob(&db, "ZTAGCOMMENTS", &bytes);
    db.execute_batch("UPDATE ZSONG SET ZCOMMENT='separate source comment'")
        .unwrap();
    drop(db);
    let (_, rows) = read(SourceProvider::MixedInKey11, &capture(root.path()));
    let track = rows.tracks()[0];
    assert_eq!(
        track.cached_tags["comments"],
        Presence::Value(
            serde_json::json!([{"comment":"first","isPrimary":true,"tagVersion":2},{"comment":"second","isPrimary":false,"tagVersion":1}])
        )
    );
    assert_eq!(
        track.metadata["comment"],
        Presence::Value(serde_json::json!("separate source comment"))
    );
    assert!(rows.blobs.values().any(|raw| raw == &bytes));
}
