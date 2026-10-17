use camelot_core::{Engine, EngineSettings};

#[test]
fn reusable_engine_analyzes_a_file_without_any_reference_manifest() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("silence.wav");
    let mut writer = hound::WavWriter::create(
        &path,
        hound::WavSpec {
            channels: 1,
            sample_rate: 11025,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        },
    )
    .unwrap();
    for _ in 0..11025 * 2 {
        writer.write_sample(0_i16).unwrap();
    }
    writer.finalize().unwrap();
    let mut engine = Engine::new(&EngineSettings::default()).unwrap();
    let result = engine.analyze_file(&path).unwrap();
    assert!(result.complete);
    assert!(result.analysis.key.is_none());
    assert!(result.analysis.tempo.is_none());
    assert_eq!(result.analysis.duration_seconds, 2.0);
    assert!(engine.model_identity().is_empty());
    assert_eq!(
        result.analysis.config_sha256,
        "1da41c6848b1c63d06ff7b7bcf774f6eada7e558739c62ac80ba80635cc1b502"
    );
    assert!(
        serde_json::to_value(&result)
            .unwrap()
            .get("openkeyscan_sample_rate")
            .is_none()
    );
    let mut enriched = Engine::new(&EngineSettings {
        structure_features: true,
        ..Default::default()
    })
    .unwrap();
    let with_features = enriched.analyze_file(&path).unwrap();
    assert!(with_features.analysis.structure.is_some());
    assert_ne!(
        result.analysis.config_sha256,
        with_features.analysis.config_sha256
    );
}

#[test]
fn engine_rejects_incomplete_model_configuration_before_reading_audio() {
    let settings = EngineSettings {
        beat_model: Some("missing-beat.onnx".into()),
        ..Default::default()
    };
    assert!(Engine::new(&settings).is_err());
    let settings = EngineSettings {
        key_model: Some("profiles.json".into()),
        key_neural_model: Some("skey.onnx".into()),
        ..Default::default()
    };
    assert!(settings.validate().is_err());
    let settings = EngineSettings {
        cue_config: Some(Default::default()),
        ..Default::default()
    };
    assert!(Engine::new(&settings).is_err());
}

#[test]
fn misleading_filenames_and_wave_info_tags_do_not_change_pcm_predictions() {
    let temp = tempfile::tempdir().unwrap();
    let plain = temp.path().join("plain.wav");
    let tagged = temp.path().join("12B - 240 BPM - Energy 9.wav");
    let mut writer = hound::WavWriter::create(
        &plain,
        hound::WavSpec {
            channels: 1,
            sample_rate: 11025,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        },
    )
    .unwrap();
    for i in 0..11025 * 4 {
        let t = i as f64 / 11025.0;
        let sample = [220.0, 261.6256, 329.6276]
            .iter()
            .map(|f| (std::f64::consts::TAU * f * t).sin())
            .sum::<f64>()
            * 6000.0;
        writer.write_sample(sample as i16).unwrap();
    }
    writer.finalize().unwrap();
    let mut bytes = std::fs::read(&plain).unwrap();
    let comment = b"KEY=12B;BPM=240;ENERGY=9;CUEPOINTS=[0,1,2,3]\0";
    let mut info = b"INFOICMT".to_vec();
    info.extend_from_slice(&(comment.len() as u32).to_le_bytes());
    info.extend_from_slice(comment);
    if !comment.len().is_multiple_of(2) {
        info.push(0);
    }
    bytes.extend_from_slice(b"LIST");
    bytes.extend_from_slice(&(info.len() as u32).to_le_bytes());
    bytes.extend_from_slice(&info);
    let riff_size = (bytes.len() - 8) as u32;
    bytes[4..8].copy_from_slice(&riff_size.to_le_bytes());
    std::fs::write(&tagged, bytes).unwrap();
    let mut engine = Engine::new(&EngineSettings {
        structure_features: true,
        ..Default::default()
    })
    .unwrap();
    let a = engine.analyze_file(&plain).unwrap();
    let b = engine.analyze_file(&tagged).unwrap();
    assert!(a.analysis.key.is_some());
    assert_eq!(
        serde_json::to_value(a.analysis).unwrap(),
        serde_json::to_value(b.analysis).unwrap()
    );
    assert!(a.complete && b.complete);
}
