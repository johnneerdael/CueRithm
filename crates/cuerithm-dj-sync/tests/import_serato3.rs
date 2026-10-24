#[path = "import_fixture_support/mod.rs"]
mod support;
use cuerithm_dj_sync::import::*;
use cuerithm_media::CancellationToken;
use std::{fs, path::Path};
use support::*;
fn chunk(name: &[u8; 4], body: &[u8]) -> Vec<u8> {
    let mut data = name.to_vec();
    data.extend((body.len() as u32).to_be_bytes());
    data.extend(body);
    data
}
fn text(name: &[u8; 4], value: &str) -> Vec<u8> {
    chunk(
        name,
        &value
            .encode_utf16()
            .flat_map(u16::to_be_bytes)
            .collect::<Vec<_>>(),
    )
}
fn write(root: &Path, path: &str, body: &[u8]) {
    let path = root.join(path);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, body).unwrap();
}
#[test]
fn serato3_chunks_are_parent_bounded_and_volume_scoped() {
    let root = tempfile::tempdir().unwrap();
    let mut names = Vec::new();
    for (volume, title) in [("A", "First"), ("B", "Second")] {
        let mut data = text(b"vrsn", "2.0");
        let mut record = text(b"pfil", "Music/same.mp3");
        record.extend(text(b"tsng", title));
        record.extend(text(b"tkey", "8A"));
        record.extend(chunk(b"xraw", b"opaque"));
        data.extend(chunk(b"otrk", &record));
        let path = format!("{volume}/_Serato_/database V2");
        write(root.path(), &path, &data);
        names.push(path);
        let mut crate_ = text(b"vrsn", "1.0/Serato ScratchLive Crate");
        for _ in 0..2 {
            crate_.extend(chunk(b"otrk", &text(b"ptrk", "Music/same.mp3")));
        }
        let path = format!("{volume}/_Serato_/Subcrates/Parent%%Set.crate");
        write(root.path(), &path, &crate_);
        names.push(path);
    }
    let refs = names.iter().map(String::as_str).collect::<Vec<_>>();
    let snapshot = snapshot(root.path(), SourceProvider::Serato3, &refs);
    let (_, rows) = read(SourceProvider::Serato3, &snapshot);
    let tracks = rows.tracks();
    assert_eq!(tracks.len(), 2);
    assert_ne!(tracks[0].external_id, tracks[1].external_id);
    assert!(
        tracks
            .iter()
            .all(|t| t.key.as_ref().unwrap().camelot.as_deref() == Some("8A"))
    );
    assert!(tracks.iter().all(|t| {
        t.issues
            .iter()
            .any(|i| i.code == "serato_performance_unavailable")
    }));
    let members = rows.members();
    assert_eq!(members.len(), 4);
    assert_eq!(
        members[0].1.track_external_id,
        members[1].1.track_external_id
    );
    assert_ne!(
        members[0].1.external_occurrence_id,
        members[1].1.external_occurrence_id
    );
    assert_ne!(
        members[0].1.track_external_id,
        members[2].1.track_external_id
    );
    assert_eq!(rows.collections().len(), 4);
}
#[test]
fn serato3_truncated_child_cannot_consume_the_next_native_track() {
    let root = tempfile::tempdir().unwrap();
    let mut data = text(b"vrsn", "2.0");
    let mut broken = b"pfil".to_vec();
    broken.extend(100u32.to_be_bytes());
    broken.extend([0, 65]);
    data.extend(chunk(b"otrk", &broken));
    data.extend(chunk(b"otrk", &text(b"pfil", "Later.mp3")));
    write(root.path(), "_Serato_/database V2", &data);
    let snapshot = snapshot(
        root.path(),
        SourceProvider::Serato3,
        &["_Serato_/database V2"],
    );
    assert!(
        adapter(SourceProvider::Serato3)
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
fn serato3_file_owned_cues_keep_database_key_bpm_and_source_music_unchanged() {
    use base64::{Engine, engine::general_purpose::STANDARD};
    use lofty::{
        config::{ParseOptions, WriteOptions},
        file::AudioFile,
    };
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("Music/tagged.flac");
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::copy(
        concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/src/serato4/fixtures/silence.flac"
        ),
        &path,
    )
    .unwrap();
    let vector:serde_json::Value=serde_json::from_str(include_str!("../../../docs/research/dj-library-import-handover/fixtures/lexicon-serato-codec-vectors.json")).unwrap();
    let marker = STANDARD
        .decode(vector["markers2"]["base64"].as_str().unwrap())
        .unwrap();
    let mut envelope = b"application/octet-stream\0\0Serato Markers2\0".to_vec();
    envelope.extend(marker);
    let mut input = fs::File::open(&path).unwrap();
    let mut audio = lofty::flac::FlacFile::read_from(&mut input, ParseOptions::default()).unwrap();
    let mut tags = audio.vorbis_comments().cloned().unwrap_or_default();
    tags.insert("SERATO_MARKERS_V2".into(), STANDARD.encode(envelope));
    tags.insert("BPM".into(), "130".into());
    tags.insert("KEY".into(), "9A".into());
    tags.insert("TITLE".into(), "Current file title".into());
    audio.set_vorbis_comments(tags);
    audio.save_to_path(&path, WriteOptions::default()).unwrap();
    let before = fs::read(&path).unwrap();
    let mut record = text(b"pfil", "Music/tagged.flac");
    record.extend(text(b"tsng", "Native database title"));
    record.extend(text(b"tkey", "8A"));
    record.extend(text(b"tbpm", "121"));
    let mut body = text(b"vrsn", "2.0");
    body.extend(chunk(b"otrk", &record));
    write(root.path(), "_Serato_/database V2", &body);
    let snapshot = snapshot(
        root.path(),
        SourceProvider::Serato3,
        &["_Serato_/database V2"],
    );
    let (_, rows) = read(SourceProvider::Serato3, &snapshot);
    let tracks = rows.tracks();
    let track = tracks[0];
    assert_eq!(track.cues.len(), 2);
    assert_eq!(track.cues[0].native_start_seconds, Some(1.25));
    assert_eq!(track.bpm.as_ref().unwrap().value, Some(121.));
    assert_eq!(track.key.as_ref().unwrap().camelot.as_deref(), Some("8A"));
    assert_eq!(
        track.metadata["title"],
        Presence::Value(serde_json::json!("Native database title"))
    );
    assert!(track.file_tags.is_some());
    assert_eq!(fs::read(path).unwrap(), before);
}
#[test]
fn serato3_smart_rules_neworder_and_empty_crates_remain_observed() {
    let root = tempfile::tempdir().unwrap();
    let crate_ = text(b"vrsn", "1.0/Serato ScratchLive Crate");
    write(root.path(), "_Serato_/Subcrates/Empty.crate", &crate_);
    let mut smart = text(b"vrsn", "1.0/Serato SmartCrate");
    smart.extend(chunk(
        b"rart",
        &chunk(b"rurt", &text(b"trpt", "unknown_operator")),
    ));
    write(root.path(), "_Serato_/SmartCrates/Dynamic.scrate", &smart);
    let order = "\u{feff}[crate]\nDynamic\n[crate]\nEmpty\n"
        .encode_utf16()
        .flat_map(u16::to_be_bytes)
        .collect::<Vec<_>>();
    write(root.path(), "_Serato_/neworder.pref", &order);
    let snapshot = snapshot(
        root.path(),
        SourceProvider::Serato3,
        &[
            "_Serato_/Subcrates/Empty.crate",
            "_Serato_/SmartCrates/Dynamic.scrate",
            "_Serato_/neworder.pref",
        ],
    );
    let (_, rows) = read(SourceProvider::Serato3, &snapshot);
    assert!(rows.tracks().is_empty());
    assert!(rows.members().is_empty());
    let cs = rows.collections();
    assert_eq!(cs.len(), 2);
    let smart = cs.iter().find(|c| c.name == "Dynamic").unwrap();
    assert_eq!(smart.kind, CollectionKind::Smart);
    assert!(smart.extensions.contains_key("smart_rules"));
    assert_eq!(smart.extensions["display_order"], 0);
    assert_eq!(
        cs.iter().find(|c| c.name == "Empty").unwrap().extensions["display_order"],
        1
    );
}

#[test]
fn serato3_obeys_configured_hierarchy_depth() {
    let root = tempfile::tempdir().unwrap();
    write(
        root.path(),
        "_Serato_/Subcrates/Parent%%Child.crate",
        &text(b"vrsn", "1.0/Serato ScratchLive Crate"),
    );
    let snapshot = snapshot(
        root.path(),
        SourceProvider::Serato3,
        &["_Serato_/Subcrates/Parent%%Child.crate"],
    );
    let mut options = ReadOptions::default();
    options.limits.hierarchy_depth = 1;
    assert!(
        adapter(SourceProvider::Serato3)
            .read(
                &snapshot,
                &options,
                &mut Records::default(),
                &CancellationToken::default()
            )
            .is_err()
    );
}
