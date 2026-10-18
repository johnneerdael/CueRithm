use cuerithm_autotag::{
    AnalysisTags, AutotagConfig, CatalogMatch, CatalogTrack, MatchStatus, SupportedTag, TagInput,
    TagMatchOutcome, TagWritePlan, has_embedded_artwork,
};
use cuerithm_media::MediaRuntime;
use std::path::Path;
#[test]
fn embeds_validated_cover_art_across_all_common_formats_without_touching_audio() {
    let d = tempfile::tempdir().unwrap();
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    tar::Archive::new(flate2::read::GzDecoder::new(
        std::fs::File::open(root.join("tools/fixtures/media.tar.gz")).unwrap(),
    ))
    .unpack(d.path())
    .unwrap();
    let cases: serde_json::Value =
        serde_json::from_slice(&std::fs::read(d.path().join("cases.json")).unwrap()).unwrap();
    let rt = MediaRuntime::from_bundle(&root.join("tools/media-runtime/bin")).unwrap();
    let image = std::fs::read(root.join("apps/desktop/src-tauri/icons/32x32.png")).unwrap();
    let result = TagMatchOutcome {
        status: MatchStatus::Matched,
        matches: vec![CatalogMatch::new(
            1.,
            CatalogTrack {
                platform: "beatport".into(),
                ..Default::default()
            },
        )],
        attempted: vec![],
        warnings: vec![],
    };
    let mut config = AutotagConfig::default();
    config.options.tags = vec![SupportedTag::AlbumArt];
    for case in cases.as_array().unwrap() {
        let path = d.path().join(case["file"].as_str().unwrap());
        let before = rt.decode(&path, d.path()).unwrap();
        assert!(
            !has_embedded_artwork(&path, &before.info).unwrap(),
            "{}",
            path.display()
        );
        let input = TagInput {
            path: path.clone(),
            source_sha256: before.source_sha256.clone(),
            media: before.info.clone(),
            duration_seconds: before.clock.duration_seconds(),
        };
        let plan = TagWritePlan::build(input, &result, &config, &AnalysisTags::default())
            .unwrap()
            .with_artwork(image.clone())
            .unwrap();
        let staged = plan
            .stage_prepared(&rt, &before, d.path())
            .unwrap_or_else(|e| panic!("{}: {e:#}", path.display()));
        let art = rt
            .artwork(&staged.path)
            .unwrap()
            .unwrap_or_else(|| panic!("Missing {} artwork", path.display()));
        assert_eq!(art.bytes, image, "{}", path.display());
        assert_eq!(art.mime, "image/png");
        assert!(
            has_embedded_artwork(&staged.path, &rt.probe(&staged.path).unwrap()).unwrap(),
            "{}",
            path.display()
        );
        assert!(rt.artwork(&path).unwrap().is_none());
    }
}
#[test]
fn refuses_malformed_or_unselected_artwork() {
    let input = TagInput {
        path: "none.flac".into(),
        source_sha256: "0".repeat(64),
        duration_seconds: 1.,
        media: cuerithm_media::MediaInfo {
            codec: "flac".into(),
            sample_rate: 44100,
            channels: 2,
            bits_per_sample: Some(16),
            tags: Default::default(),
            properties: Default::default(),
        },
    };
    let result = TagMatchOutcome {
        status: MatchStatus::Matched,
        matches: vec![CatalogMatch::new(
            1.,
            CatalogTrack {
                platform: "beatport".into(),
                ..Default::default()
            },
        )],
        attempted: vec![],
        warnings: vec![],
    };
    let mut config = AutotagConfig::default();
    config.options.tags = vec![SupportedTag::AlbumArt];
    assert!(
        TagWritePlan::build(input.clone(), &result, &config, &AnalysisTags::default())
            .unwrap()
            .with_artwork(b"not an image".to_vec())
            .is_err()
    );
    config.options.tags.clear();
    assert!(
        TagWritePlan::build(input, &result, &config, &AnalysisTags::default())
            .unwrap()
            .with_artwork(vec![0; 10])
            .is_err()
    );
}
#[test]
fn external_cover_file_is_staged_without_overwriting_existing_picture() {
    let d = tempfile::tempdir().unwrap();
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let rt = MediaRuntime::from_bundle(&root.join("tools/media-runtime/bin")).unwrap();
    let path = d.path().join("source.flac");
    std::fs::copy(
        root.join("crates/cuerithm-media/tests/fixtures/metadata.flac"),
        &path,
    )
    .unwrap();
    let audio = rt.decode(&path, d.path()).unwrap();
    let target = d.path().join("cover-prepared title.jpg");
    std::fs::write(&target, b"existing user's picture").unwrap();
    let input = TagInput {
        path: path.clone(),
        source_sha256: audio.source_sha256.clone(),
        media: audio.info.clone(),
        duration_seconds: audio.clock.duration_seconds(),
    };
    let result = TagMatchOutcome {
        status: MatchStatus::Matched,
        matches: vec![CatalogMatch::new(
            1.,
            CatalogTrack {
                platform: "beatport".into(),
                title: "Prepared Title".into(),
                ..Default::default()
            },
        )],
        attempted: vec![],
        warnings: vec![],
    };
    let mut config = AutotagConfig::default();
    config.options.tags = vec![SupportedTag::Title, SupportedTag::AlbumArt];
    config.options.album_art_file = true;
    config.options.cover_filename = Some("cover-%title.lower()%".into());
    let image = std::fs::read(root.join("apps/desktop/src-tauri/icons/128x128.png")).unwrap();
    let staged = TagWritePlan::build(input, &result, &config, &AnalysisTags::default())
        .unwrap()
        .with_artwork(image)
        .unwrap()
        .stage_prepared(&rt, &audio, d.path())
        .unwrap();
    assert_eq!(staged.sidecars.len(), 1);
    assert_eq!(
        staged.sidecars[0].target_path,
        target.canonicalize().unwrap()
    );
    assert_eq!(std::fs::read(&target).unwrap(), b"existing user's picture");
    assert_eq!(
        std::fs::read(staged.sidecars[0].backup_path.as_ref().unwrap()).unwrap(),
        b"existing user's picture"
    );
    assert!(
        std::fs::read(&staged.sidecars[0].path)
            .unwrap()
            .starts_with(&[0xff, 0xd8, 0xff])
    );
    assert_eq!(
        cuerithm_media::file_sha256(&path).unwrap(),
        audio.source_sha256
    );
}
