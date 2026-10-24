#[path = "import_fixture_support/mod.rs"]
mod support;
use base64::{Engine, engine::general_purpose::STANDARD};
use cuerithm_dj_sync::import::*;
use cuerithm_media::CancellationToken;
use rusqlite::{Connection, params};
use serde_json::Value;
use std::fs;
use support::*;
const UUID: &str = "00000000-1111-2222-3333-444444444444";
fn vectors() -> (Vec<u8>, Vec<u8>, Vec<u8>) {
    let value:Value=serde_json::from_str(include_str!("../../../docs/research/dj-library-import-handover/fixtures/lexicon-engine-codec-vectors.json")).unwrap();
    let hex = value["loops_hex"].as_str().unwrap().as_bytes();
    (
        STANDARD
            .decode(value["beatData_base64"].as_str().unwrap())
            .unwrap(),
        STANDARD
            .decode(value["quickCues_base64"].as_str().unwrap())
            .unwrap(),
        hex.as_chunks::<2>()
            .0
            .iter()
            .map(|p| u8::from_str_radix(std::str::from_utf8(p).unwrap(), 16).unwrap())
            .collect(),
    )
}
fn database(root: &std::path::Path, variant: &str) -> Connection {
    let directory = root.join("Engine Library/Database2");
    fs::create_dir_all(&directory).unwrap();
    let db = Connection::open(directory.join("m.db")).unwrap();
    db.execute_batch("CREATE TABLE Information(uuid TEXT,schemaVersionMajor INTEGER,schemaVersionMinor INTEGER,schemaVersionPatch INTEGER);CREATE TABLE Track(id INTEGER PRIMARY KEY,originDatabaseUuid TEXT,originTrackId INTEGER,path TEXT,artist TEXT,title TEXT,bpmAnalyzed REAL,key INTEGER,rating INTEGER,length REAL,isBeatGridLocked INTEGER,streamingSource TEXT,uri TEXT);CREATE TABLE Playlist(id INTEGER PRIMARY KEY,title TEXT,parentListId INTEGER,nextListId INTEGER,isPersisted INTEGER,isExplicitlyExported INTEGER);CREATE TABLE PlaylistEntity(id INTEGER PRIMARY KEY,listId INTEGER,trackId INTEGER,databaseUuid TEXT,nextEntityId INTEGER,membershipReference INTEGER);").unwrap();
    db.execute(
        "INSERT INTO Information VALUES(?1,?2,?3,?4)",
        params![
            UUID,
            if variant == "table" { 3 } else { 2 },
            if variant == "table" { 0 } else { 20 },
            if variant == "table" { 0 } else { 2 }
        ],
    )
    .unwrap();
    db.execute("INSERT INTO Track VALUES(1,?1,100,'../Music/one.flac',NULL,'Native',121,0,80,10.5,1,NULL,NULL),(2,?1,200,NULL,'Artist','Remote',NULL,23,0,NULL,0,'Beatport','beatport://track/200')",[UUID]).unwrap();
    if variant == "table" {
        db.execute_batch("CREATE TABLE PerformanceData(trackId INTEGER PRIMARY KEY,beatData BLOB,quickCues BLOB,loops BLOB,thirdPartySourceId INTEGER,activeOnLoadLoops INTEGER);").unwrap();
    } else {
        for (name, kind) in [
            ("beatData", "BLOB"),
            ("quickCues", "BLOB"),
            ("loops", "BLOB"),
            ("thirdPartySourceId", "INTEGER"),
            ("activeOnLoadLoops", "INTEGER"),
        ] {
            db.execute_batch(&format!("ALTER TABLE Track ADD COLUMN {name} {kind};"))
                .unwrap();
        }
        if variant == "view" {
            db.execute_batch("CREATE VIEW PerformanceData AS SELECT id AS trackId,beatData,quickCues,loops,thirdPartySourceId,activeOnLoadLoops FROM Track;").unwrap();
        }
    }
    let (grid, cues, loops) = vectors();
    if variant == "table" {
        db.execute(
            "INSERT INTO PerformanceData VALUES(1,?1,?2,?3,42,1)",
            params![grid, cues, loops],
        )
        .unwrap();
    } else {
        db.execute("UPDATE Track SET beatData=?1,quickCues=?2,loops=?3,thirdPartySourceId=42,activeOnLoadLoops=1 WHERE id=1",params![grid,cues,loops]).unwrap();
    }
    db.execute_batch("INSERT INTO Playlist VALUES(100,'Folder',0,20,1,0),(20,'Set',100,0,1,0),(70,'Disconnected',0,0,1,0);").unwrap();
    db.execute("INSERT INTO PlaylistEntity VALUES(90,20,100,?1,10,0),(10,20,200,?1,99,0),(99,20,100,?1,0,0),(111,100,100,?1,0,1)",[UUID]).unwrap();
    db
}
fn capture(root: &std::path::Path) -> ImportSnapshot {
    let request = SourceCaptureRequest::from_selected_root(
        SourceProvider::EngineDj,
        root,
        vec![ArtifactRequest::sqlite("Engine Library/Database2/m.db")],
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
fn engine_reads_legacy_and_performance_view_without_migration() {
    for variant in ["legacy", "view", "table"] {
        let root = tempfile::tempdir().unwrap();
        drop(database(root.path(), variant));
        let before = fs::read(root.path().join("Engine Library/Database2/m.db")).unwrap();
        let snapshot = capture(root.path());
        let (_, rows) = read(SourceProvider::EngineDj, &snapshot);
        assert_eq!(rows.tracks().len(), 2);
        let tracks = rows.tracks();
        let one = tracks
            .iter()
            .find(|t| t.extensions["native"]["id"] == 1)
            .unwrap();
        assert!(one.external_id.contains(UUID));
        assert!(one.external_id.ends_with("100"));
        assert_eq!(one.metadata["artist"], Presence::Null);
        assert_eq!(
            one.metadata["rating"],
            Presence::Value(serde_json::json!({"raw":80,"scale_max":100}))
        );
        assert_eq!(one.key.as_ref().unwrap().camelot.as_deref(), Some("8B"));
        let remote = tracks
            .iter()
            .find(|t| t.extensions["native"]["id"] == 2)
            .unwrap();
        assert_eq!(remote.locations.availability, "remote");
        let cs = rows.collections();
        assert_eq!(cs.len(), 3);
        let set = cs.iter().find(|c| c.name == "Set").unwrap();
        let members = rows.members();
        let refs = members
            .iter()
            .filter(|(id, _)| *id == set.external_id)
            .map(|(_, m)| m.external_occurrence_id.as_str())
            .collect::<Vec<_>>();
        assert_eq!(refs, ["90", "10", "99"]);
        assert_eq!(
            members.len(),
            3,
            "inherited rows are not explicit membership"
        );
        let folder = cs.iter().find(|c| c.name == "Folder").unwrap();
        assert!(folder.extensions.contains_key("inherited_membership_pages"));
        assert_eq!(
            fs::read(root.path().join("Engine Library/Database2/m.db")).unwrap(),
            before
        );
    }
}
#[test]
fn engine_vectors_keep_48k_sample_units() {
    let root = tempfile::tempdir().unwrap();
    drop(database(root.path(), "table"));
    let snapshot = capture(root.path());
    let (_, rows) = read(SourceProvider::EngineDj, &snapshot);
    let tracks = rows.tracks();
    let one = tracks
        .iter()
        .find(|t| t.extensions["native"]["id"] == 1)
        .unwrap();
    let grid = one.grid.as_ref().unwrap();
    assert_eq!(grid.points.len(), 2);
    assert_eq!(grid.sample_rate, Some(48000.));
    assert_eq!(grid.points[0].native_seconds, 1.25);
    assert_eq!(grid.points[1].native_seconds, 5.25);
    assert_eq!(grid.points[0].beat_index, Some(0));
    assert_eq!(grid.points[1].beat_index, Some(8));
    assert_eq!(grid.bpm, Some(120.));
    assert_eq!(grid.extensions["first_list"].as_array().unwrap().len(), 2);
    assert_eq!(grid.extensions["native_flag"], 1);
    assert!(grid.canonical_first_beat_seconds.is_none());
    let hot = one
        .cues
        .iter()
        .find(|c| c.family == CueFamily::Hot)
        .unwrap();
    assert_eq!(hot.native_start_seconds, Some(1.25));
    let loop_ = one
        .cues
        .iter()
        .find(|c| c.family == CueFamily::Loop)
        .unwrap();
    assert_eq!(loop_.native_start_seconds, Some(4.));
    assert_eq!(loop_.native_end_seconds, Some(8.));
    assert_eq!(loop_.flags["active_on_load"], true);
    assert!(one.cues.iter().all(|c| c.canonical_start_seconds.is_none()));
    assert_eq!(one.bpm.as_ref().unwrap().value, Some(121.));
}

fn inflated(bytes: &[u8]) -> Vec<u8> {
    use std::io::Read;
    let mut out = Vec::new();
    flate2::read::ZlibDecoder::new(&bytes[4..])
        .read_to_end(&mut out)
        .unwrap();
    out
}
fn compressed(bytes: &[u8]) -> Vec<u8> {
    use std::io::Write;
    let mut out = (bytes.len() as u32).to_be_bytes().to_vec();
    let mut encoder = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::default());
    encoder.write_all(bytes).unwrap();
    out.extend(encoder.finish().unwrap());
    out
}
#[test]
fn engine_corrupt_sizes_counts_and_streams_preserve_rows_without_zero_grid() {
    let (grid, _, _) = vectors();
    let mut bomb = grid.clone();
    bomb[..4].copy_from_slice(&u32::MAX.to_be_bytes());
    let mut mismatch = grid.clone();
    mismatch[..4].copy_from_slice(&131u32.to_be_bytes());
    let truncated = grid[..grid.len() - 3].to_vec();
    let mut count = inflated(&grid);
    count[17..25].copy_from_slice(&u64::MAX.to_be_bytes());
    for bytes in [bomb, mismatch, truncated, compressed(&count)] {
        let root = tempfile::tempdir().unwrap();
        let db = database(root.path(), "table");
        db.execute("UPDATE PerformanceData SET beatData=?1", [&bytes])
            .unwrap();
        drop(db);
        let snapshot = capture(root.path());
        let (_, rows) = read(SourceProvider::EngineDj, &snapshot);
        assert_eq!(rows.tracks().len(), 2);
        let tracks = rows.tracks();
        let one = tracks
            .iter()
            .find(|t| t.extensions["native"]["id"] == 1)
            .unwrap();
        assert!(one.grid.is_none());
        assert!(
            one.issues
                .iter()
                .any(|i| i.code == "engine_grid_decode_failed")
        );
        assert_eq!(one.bpm.as_ref().unwrap().value, Some(121.));
        let reference: BlobRef =
            serde_json::from_value(one.extensions["native"]["beatData"]["blob"].clone()).unwrap();
        assert_eq!(rows.blobs[&reference.sha256], bytes);
    }
}
#[test]
fn engine_invalid_rate_keeps_sample_evidence_without_44100_guess() {
    let (grid, _, _) = vectors();
    let mut body = inflated(&grid);
    body[..8].copy_from_slice(&0f64.to_be_bytes());
    let root = tempfile::tempdir().unwrap();
    let db = database(root.path(), "table");
    db.execute(
        "UPDATE PerformanceData SET beatData=?1",
        [compressed(&body)],
    )
    .unwrap();
    drop(db);
    let snapshot = capture(root.path());
    let (_, rows) = read(SourceProvider::EngineDj, &snapshot);
    let tracks = rows.tracks();
    let one = tracks
        .iter()
        .find(|t| t.extensions["native"]["id"] == 1)
        .unwrap();
    assert!(one.grid.is_none());
    assert_eq!(one.cues.len(), 2);
    assert!(
        one.cues
            .iter()
            .all(|c| c.native_start_seconds.is_none() && c.canonical_start_seconds.is_none())
    );
    assert_eq!(
        one.extensions["native_beat_data"]["second_list"][0]["position_samples"],
        60000.
    );
    assert!(
        one.issues
            .iter()
            .any(|i| i.code == "unresolved_engine_sample_rate")
    );
}
#[test]
fn engine_utf8_names_sentinels_and_negative_positions_keep_original_slots() {
    let mut body = 3u64.to_be_bytes().to_vec();
    for (name, position) in [("é🎵", 60000f64), ("Empty", -1.), ("Preroll", -2400.)] {
        body.push(name.len() as u8);
        body.extend(name.as_bytes());
        body.extend(position.to_be_bytes());
        body.extend([255, 1, 2, 3]);
    }
    body.extend([0; 17]);
    let root = tempfile::tempdir().unwrap();
    let db = database(root.path(), "table");
    db.execute(
        "UPDATE PerformanceData SET quickCues=?1",
        [compressed(&body)],
    )
    .unwrap();
    drop(db);
    let snapshot = capture(root.path());
    let (_, rows) = read(SourceProvider::EngineDj, &snapshot);
    let tracks = rows.tracks();
    let one = tracks
        .iter()
        .find(|t| t.extensions["native"]["id"] == 1)
        .unwrap();
    let hot = one
        .cues
        .iter()
        .filter(|c| c.family == CueFamily::Hot)
        .collect::<Vec<_>>();
    assert_eq!(hot.len(), 2);
    assert_eq!(hot[0].name.as_deref(), Some("é🎵"));
    assert_eq!(hot[0].source_slot, Some(0));
    assert_eq!(hot[0].native_start_seconds, Some(1.25));
    assert_eq!(hot[1].source_slot, Some(2));
    assert_eq!(hot[1].native_start_seconds, Some(-0.05));
    assert_eq!(hot[1].original["position_samples"], -2400.);
}
#[test]
fn engine_unknown_identity_schema_and_cyclic_links_never_repair_source() {
    for sql in [
        "UPDATE Information SET uuid=NULL",
        "UPDATE Information SET schemaVersionMajor=99",
        "UPDATE PlaylistEntity SET nextEntityId=90 WHERE id=10",
        "UPDATE Playlist SET parentListId=20 WHERE id=100",
    ] {
        let root = tempfile::tempdir().unwrap();
        let db = database(root.path(), "table");
        db.execute_batch(sql).unwrap();
        drop(db);
        let snapshot = capture(root.path());
        assert!(
            adapter(SourceProvider::EngineDj)
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
fn engine_native_smart_rules_are_retained_without_becoming_live_predicates() {
    let root = tempfile::tempdir().unwrap();
    let db = database(root.path(), "table");
    db.execute_batch("CREATE TABLE Smartlist(listUuid TEXT PRIMARY KEY,title TEXT,parentPlaylistPath TEXT,nextListUuid TEXT,nextPlaylistPath TEXT,rules TEXT,lastEditTime TEXT);INSERT INTO Smartlist VALUES('smart1','First','Folder;','smart2','','unknown-predicate-1',''),('smart2','Second','Folder;','','','unknown-predicate-2',''),('smart3','Third','','','','unknown-predicate-3','');").unwrap();
    drop(db);
    let snapshot = capture(root.path());
    let (header, rows) = read(SourceProvider::EngineDj, &snapshot);
    assert_eq!(header.accounting.unrepresented_records, 0);
    let cs = rows.collections();
    let smart = cs
        .iter()
        .filter(|c| c.kind == CollectionKind::Smart)
        .collect::<Vec<_>>();
    assert_eq!(smart.len(), 3);
    let first = smart.iter().find(|c| c.name == "First").unwrap();
    assert_eq!(first.extensions["smart_rules"], "unknown-predicate-1");
    assert_eq!(
        first.parent_id.as_deref(),
        cs.iter()
            .find(|c| c.name == "Folder")
            .map(|c| c.external_id.as_str())
    );
    assert!(
        first.extensions["issues"]
            .as_array()
            .unwrap()
            .iter()
            .any(|i| i["code"] == "unsupported_engine_smart_predicate")
    );
}
#[test]
fn engine_coherent_sidecars_keep_origin_namespaces_and_master_identity() {
    let root = tempfile::tempdir().unwrap();
    drop(database(root.path(), "table"));
    let directory = root.path().join("Engine Library/Database2");
    fs::copy(directory.join("m.db"), directory.join("stm.db")).unwrap();
    let db = Connection::open(directory.join("stm.db")).unwrap();
    let remote = "55555555-1111-2222-3333-444444444444";
    db.execute("UPDATE Information SET uuid=?1", [remote])
        .unwrap();
    db.execute("UPDATE Track SET originDatabaseUuid=?1", [remote])
        .unwrap();
    db.execute("UPDATE PlaylistEntity SET databaseUuid=?1", [remote])
        .unwrap();
    drop(db);
    let request = SourceCaptureRequest::from_selected_root(
        SourceProvider::EngineDj,
        root.path(),
        vec![
            ArtifactRequest::sqlite("Engine Library/Database2/stm.db"),
            ArtifactRequest::sqlite("Engine Library/Database2/m.db"),
        ],
    )
    .unwrap();
    let snapshot = capture_source(
        &request,
        root.path(),
        &ImportLimits::default(),
        &CancellationToken::default(),
    )
    .unwrap();
    let (header, rows) = read(SourceProvider::EngineDj, &snapshot);
    assert_eq!(rows.tracks().len(), 4);
    assert_eq!(header.source.library_instance_id, UUID);
    assert_eq!(rows.collections().len(), 6);
}

#[test]
fn engine_conflicting_replicas_reject_instead_of_choosing_database_order() {
    let root = tempfile::tempdir().unwrap();
    drop(database(root.path(), "table"));
    let directory = root.path().join("Engine Library/Database2");
    fs::copy(directory.join("m.db"), directory.join("stm.db")).unwrap();
    let db = Connection::open(directory.join("stm.db")).unwrap();
    db.execute(
        "UPDATE Track SET title='Conflicting cached title',bpmAnalyzed=130 WHERE id=1",
        [],
    )
    .unwrap();
    drop(db);
    let request = SourceCaptureRequest::from_selected_root(
        SourceProvider::EngineDj,
        root.path(),
        vec![
            ArtifactRequest::sqlite("Engine Library/Database2/m.db"),
            ArtifactRequest::sqlite("Engine Library/Database2/stm.db"),
        ],
    )
    .unwrap();
    let snapshot = capture_source(
        &request,
        root.path(),
        &ImportLimits::default(),
        &CancellationToken::default(),
    )
    .unwrap();
    assert!(
        adapter(SourceProvider::EngineDj)
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
fn engine_unqualified_history_rows_are_counted_individually_as_partial() {
    let root = tempfile::tempdir().unwrap();
    let db = database(root.path(), "table");
    db.execute_batch("CREATE TABLE Historylist(id INTEGER PRIMARY KEY,opaque TEXT);INSERT INTO Historylist VALUES(1,'one'),(2,'two');CREATE TABLE HistorylistEntity(id INTEGER PRIMARY KEY,opaque TEXT);INSERT INTO HistorylistEntity VALUES(1,'entry');").unwrap();
    drop(db);
    let snapshot = capture(root.path());
    let options = ReadOptions {
        include_history: true,
        ..Default::default()
    };
    let mut rows = Records::default();
    let header = adapter(SourceProvider::EngineDj)
        .read(
            &snapshot,
            &options,
            &mut rows,
            &CancellationToken::default(),
        )
        .unwrap();
    assert_eq!(header.completeness, Completeness::PartialWithErrors);
    assert_eq!(header.accounting.unrepresented_records, 3);
    let mut reader = rows.rows.clone().into_iter();
    validate_observation(&header, &mut reader, &ImportLimits::default()).unwrap();
    assert_eq!(
        rows.rows
            .iter()
            .filter(|r| matches!(r,ObservationRecord::Issue(i)if i.record_id.is_some()))
            .count(),
        3
    );
    let (default, _) = read(SourceProvider::EngineDj, &snapshot);
    assert_eq!(default.accounting.unrepresented_records, 0);
}

#[test]
fn engine_music_table_views_cannot_read_other_source_tables() {
    let root = tempfile::tempdir().unwrap();
    let db = database(root.path(), "table");
    db.execute_batch("ALTER TABLE Track RENAME TO unrelated_storage;CREATE VIEW Track AS SELECT * FROM unrelated_storage;").unwrap();
    drop(db);
    let snapshot = capture(root.path());
    assert!(
        adapter(SourceProvider::EngineDj)
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
fn engine_foreign_decoder_origin_does_not_hide_portable_media_or_align_cues() {
    let root = tempfile::tempdir().unwrap();
    drop(database(root.path(), "table"));
    fs::create_dir_all(root.path().join("Music")).unwrap();
    fs::write(
        root.path().join("Music/one.flac"),
        b"invented media reference",
    )
    .unwrap();
    let snapshot = capture(root.path());
    let origin = ClockIdentity {
        source_profile: "unqualified-engine".into(),
        codec: "flac".into(),
        sample_rate: 48000,
        media_sha256: "00".repeat(32),
        decoder: "unqualified".into(),
        origin_os: "windows".into(),
        renderer: None,
    };
    let options = ReadOptions {
        origin: Some(origin),
        ..Default::default()
    };
    let mut rows = Records::default();
    adapter(SourceProvider::EngineDj)
        .read(
            &snapshot,
            &options,
            &mut rows,
            &CancellationToken::default(),
        )
        .unwrap();
    let tracks = rows.tracks();
    let one = tracks
        .iter()
        .find(|t| t.extensions["native"]["id"] == 1)
        .unwrap();
    assert_eq!(one.locations.availability, "available");
    assert_eq!(
        one.locations.resolved.as_deref(),
        root.path()
            .join("Music/one.flac")
            .canonicalize()
            .unwrap()
            .to_str()
    );
    assert!(one.cues.iter().all(|c| c.canonical_start_seconds.is_none()));
    assert!(
        one.grid
            .as_ref()
            .unwrap()
            .canonical_first_beat_seconds
            .is_none()
    );
}

#[test]
fn engine_cancelled_sink_cannot_receive_a_complete_header() {
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
    drop(database(root.path(), "table"));
    let snapshot = capture(root.path());
    let cancel = CancellationToken::default();
    let mut sink = CancelSink {
        rows: Records::default(),
        token: cancel.clone(),
    };
    assert!(
        adapter(SourceProvider::EngineDj)
            .read(&snapshot, &ReadOptions::default(), &mut sink, &cancel)
            .is_err()
    );
    assert!(!sink.rows.rows.is_empty());
    snapshot
        .validate_source(&CancellationToken::default())
        .unwrap();
}
