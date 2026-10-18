use camelot_core::{Engine, EngineSettings, lexicon_timing};
use std::path::Path;

#[path = "support/lexicon_pcm.rs"]
mod fixture;

fn wave(path: &Path, left: &[f32], right: &[f32], rate: u32) {
    assert_eq!(left.len(), right.len());
    let mut writer = hound::WavWriter::create(
        path,
        hound::WavSpec {
            channels: 2,
            sample_rate: rate,
            bits_per_sample: 32,
            sample_format: hound::SampleFormat::Float,
        },
    )
    .unwrap();
    for (&left, &right) in left.iter().zip(right) {
        writer.write_sample(left).unwrap();
        writer.write_sample(right).unwrap();
    }
    writer.finalize().unwrap();
}
fn settings() -> EngineSettings {
    EngineSettings {
        lexicon_timing: true,
        lexicon_config: Some(Default::default()),
        lexicon_energy: true,
        ..Default::default()
    }
}

#[test]
fn native_timing_keeps_full_fractional_bpm_grid_and_cues_on_the_left_channel_clock() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("asymmetric.wav");
    let rate = 44100;
    let recipe: fixture::Recipe = serde_json::from_value(serde_json::json!({
        "frames": 64 * rate, "bpm": 127.234, "offset_frames": 5292,
        "quiet_until_frames": 0, "offbeat_hats": false,
    }))
    .unwrap();
    let left = fixture::signal(&recipe, rate);
    wave(&path, &left, &vec![0.75; left.len()], rate);
    let expected = lexicon_timing::analyze(&left, rate, &Default::default())
        .unwrap()
        .unwrap();
    assert!(
        (expected.bpm - (expected.bpm * 100.).round() / 100.).abs() > 1e-8,
        "Fixture must detect premature two-decimal rounding: {}",
        expected.bpm
    );
    let settings = settings();
    let mut engine = Engine::new(&settings).unwrap();
    let result = engine.analyze_file(&path).unwrap();
    let timing = result.analysis.tempo.as_ref().unwrap();
    assert_eq!(timing.method, "lexicon-timing-v1");
    assert_eq!(timing.bpm.to_bits(), expected.bpm.to_bits());
    assert_eq!(
        timing.downbeat_time.unwrap().to_bits(),
        expected.first_beat_seconds.to_bits()
    );
    assert_eq!(timing.strength, Some(expected.confidence));
    assert!(timing.downbeats_seconds.is_none() && timing.candidates.is_empty());
    let period = 60. / expected.bpm;
    for (index, &beat) in timing.beats_seconds.iter().enumerate() {
        assert_eq!(beat, expected.first_beat_seconds + index as f64 * period);
        assert!(beat >= 0. && beat < left.len() as f64 / rate as f64);
    }
    assert!(timing.beats_seconds.len() > 100);
    assert!(result.complete && result.source_timing.is_none());
    let expected_cues = camelot_core::lexicon_cues::try_generate_structural_cues(
        &left,
        rate,
        expected.bpm,
        expected.first_beat_seconds,
        settings.lexicon_config.as_ref().unwrap(),
    )
    .unwrap();
    assert_eq!(
        serde_json::to_value(&result.analysis.lexicon_cues).unwrap(),
        serde_json::to_value(Some(expected_cues)).unwrap()
    );
    let fingerprint = &result.analysis.models["lexicon-timing-dsp"];
    assert_eq!(fingerprint.len(), 64);
    assert!(fingerprint.bytes().all(|c| c.is_ascii_hexdigit()));
    assert_eq!(&engine.model_identity()["lexicon-timing-dsp"], fingerprint);
    let repeat = engine.analyze_file(&path).unwrap();
    assert_eq!(result.analysis.config_sha256, repeat.analysis.config_sha256);
    let mut prior = settings.clone();
    prior.lexicon_timing = false;
    let prior = Engine::new(&prior).unwrap().analyze_file(&path).unwrap();
    assert_ne!(result.analysis.config_sha256, prior.analysis.config_sha256);
    assert_eq!(
        serde_json::to_value(&result.analysis.key).unwrap(),
        serde_json::to_value(prior.analysis.key).unwrap()
    );
    assert_eq!(
        serde_json::to_value(&result.analysis.energy).unwrap(),
        serde_json::to_value(prior.analysis.energy).unwrap()
    );
}

