use base64::{Engine, engine::general_purpose::STANDARD};
use cuerithm_dj_sync::import::{readers::serato_tags::decode_serato_performance, *};
use cuerithm_media::CancellationToken;
use serde_json::Value;
use std::fs;
fn vector() -> (Vec<u8>, Vec<u8>) {
    let value: Value=serde_json::from_str(include_str!("../../../docs/research/dj-library-import-handover/fixtures/lexicon-serato-codec-vectors.json")).unwrap();
    let markers = STANDARD
        .decode(value["markers2"]["base64"].as_str().unwrap())
        .unwrap();
    let hex = value["beatgrid"]["hex"].as_str().unwrap().as_bytes();
    let grid = hex
        .as_chunks::<2>()
        .0
        .iter()
        .map(|p| u8::from_str_radix(std::str::from_utf8(p).unwrap(), 16).unwrap())
        .collect();
    (markers, grid)
}
fn atom(id: &[u8; 4], payload: &[u8]) -> Vec<u8> {
    let mut bytes = ((payload.len() + 8) as u32).to_be_bytes().to_vec();
    bytes.extend(id);
    bytes.extend(payload);
    bytes
}
fn wrapped(name: &str, data: &[u8]) -> String {
    let mut bytes = b"application/octet-stream\0\0".to_vec();
    bytes.extend(name.as_bytes());
    bytes.push(0);
    bytes.extend(data);
    STANDARD.encode(bytes)
}
fn comments(entries: &[(&str, String)]) -> Vec<u8> {
    let mut bytes = 0u32.to_le_bytes().to_vec();
    bytes.extend((entries.len() as u32).to_le_bytes());
    for (key, value) in entries {
        let data = format!("{key}={value}");
        bytes.extend((data.len() as u32).to_le_bytes());
        bytes.extend(data.as_bytes());
    }
    bytes
}
fn fixture(kind: &str, markers: &[u8], grid: &[u8]) -> Vec<u8> {
    match kind {
        "id3" => {
            let mut body = Vec::new();
            for (name, data) in [("Serato Markers2", markers), ("Serato BeatGrid", grid)] {
                let mut payload = b"\0application/octet-stream\0\0".to_vec();
                payload.extend(name.as_bytes());
                payload.push(0);
                payload.extend(data);
                body.extend(b"GEOB");
                body.extend((payload.len() as u32).to_be_bytes());
                body.extend([0, 0]);
                body.extend(payload);
            }
            let n = body.len();
            let mut bytes = b"ID3\x03\0\0".to_vec();
            bytes.extend([
                (n >> 21) as u8 & 127,
                (n >> 14) as u8 & 127,
                (n >> 7) as u8 & 127,
                n as u8 & 127,
            ]);
            bytes.extend(body);
            bytes
        }
        "flac" => {
            let body = comments(&[
                ("SERATO_MARKERS_V2", wrapped("Serato Markers2", markers)),
                ("SERATO_BEATGRID", wrapped("Serato BeatGrid", grid)),
            ]);
            let mut bytes = b"fLaC\0\0\0\x22".to_vec();
            bytes.extend([0; 34]);
            bytes.push(0x84);
            bytes.extend(&(body.len() as u32).to_be_bytes()[1..]);
            bytes.extend(body);
            bytes
        }
        "mp4" => {
            let mut items = Vec::new();
            for (name, label, data) in [
                ("markersv2", "Serato Markers2", markers),
                ("beatgrid", "Serato BeatGrid", grid),
            ] {
                let mut payload = atom(b"mean", &[&[0; 4][..], b"com.serato.dj"].concat());
                payload.extend(atom(b"name", &[&[0; 4][..], name.as_bytes()].concat()));
                payload.extend(atom(
                    b"data",
                    &[
                        &[0, 0, 0, 1, 0, 0, 0, 0][..],
                        wrapped(label, data).as_bytes(),
                    ]
                    .concat(),
                ));
                items.extend(atom(b"----", &payload));
            }
            let mut bytes = atom(b"ftyp", b"M4A \0\0\0\0");
            bytes.extend(atom(
                b"moov",
                &atom(
                    b"udta",
                    &atom(b"meta", &[&[0; 4][..], &atom(b"ilst", &items)].concat()),
                ),
            ));
            bytes
        }
        "ogg" => {
            let inner = markers[2..].split(|b| *b == 0).next().unwrap();
            let body = comments(&[
                (
                    "SERATO_MARKERS2",
                    String::from_utf8(inner.to_vec()).unwrap(),
                ),
                (
                    "SERATO_BEATGRID",
                    "000000010000000000000001(1.25,120.0)".into(),
                ),
            ]);
            let mut packet = b"\x03vorbis".to_vec();
            packet.extend(body);
            let mut h = b"OggS\0\x02".to_vec();
            h.extend([0; 20]);
            let mut lengths = vec![255; packet.len() / 255];
            lengths.push((packet.len() % 255) as u8);
            h.push(lengths.len() as u8);
            h.extend(lengths);
            h.extend(packet);
            h
        }
        _ => panic!(),
    }
}
#[test]
fn serato_markers2_vectors_preserve_slots_and_loops_in_all_wrappers() {
    let (markers, grid) = vector();
    let root = tempfile::tempdir().unwrap();
    for kind in ["id3", "flac", "mp4", "ogg"] {
        let path = root.path().join(kind);
        fs::write(&path, fixture(kind, &markers, &grid)).unwrap();
        let before = fs::read(&path).unwrap();
        let tags = read_file_tags(
            &path,
            &ImportLimits::default(),
            &CancellationToken::default(),
        )
        .unwrap();
        let decoded = decode_serato_performance(&tags, &ImportLimits::default()).unwrap();
        let Presence::Value(cues) = decoded.cues else {
            panic!("{kind}: {:?}", decoded.issues)
        };
        assert_eq!(cues.len(), 2, "{kind}");
        assert_eq!(cues[0].native_start_seconds, Some(1.25));
        assert_eq!(cues[0].source_slot, Some(0));
        assert_eq!(cues[0].name.as_deref(), Some("Intro"));
        assert_eq!(cues[1].native_start_seconds, Some(4.));
        assert_eq!(cues[1].native_end_seconds, Some(8.));
        assert_eq!(cues[1].flags["locked"], true);
        assert!(cues.iter().all(|c| c.canonical_start_seconds.is_none()));
        let Presence::Value(grid) = decoded.grid else {
            panic!("{kind}: {:?}", decoded.issues)
        };
        assert_eq!(grid.points[0].native_seconds, 1.25);
        assert_eq!(grid.points[0].bpm, Some(120.));
        if kind != "ogg" {
            assert_eq!(grid.points[1].native_seconds, 5.25);
        }
        assert_eq!(fs::read(path).unwrap(), before);
    }
}
#[test]
fn legacy_markers_are_unavailable_rather_than_empty_replacement() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("legacy");
    let mut body = b"\0application/octet-stream\0\0Serato Markers_\0opaque".to_vec();
    let mut frame = b"GEOB".to_vec();
    frame.extend((body.len() as u32).to_be_bytes());
    frame.extend([0, 0]);
    frame.append(&mut body);
    let n = frame.len();
    let mut raw = b"ID3\x03\0\0".to_vec();
    raw.extend([0, 0, (n >> 7) as u8, n as u8 & 127]);
    raw.extend(frame);
    fs::write(&path, raw).unwrap();
    let tags = read_file_tags(
        &path,
        &ImportLimits::default(),
        &CancellationToken::default(),
    )
    .unwrap();
    let out = decode_serato_performance(&tags, &ImportLimits::default()).unwrap();
    assert!(matches!(out.cues, Presence::Absent));
    assert!(
        out.issues
            .iter()
            .any(|i| i.code == "unsupported_legacy_serato_markers")
    );
}

