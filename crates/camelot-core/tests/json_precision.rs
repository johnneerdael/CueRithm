use camelot_core::{Analysis, AnalysisConfig, Cue, analyze_pcm};

// Generated finite values, unrelated to music-library reference labels.
const CUE_TIME_BITS: u64 = 4_642_300_688_593_338_695;

#[test]
fn cue_json_round_trip_preserves_fractional_seconds_exactly() {
    let original = [
        Cue {
            time_seconds: f64::from_bits(CUE_TIME_BITS),
            name: "Candidate".into(),
        },
        Cue {
            time_seconds: -f64::from_bits(CUE_TIME_BITS),
            name: "Candidate".into(),
        },
        Cue {
            time_seconds: f64::from_bits(CUE_TIME_BITS),
            name: "Candidate".into(),
        },
    ];
    let json = serde_json::to_vec(&original).unwrap();
    let restored: Vec<Cue> = serde_json::from_slice(&json).unwrap();
    assert_eq!(restored.len(), original.len());
    for (before, after) in original.iter().zip(&restored) {
        assert_eq!(after.time_seconds.to_bits(), before.time_seconds.to_bits());
        assert_eq!(after.name, before.name);
    }
}

#[test]
fn loading_saved_analysis_retains_fractional_measurement_bits() {
    let mut original = analyze_pcm(&[], 44100, &AnalysisConfig::default()).unwrap();
    original.duration_seconds = f64::from_bits(CUE_TIME_BITS);
    original.cues = Some(vec![Cue {
        time_seconds: f64::from_bits(CUE_TIME_BITS),
        name: "Candidate".into(),
    }]);
    let json = serde_json::to_vec(&original).unwrap();
    let restored: Analysis = serde_json::from_reader(json.as_slice()).unwrap();
    assert_eq!(restored.duration_seconds.to_bits(), CUE_TIME_BITS);
    assert_eq!(
        restored.cues.unwrap()[0].time_seconds.to_bits(),
        CUE_TIME_BITS
    );
}
