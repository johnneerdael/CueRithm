use cuerithm_autotag::{
    AnalysisTags, AutotagConfig, MatchStatus, MetadataField, TagInput, TagMatchOutcome,
    TagWritePlan, TransferDate, TransferPatch, TransferSnapshot, TransferText,
    read_transfer_metadata,
};
use cuerithm_media::{MediaRuntime, file_sha256};
use std::path::Path;
fn config() -> AutotagConfig {
    let mut c = AutotagConfig::default();
    c.options.tags.clear();
    c.write_energy = false;
    c
}
fn outcome() -> TagMatchOutcome {
    TagMatchOutcome {
        status: MatchStatus::NoMatch,
        matches: vec![],
        attempted: vec![],
        warnings: vec![],
    }
}
fn input(runtime: &MediaRuntime, path: &Path, cache: &Path) -> TagInput {
    let a = runtime.decode(path, cache).unwrap();
    TagInput {
        path: path.into(),
        source_sha256: a.source_sha256,
        media: a.info,
        duration_seconds: a.clock.duration_seconds(),
    }
}
fn desired() -> TransferSnapshot {
    let mut s = TransferSnapshot::default();
    for field in MetadataField::ALL {
        s.text.insert(*field, vec![format!("Donor {field:?}")]);
    }
    s.text.insert(
        MetadataField::Artist,
        vec!["Earth, Wind & Fire".into(), "North".into()],
    );
    s.extra.insert(TransferText::Key, vec!["8A".into()]);
    s.extra.insert(TransferText::Energy, vec!["7".into()]);
    s.extra.insert(TransferText::Color, vec!["blue".into()]);
    s.extra.insert(TransferText::Extra1, vec!["Warmup".into()]);
    s.extra.insert(TransferText::Extra2, vec!["Vinyl".into()]);
    s.bpm = Some(128.375);
    s.date = Some(TransferDate {
        year: 2021,
        month: Some(5),
        day: Some(6),
    });
    s.rating = Some(4);
    s.track_number = Some(7);
    s
}
#[test]
fn transfers_all_supported_fields_across_fourteen_layouts_with_pcm_and_original_byte_proofs() {
    let d = tempfile::tempdir().unwrap();
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    tar::Archive::new(flate2::read::GzDecoder::new(
        std::fs::File::open(root.join("tools/fixtures/media.tar.gz")).unwrap(),
    ))
    .unpack(d.path())
    .unwrap();
    let cases: serde_json::Value =
        serde_json::from_slice(&std::fs::read(d.path().join("cases.json")).unwrap()).unwrap();
    assert_eq!(cases.as_array().unwrap().len(), 14);
    let runtime = MediaRuntime::from_bundle(&root.join("tools/media-runtime/bin")).unwrap();
    let cache = d.path().join("cache");
    for row in cases.as_array().unwrap() {
        let path = d.path().join(row["file"].as_str().unwrap());
        let incoming = input(&runtime, &path, &cache);
        let before = read_transfer_metadata(&path, &incoming.media).unwrap();
        let hash = file_sha256(&path).unwrap();
        let patch = TransferPatch {
            before,
            after: desired(),
        };
        let staged = TagWritePlan::build(incoming, &outcome(), &config(), &AnalysisTags::default())
            .unwrap()
            .with_transfer(patch.clone())
            .unwrap()
            .stage(&runtime, &cache)
            .unwrap_or_else(|e| panic!("{}: {e:#}", row["file"]));
        assert_eq!(file_sha256(&path).unwrap(), hash);
        assert_eq!(
            read_transfer_metadata(&staged.path, &staged.metadata).unwrap(),
            patch.after,
            "{}",
            row["file"]
        );
        let a = runtime.decode(&path, &cache).unwrap();
        let b = runtime.decode(&staged.path, &cache).unwrap();
        assert_eq!(a.frames, b.frames);
        assert_eq!(a.pcm_sha256().unwrap(), b.pcm_sha256().unwrap());
        let empty = TransferPatch {
            before: patch.after,
            after: TransferSnapshot::default(),
        };
        let cleared = TagWritePlan::build(
            input(&runtime, &staged.path, &cache),
            &outcome(),
            &config(),
            &AnalysisTags::default(),
        )
        .unwrap()
        .with_transfer(empty)
        .unwrap()
        .stage(&runtime, &cache)
        .unwrap_or_else(|e| panic!("clear {}: {e:#}", row["file"]));
        assert_eq!(
            read_transfer_metadata(&cleared.path, &cleared.metadata).unwrap(),
            TransferSnapshot::default(),
            "clear {}",
            row["file"]
        );
    }
}
#[test]
fn identical_transfer_keeps_exact_bytes_and_stale_or_mixed_edits_are_rejected() {
    let d = tempfile::tempdir().unwrap();
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let runtime = MediaRuntime::from_bundle(&root.join("tools/media-runtime/bin")).unwrap();
    let path = d.path().join("track.flac");
    std::fs::copy(
        root.join("crates/cuerithm-media/tests/fixtures/covered.flac"),
        &path,
    )
    .unwrap();
    let cache = d.path().join("cache");
    let incoming = input(&runtime, &path, &cache);
    let before = read_transfer_metadata(&path, &incoming.media).unwrap();
    let hash = file_sha256(&path).unwrap();
    let plan = TagWritePlan::build(
        incoming.clone(),
        &outcome(),
        &config(),
        &AnalysisTags::default(),
    )
    .unwrap()
    .with_transfer(TransferPatch {
        before: before.clone(),
        after: before.clone(),
    })
    .unwrap();
    let stage = plan.stage(&runtime, &cache).unwrap();
    assert_eq!(stage.source_after_sha256, hash);
    let mut stale = before.clone();
    stale.bpm = Some(180.);
    assert!(
        TagWritePlan::build(
            incoming.clone(),
            &outcome(),
            &config(),
            &AnalysisTags::default()
        )
        .unwrap()
        .with_transfer(TransferPatch {
            before: stale.clone(),
            after: stale
        })
        .unwrap()
        .stage(&runtime, &cache)
        .is_err()
    );
    let mixed = plan
        .with_metadata_edits(vec![cuerithm_autotag::FieldChange {
            field: MetadataField::Title,
            before: before
                .text
                .get(&MetadataField::Title)
                .cloned()
                .unwrap_or_default(),
            after: vec!["Other".into()],
        }])
        .unwrap();
    assert!(mixed.stage(&runtime, &cache).is_err());
    assert_eq!(file_sha256(&path).unwrap(), hash);
}

