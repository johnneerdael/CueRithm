use camelot_core::{Engine, EngineSettings};
use cuerithm_media::MediaRuntime;
use std::path::{Path, PathBuf};
fn runtime() -> MediaRuntime {
    let root = std::env::var_os("CUERITHM_MEDIA_RUNTIME")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools/media-runtime/bin")
        });
    MediaRuntime::from_bundle(&root).unwrap()
}
fn configuration() -> EngineSettings {
    EngineSettings {
        lexicon_timing: true,
        lexicon_config: Some(Default::default()),
        ..Default::default()
    }
}

#[test]
fn prepared_cues_use_each_tracks_genre_and_never_leak_it_into_the_next_track() {
    let d = tempfile::tempdir().unwrap();
    let source = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../camelot-source-analysis/tests/fixtures/pulses64-stereo16.flac");
    let mut audio = runtime().decode(&source, d.path()).unwrap();
    audio.info.tags.remove("genre");
    let settings = configuration();
    let mut reusable = Engine::new(&settings).unwrap();
    let baseline = reusable.analyze_prepared(&audio).unwrap();
    audio
        .info
        .tags
        .insert("genre".into(), "Melodic House & Techno".into());
    let actual = reusable.analyze_prepared(&audio).unwrap();
    let mut explicit = settings.clone();
    explicit.lexicon_config.as_mut().unwrap().genre = Some("Melodic House & Techno".into());
    let expected = Engine::new(&explicit)
        .unwrap()
        .analyze_prepared(&audio)
        .unwrap();
    assert_eq!(
        actual.analysis.config_sha256, expected.analysis.config_sha256,
        "The effective per-track genre must enter cue configuration identity"
    );
    assert_eq!(
        serde_json::to_value(&actual.analysis.lexicon_cues).unwrap(),
        serde_json::to_value(&expected.analysis.lexicon_cues).unwrap()
    );
    assert_eq!(
        serde_json::to_value(&actual.media).unwrap()["cue_genre"],
        "Melodic House & Techno"
    );
    audio.source_sha256 = "changed".into();
    assert!(reusable.analyze_prepared(&audio).is_err());
    audio.source_sha256 = cuerithm_media::file_sha256(&audio.source_path).unwrap();
    audio.info.tags.remove("genre");
    let next = reusable.analyze_prepared(&audio).unwrap();
    assert_eq!(next.analysis.config_sha256, baseline.analysis.config_sha256);
}

#[test]
fn explicit_cue_genre_remains_an_override_and_failed_analysis_restores_settings() {
    let d = tempfile::tempdir().unwrap();
    let source = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../camelot-source-analysis/tests/fixtures/pulses64-stereo16.flac");
    let mut audio = runtime().decode(&source, d.path()).unwrap();
    audio.info.tags.insert("genre".into(), "Techno".into());
    let mut settings = configuration();
    settings.lexicon_config.as_mut().unwrap().genre = Some("House".into());
    let mut engine = Engine::new(&settings).unwrap();
    let result = engine.analyze_prepared(&audio).unwrap();
    assert_eq!(
        serde_json::to_value(result.media).unwrap()["cue_genre"],
        "House"
    );
    audio.source_sha256 = "wrong".into();
    assert!(engine.analyze_prepared(&audio).is_err());
    audio.source_sha256 = cuerithm_media::file_sha256(&audio.source_path).unwrap();
    let restored = engine.analyze_prepared(&audio).unwrap();
    assert_eq!(
        serde_json::to_value(restored.media).unwrap()["cue_genre"],
        "House"
    );
}

#[test]
fn final_tag_genre_refresh_changes_only_cues_and_keeps_cached_measurements() {
    let d = tempfile::tempdir().unwrap();
    let source = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../camelot-source-analysis/tests/fixtures/pulses64-stereo16.flac");
    let mut audio = runtime().decode(&source, d.path()).unwrap();
    audio.info.tags.insert("genre".into(), "House".into());
    let engine_settings = configuration();
    let mut engine = Engine::new(&engine_settings).unwrap();
    let mut result = engine.analyze_prepared(&audio).unwrap();
    let before = serde_json::to_value(&result.analysis).unwrap();
    assert!(
        engine
            .refresh_prepared_cues(&audio, &mut result, Some("Techno"))
            .unwrap()
    );
    let after = serde_json::to_value(&result.analysis).unwrap();
    for field in ["key", "tempo", "energy", "duration_seconds", "models"] {
        assert_eq!(before[field], after[field], "{field}");
    }
    assert_eq!(
        result.media.as_ref().unwrap().cue_genre.as_deref(),
        Some("Techno")
    );
    let identity = result.analysis.config_sha256.clone();
    assert!(
        !engine
            .refresh_prepared_cues(&audio, &mut result, Some("Techno"))
            .unwrap()
    );
    assert_eq!(identity, result.analysis.config_sha256);
    let mut explicit = engine_settings;
    explicit.lexicon_config.as_mut().unwrap().genre = Some("Techno".into());
    let expected = Engine::new(&explicit)
        .unwrap()
        .analyze_prepared(&audio)
        .unwrap();
    assert_eq!(
        serde_json::to_value(result.analysis.lexicon_cues).unwrap(),
        serde_json::to_value(expected.analysis.lexicon_cues).unwrap()
    );
}
