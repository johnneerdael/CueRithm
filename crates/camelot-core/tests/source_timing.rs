use camelot_core::{Engine, EngineSettings, FileAnalysis, TempoEstimate};
use std::path::PathBuf;

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../camelot-source-analysis/tests/fixtures")
        .join(name)
}

#[test]
fn selected_source_timing_preserves_api_bits_without_inventing_confidence() {
    let path = fixture("pulses64-stereo16.flac");
    let expected = camelot_source_analysis::analyze_timing_file(&path).unwrap();
    let mut engine = Engine::new(&EngineSettings {
        source_profile_timing: true,
        ..Default::default()
    })
    .unwrap();
    let result = engine.analyze_file(&path).unwrap();
    let timing = result.analysis.tempo.as_ref().unwrap();
    assert_eq!(timing.bpm.to_bits(), expected.bpm.to_bits());
    assert_eq!(
        timing.downbeat_time.unwrap().to_bits(),
        expected.downbeat_time.to_bits()
    );
    assert_eq!(timing.beats_seconds, expected.beats_seconds);
    assert_eq!(timing.strength, None);
    assert!(timing.candidates.is_empty());
    assert!(timing.downbeats_seconds.is_none());
    assert_eq!(timing.method, "source-profile-timing-v1");
    assert!(result.complete);
    assert_eq!(result.verified_integrity, Some(true));
    let profile = result.source_timing.as_ref().unwrap();
    assert_eq!(profile.profile, expected.profile);
    assert_eq!(profile.full_source_frames, expected.full_source_frames);
    assert_eq!(profile.analyzed_frames, expected.analyzed_frames);
    assert!(result.analysis.cues.is_none());
    let json = serde_json::to_value(&result).unwrap();
    assert!(json["analysis"]["tempo"].get("strength").is_none());
    let restored: FileAnalysis = serde_json::from_value(json).unwrap();
    assert_eq!(
        restored.analysis.tempo.unwrap().bpm.to_bits(),
        expected.bpm.to_bits()
    );
    let default = Engine::new(&EngineSettings::default())
        .unwrap()
        .analyze_file(&path)
        .unwrap();
    assert_ne!(
        result.analysis.config_sha256,
        default.analysis.config_sha256
    );
    assert!(default.source_timing.is_none());
    let mut old_result = serde_json::to_value(default).unwrap();
    old_result.as_object_mut().unwrap().remove("source_timing");
    if let Some(tempo) = old_result["analysis"]["tempo"].as_object_mut() {
        tempo.remove("downbeat_time");
    }
    let old_result: FileAnalysis = serde_json::from_value(old_result).unwrap();
    assert!(old_result.source_timing.is_none());
}

#[test]
fn source_timing_rejects_partial_or_incompatible_settings_but_allows_key_models() {
    let base = EngineSettings {
        source_profile_timing: true,
        ..Default::default()
    };
    assert!(base.validate().is_ok());
    let mut settings = base.clone();
    settings.max_seconds = Some(64.0);
    assert!(
        settings
            .validate()
            .unwrap_err()
            .to_string()
            .contains("complete input")
    );
    let mut settings = base.clone();
    settings.beat_model = Some("beat.onnx".into());
    settings.mel_model = Some("mel.onnx".into());
    assert!(
        settings
            .validate()
            .unwrap_err()
            .to_string()
            .contains("beat models")
    );
    let mut settings = base.clone();
    settings.cue_config = Some(Default::default());
    assert!(
        settings
            .validate()
            .unwrap_err()
            .to_string()
            .contains("cue suggestions")
    );
    let mut settings = base;
    settings.key_neural_model = Some("key.onnx".into());
    assert!(settings.validate().is_ok());
}

#[test]
fn source_timing_does_not_fall_back_on_unsupported_or_unresolved_inputs() {
    let mut engine = Engine::new(&EngineSettings {
        source_profile_timing: true,
        ..Default::default()
    })
    .unwrap();
    let error = engine
        .analyze_file(&fixture("stereo16-48000.flac"))
        .unwrap_err();
    assert!(format!("{error:#}").contains("44100"));
    assert!(
        engine
            .analyze_file(&fixture("stereo16-44100.flac"))
            .is_err()
    );
}

#[test]
fn older_settings_and_tempo_json_stay_readable() {
    let mut settings = serde_json::to_value(EngineSettings::default()).unwrap();
    settings
        .as_object_mut()
        .unwrap()
        .remove("source_profile_timing");
    let restored: EngineSettings = serde_json::from_value(settings).unwrap();
    assert!(!restored.source_profile_timing);
    let old = serde_json::json!({"bpm": 123.0,"strength":0.7,"candidates":[],"beats_seconds":[0.1,0.6],"downbeats_seconds":null,"method":"old"});
    let tempo: TempoEstimate = serde_json::from_value(old).unwrap();
    assert_eq!(tempo.strength, Some(0.7));
    assert_eq!(tempo.downbeat_time, None);
    assert_eq!(serde_json::to_value(tempo).unwrap()["strength"], 0.7);
}
