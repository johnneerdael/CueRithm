#[path = "import_fixture_support/mod.rs"]
mod support;
use cuerithm_dj_sync::import::*;
use cuerithm_media::CancellationToken;
use rusqlite::{Connection, params};
use support::*;
#[derive(Clone)]
enum V<'a> {
    Text(&'a str),
    Double(f64),
    Small(u8),
    Date(f64),
    Array(Vec<V<'a>>),
    Object(&'a str, Vec<(&'a str, V<'a>)>),
    Binary(&'a [u8]),
    Uri(&'a str),
    Null,
}
fn text(bytes: &mut Vec<u8>, s: &str) {
    bytes.push(8);
    bytes.extend(s.as_bytes());
    bytes.push(0);
}
fn aligned(bytes: &mut Vec<u8>, tag: u8, align: usize, data: &[u8]) {
    bytes.push(tag);
    while !bytes.len().is_multiple_of(align) {
        bytes.push(0)
    }
    bytes.extend(data);
}
fn value(bytes: &mut Vec<u8>, v: &V<'_>) {
    match v {
        V::Text(s) => text(bytes, s),
        V::Double(v) => aligned(bytes, 20, 8, &v.to_le_bytes()),
        V::Date(v) => aligned(bytes, 48, 8, &v.to_le_bytes()),
        V::Small(0) => bytes.push(46),
        V::Small(1) => bytes.push(45),
        V::Small(n) => bytes.extend([15, *n]),
        V::Null => bytes.push(0),
        V::Uri(s) => {
            bytes.push(33);
            text(bytes, s);
            bytes.push(0);
        }
        V::Binary(data) => {
            aligned(bytes, 21, 4, &(data.len() as u32).to_le_bytes());
            bytes.extend(*data);
            while !bytes.len().is_multiple_of(4) {
                bytes.push(0)
            }
        }
        V::Array(items) => {
            aligned(bytes, 10, 4, &(items.len() as u32).to_le_bytes());
            for item in items {
                value(bytes, item)
            }
        }
        V::Object(class, fields) => {
            bytes.push(43);
            text(bytes, class);
            for (k, v) in fields {
                value(bytes, v);
                text(bytes, k)
            }
            bytes.push(0);
        }
    }
}
fn record(class: &str, fields: Vec<(&str, V<'_>)>) -> Vec<u8> {
    let mut bytes = b"TSAF\x03\0\x03\0".to_vec();
    bytes.extend([0; 12]);
    value(&mut bytes, &V::Object(class, fields));
    bytes
}
fn database(root: &std::path::Path) -> Connection {
    let db = Connection::open(root.join("MediaLibrary.db")).unwrap();
    db.execute_batch("CREATE TABLE database2(rowid INTEGER PRIMARY KEY,collection TEXT NOT NULL,key TEXT NOT NULL,data BLOB,metadata BLOB,UNIQUE(collection,key));").unwrap();
    db
}
fn insert(db: &Connection, collection: &str, key: &str, bytes: Vec<u8>) {
    db.execute(
        "INSERT INTO database2(collection,key,data) VALUES(?1,?2,?3)",
        params![collection, key, bytes],
    )
    .unwrap();
}
fn capture(root: &std::path::Path) -> ImportSnapshot {
    let request = SourceCaptureRequest::from_selected_root(
        SourceProvider::DjayPro,
        root,
        vec![ArtifactRequest::sqlite("MediaLibrary.db")],
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
fn djay_typed_stream_alignment_and_pointers_are_bounded() {
    let root = tempfile::tempdir().unwrap();
    let db = database(root.path());
    // Root class ends at byte35; Double payload starts at absolute byte40, not36.
    let mut bytes = b"TSAF\x03\0\x03\0".to_vec();
    bytes.extend([0; 12]);
    bytes.extend([43, 8]);
    bytes.extend(b"ADCMediaItem\0");
    assert_eq!(bytes.len(), 35);
    bytes.extend([20, 0, 0, 0, 0]);
    bytes.extend(1.25f64.to_le_bytes());
    text(&mut bytes, "duration");
    value(&mut bytes, &V::Text("Native title"));
    text(&mut bytes, "title");
    value(&mut bytes, &V::Date(0.));
    text(&mut bytes, "addedDate");
    value(&mut bytes, &V::Binary(&[1, 2, 3]));
    text(&mut bytes, "opaquePayload");
    value(
        &mut bytes,
        &V::Array(vec![
            V::Null,
            V::Small(1),
            V::Uri("file:///DemoMusic/one.flac"),
        ]),
    );
    text(&mut bytes, "unknownArray");
    bytes.push(0);
    insert(&db, "mediaItems", "one", bytes);
    drop(db);
    let snapshot = capture(root.path());
    let (_, rows) = read(SourceProvider::DjayPro, &snapshot);
    let tracks = rows.tracks();
    let one = tracks[0];
    assert_eq!(
        one.metadata["title"],
        Presence::Value(serde_json::json!("Native title"))
    );
    assert_eq!(one.extensions["duration_seconds"], 1.25);
    assert_eq!(
        one.metadata["date_added"],
        Presence::Value(serde_json::json!("2001-01-01T00:00:00Z"))
    );
    assert_eq!(
        one.extensions["native_records"]["mediaItems"]["opaquePayload"]["$binary"]["bytes"],
        3
    );
    assert_eq!(
        one.extensions["native_records"]["mediaItems"]["unknownArray"][0],
        serde_json::Value::Null
    );
}
#[test]
fn djay_joins_collections_without_dropping_missing_locations() {
    let root = tempfile::tempdir().unwrap();
    let db = database(root.path());
    for (id, title) in [("one", "First"), ("two", "Second")] {
        insert(
            &db,
            "mediaItems",
            id,
            record(
                "ADCMediaItem",
                vec![
                    ("title", V::Text(title)),
                    ("artistUUIDs", V::Array(vec![V::Text("missing-artist")])),
                ],
            ),
        );
    }
    insert(
        &db,
        "localMediaItemLocations",
        "one",
        record(
            "ADCMediaItemLocation",
            vec![(
                "sourceURIs",
                V::Array(vec![
                    V::Uri("file:///Unavailable/one.flac"),
                    V::Uri("file:///Other/one.flac"),
                ]),
            )],
        ),
    );
    for (key, kind, name, parent, children, items) in [
        (
            "mediaItemPlaylist-root",
            1,
            "Root",
            "",
            vec!["folder"],
            vec![],
        ),
        (
            "folder",
            1,
            "Sets",
            "mediaItemPlaylist-root",
            vec!["set"],
            vec![],
        ),
        ("set", 0, "Order", "folder", vec![], vec!["a", "b", "again"]),
    ] {
        insert(
            &db,
            "mediaItemPlaylists",
            key,
            record(
                "ADCMediaItemPlaylist",
                vec![
                    ("type", V::Small(kind)),
                    ("name", V::Text(name)),
                    ("parentUUID", V::Text(parent)),
                    (
                        "childUUIDs",
                        V::Array(children.into_iter().map(V::Text).collect()),
                    ),
                    (
                        "itemUUIDs",
                        V::Array(items.into_iter().map(V::Text).collect()),
                    ),
                ],
            ),
        );
    }
    for (id, track) in [("a", "one"), ("b", "two"), ("again", "one")] {
        insert(
            &db,
            "mediaItemPlaylistItems",
            id,
            record(
                "ADCMediaItemPlaylistItem",
                vec![("mediaItemUUID", V::Text(track))],
            ),
        );
    }
    drop(db);
    let snapshot = capture(root.path());
    let (_, rows) = read(SourceProvider::DjayPro, &snapshot);
    assert_eq!(rows.tracks().len(), 2);
    let tracks = rows.tracks();
    assert!(tracks.iter().all(|t| !t.metadata.contains_key("artist")));
    assert!(
        tracks
            .iter()
            .all(|t| t.issues.iter().any(|i| i.code == "unresolved_djay_artist"))
    );
    let one = tracks.iter().find(|t| t.external_id == "one").unwrap();
    assert_eq!(
        one.locations.extensions["alternate_candidates"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
    let two = tracks.iter().find(|t| t.external_id == "two").unwrap();
    assert!(two.locations.native_path.is_none());
    let collections = rows.collections();
    assert_eq!(collections.len(), 2);
    let members = rows.members();
    assert_eq!(
        members
            .iter()
            .map(|(_, m)| m.track_external_id.as_str())
            .collect::<Vec<_>>(),
        ["one", "two", "one"]
    );
    assert_ne!(
        members[0].1.external_occurrence_id,
        members[2].1.external_occurrence_id
    );
}
#[test]
fn djay_manual_tempo_and_key_remain_distinct() {
    let root = tempfile::tempdir().unwrap();
    let db = database(root.path());
    insert(
        &db,
        "mediaItems",
        "one",
        record(
            "ADCMediaItem",
            vec![("title", V::Text("Manual")), ("bpm", V::Double(98.))],
        ),
    );
    insert(
        &db,
        "mediaItemAnalyzedData",
        "one",
        record(
            "ADCMediaItemAnalyzedData",
            vec![
                ("bpm", V::Double(120.)),
                ("keySignatureIndex", V::Small(0)),
                ("firstDownbeatPosition", V::Double(0.25)),
            ],
        ),
    );
    insert(
        &db,
        "mediaItemUserData",
        "one",
        record(
            "ADCMediaItemUserData",
            vec![
                ("manualBPM", V::Double(100.)),
                ("manualKeySignatureIndex2", V::Small(15)),
                (
                    "beatGridEdits",
                    V::Object(
                        "ADCBeatGridEdits",
                        vec![
                            ("firstDownbeatPosition", V::Double(1.25)),
                            ("firstGridSegmentTempoExponent", V::Small(255)),
                            ("lastGridSegmentTempoExponent", V::Small(1)),
                            ("useStraightBeats", V::Double(1.)),
                            ("nrOfBeatShift", V::Double(0.)),
                        ],
                    ),
                ),
                (
                    "cuePoints",
                    V::Array(vec![
                        V::Object(
                            "ADCCuePoint",
                            vec![
                                ("time", V::Double(1.25)),
                                ("endTime", V::Double(-1.)),
                                ("number", V::Double(2.)),
                                ("comment", V::Text("Hot")),
                            ],
                        ),
                        V::Object(
                            "ADCCuePoint",
                            vec![
                                ("time", V::Double(4.)),
                                ("endTime", V::Double(8.)),
                                ("number", V::Double(5.)),
                            ],
                        ),
                    ]),
                ),
            ],
        ),
    );
    drop(db);
    let snapshot = capture(root.path());
    let (_, rows) = read(SourceProvider::DjayPro, &snapshot);
    let tracks = rows.tracks();
    let one = tracks[0];
    assert_eq!(one.key.as_ref().unwrap().camelot.as_deref(), Some("9A"));
    assert_eq!(
        one.cached_tags["djay_analyzed_key"],
        Presence::Value(serde_json::json!({"raw":0,"camelot":"8B"}))
    );
    assert_eq!(one.bpm.as_ref().unwrap().value, Some(50.));
    assert_eq!(
        one.cached_tags["djay_analyzed_bpm"],
        Presence::Value(serde_json::json!(120.))
    );
    let grid = one.grid.as_ref().unwrap();
    assert_eq!(grid.native_first_beat_seconds, Some(1.25));
    assert_eq!(grid.bpm, Some(50.));
    assert_eq!(grid.extensions["last_exponent"], 1);
    assert!(grid.canonical_first_beat_seconds.is_none());
    assert_eq!(one.cues.len(), 2);
    assert_eq!(one.cues[0].source_slot, Some(1));
    assert_eq!(one.cues[1].source_slot, Some(4));
    assert_eq!(one.cues[1].native_end_seconds, Some(8.));
    assert!(one.cues.iter().all(|c| c.canonical_start_seconds.is_none()));
}

fn plist_dictionary(fields: Vec<(&str, plist::Value)>) -> plist::Value {
    plist::Value::Dictionary(fields.into_iter().map(|(k, v)| (k.to_owned(), v)).collect())
}
fn uid(index: u64) -> plist::Value {
    plist::Value::Uid(plist::Uid::new(index))
}
#[test]
fn djay_keyed_archive_keeps_uid_objects_dates_and_unknown_classes() {
    let objects = vec![
        plist::Value::String("$null".into()),
        plist_dictionary(vec![
            ("$class", uid(2)),
            ("title", uid(3)),
            ("addedDate", uid(4)),
            ("mystery", uid(6)),
        ]),
        plist_dictionary(vec![
            ("$classname", plist::Value::String("ADCMediaItem".into())),
            (
                "$classes",
                plist::Value::Array(vec![
                    plist::Value::String("ADCMediaItem".into()),
                    plist::Value::String("NSObject".into()),
                ]),
            ),
        ]),
        plist::Value::String("Archived title".into()),
        plist_dictionary(vec![
            ("$class", uid(5)),
            ("NS.time", plist::Value::Real(0.)),
        ]),
        plist_dictionary(vec![("$classname", plist::Value::String("NSDate".into()))]),
        plist_dictionary(vec![
            ("$class", uid(7)),
            ("payload", plist::Value::Data(vec![1, 2, 3])),
        ]),
        plist_dictionary(vec![(
            "$classname",
            plist::Value::String("MysteryPayload".into()),
        )]),
    ];
    let archive = plist_dictionary(vec![
        ("$archiver", plist::Value::String("NSKeyedArchiver".into())),
        ("$version", plist::Value::Integer(100000.into())),
        ("$objects", plist::Value::Array(objects)),
        ("$top", plist_dictionary(vec![("root", uid(1))])),
    ]);
    let mut bytes = Vec::new();
    archive.to_writer_binary(&mut bytes).unwrap();
    let root = tempfile::tempdir().unwrap();
    let db = database(root.path());
    insert(&db, "mediaItems", "one", bytes);
    drop(db);
    let snapshot = capture(root.path());
    let (_, rows) = read(SourceProvider::DjayPro, &snapshot);
    let tracks = rows.tracks();
    let one = tracks[0];
    assert_eq!(
        one.metadata["title"],
        Presence::Value(serde_json::json!("Archived title"))
    );
    assert_eq!(
        one.metadata["date_added"],
        Presence::Value(serde_json::json!("2001-01-01T00:00:00Z"))
    );
    assert_eq!(
        one.extensions["native_records"]["mediaItems"]["mystery"]["$class"],
        "MysteryPayload"
    );
    assert_eq!(
        one.extensions["native_records"]["mediaItems"]["mystery"]["payload"]["$binary"]["bytes"],
        3
    );
}

#[test]
fn djay_prior_pointers_and_counted_class_arrays_preserve_values() {
    use cuerithm_dj_sync::import::readers::djay_codec::decode_djay_stream;
    let mut bytes = b"TSAF\x03\0\x03\0".to_vec();
    bytes.extend([0; 12]);
    bytes.extend([43, 8]);
    bytes.extend(b"ADCMediaItem\0");
    text(&mut bytes, "Pointer title");
    text(&mut bytes, "title");
    bytes.extend([5, 1]);
    text(&mut bytes, "titleAlias");
    aligned(&mut bytes, 19, 4, &1.5f32.to_le_bytes());
    text(&mut bytes, "floatValue");
    bytes.extend([2, 0]);
    text(&mut bytes, "floatAlias");
    aligned(&mut bytes, 27, 4, &1u32.to_le_bytes());
    aligned(&mut bytes, 24, 4, &2u32.to_le_bytes());
    value(&mut bytes, &V::Double(1.25));
    text(&mut bytes, "position");
    value(&mut bytes, &V::Small(255));
    text(&mut bytes, "numberOfBars");
    text(&mut bytes, "countedMarkers");
    bytes.push(0);
    let graph = decode_djay_stream(&bytes, &ImportLimits::default()).unwrap();
    let json = graph.to_json().unwrap();
    assert_eq!(json["titleAlias"], "Pointer title");
    assert_eq!(json["floatAlias"], 1.5);
    assert_eq!(json["countedMarkers"][0]["position"], 1.25);
    assert_eq!(json["countedMarkers"][0]["numberOfBars"], 255);
    assert!(!graph.strings.is_empty());
}
#[test]
fn djay_malformed_pointers_alignment_counts_and_headers_fail_without_placeholders() {
    use cuerithm_dj_sync::import::readers::djay_codec::decode_djay_stream;
    let good = record("ADCMediaItem", vec![("title", V::Text("Native"))]);
    let mut cases = Vec::new();
    let mut pointer = good[..35].to_vec();
    pointer.extend([5, 255]);
    text(&mut pointer, "title");
    pointer.push(0);
    cases.push(pointer);
    let mut count = good[..35].to_vec();
    aligned(&mut count, 10, 4, &u32::MAX.to_le_bytes());
    cases.push(count);
    let mut truncated = good[..35].to_vec();
    truncated.push(20);
    cases.push(truncated);
    let mut wrong = good.clone();
    wrong[4] = 2;
    cases.push(wrong);
    let mut marker = good.clone();
    marker[20] = 0;
    cases.push(marker);
    for bytes in cases {
        assert!(decode_djay_stream(&bytes, &ImportLimits::default()).is_err());
    }
    let root = tempfile::tempdir().unwrap();
    let db = database(root.path());
    let mut broken = good[..35].to_vec();
    broken.extend([5, 255]);
    text(&mut broken, "title");
    broken.push(0);
    insert(&db, "mediaItems", "broken", broken);
    drop(db);
    let snapshot = capture(root.path());
    let (_, rows) = read(SourceProvider::DjayPro, &snapshot);
    let track = rows.tracks()[0];
    assert!(!track.metadata.contains_key("title"));
    assert!(
        track
            .issues
            .iter()
            .any(|i| i.code == "djay_record_decode_failed")
    );
    assert!(
        track.extensions["raw_records"]["mediaItems"]
            .get("data")
            .is_some()
    );
}
#[test]
fn djay_empty_root_recovers_parents_and_missing_children_stay_visible() {
    let root = tempfile::tempdir().unwrap();
    let db = database(root.path());
    insert(
        &db,
        "mediaItemPlaylists",
        "mediaItemPlaylist-root",
        record(
            "ADCMediaItemPlaylist",
            vec![
                ("type", V::Small(1)),
                ("name", V::Text("Root")),
                ("childUUIDs", V::Array(vec![])),
            ],
        ),
    );
    insert(
        &db,
        "mediaItemPlaylists",
        "folder",
        record(
            "ADCMediaItemPlaylist",
            vec![
                ("type", V::Small(1)),
                ("name", V::Text("Parent")),
                ("parentUUID", V::Text("mediaItemPlaylist-root")),
                ("childUUIDs", V::Array(vec![V::Text("missing-child")])),
            ],
        ),
    );
    insert(
        &db,
        "mediaItemPlaylists",
        "set",
        record(
            "ADCMediaItemPlaylist",
            vec![
                ("type", V::Small(0)),
                ("name", V::Text("Recovered")),
                ("parentUUID", V::Text("folder")),
            ],
        ),
    );
    drop(db);
    let snapshot = capture(root.path());
    let (header, rows) = read(SourceProvider::DjayPro, &snapshot);
    let cs = rows.collections();
    assert_eq!(cs.len(), 3);
    assert_eq!(
        cs.iter()
            .find(|c| c.name == "Recovered")
            .unwrap()
            .parent_id
            .as_deref(),
        Some("folder")
    );
    let missing = cs
        .iter()
        .find(|c| c.external_id == "missing-child")
        .unwrap();
    assert!(missing.name.is_empty());
    assert_eq!(missing.parent_id.as_deref(), Some("folder"));
    assert!(header.extensions.contains_key("playlist_root_record"));
}
#[test]
fn djay_alternate_grid_is_preserved_without_fabricating_straight_beats() {
    let root = tempfile::tempdir().unwrap();
    let db = database(root.path());
    insert(
        &db,
        "mediaItems",
        "one",
        record("ADCMediaItem", vec![("title", V::Text("Alternate"))]),
    );
    insert(
        &db,
        "mediaItemAnalyzedData",
        "one",
        record(
            "ADCMediaItemAnalyzedData",
            vec![
                ("bpm", V::Double(120.)),
                ("firstDownbeatPosition", V::Double(0.25)),
            ],
        ),
    );
    insert(
        &db,
        "mediaItemUserData",
        "one",
        record(
            "ADCMediaItemUserData",
            vec![(
                "beatGridEdits",
                V::Object(
                    "ADCBeatGridEdits",
                    vec![
                        ("useStraightBeats", V::Small(0)),
                        ("alternateGrid", V::Binary(&[9, 8, 7])),
                        ("firstDownbeatPosition", V::Double(1.25)),
                    ],
                ),
            )],
        ),
    );
    drop(db);
    let snapshot = capture(root.path());
    let (_, rows) = read(SourceProvider::DjayPro, &snapshot);
    let one = rows.tracks()[0];
    assert!(one.grid.is_none());
    assert_eq!(one.bpm.as_ref().unwrap().value, Some(120.));
    assert!(
        one.issues
            .iter()
            .any(|i| i.code == "unsupported_djay_alternate_grid")
    );
    assert_eq!(
        one.extensions["native_records"]["mediaItemUserData"]["beatGridEdits"]["alternateGrid"]["$binary"]
            ["bytes"],
        3
    );
}

#[test]
fn keyed_archive_preserves_uid_graph_and_bounds_cycles_and_declared_tables() {
    use cuerithm_dj_sync::import::readers::keyed_archive::{ArchiveNode, decode_keyed_archive};
    let archive = plist_dictionary(vec![
        ("$archiver", plist::Value::String("NSKeyedArchiver".into())),
        ("$version", plist::Value::Integer(100000.into())),
        (
            "$objects",
            plist::Value::Array(vec![
                plist::Value::String("$null".into()),
                plist_dictionary(vec![
                    ("title", uid(2)),
                    ("owner", uid(1)),
                    ("opaque", plist::Value::Data(vec![1, 2, 3])),
                ]),
                plist::Value::String("Kept title".into()),
            ]),
        ),
        ("$top", plist_dictionary(vec![("root", uid(1))])),
    ]);
    let mut bytes = Vec::new();
    archive.to_writer_binary(&mut bytes).unwrap();
    let graph = decode_keyed_archive(&bytes, &ImportLimits::default()).unwrap();
    assert!(matches!(graph.root, ArchiveNode::Uid(1)));
    assert_eq!(graph.objects.len(), 3);
    let json = graph.to_json().unwrap();
    assert_eq!(json["title"], "Kept title");
    assert_eq!(json["owner"]["$cycle"], true);
    let opaque = graph.field(&graph.root, "opaque").unwrap().unwrap();
    assert_eq!(graph.data(opaque).unwrap(), Some([1, 2, 3].as_slice()));
    let mut bad = bytes.clone();
    let at = bad.len() - 24;
    bad[at..at + 8].copy_from_slice(&u64::MAX.to_be_bytes());
    assert!(decode_keyed_archive(&bad, &ImportLimits::default()).is_err());
    let mut missing = archive;
    missing
        .as_dictionary_mut()
        .unwrap()
        .get_mut("$top")
        .unwrap()
        .as_dictionary_mut()
        .unwrap()
        .insert("root".into(), uid(999));
    let mut bad = Vec::new();
    missing.to_writer_binary(&mut bad).unwrap();
    assert!(decode_keyed_archive(&bad, &ImportLimits::default()).is_err());
}
#[test]
fn djay_orphan_items_smart_filters_and_cycles_have_explicit_outcomes() {
    let root = tempfile::tempdir().unwrap();
    let db = database(root.path());
    insert(
        &db,
        "mediaItemAnalyzedData",
        "orphan",
        record("ADCMediaItemAnalyzedData", vec![("bpm", V::Double(120.))]),
    );
    insert(
        &db,
        "mediaArtists",
        "unused",
        record("ADCMediaArtist", vec![("name", V::Text("Unused artist"))]),
    );
    insert(
        &db,
        "mediaItemPlaylists",
        "smart",
        record(
            "ADCMediaItemPlaylist",
            vec![
                ("type", V::Small(2)),
                ("name", V::Text("Smart")),
                (
                    "filterSet",
                    V::Object(
                        "UnknownNativeFilter",
                        vec![("operand", V::Text("safe opaque"))],
                    ),
                ),
            ],
        ),
    );
    drop(db);
    let snapshot = capture(root.path());
    let (header, rows) = read(SourceProvider::DjayPro, &snapshot);
    assert_eq!(rows.tracks().len(), 1);
    assert!(
        rows.tracks()[0]
            .issues
            .iter()
            .any(|i| i.code == "missing_djay_media_item")
    );
    assert!(
        !header.extensions["auxiliary_record_pages"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    let cs = rows.collections();
    assert_eq!(cs[0].kind, CollectionKind::Smart);
    assert!(
        cs[0].extensions["issues"]
            .as_array()
            .unwrap()
            .iter()
            .any(|i| i["code"] == "unsupported_djay_smart_filter")
    );
    let root = tempfile::tempdir().unwrap();
    let db = database(root.path());
    for (id, parent) in [("a", "b"), ("b", "a")] {
        insert(
            &db,
            "mediaItemPlaylists",
            id,
            record(
                "ADCMediaItemPlaylist",
                vec![
                    ("type", V::Small(1)),
                    ("name", V::Text(id)),
                    ("parentUUID", V::Text(parent)),
                ],
            ),
        );
    }
    drop(db);
    let snapshot = capture(root.path());
    assert!(
        adapter(SourceProvider::DjayPro)
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

#[test]
fn djay_segment_bars_and_terminal_exponent_preserve_variable_tempo() {
    let root = tempfile::tempdir().unwrap();
    let db = database(root.path());
    insert(
        &db,
        "mediaItems",
        "one",
        record("ADCMediaItem", vec![("title", V::Text("Segments"))]),
    );
    insert(
        &db,
        "mediaItemAnalyzedData",
        "one",
        record("ADCMediaItemAnalyzedData", vec![("bpm", V::Double(100.))]),
    );
    insert(
        &db,
        "mediaItemUserData",
        "one",
        record(
            "ADCMediaItemUserData",
            vec![(
                "beatGridEdits",
                V::Object(
                    "ADCBeatGridEdits",
                    vec![
                        ("firstGridSegmentTempoExponent", V::Small(0)),
                        ("lastGridSegmentTempoExponent", V::Small(1)),
                        ("nrOfBeatShift", V::Double(2.)),
                        ("fractionalBeatShift", V::Double(0.5)),
                        ("useStraightBeats", V::Small(1)),
                        (
                            "downbeatMarkers",
                            V::Array(vec![
                                V::Object(
                                    "Marker",
                                    vec![
                                        ("position", V::Double(1.25)),
                                        ("numberOfBars", V::Small(2)),
                                    ],
                                ),
                                V::Object(
                                    "Marker",
                                    vec![
                                        ("position", V::Double(5.25)),
                                        ("numberOfBars", V::Small(255)),
                                    ],
                                ),
                            ]),
                        ),
                    ],
                ),
            )],
        ),
    );
    drop(db);
    let snapshot = capture(root.path());
    let (_, rows) = read(SourceProvider::DjayPro, &snapshot);
    let one = rows.tracks()[0];
    let grid = one.grid.as_ref().unwrap();
    assert_eq!(grid.points.len(), 2);
    assert_eq!(grid.points[0].bpm, Some(120.));
    assert_eq!(grid.points[1].bpm, Some(200.));
    assert_eq!(grid.points[0].native_seconds, 1.25);
    assert_eq!(grid.extensions["native_edits"]["fractionalBeatShift"], 0.5);
    assert_eq!(one.bpm.as_ref().unwrap().value, Some(100.));
}
#[test]
fn djay_cancelled_stream_does_not_claim_completeness() {
    struct CancelSink {
        rows: Records,
        token: CancellationToken,
    }
    impl ObservationSink for CancelSink {
        fn put_blob(&mut self, bytes: &[u8], kind: Option<&str>) -> anyhow::Result<BlobRef> {
            self.rows.put_blob(bytes, kind)
        }
        fn push(&mut self, row: ObservationRecord) -> anyhow::Result<()> {
            self.rows.push(row)?;
            self.token.cancel();
            Ok(())
        }
    }
    let root = tempfile::tempdir().unwrap();
    let db = database(root.path());
    insert(
        &db,
        "mediaItems",
        "one",
        record("ADCMediaItem", vec![("title", V::Text("One"))]),
    );
    drop(db);
    let snapshot = capture(root.path());
    let token = CancellationToken::default();
    let mut sink = CancelSink {
        rows: Records::default(),
        token: token.clone(),
    };
    assert!(
        adapter(SourceProvider::DjayPro)
            .read(&snapshot, &ReadOptions::default(), &mut sink, &token)
            .is_err()
    );
    assert!(!sink.rows.rows.is_empty());
    snapshot
        .validate_source(&CancellationToken::default())
        .unwrap();
}

#[test]
fn djay_unresolved_archive_values_never_become_title_tags() {
    let archive = plist_dictionary(vec![
        ("$archiver", plist::Value::String("NSKeyedArchiver".into())),
        ("$version", plist::Value::Integer(100000.into())),
        (
            "$objects",
            plist::Value::Array(vec![
                plist::Value::String("$null".into()),
                plist_dictionary(vec![("title", uid(1))]),
            ]),
        ),
        ("$top", plist_dictionary(vec![("root", uid(1))])),
    ]);
    let mut bytes = Vec::new();
    archive.to_writer_binary(&mut bytes).unwrap();
    let root = tempfile::tempdir().unwrap();
    let db = database(root.path());
    insert(&db, "mediaItems", "one", bytes);
    drop(db);
    let snapshot = capture(root.path());
    let (_, rows) = read(SourceProvider::DjayPro, &snapshot);
    let one = rows.tracks()[0];
    assert!(!one.metadata.contains_key("title"));
    assert!(
        one.issues
            .iter()
            .any(|i| i.code == "unresolved_djay_metadata_field")
    );
    assert_eq!(
        one.extensions["native_records"]["mediaItems"]["title"]["$cycle"],
        true
    );
}