#[test]
fn clearing_default_comments_keeps_named_dj_comments() {
    use id3::{TagLike, Version, frame::Comment};
    let d = tempfile::tempdir().unwrap();
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    tar::Archive::new(flate2::read::GzDecoder::new(
        std::fs::File::open(root.join("tools/fixtures/media.tar.gz")).unwrap(),
    ))
    .unpack(d.path())
    .unwrap();
    let path = d.path().join("pulses.mp3");
    let mut tag = id3::Tag::read_from_path(&path).unwrap();
    for (lang, description, text) in [
        ("eng", "", "English default"),
        ("nld", "", "Dutch default"),
        ("eng", "DJ_NOTE", "Keep named note"),
    ] {
        tag.add_frame(Comment {
            lang: lang.into(),
            description: description.into(),
            text: text.into(),
        });
    }
    tag.write_to_path(&path, Version::Id3v24).unwrap();
    let runtime = MediaRuntime::from_bundle(&root.join("tools/media-runtime/bin")).unwrap();
    let cache = d.path().join("cache");
    let incoming = input(&runtime, &path, &cache);
    let before = read_transfer_metadata(&path, &incoming.media).unwrap();
    let mut after = before.clone();
    after.text.remove(&MetadataField::Comment);
    let stage = TagWritePlan::build(incoming, &outcome(), &config(), &AnalysisTags::default())
        .unwrap()
        .with_transfer(TransferPatch { before, after })
        .unwrap()
        .stage(&runtime, &cache)
        .unwrap();
    let tag = id3::Tag::read_from_path(&stage.path).unwrap();
    assert!(!tag.comments().any(|c| c.description.is_empty()));
    assert!(
        tag.comments()
            .any(|c| c.description == "DJ_NOTE" && c.text == "Keep named note")
    );
}

