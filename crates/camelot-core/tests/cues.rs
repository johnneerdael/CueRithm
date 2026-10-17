use camelot_core::{
    TempoEstimate,
    cues::{CueConfig, suggest},
    structure::extract,
};

fn tempo(duration: f64) -> TempoEstimate {
    TempoEstimate {
        bpm: 120.0,
        strength: Some(1.0),
        downbeat_time: None,
        candidates: vec![],
        beats_seconds: (0..(duration * 2.0) as usize)
            .map(|i| i as f64 / 2.0)
            .collect(),
        downbeats_seconds: Some(
            (0..(duration / 2.0) as usize)
                .map(|i| i as f64 * 2.0)
                .collect(),
        ),
        method: "generated-known-phase".into(),
    }
}

#[test]
fn spectral_section_changes_generate_bounded_chronological_cue_candidates() {
    let rate = 11025;
    let samples: Vec<f32> = (0..rate * 40)
        .map(|i| {
            let t = i as f64 / rate as f64;
            let frequency = if t < 16.0 { 80.0 } else { 1600.0 };
            (0.25 * (std::f64::consts::TAU * frequency * t).sin()) as f32
        })
        .collect();
    let structure = extract(&samples, rate).unwrap();
    let config = CueConfig {
        max_cues: 2,
        ..Default::default()
    };
    let mut beats = tempo(40.0);
    beats.beats_seconds[0] = 0.02; // Model quantization leaves the section 32 musical beats away.
    let cues = suggest(&structure, &beats, &config).unwrap();
    assert_eq!(cues.len(), 2);
    assert_eq!(cues[0].time_seconds, 0.02);
    assert!((cues[1].time_seconds - 16.0).abs() < 0.01);
    assert!(cues.iter().all(|c| c.name == "Candidate"));
}

#[test]
fn invalid_inputs_and_unknown_phase_do_not_create_cues() {
    let structure = extract(&vec![0.0; 11025], 11025).unwrap();
    let mut beats = tempo(1.0);
    assert!(
        suggest(&structure, &beats, &CueConfig::default())
            .unwrap()
            .is_empty()
    );
    beats.beats_seconds.clear();
    assert!(
        suggest(&structure, &beats, &CueConfig::default())
            .unwrap()
            .is_empty()
    );
    beats.beats_seconds = vec![0.0, f64::NAN];
    assert!(suggest(&structure, &beats, &CueConfig::default()).is_err());
    beats = tempo(1.0);
    let invalid = CueConfig {
        max_cues: 0,
        ..Default::default()
    };
    assert!(suggest(&structure, &beats, &invalid).is_err());
    let mut invalid = structure;
    invalid.frames[0].rms_db = f64::NAN;
    assert!(suggest(&invalid, &beats, &CueConfig::default()).is_err());
}

#[test]
fn single_cue_limit_and_end_of_audio_are_respected() {
    let structure = extract(&vec![0.1; 11025 * 3], 11025).unwrap();
    let mut beats = tempo(4.0); // Some detections extend past the decoded audio.
    beats.beats_seconds.insert(0, -0.5);
    let config = CueConfig {
        max_cues: 1,
        ..Default::default()
    };
    let cues = suggest(&structure, &beats, &config).unwrap();
    assert_eq!(cues.len(), 1);
    assert_eq!(cues[0].time_seconds, 0.0);
}
