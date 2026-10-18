use cuerithm_autotag::{
    AnalysisTags, AutotagConfig, CatalogMatch, CatalogTrack, MatchStatus, ProviderId, SupportedTag,
    TagInput, TagMatchOutcome, TagWritePlan,
};
use cuerithm_media::{MediaRuntime, file_sha256};
use std::path::Path;
fn runtime() -> MediaRuntime {
    MediaRuntime::from_bundle(
        &Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools/media-runtime/bin"),
    )
    .unwrap()
}
fn outcome() -> TagMatchOutcome {
    TagMatchOutcome {
        status: MatchStatus::Matched,
        matches: vec![CatalogMatch::new(
            1.,
            CatalogTrack {
                platform: "beatport".into(),
                title: "Prepared title".into(),
                artists: vec!["Prepared artist".into()],
                label: Some("Prepared label".into()),
                genres: vec!["House".into()],
                key: Some("Dm".into()),
                bpm: Some(100),
                ..Default::default()
            },
        )],
        attempted: vec![ProviderId::Beatport],
        warnings: vec![],
    }
}
fn analysis() -> AnalysisTags {
    AnalysisTags {
        camelot_key: Some("8A".into()),
        standard_key: Some("Am".into()),
        bpm: Some(123.45),
        energy: Some(6),
    }
}
fn input(rt: &MediaRuntime, path: &Path, cache: &Path) -> TagInput {
    let audio = rt.decode(path, cache).unwrap();
    TagInput {
        path: path.to_owned(),
        source_sha256: audio.source_sha256,
        media: audio.info,
        duration_seconds: audio.clock.duration_seconds(),
    }
}
fn value<'a>(
    tags: &'a std::collections::BTreeMap<String, String>,
    names: &[&str],
) -> Option<&'a str> {
    names.iter().find_map(|k| tags.get(*k).map(String::as_str))
}
#[test]
fn every_common_format_is_tagged_without_changing_playable_pcm_or_original_bytes() {
    let d = tempfile::tempdir().unwrap();
    let archive = std::fs::File::open(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools/fixtures/media.tar.gz"),
    )
    .unwrap();
    tar::Archive::new(flate2::read::GzDecoder::new(archive))
        .unpack(d.path())
        .unwrap();
    let cases: serde_json::Value =
        serde_json::from_slice(&std::fs::read(d.path().join("cases.json")).unwrap()).unwrap();
    let rt = runtime();
    for row in cases.as_array().unwrap() {
        let path = d.path().join(row["file"].as_str().unwrap());
        let source = input(&rt, &path, d.path());
        let before = rt.decode(&path, d.path()).unwrap();
        let hash = source.source_sha256.clone();
        let mut config = AutotagConfig::default();
        config.options.camelot = true;
        config.options.tags = vec![
            SupportedTag::Title,
            SupportedTag::Artist,
            SupportedTag::Label,
            SupportedTag::Genre,
            SupportedTag::BPM,
            SupportedTag::Key,
        ];
        let plan = TagWritePlan::build(source, &outcome(), &config, &analysis()).unwrap();
        let staged = plan
            .stage_prepared(&rt, &before, d.path())
            .unwrap_or_else(|e| panic!("{}: {e:#}", path.display()));
        assert_eq!(file_sha256(&path).unwrap(), hash, "original changed");
        assert_eq!(file_sha256(&staged.backup_path).unwrap(), hash);
        assert_eq!(staged.source_before_sha256, hash);
        assert_eq!(
            staged.source_after_sha256,
            file_sha256(&staged.path).unwrap()
        );
        let after = rt.decode(&staged.path, d.path()).unwrap();
        assert_eq!(
            before.frames,
            after.frames,
            "{} clock changed",
            path.display()
        );
        assert_eq!(
            before.pcm_sha256().unwrap(),
            after.pcm_sha256().unwrap(),
            "{} audio changed",
            path.display()
        );
        let tags = &staged.metadata.tags;
        assert_eq!(
            tags.get("title").map(String::as_str),
            Some("Prepared title"),
            "{}",
            path.display()
        );
        assert_eq!(
            value(tags, &["artist", "author"]),
            Some("Prepared artist"),
            "{}",
            path.display()
        );
        assert_eq!(
            value(tags, &["label", "publisher", "wm/publisher", "tpub"]),
            Some("Prepared label"),
            "{}",
            path.display()
        );
        assert_eq!(
            value(tags, &["bpm", "tbpm", "wm/beatsperminute", "tmpo"]),
            Some("123.45"),
            "{}",
            path.display()
        );
        assert_eq!(
            value(
                tags,
                &["key", "initialkey", "initial_key", "tkey", "wm/initialkey"]
            ),
            Some("8A"),
            "{}",
            path.display()
        );
        assert_eq!(value(tags, &["energy"]), Some("6"), "{}", path.display());
    }
}
#[test]
fn no_op_and_source_change_checks_leave_originals_untouched() {
    let d = tempfile::tempdir().unwrap();
    let rt = runtime();
    let path = d.path().join("wrong 演奏 suffix.bin");
    std::fs::copy(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../cuerithm-media/tests/fixtures/metadata.flac"),
        &path,
    )
    .unwrap();
    let source = input(&rt, &path, d.path());
    let hash = source.source_sha256.clone();
    let mut config = AutotagConfig::default();
    config.options.tags.clear();
    let plan = TagWritePlan::build(source, &outcome(), &config, &AnalysisTags::default()).unwrap();
    assert!(plan.is_noop());
    let staged = plan.stage(&rt, d.path()).unwrap();
    assert_eq!(staged.source_after_sha256, hash);
    assert_eq!(file_sha256(&path).unwrap(), hash);
    std::fs::write(&path, b"new user edit").unwrap();
    assert!(plan.stage(&rt, d.path()).is_err());
    assert_eq!(std::fs::read(&path).unwrap(), b"new user edit");
}
#[test]
fn selected_fields_and_overwrite_policies_preserve_other_metadata() {
    let d = tempfile::tempdir().unwrap();
    let rt = runtime();
    let path = d.path().join("source.flac");
    std::fs::copy(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../cuerithm-media/tests/fixtures/metadata.flac"),
        &path,
    )
    .unwrap();
    let source = input(&rt, &path, d.path());
    let mut config = AutotagConfig::default();
    config.options.tags = vec![SupportedTag::Title, SupportedTag::Label];
    config.options.overwrite = false;
    let plan = TagWritePlan::build(
        source.clone(),
        &outcome(),
        &config,
        &AnalysisTags::default(),
    )
    .unwrap();
    let staged = plan.stage(&rt, d.path()).unwrap();
    assert_eq!(
        staged.metadata.tags.get("title"),
        source.media.tags.get("title")
    );
    assert_eq!(
        staged.metadata.tags.get("label"),
        source.media.tags.get("label")
    );
    assert_eq!(
        staged.metadata.tags.get("comment"),
        source.media.tags.get("comment")
    );
    assert_eq!(
        staged.metadata.tags.get("custom:mood"),
        source.media.tags.get("custom:mood")
    );
    config.options.overwrite_tags = vec![SupportedTag::Label];
    let staged = TagWritePlan::build(
        source.clone(),
        &outcome(),
        &config,
        &AnalysisTags::default(),
    )
    .unwrap()
    .stage(&rt, d.path())
    .unwrap();
    assert_eq!(
        staged.metadata.tags.get("title"),
        source.media.tags.get("title")
    );
    assert_eq!(
        staged.metadata.tags.get("label").map(String::as_str),
        Some("Prepared label")
    );
    assert_eq!(
        staged.metadata.tags.get("custom:mood"),
        source.media.tags.get("custom:mood")
    );
}
#[test]
fn measured_tags_are_written_even_when_catalog_has_no_match() {
    let d = tempfile::tempdir().unwrap();
    let rt = runtime();
    let path = d.path().join("source.flac");
    std::fs::copy(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../cuerithm-media/tests/fixtures/metadata.flac"),
        &path,
    )
    .unwrap();
    let mut config = AutotagConfig::default();
    config.options.camelot = true;
    config.options.tags = vec![SupportedTag::Key, SupportedTag::BPM];
    let no_match = TagMatchOutcome {
        status: MatchStatus::NoMatch,
        matches: vec![],
        attempted: vec![ProviderId::Beatport],
        warnings: vec![],
    };
    let staged = TagWritePlan::build(input(&rt, &path, d.path()), &no_match, &config, &analysis())
        .unwrap()
        .stage(&rt, d.path())
        .unwrap();
    assert_eq!(
        value(&staged.metadata.tags, &["key", "initialkey", "initial_key"]),
        Some("8A")
    );
    assert_eq!(
        value(&staged.metadata.tags, &["bpm", "tbpm"]),
        Some("123.45")
    );
}
#[test]
fn energy_file_write_can_be_disabled_without_disabling_analysis() {
    let d = tempfile::tempdir().unwrap();
    let rt = runtime();
    let path = d.path().join("source.flac");
    std::fs::copy(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../cuerithm-media/tests/fixtures/metadata.flac"),
        &path,
    )
    .unwrap();
    let mut config = AutotagConfig::default();
    config.options.tags.clear();
    config.write_energy = false;
    let plan = TagWritePlan::build(
        input(&rt, &path, d.path()),
        &outcome(),
        &config,
        &analysis(),
    )
    .unwrap();
    assert!(plan.is_noop());
    let staged = plan.stage(&rt, d.path()).unwrap();
    assert!(!staged.metadata.tags.contains_key("energy"));
}
