use camelot_core::{
    Engine, EngineSettings,
    lexicon_cues::{LexiconConfig, try_generate_structural_cues as generate_structural_cues},
};

#[test]
fn cue_configuration_accepts_source_bpm_and_rejects_invalid_domain() {
    let settings = EngineSettings {
        source_profile_timing: true,
        lexicon_config: Some(Default::default()),
        ..Default::default()
    };
    assert!(settings.validate().is_ok());
    assert!(generate_structural_cues(&[1.; 100], 0, 120., 0., &LexiconConfig::default()).is_err());
    for bpm in [0., -120., f64::NAN, f64::INFINITY] {
        assert!(
            generate_structural_cues(&[1.; 100], 44100, bpm, 0., &LexiconConfig::default())
                .is_err()
        );
    }
    assert!(
        generate_structural_cues(&[f32::NAN], 44100, 120., 0., &LexiconConfig::default()).is_err()
    );
    assert!(
        generate_structural_cues(&[], 44100, 120., 0., &LexiconConfig::default())
            .unwrap()
            .cues
            .is_empty()
    );
}

#[test]
fn silent_audio_with_cues_requested_is_an_empty_result_and_keeps_key_tempo_unknown() {
    let d = tempfile::tempdir().unwrap();
    let path = d.path().join("silent.wav");
    let mut w = hound::WavWriter::create(
        &path,
        hound::WavSpec {
            channels: 1,
            sample_rate: 11025,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        },
    )
    .unwrap();
    for _ in 0..22050 {
        w.write_sample(0_i16).unwrap();
    }
    w.finalize().unwrap();
    let mut engine = Engine::new(&EngineSettings {
        lexicon_config: Some(Default::default()),
        ..Default::default()
    })
    .unwrap();
    let result = engine.analyze_file(&path).unwrap();
    assert!(result.analysis.cues.unwrap().is_empty());
    assert!(result.analysis.lexicon_cues.unwrap().cues.is_empty());
    assert!(result.analysis.key.is_none() && result.analysis.tempo.is_none());
    assert!(result.analysis.energy.is_none());
}

#[test]
fn source_bpm_and_structural_cues_share_the_same_first_beat() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../camelot-source-analysis/tests/fixtures/pulses64-stereo16.flac");
    let config = LexiconConfig {
        add_emergency_loop: false,
        ..Default::default()
    };
    let mut engine = Engine::new(&EngineSettings {
        source_profile_timing: true,
        lexicon_config: Some(config.clone()),
        ..Default::default()
    })
    .unwrap();
    let result = engine.analyze_file(&path).unwrap();
    let timing = result.analysis.tempo.as_ref().unwrap();
    assert_eq!(timing.method, "source-profile-timing-v1");
    let audio = camelot_core::audio::decode(&path, 44100, None).unwrap();
    let expected = generate_structural_cues(
        &audio.samples,
        44100,
        timing.bpm,
        timing.downbeat_time.unwrap(),
        &config,
    )
    .unwrap();
    assert_eq!(result.analysis.lexicon_cues.as_ref().unwrap(), &expected);
    assert_eq!(
        result
            .analysis
            .cues
            .as_ref()
            .unwrap()
            .iter()
            .map(|c| c.time_seconds)
            .collect::<Vec<_>>(),
        expected
            .cues
            .iter()
            .map(|c| c.time_seconds)
            .collect::<Vec<_>>()
    );
}