#[test]
fn wav_clears_mirrored_info_fields_and_preserves_unrelated_info_chunks() {
    let d = tempfile::tempdir().unwrap();
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    tar::Archive::new(flate2::read::GzDecoder::new(
        std::fs::File::open(root.join("tools/fixtures/media.tar.gz")).unwrap(),
    ))
    .unpack(d.path())
    .unwrap();
    let path = d.path().join("pulses.wav");
    let mut bytes = std::fs::read(&path).unwrap();
    let chunk = |id: &[u8], text: &[u8]| {
        let mut b = id.to_vec();
        b.extend_from_slice(&(text.len() as u32).to_le_bytes());
        b.extend_from_slice(text);
        if !text.len().is_multiple_of(2) {
            b.push(0);
        }
        b
    };
    let engineer = chunk(b"IENG", b"Keep engineer\0");
    let mut info = b"INFO".to_vec();
    info.extend_from_slice(&engineer);
    info.extend_from_slice(&chunk(b"ICMT", b"Legacy comment\0"));
    bytes.extend_from_slice(&chunk(b"LIST", &info));
    let mut associated = b"adtl".to_vec();
    let mut label = 1u32.to_le_bytes().to_vec();
    label.extend_from_slice(b"Keep cue label\0");
    associated.extend_from_slice(&chunk(b"labl", &label));
    let associated = chunk(b"LIST", &associated);
    bytes.extend_from_slice(&associated);
    let size = (bytes.len() - 8) as u32;
    bytes[4..8].copy_from_slice(&size.to_le_bytes());
    std::fs::write(&path, bytes).unwrap();
    let runtime = MediaRuntime::from_bundle(&root.join("tools/media-runtime/bin")).unwrap();
    let cache = d.path().join("cache");
    let incoming = input(&runtime, &path, &cache);
    let before = read_transfer_metadata(&path, &incoming.media).unwrap();
    assert_eq!(
        before.text.get(&MetadataField::Comment),
        Some(&vec!["Legacy comment".to_owned()])
    );
    let stage = TagWritePlan::build(incoming, &outcome(), &config(), &AnalysisTags::default())
        .unwrap()
        .with_transfer(TransferPatch {
            before,
            after: TransferSnapshot::default(),
        })
        .unwrap()
        .stage(&runtime, &cache)
        .unwrap();
    assert_eq!(
        read_transfer_metadata(&stage.path, &stage.metadata).unwrap(),
        TransferSnapshot::default()
    );
    let bytes = std::fs::read(stage.path).unwrap();
    assert!(bytes.windows(engineer.len()).any(|b| b == engineer));
    assert!(bytes.windows(associated.len()).any(|b| b == associated));
}

#[test]
fn wav_legacy_dates_and_track_numbers_round_trip_without_resurrecting_cleared_values() {
    let d = tempfile::tempdir().unwrap();
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    tar::Archive::new(flate2::read::GzDecoder::new(
        std::fs::File::open(root.join("tools/fixtures/media.tar.gz")).unwrap(),
    ))
    .unpack(d.path())
    .unwrap();
    let path = d.path().join("pulses.wav");
    let mut bytes = std::fs::read(&path).unwrap();
    let mut info = b"INFO".to_vec();
    for (id, value) in [
        (b"ICRD", b"2020-03-04\0".as_slice()),
        (b"ITRK", b"2/20\0".as_slice()),
    ] {
        info.extend_from_slice(id);
        info.extend_from_slice(&(value.len() as u32).to_le_bytes());
        info.extend_from_slice(value);
        if !value.len().is_multiple_of(2) {
            info.push(0);
        }
    }
    bytes.extend_from_slice(b"LIST");
    bytes.extend_from_slice(&(info.len() as u32).to_le_bytes());
    bytes.extend_from_slice(&info);
    let len = (bytes.len() - 8) as u32;
    bytes[4..8].copy_from_slice(&len.to_le_bytes());
    std::fs::write(&path, bytes).unwrap();
    let runtime = MediaRuntime::from_bundle(&root.join("tools/media-runtime/bin")).unwrap();
    let cache = d.path().join("cache");
    let incoming = input(&runtime, &path, &cache);
    let before = read_transfer_metadata(&path, &incoming.media).unwrap();
    assert_eq!(
        before.date,
        Some(TransferDate {
            year: 2020,
            month: Some(3),
            day: Some(4)
        })
    );
    assert_eq!(before.track_number, Some(2));
    let mut after = before.clone();
    after.date = None;
    after.track_number = None;
    let stage = TagWritePlan::build(incoming, &outcome(), &config(), &AnalysisTags::default())
        .unwrap()
        .with_transfer(TransferPatch {
            before,
            after: after.clone(),
        })
        .unwrap()
        .stage(&runtime, &cache)
        .unwrap();
    assert_eq!(
        read_transfer_metadata(&stage.path, &stage.metadata).unwrap(),
        after
    );
    assert!(!stage.metadata.tags.contains_key("date"));
    let tag = id3::Tag::read_from_path(&stage.path).unwrap();
    use id3::TagLike;
    assert_eq!(tag.total_tracks(), Some(20));
}
