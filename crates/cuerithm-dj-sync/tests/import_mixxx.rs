#[path = "import_fixture_support/mod.rs"]
mod support;
use cuerithm_dj_sync::import::*;
use cuerithm_media::CancellationToken;
use rusqlite::{Connection, params};
use serde_json::json;
use support::*;
const GRID: &[u8] = &[
    0x0a, 0x09, 0x09, 0, 0, 0, 0, 0, 0, 0x5e, 0x40, 0x12, 0x04, 0x08, 0xa2, 0xac, 0x01,
];
const MAP: &[u8] = &[
    0x0a, 0x02, 0x08, 0, 0x0a, 0x08, 0x08, 0xa2, 0xac, 0x01, 0x10, 0, 0x18, 2, 0x0a, 0x04, 0x08,
    0xd5, 0xae, 0x03,
];
fn database(root: &std::path::Path, version: u32) -> Connection {
    let db = Connection::open(root.join("mixxxdb.sqlite")).unwrap();
    db.execute_batch("PRAGMA foreign_keys=OFF").unwrap();
    db.execute_batch(match version {
        39 => include_str!("fixtures/import/mixxx/schema39.sql"),
        40 => include_str!("fixtures/import/mixxx/schema40.sql"),
        _ => panic!("unsupported fixture version"),
    })
    .unwrap();
    db
}
fn capture(root: &std::path::Path) -> ImportSnapshot {
    let request = SourceCaptureRequest::from_selected_root(
        SourceProvider::Mixxx,
        root,
        vec![ArtifactRequest::sqlite("mixxxdb.sqlite")],
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
fn track(db: &Connection, id: i64, rate: f64, channels: i64) {
    db.execute("INSERT INTO track_locations(id,location,filename,directory,filesize,fs_deleted,needs_verification) VALUES(?1,?2,?3,'/Invented',1,0,0)",params![id,format!("/Invented/{id}.flac"),format!("{id}.flac")]).unwrap();
    db.execute("INSERT INTO library(id,location,title,samplerate,channels,bpm,beats,beats_version) VALUES(?1,?1,?2,?3,?4,121,?5,'BeatGrid-2.0')",params![id,format!("Track{id}"),rate,channels,GRID]).unwrap();
}
fn raw_blob<'a>(track: &TrackObservation, rows: &'a Records, field: &str) -> &'a [u8] {
    let reference: BlobRef =
        serde_json::from_value(track.extensions["native"][field]["blob"].clone()).unwrap();
    &rows.blobs[&reference.sha256]
}
#[test]
fn mixxx_cues_use_two_engine_channels_at_every_source_channel_count() {
    let root = tempfile::tempdir().unwrap();
    let db = database(root.path(), 40);
    let mut id = 0i64;
    for rate in [44100., 48000., 96000.] {
        for channels in [1, 2, 6] {
            id += 1;
            track(&db, id, rate, channels);
            for (offset, kind, position, length, slot, color) in [
                (0, 1, 2. * rate * 20., 0., 0, 0xff0000),
                (1, 2, -1., 2. * rate * 2., -1, 0),
                (2, 6, 0., 2. * rate, -1, 0),
                (3, 1, -rate * 0.25, 0., 7, 0x00ff00),
                (4, 1, 2. * rate * 20. + 0.25, 0., 9, 0xff0000),
            ] {
                db.execute("INSERT INTO cues(id,track_id,type,position,length,hotcue,label,color) VALUES(?1,?2,?3,?4,?5,?6,?7,?8)",params![id*100+offset,id,kind,position,length,slot,format!("cue{offset}"),color]).unwrap();
            }
        }
    }
    drop(db);
    let snapshot = capture(root.path());
    let (_, rows) = read(SourceProvider::Mixxx, &snapshot);
    assert_eq!(rows.tracks().len(), 9);
    for track in rows.tracks() {
        let rate = track.extensions["native"]["samplerate"].as_f64().unwrap();
        let by_label = |name: &str| {
            track
                .cues
                .iter()
                .find(|c| c.name.as_deref() == Some(name))
                .unwrap()
        };
        assert_eq!(by_label("cue0").native_start_seconds, Some(20.));
        assert!(by_label("cue1").native_start_seconds.is_none());
        assert_eq!(by_label("cue1").native_end_seconds, Some(2.));
        assert_eq!(by_label("cue2").native_start_seconds, Some(0.));
        assert_eq!(by_label("cue2").native_end_seconds, Some(1.));
        assert_eq!(by_label("cue3").native_start_seconds, Some(-0.125));
        assert!(
            (by_label("cue4").native_start_seconds.unwrap() - (20. + 0.25 / (2. * rate))).abs()
                < 1e-12
        );
        assert!(
            track
                .cues
                .iter()
                .all(|c| c.clock_status == ClockStatus::UnresolvedNative
                    && c.canonical_start_seconds.is_none())
        );
        let grid = track.grid.as_ref().unwrap();
        assert_eq!(grid.native_first_beat_seconds, Some(22050. / rate));
        assert_eq!(grid.bpm, Some(120.));
        assert_eq!(track.bpm.as_ref().unwrap().value, Some(121.));
        assert!(grid.canonical_first_beat_seconds.is_none());
        assert_eq!(track.metadata["artist"], Presence::Null);
    }
}
#[test]
fn mixxx_preserves_all_keys_variable_beats_and_occurrences() {
    let root = tempfile::tempdir().unwrap();
    let db = database(root.path(), 39);
    for key in 1..=24i64 {
        track(&db, key, 44100., 2);
        let mut bytes = vec![0x08, key as u8, 0x20, 2];
        bytes.extend([0x9d, 0x06, 1, 2, 3, 4]);
        db.execute("UPDATE library SET keys=?1,keys_version='KeyMap-1.0',key='independent text',key_id=0 WHERE id=?2",params![bytes,key]).unwrap();
    }
    db.execute(
        "UPDATE library SET beats=?1,beats_version='BeatMap-1.0' WHERE id=1",
        [MAP],
    )
    .unwrap();
    db.execute_batch("INSERT INTO Playlists(id,name,position,hidden) VALUES(1,'Set',0,0);INSERT INTO PlaylistTracks(id,playlist_id,track_id,position) VALUES(10,1,1,1),(11,1,2,2),(12,1,1,3);INSERT INTO crates(id,name) VALUES(5,'Folder/Is a name');INSERT INTO crate_tracks(crate_id,track_id) VALUES(5,1),(5,2);INSERT INTO cues(id,track_id,type,position,length,hotcue,label,color) VALUES(1,1,1,0,0,0,'Colored zero',65280),(2,1,1,0,0,0,'Duplicate',16711680),(3,1,1,0,0,7,'Historical black',0);").unwrap();
    drop(db);
    let snapshot = capture(root.path());
    let (_, rows) = read(SourceProvider::Mixxx, &snapshot);
    let expected = [
        "8B", "3B", "10B", "5B", "12B", "7B", "2B", "9B", "4B", "11B", "6B", "1B", "5A", "12A",
        "7A", "2A", "9A", "4A", "11A", "6A", "1A", "8A", "3A", "10A",
    ];
    for (i, key) in expected.iter().enumerate() {
        let tracks = rows.tracks();
        let track = tracks
            .iter()
            .find(|t| t.external_id == (i + 1).to_string())
            .unwrap();
        assert_eq!(track.key.as_ref().unwrap().camelot.as_deref(), Some(*key));
    }
    let tracks = rows.tracks();
    let first = tracks.iter().find(|t| t.external_id == "1").unwrap();
    let grid = first.grid.as_ref().unwrap();
    assert_eq!(grid.points.len(), 3);
    assert_eq!(grid.points[0].native_seconds, 0.);
    assert_eq!(grid.points[1].native_seconds, 0.5);
    assert_eq!(grid.points[2].native_seconds, 1.25);
    assert_eq!(grid.points[1].original["enabled"], false);
    assert_eq!(grid.points[0].original["enabled"], serde_json::Value::Null);
    assert_eq!(first.cues.len(), 3);
    assert_eq!(
        first
            .cues
            .iter()
            .filter(|c| c.source_slot == Some(0))
            .count(),
        2
    );
    assert!(
        first
            .issues
            .iter()
            .any(|i| i.code == "duplicate_hotcue_slot")
    );
    assert!(
        first
            .issues
            .iter()
            .any(|i| i.code == "historical_black_zero_cue")
    );
    assert_eq!(raw_blob(first, &rows, "beats"), MAP);
    assert!(raw_blob(first, &rows, "keys").ends_with(&[0x9d, 0x06, 1, 2, 3, 4]));
    let collections = rows.collections();
    let playlist = collections.iter().find(|c| c.name == "Set").unwrap();
    let members = rows.members();
    let ordered = members
        .iter()
        .filter(|(id, _)| *id == playlist.external_id)
        .map(|(_, m)| m.track_external_id.as_str())
        .collect::<Vec<_>>();
    assert_eq!(ordered, vec!["1", "2", "1"]);
    let crate_ = collections
        .iter()
        .find(|c| c.name == "Folder/Is a name")
        .unwrap();
    assert_eq!(crate_.kind, CollectionKind::Crate);
    assert!(crate_.parent_id.is_none());
    assert!(
        members
            .iter()
            .filter(|(id, _)| *id == crate_.external_id)
            .all(|(_, m)| m.position.is_none())
    );
}
#[test]
fn mixxx_bad_grid_unknown_key_codec_and_missing_location_preserve_rows() {
    let root = tempfile::tempdir().unwrap();
    let db = database(root.path(), 40);
    for id in 1..=4 {
        track(&db, id, 44100., 2);
    }
    db.execute(
        "UPDATE library SET beats=?1 WHERE id=1",
        [&[0x0a, 0xff][..]],
    )
    .unwrap();
    db.execute(
        "UPDATE library SET keys=?1,keys_version='FutureKey-9',key='8A',key_id=22 WHERE id=2",
        [&[8, 22][..]],
    )
    .unwrap();
    db.execute("UPDATE library SET samplerate=0 WHERE id=3", [])
        .unwrap();
    db.execute("DELETE FROM track_locations WHERE id=4", [])
        .unwrap();
    db.execute("UPDATE library SET mixxx_deleted=1 WHERE id=4", [])
        .unwrap();
    drop(db);
    let snapshot = capture(root.path());
    let (_, rows) = read(SourceProvider::Mixxx, &snapshot);
    let tracks = rows.tracks();
    assert_eq!(tracks.len(), 4);
    let one = tracks.iter().find(|t| t.external_id == "1").unwrap();
    assert!(one.grid.is_none());
    assert!(one.issues.iter().any(|i| i.code == "grid_decode_failed"));
    assert_eq!(raw_blob(one, &rows, "beats"), &[0x0a, 0xff]);
    assert_eq!(one.bpm.as_ref().unwrap().value, Some(121.));
    let two = tracks.iter().find(|t| t.external_id == "2").unwrap();
    assert!(two.key.is_none());
    assert_eq!(two.extensions["native"]["key"], "8A");
    assert!(two.issues.iter().any(|i| i.code == "key_decode_failed"));
    let three = tracks.iter().find(|t| t.external_id == "3").unwrap();
    assert!(three.grid.is_none());
    assert!(three.issues.iter().any(|i| i.code == "invalid_sample_rate"));
    let four = tracks.iter().find(|t| t.external_id == "4").unwrap();
    assert!(four.locations.native_path.is_none());
    assert_eq!(four.extensions["native"]["mixxx_deleted"], 1);
    assert!(
        four.issues
            .iter()
            .any(|i| i.code == "missing_location_record")
    );
}
#[test]
fn mixxx_rejects_settings_version_and_schema_drift_without_migration() {
    for drift in [
        "UPDATE settings SET value='41' WHERE name='mixxx.schema.version';PRAGMA user_version=40;",
        "ALTER TABLE library ADD COLUMN unqualified_extension TEXT;",
    ] {
        let root = tempfile::tempdir().unwrap();
        let db = database(root.path(), 40);
        db.execute_batch(drift).unwrap();
        drop(db);
        let snapshot = capture(root.path());
        assert!(
            adapter(SourceProvider::Mixxx)
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
fn mixxx_legacy_grid_requires_explicit_source_byte_order_without_clock_authority() {
    for order in [SourceByteOrder::Little, SourceByteOrder::Big] {
        for version in ["BeatGrid-1.0", "BeatGrid-2.0"] {
            let root = tempfile::tempdir().unwrap();
            let db = database(root.path(), 40);
            track(&db, 1, 44100., 2);
            let bytes: Vec<u8> = [120f64, 22050.25]
                .into_iter()
                .flat_map(|n| match order {
                    SourceByteOrder::Little => n.to_le_bytes(),
                    SourceByteOrder::Big => n.to_be_bytes(),
                })
                .collect();
            db.execute(
                "UPDATE library SET beats=?1,beats_version=?2",
                params![bytes, version],
            )
            .unwrap();
            drop(db);
            let snapshot = capture(root.path());
            let (_, unresolved) = read(SourceProvider::Mixxx, &snapshot);
            assert!(unresolved.tracks()[0].grid.is_none());
            let mut rows = Records::default();
            adapter(SourceProvider::Mixxx)
                .read(
                    &snapshot,
                    &ReadOptions {
                        source_byte_order: Some(order),
                        ..Default::default()
                    },
                    &mut rows,
                    &CancellationToken::default(),
                )
                .unwrap();
            let tracks = rows.tracks();
            let grid = tracks[0]
                .grid
                .as_ref()
                .expect("qualified native byte order");
            assert_eq!(grid.native_first_beat_seconds, Some(22050.25 / 44100.));
            assert_eq!(grid.bpm, Some(120.));
            assert!(grid.canonical_first_beat_seconds.is_none());
            assert_eq!(raw_blob(tracks[0], &rows, "beats"), bytes);
        }
    }
}

#[test]
fn mixxx_key_presence_changes_tuning_and_conflicting_evidence_stay_separate() {
    let root = tempfile::tempdir().unwrap();
    let db = database(root.path(), 40);
    track(&db, 1, 44100., 2);
    // Signed int32 -1 is ten-byte sign extended; optional key-change tuning absent.
    let mut change = vec![8];
    change.extend([255; 9]);
    change.extend([1, 16, 13]);
    let mut keys = vec![8, 1, 26, change.len() as u8];
    keys.extend(change);
    keys.extend([41]);
    keys.extend(442f64.to_le_bytes());
    db.execute("UPDATE library SET keys=?1,keys_version='KeyMap-1.0',key='8A',key_id=22,tuning_frequency_hz=440",[&keys]).unwrap();
    drop(db);
    let snapshot = capture(root.path());
    let (_, rows) = read(SourceProvider::Mixxx, &snapshot);
    let tracks = rows.tracks();
    let one = tracks[0];
    let key = one.key.as_ref().unwrap();
    assert_eq!(key.camelot.as_deref(), Some("8B"));
    assert_eq!(key.extensions["global_key_text"], json!(null));
    assert_eq!(key.extensions["global_tuning_frequency_hz"], json!(442.));
    assert_eq!(key.extensions["key_changes"][0]["frame_position"], -1);
    assert_eq!(
        key.extensions["key_changes"][0]["tuning_frequency_hz"],
        json!(null)
    );
    assert_eq!(one.extensions["native"]["tuning_frequency_hz"], json!(440.));
    assert!(
        one.issues
            .iter()
            .any(|i| i.code == "conflicting_key_evidence")
    );
    assert_eq!(raw_blob(one, &rows, "keys"), keys);
}

#[test]
fn mixxx_history_queue_and_equal_position_occurrences_are_explicit() {
    let root = tempfile::tempdir().unwrap();
    let db = database(root.path(), 40);
    track(&db, 1, 44100., 2);
    db.execute_batch("UPDATE library SET timesplayed=3,mixxx_deleted=1;UPDATE track_locations SET fs_deleted=1;INSERT INTO Playlists(id,name,position,hidden) VALUES(1,'Set',0,0),(2,'Auto DJ',1,1),(3,'History',2,2);INSERT INTO PlaylistTracks(id,playlist_id,track_id,position) VALUES(12,1,1,1),(10,1,1,1),(15,2,1,1),(16,3,1,1);").unwrap();
    drop(db);
    let snapshot = capture(root.path());
    let (_, rows) = read(SourceProvider::Mixxx, &snapshot);
    assert_eq!(rows.collections().len(), 1);
    assert_eq!(
        rows.members()
            .iter()
            .map(|(_, m)| m.external_occurrence_id.as_str())
            .collect::<Vec<_>>(),
        ["10", "12"]
    );
    assert!(
        rows.collections()[0].extensions["issues"]
            .as_array()
            .unwrap()
            .iter()
            .any(|i| i["code"] == "duplicate_membership_position")
    );
    assert!(!rows.tracks()[0].metadata.contains_key("play_count"));
    assert_eq!(rows.tracks()[0].extensions["native"]["mixxx_deleted"], 1);
    assert_eq!(rows.tracks()[0].locations.extensions["fs_deleted"], 1);
    let mut selected = Records::default();
    adapter(SourceProvider::Mixxx)
        .read(
            &snapshot,
            &ReadOptions {
                include_queue: true,
                include_history: true,
                ..Default::default()
            },
            &mut selected,
            &CancellationToken::default(),
        )
        .unwrap();
    assert_eq!(selected.collections().len(), 3);
    assert_eq!(
        selected.tracks()[0].metadata["play_count"],
        Presence::Value(json!(3))
    );
    let canceled = CancellationToken::default();
    canceled.cancel();
    assert!(
        adapter(SourceProvider::Mixxx)
            .read(
                &snapshot,
                &ReadOptions::default(),
                &mut Records::default(),
                &canceled
            )
            .is_err()
    );
}

#[test]
fn mixxx_unknown_proto_groups_survive_but_missing_anchor_never_becomes_zero() {
    let root = tempfile::tempdir().unwrap();
    let db = database(root.path(), 40);
    for id in 1..=3 {
        track(&db, id, 44100., 2);
    }
    let mut grouped = GRID.to_vec();
    // Unknown group 99 containing unknown varint field 1, closed with matching tag.
    grouped.extend([0x9b, 0x06, 0x08, 0x07, 0x9c, 0x06]);
    db.execute("UPDATE library SET beats=?1 WHERE id=1", [&grouped])
        .unwrap();
    let mut missing = GRID[..11].to_vec();
    missing.extend([0x12, 0]);
    db.execute("UPDATE library SET beats=?1 WHERE id=2", [&missing])
        .unwrap();
    let mut overflow = vec![8];
    overflow.extend([255; 10]);
    db.execute(
        "UPDATE library SET keys=?1,keys_version='KeyMap-1.0' WHERE id=3",
        [&overflow],
    )
    .unwrap();
    drop(db);
    let snapshot = capture(root.path());
    let (_, rows) = read(SourceProvider::Mixxx, &snapshot);
    let tracks = rows.tracks();
    let one = tracks.iter().find(|t| t.external_id == "1").unwrap();
    assert_eq!(
        one.grid
            .as_ref()
            .expect("unknown valid protobuf fields")
            .native_first_beat_seconds,
        Some(0.5)
    );
    assert_eq!(raw_blob(one, &rows, "beats"), grouped);
    let two = tracks.iter().find(|t| t.external_id == "2").unwrap();
    assert!(two.grid.is_none());
    assert!(two.issues.iter().any(|i| i.code == "grid_decode_failed"));
    let three = tracks.iter().find(|t| t.external_id == "3").unwrap();
    assert!(three.key.is_none());
    assert!(three.issues.iter().any(|i| i.code == "key_decode_failed"));
    assert_eq!(raw_blob(three, &rows, "keys"), overflow);
}
