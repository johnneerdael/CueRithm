use camelot_core::{Engine, EngineSettings};
use serde_json::json;

fn settings(extra: serde_json::Value) -> EngineSettings {
    let mut value = serde_json::to_value(EngineSettings::default()).unwrap();
    value
        .as_object_mut()
        .unwrap()
        .extend(extra.as_object().unwrap().clone());
    serde_json::from_value(value).unwrap()
}

#[test]
fn selected_openkeyscan_model_survives_settings_round_trip() {
    let selected = settings(json!({"key_openkeyscan_model": "openkeyscan.onnx"}));
    assert_eq!(
        serde_json::to_value(selected).unwrap()["key_openkeyscan_model"],
        "openkeyscan.onnx"
    );
}

#[test]
fn openkeyscan_conflicts_are_rejected_before_reading_model_files() {
    for other in ["key_model", "key_neural_model"] {
        let selected =
            settings(json!({"key_openkeyscan_model": "missing.onnx", other: "also-missing"}));
        assert!(
            selected.validate().is_err(),
            "conflict with {other} must be rejected"
        );
    }
}

#[cfg(not(feature = "openkeyscan-key"))]
#[test]
fn missing_openkeyscan_feature_is_an_explicit_error() {
    let selected = settings(json!({"key_openkeyscan_model": "missing.onnx"}));
    let error = Engine::new(&selected)
        .err()
        .expect("backend selection must not be silently ignored");
    assert!(error.to_string().contains("openkeyscan-key"), "{error:#}");
}

#[test]
fn older_settings_keep_the_default_backend_and_json_shape() {
    let value = serde_json::to_value(EngineSettings::default()).unwrap();
    assert!(value.get("key_openkeyscan_model").is_none());
    let legacy: EngineSettings = serde_json::from_value(value).unwrap();
    legacy.validate().unwrap();
    assert!(Engine::new(&legacy).unwrap().model_identity().is_empty());
}

#[cfg(feature = "openkeyscan-key")]
#[test]
#[ignore = "requires CAMELOT_OPENKEYSCAN_MODEL pointing at the pinned optional graph"]
fn engine_uses_direct_44100_input_and_records_model_config_and_excerpt_scope() {
    let model = std::env::var_os("CAMELOT_OPENKEYSCAN_MODEL").expect("model path required");
    let temp = tempfile::tempdir().unwrap();
    let audio = temp.path().join("unlabeled.wav");
    let mut writer = hound::WavWriter::create(
        &audio,
        hound::WavSpec {
            channels: 1,
            sample_rate: 44100,
            bits_per_sample: 32,
            sample_format: hound::SampleFormat::Float,
        },
    )
    .unwrap();
    for bytes in include_bytes!("fixtures/openkeyscan/noise5-pcm.f32le")
        .as_chunks::<4>()
        .0
        .iter()
    {
        writer.write_sample(f32::from_le_bytes(*bytes)).unwrap();
    }
    writer.finalize().unwrap();
    let mut selected = settings(json!({"key_openkeyscan_model": std::path::PathBuf::from(&model)}));
    let mut engine = Engine::new(&selected).unwrap();
    let original = engine.analyze_file(&audio).unwrap();
    assert_eq!(original.openkeyscan_sample_rate, Some(44100));
    assert_eq!(original.analysis.sample_rate, 11025);
    assert_eq!(original.analysis.key.as_ref().unwrap().camelot, "6A");
    assert!(
        original
            .analysis
            .engine
            .contains("+openkeyscan-cqt-rten-v1")
    );
    assert_eq!(
        original.analysis.models["openkeyscan-key"],
        camelot_core::openkeyscan::MODEL_SHA256
    );
    assert_eq!(original.analysis.models["openkeyscan-frontend"].len(), 64);
    assert_eq!(original.analysis.models["openkeyscan-native"].len(), 64);
    let mut legacy = serde_json::to_value(&original).unwrap();
    legacy
        .as_object_mut()
        .unwrap()
        .remove("openkeyscan_sample_rate");
    assert!(
        serde_json::from_value::<camelot_core::FileAnalysis>(legacy)
            .unwrap()
            .openkeyscan_sample_rate
            .is_none()
    );
    selected.sample_rate = 44100;
    let direct = Engine::new(&selected)
        .unwrap()
        .analyze_file(&audio)
        .unwrap();
    let scores = |x: &camelot_core::FileAnalysis| {
        x.analysis
            .key
            .as_ref()
            .unwrap()
            .candidates
            .iter()
            .map(|k| k.score)
            .collect::<Vec<_>>()
    };
    assert_eq!(scores(&original), scores(&direct));
    assert_ne!(
        original.analysis.config_sha256,
        direct.analysis.config_sha256
    );
    let duplicate = temp.path().join("graph-copy.onnx");
    std::fs::copy(std::path::Path::new(&model), &duplicate).unwrap();
    selected.sample_rate = 11025;
    selected.key_openkeyscan_model = Some(duplicate);
    let relocated = Engine::new(&selected)
        .unwrap()
        .analyze_file(&audio)
        .unwrap();
    assert_eq!(
        original.analysis.config_sha256,
        relocated.analysis.config_sha256
    );
    selected.max_seconds = Some(1.0);
    let partial = Engine::new(&selected)
        .unwrap()
        .analyze_file(&audio)
        .unwrap();
    assert!(!partial.complete);
    assert!(partial.analysis.key.is_none());
    assert_eq!(partial.openkeyscan_sample_rate, Some(44100));
    assert_ne!(
        partial.analysis.config_sha256,
        original.analysis.config_sha256
    );
}