fn marker_entries(entries: &[(&str, Vec<u8>)]) -> Vec<u8> {
    let mut body = vec![1, 1];
    for (name, data) in entries {
        body.extend(name.as_bytes());
        body.push(0);
        body.extend((data.len() as u32).to_be_bytes());
        body.extend(data);
    }
    let mut out = vec![1, 1];
    out.extend(STANDARD.encode(body).as_bytes());
    out.push(0);
    out
}
#[test]
fn loop_only_sparse_unknown_and_duplicate_slots_remain_native_observations() {
    let mut loop_ = vec![0, 7];
    loop_.extend(4000u32.to_be_bytes());
    loop_.extend(8000u32.to_be_bytes());
    loop_.extend([0; 8]);
    loop_.extend([0, 1]);
    loop_.extend(b"Only loop\0");
    let markers = marker_entries(&[
        ("LOOP", loop_.clone()),
        ("FLIP", vec![1, 2, 3]),
        ("LOOP", loop_),
    ]);
    let (_, grid) = vector();
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("only-loop");
    fs::write(&path, fixture("id3", &markers, &grid)).unwrap();
    let tags = read_file_tags(
        &path,
        &ImportLimits::default(),
        &CancellationToken::default(),
    )
    .unwrap();
    let decoded = decode_serato_performance(&tags, &ImportLimits::default()).unwrap();
    let Presence::Value(cues) = decoded.cues else {
        panic!()
    };
    assert_eq!(cues.len(), 2);
    assert!(
        cues.iter()
            .all(|c| c.family == CueFamily::Loop && c.source_slot == Some(7))
    );
    assert!(
        decoded
            .issues
            .iter()
            .any(|i| i.code == "duplicate_serato_slot")
    );
    assert!(
        decoded
            .issues
            .iter()
            .any(|i| i.code == "unsupported_serato_marker_entry")
    );
    assert_eq!(decoded.extensions["marker_entries"][1]["name"], "FLIP");
}
#[test]
fn truncated_base64_count_overflow_and_duplicate_tags_never_erase_good_performance() {
    let (markers, grid) = vector();
    let root = tempfile::tempdir().unwrap();
    let invalid_grid = [1, 0, 255, 255, 255, 255, 0];
    let mut truncated = markers.clone();
    truncated[2] = b'?';
    for (name, m, g) in [
        ("bad-marker", truncated.as_slice(), grid.as_slice()),
        ("bad-grid", markers.as_slice(), invalid_grid.as_slice()),
    ] {
        let path = root.path().join(name);
        fs::write(&path, fixture("id3", m, g)).unwrap();
        let tags = read_file_tags(
            &path,
            &ImportLimits::default(),
            &CancellationToken::default(),
        )
        .unwrap();
        let out = decode_serato_performance(&tags, &ImportLimits::default()).unwrap();
        assert!(
            out.issues
                .iter()
                .any(|i| i.code == "serato_performance_decode_failed")
        );
        if name == "bad-marker" {
            assert!(matches!(out.cues, Presence::Absent));
            assert!(matches!(out.grid, Presence::Value(_)));
        } else {
            assert!(matches!(out.cues, Presence::Value(_)));
            assert!(matches!(out.grid, Presence::Absent));
        }
    }
    let path = root.path().join("duplicates");
    fs::write(&path, fixture("id3", &markers, &grid)).unwrap();
    let mut tags = read_file_tags(
        &path,
        &ImportLimits::default(),
        &CancellationToken::default(),
    )
    .unwrap();
    let mut duplicate = tags
        .records
        .iter()
        .find(|r| {
            r.identity
                .get("description")
                .is_some_and(|s| s == "Serato Markers2")
        })
        .unwrap()
        .clone();
    duplicate.ordinal = tags.records.len();
    tags.records.push(duplicate);
    let out = decode_serato_performance(&tags, &ImportLimits::default()).unwrap();
    assert!(matches!(out.cues, Presence::Absent));
    assert!(
        out.issues
            .iter()
            .any(|i| i.code == "ambiguous_serato_performance_tags")
    );
}