#[test]
fn silent_left_channel_never_borrows_right_channel_rhythm_or_a_legacy_fallback() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("silent-left.wav");
    let recipe: fixture::Recipe = serde_json::from_value(serde_json::json!({
        "frames": 12 * 44100, "bpm": 128., "offset_frames": 5513,
        "quiet_until_frames": 0, "offbeat_hats": false,
    }))
    .unwrap();
    let right = fixture::signal(&recipe, 44100);
    wave(&path, &vec![0.; right.len()], &right, 44100);
    let result = Engine::new(&settings())
        .unwrap()
        .analyze_file(&path)
        .unwrap();
    assert!(result.complete);
    assert!(result.analysis.tempo.is_none());
    assert!(result.analysis.cues.unwrap().is_empty());
    assert!(result.analysis.lexicon_cues.unwrap().cues.is_empty());
    assert!(result.analysis.energy.is_some());
}

#[test]
fn timing_mode_validation_precedes_loading_models_and_preserves_old_settings() {
    let base = settings();
    assert!(base.validate().is_ok());
    for (source, learned, duration, message) in [
        (true, false, None, "timing method"),
        (false, true, None, "beat models"),
        (false, false, Some(64.), "complete input"),
    ] {
        let mut settings = base.clone();
        settings.source_profile_timing = source;
        settings.beat_model = learned.then(|| "missing-beat.onnx".into());
        settings.mel_model = learned.then(|| "missing-mel.onnx".into());
        settings.max_seconds = duration;
        settings.lexicon_energy = false;
        let error = Engine::new(&settings).err().expect("Invalid timing mode");
        assert!(error.to_string().contains(message), "{error:#}");
    }
    let mut key_settings = base;
    key_settings.key_openkeyscan_model = Some("key.onnx".into());
    assert!(key_settings.validate().is_ok());
    let mut old = serde_json::to_value(EngineSettings::default()).unwrap();
    old.as_object_mut().unwrap().remove("lexicon_timing");
    let restored: EngineSettings = serde_json::from_value(old).unwrap();
    assert!(!restored.lexicon_timing);
    assert!(restored.validate().is_ok());
}

#[test]
fn native_timing_reports_the_complete_input_clock_even_when_the_key_rate_rounds_frames() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("odd-length.wav");
    let recipe: fixture::Recipe = serde_json::from_value(serde_json::json!({
        "frames": 12 * 44100 + 17, "bpm": 128., "offset_frames": 5513,
        "quiet_until_frames": 0, "offbeat_hats": false,
    }))
    .unwrap();
    let left = fixture::signal(&recipe, 44100);
    wave(&path, &left, &vec![0.; left.len()], 44100);
    let mut settings = settings();
    settings.lexicon_energy = false;
    let actual = Engine::new(&settings).unwrap().analyze_file(&path).unwrap();
    assert_eq!(actual.analysis.duration_seconds, left.len() as f64 / 44100.);
    assert!(actual.analysis.tempo.is_some());
}

#[test]
fn lexicon_cues_share_the_native_first_channel_with_timing_instead_of_the_key_mix() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("quiet-left-loud-right.wav");
    let recipe: fixture::Recipe = serde_json::from_value(serde_json::json!({
        "frames": 64 * 44100, "bpm": 128., "offset_frames": 5513,
        "quiet_until_frames": 16 * 44100, "offbeat_hats": false,
    }))
    .unwrap();
    let left = fixture::signal(&recipe, 44100);
    wave(&path, &left, &vec![0.75; left.len()], 44100);
    let expected_timing = lexicon_timing::analyze(&left, 44100, &Default::default())
        .unwrap()
        .unwrap();
    let config = Default::default();
    let expected_cues = camelot_core::lexicon_cues::try_generate_structural_cues(
        &left,
        44100,
        expected_timing.bpm,
        expected_timing.first_beat_seconds,
        &config,
    )
    .unwrap();
    let mixed = camelot_core::audio::decode(&path, 44100, None).unwrap();
    let mixed_cues = camelot_core::lexicon_cues::try_generate_structural_cues(
        &mixed.samples,
        44100,
        expected_timing.bpm,
        expected_timing.first_beat_seconds,
        &config,
    )
    .unwrap();
    let expected = serde_json::to_value(&expected_cues).unwrap();
    assert_ne!(
        expected,
        serde_json::to_value(mixed_cues).unwrap(),
        "The asymmetric fixture must distinguish first-channel routing"
    );
    let actual = Engine::new(&settings())
        .unwrap()
        .analyze_file(&path)
        .unwrap();
    assert_eq!(
        serde_json::to_value(actual.analysis.lexicon_cues.unwrap()).unwrap(),
        expected
    );
}
