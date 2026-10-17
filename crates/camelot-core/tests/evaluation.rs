use camelot_core::{
    Cue, MusicalKey,
    evaluation::{Comparison, evaluate},
};

fn cue(time_seconds: f64) -> Cue {
    Cue {
        time_seconds,
        name: "Cue".into(),
    }
}
fn row() -> Comparison {
    Comparison {
        reference_key: MusicalKey::from_camelot("8A").unwrap(),
        reference_bpm: Some(125.0),
        reference_cues: vec![cue(0.0), cue(30.0)],
        predicted_key: Some(MusicalKey::from_camelot("8A").unwrap()),
        predicted_bpm: Some(125.001),
        predicted_cues: Some(vec![cue(0.01), cue(30.02)]),
        failed: false,
    }
}

#[test]
fn failed_and_unknown_predictions_remain_in_denominators() {
    let success = row();
    let failure = Comparison {
        predicted_key: None,
        predicted_bpm: None,
        predicted_cues: None,
        failed: true,
        ..row()
    };
    let unknown = Comparison {
        predicted_key: None,
        predicted_bpm: None,
        predicted_cues: None,
        ..row()
    };
    let score = evaluate(&[success, failure, unknown], 0.05).unwrap();
    assert_eq!(score.tracks, 3);
    assert_eq!(score.failed_tracks, 1);
    assert_eq!(score.exact_key_matches, 1);
    assert_eq!(score.key_agreement, 1.0 / 3.0);
    assert_eq!(score.exact_display_bpm_matches, 1);
    assert_eq!(score.bpm_agreement_within_half, 1.0 / 3.0);
    assert_eq!(score.cues.reference_count, 6);
    assert_eq!(score.cues.matched_count, 2);
}

#[test]
fn one_prediction_cannot_match_multiple_reference_cues() {
    let comparison = Comparison {
        reference_cues: vec![cue(0.0), cue(0.02)],
        predicted_cues: Some(vec![cue(0.01)]),
        ..row()
    };
    let score = evaluate(&[comparison], 0.05).unwrap();
    assert_eq!(score.cues.matched_count, 1);
    assert_eq!(score.cues.predicted_count, 1);
    assert_eq!(score.cues.reference_count, 2);
    assert_eq!(score.cues.recall, 0.5);
}

#[test]
fn optimal_cue_assignment_prefers_smallest_error_at_equal_match_count() {
    let comparison = Comparison {
        reference_cues: vec![cue(0.0), cue(0.04)],
        predicted_cues: Some(vec![cue(0.035)]),
        ..row()
    };
    let score = evaluate(&[comparison], 0.05).unwrap();
    assert_eq!(score.cues.matched_count, 1);
    assert!((score.cues.mean_absolute_error_seconds.unwrap() - 0.005).abs() < 1e-10);
}

#[test]
fn half_tempo_is_reported_as_a_metrical_error_not_exact_agreement() {
    let comparison = Comparison {
        predicted_bpm: Some(62.5),
        ..row()
    };
    let score = evaluate(&[comparison], 0.05).unwrap();
    assert_eq!(score.exact_display_bpm_matches, 0);
    assert_eq!(score.half_double_bpm_matches, 1);
}

#[test]
fn invalid_metrics_input_is_rejected() {
    assert!(evaluate(&[row()], f64::NAN).is_err());
    assert!(
        evaluate(
            &[Comparison {
                predicted_bpm: Some(f64::NAN),
                ..row()
            }],
            0.05
        )
        .is_err()
    );
    assert!(
        evaluate(
            &[Comparison {
                reference_cues: vec![cue(f64::INFINITY)],
                ..row()
            }],
            0.05
        )
        .is_err()
    );
}