#[test]
fn markers_track_color_and_grid_lock_remain_distinct_native_values() {
    let (_, grid) = vector();
    let markers = marker_entries(&[("COLOR", vec![0, 230, 40, 40]), ("BPMLOCK", vec![1])]);
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("flags");
    fs::write(&path, fixture("id3", &markers, &grid)).unwrap();
    let tags = read_file_tags(
        &path,
        &ImportLimits::default(),
        &CancellationToken::default(),
    )
    .unwrap();
    let out = decode_serato_performance(&tags, &ImportLimits::default()).unwrap();
    assert_eq!(out.extensions["marker_entries"][0]["value"], 0xe62828);
    assert_eq!(out.extensions["marker_entries"][1]["value"], true);
    assert!(matches!(out.cues, Presence::Empty(_)));
}

#[test]
fn autotags_ascii_values_keep_bpm_and_both_gains_separate() {
    let (markers, grid) = vector();
    let mut bytes = fixture("id3", &markers, &grid);
    let payload = b"\0application/octet-stream\0\0Serato Autotags\0\x01\x01120.5\0-2.0\0-3.5\0";
    bytes.extend(b"GEOB");
    bytes.extend((payload.len() as u32).to_be_bytes());
    bytes.extend([0, 0]);
    bytes.extend(payload);
    let n = bytes.len() - 10;
    bytes[6..10].copy_from_slice(&[
        (n >> 21) as u8 & 127,
        (n >> 14) as u8 & 127,
        (n >> 7) as u8 & 127,
        n as u8 & 127,
    ]);
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("autotags");
    fs::write(&path, bytes).unwrap();
    let tags = read_file_tags(
        &path,
        &ImportLimits::default(),
        &CancellationToken::default(),
    )
    .unwrap();
    let out = decode_serato_performance(&tags, &ImportLimits::default()).unwrap();
    assert_eq!(
        out.autotags,
        Presence::Value(serde_json::json!({"bpm":120.5,"auto_gain":-2.0,"gain_db":-3.5}))
    );
}
