use super::onset;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

#[path = "../../tests/support/lexicon_pcm.rs"]
mod fixture;

fn equal(actual: &Value, expected: &Value, context: &str) {
    match (actual, expected) {
        (Value::Number(a), Value::Number(b)) => {
            assert!(
                (a.as_f64().unwrap() - b.as_f64().unwrap()).abs() <= 1e-12,
                "{context}: {actual} vs {expected}"
            );
        }
        (Value::Object(a), Value::Object(b)) => {
            assert_eq!(a.len(), b.len(), "{context}");
            for (key, value) in b {
                equal(&a[key], value, &format!("{context}.{key}"));
            }
        }
        (Value::Array(a), Value::Array(b)) => {
            assert_eq!(a.len(), b.len(), "{context}");
            for (index, (actual, expected)) in a.iter().zip(b).enumerate() {
                equal(actual, expected, &format!("{context}[{index}]"));
            }
        }
        _ => assert_eq!(actual, expected, "{context}"),
    }
}
#[test]
fn all_original_onset_attack_and_snapping_references_match() {
    let source: Value = serde_json::from_str(include_str!(
        "../../../../tests/fixtures/lexicon-timing/onset-reference.json"
    ))
    .unwrap();
    let rate = source["source"]["sample_rate"].as_u64().unwrap() as u32;
    let mut consumed = 0;
    for case in source["cases"].as_array().unwrap() {
        let recipe: fixture::Recipe = serde_json::from_value(case["recipe"].clone()).unwrap();
        let pcm = fixture::signal(&recipe, rate);
        let mut hash = Sha256::new();
        for sample in &pcm {
            hash.update(sample.to_le_bytes());
        }
        assert_eq!(
            format!("{:x}", hash.finalize()),
            case["pcm_sha256"].as_str().unwrap()
        );
        for row in case["rows"].as_array().unwrap() {
            let args = row["args"].as_array().unwrap();
            let number = |i: usize| args[i].as_f64().unwrap();
            let helper = row["helper"].as_str().unwrap();
            let actual = match helper {
                "computeAttackRatio" => {
                    json!(onset::attack_ratio(&pcm, rate, number(0), number(1)))
                }
                "findFirstAudioPosition" => {
                    serde_json::to_value(onset::first_audio(&pcm, rate, number(0))).unwrap()
                }
                "findDynamicOnset" => json!(onset::dynamic_onset(&pcm, rate, number(0), None)),
                "refineDropPosition" => {
                    serde_json::to_value(onset::refine_drop(&pcm, rate, number(0), number(1)))
                        .unwrap()
                }
                "findPreciseDropOnset" => {
                    json!(onset::precise_onset(&pcm, rate, number(0), number(1)))
                }
                "snapToGrid" => {
                    let peaks: Vec<f64> = serde_json::from_value(args[1].clone()).unwrap();
                    json!(onset::snap(number(0), &peaks, number(2), number(3)))
                }
                _ => panic!("Unknown original helper {helper}"),
            };
            equal(
                &actual,
                &row["result"],
                &format!("{} {helper} {args:?}", case["name"]),
            );
            consumed += 1;
        }
    }
    assert_eq!(consumed, 306);
}
#[test]
fn onset_searches_handle_edges_and_reject_invalid_coordinates_without_panics_or_loops() {
    let empty = [];
    assert_eq!(onset::attack_ratio(&empty, 44100, 0., 15.), 1.);
    assert_eq!(onset::first_audio(&empty, 44100, 10.).audio_start, 0.);
    for at in [-1., f64::NAN, f64::INFINITY, 2.] {
        let pcm = vec![0.; 44100];
        assert!(onset::attack_ratio(&pcm, 44100, at, 15.).is_nan());
        assert!(onset::dynamic_onset(&pcm, 44100, at, None).is_nan());
        assert!(onset::refine_drop(&pcm, 44100, at, 0.03).position.is_nan());
        assert!(onset::precise_onset(&pcm, 44100, at, 0.05).is_nan());
    }
    assert!(onset::dynamic_onset(&[1.; 10], 0, 0., None).is_nan());
    assert!(onset::precise_onset(&[1.; 10], 0, 0., 0.05).is_nan());
    assert!(onset::attack_ratio(&[1.; 10], 44100, 0., 0.).is_nan());
    assert!(onset::snap(0., &[], 0., 0.015).is_nan());
    assert!(onset::snap(f64::NAN, &[], 0.5, 0.015).is_nan());
    // Nearest-beat snapping deliberately follows Math.round at negative ties.
    assert_eq!(onset::snap(-0.25, &[0.], 0.5, 1.), 0.);
}

#[test]
fn original_dp_envelopes_chains_and_applied_phase_corrections_match() {
    let reference: Value = serde_json::from_str(include_str!(
        "../../../../tests/fixtures/lexicon-timing/dp-reference.json"
    ))
    .unwrap();
    let rate = reference["source"]["sample_rate"].as_u64().unwrap() as u32;
    let (mut cases, mut frames, mut guards) = (0, 0, 0);
    for case in reference["cases"].as_array().unwrap() {
        let recipe: fixture::Recipe = serde_json::from_value(case["recipe"].clone()).unwrap();
        let pcm = fixture::signal(&recipe, rate);
        let mut hash = Sha256::new();
        for sample in &pcm {
            hash.update(sample.to_le_bytes());
        }
        assert_eq!(
            format!("{:x}", hash.finalize()),
            case["pcm_sha256"].as_str().unwrap()
        );
        let actual = super::dp::envelope(&pcm, rate);
        let expected: Vec<f32> =
            serde_json::from_value(case["envelope"]["values"].clone()).unwrap();
        assert_eq!(
            actual.hop_seconds,
            case["envelope"]["hop_seconds"].as_f64().unwrap()
        );
        assert_eq!(actual.values.len(), expected.len());
        for (frame, (actual, expected)) in actual.values.iter().zip(&expected).enumerate() {
            assert_eq!(
                actual.to_bits(),
                expected.to_bits(),
                "{} envelope frame {frame}: {actual} vs {expected}",
                case["name"]
            );
        }
        frames += expected.len();
        let bpm = case["bpm"].as_f64().unwrap();
        let chain = super::dp::track(&actual, bpm, 300.);
        equal(
            &serde_json::to_value(chain).unwrap(),
            &case["chain"],
            &format!("{} DP chain", case["name"]),
        );
        let correction =
            super::dp::correct_phase(&pcm, rate, case["anchor"].as_f64().unwrap(), bpm);
        equal(
            &serde_json::to_value(correction).unwrap(),
            &case["correction"],
            &format!("{} DP phase", case["name"]),
        );
        for guard in case["guards"].as_array().unwrap() {
            let correction =
                super::dp::correct_phase(&pcm, rate, guard["anchor"].as_f64().unwrap(), bpm);
            equal(
                &serde_json::to_value(correction).unwrap(),
                &guard["result"],
                &format!("{} DP unchanged anchor {}", case["name"], guard["anchor"]),
            );
            guards += 1;
        }
        cases += 1;
    }
    assert_eq!((cases, frames, guards), (8, 88168, 16));
}

#[test]
fn dp_short_silent_invalid_and_already_early_anchors_do_not_fabricate_a_correction() {
    for pcm in [vec![], vec![0.; 511], vec![0.; 8192]] {
        let envelope = super::dp::envelope(&pcm, 44100);
        assert!(envelope.values.iter().all(|v| *v == 0.));
        assert!(super::dp::track(&envelope, 128., 300.).beats.is_empty());
        let corrected = super::dp::correct_phase(&pcm, 44100, 0.05, 128.);
        assert!(!corrected.applied);
        assert_eq!(corrected.position, 0.05);
    }
    let empty = super::dp::envelope(&[], 0);
    assert!(empty.values.is_empty());
    for bpm in [0., -1., f64::NAN, f64::INFINITY] {
        assert!(super::dp::track(&empty, bpm, 300.).beats.is_empty());
    }
    assert!(!super::dp::correct_phase(&[], 0, 0.2, 128.).applied);
}

#[test]
fn original_phase_corrections_and_tracking_match() {
    use super::phase;
    let mut consumed = 0;
    let mut changed = std::collections::BTreeSet::new();
    for text in [
        include_str!("../../../../tests/fixtures/lexicon-timing/phase-reference.json"),
        include_str!("../../../../tests/fixtures/lexicon-timing/phase-authored-reference.json"),
        include_str!("../../../../tests/fixtures/lexicon-timing/phase-edge-reference.json"),
    ] {
        let source: Value = serde_json::from_str(text).unwrap();
        let rate = source["source"]["sample_rate"].as_u64().unwrap() as u32;
        for case in source["cases"].as_array().unwrap() {
            let recipe: fixture::Recipe = serde_json::from_value(case["recipe"].clone()).unwrap();
            let pcm = fixture::signal(&recipe, rate);
            let mut hash = Sha256::new();
            for sample in &pcm {
                hash.update(sample.to_le_bytes());
            }
            assert_eq!(
                format!("{:x}", hash.finalize()),
                case["pcm_sha256"].as_str().unwrap()
            );
            for row in case["rows"].as_array().unwrap() {
                let name = row["helper"].as_str().unwrap();
                let args = row["args"].as_array().unwrap();
                let n = |i: usize| args[i].as_f64().unwrap();
                let peaks = || serde_json::from_value::<Vec<f64>>(args[2].clone()).unwrap();
                let mut state = phase::CorrectionState::default();
                let actual = match name {
                    "correctFirstBeatNearZero" => {
                        serde_json::to_value(phase::near_zero(n(0))).unwrap()
                    }
                    "correctFirstBeatFromSubsequentBeats" => {
                        let options = args
                            .get(3)
                            .map(|v| serde_json::from_value(v.clone()).unwrap())
                            .unwrap_or_default();
                        serde_json::to_value(phase::subsequent(
                            &pcm,
                            rate,
                            n(0),
                            n(1),
                            &options,
                            &mut state,
                        ))
                        .unwrap()
                    }
                    "calibrateOnsetFromLaterBeats" => serde_json::to_value(phase::calibrate_onset(
                        &pcm,
                        rate,
                        n(0),
                        n(1),
                        &peaks(),
                    ))
                    .unwrap(),
                    "calibrateFirstBeatFromDrop" => serde_json::to_value(phase::calibrate_drop(
                        &pcm,
                        rate,
                        n(0),
                        n(1),
                        &peaks(),
                    ))
                    .unwrap(),
                    "verifyPhaseFromDrop" => {
                        serde_json::to_value(phase::verify_drop(&pcm, rate, n(0), n(1), &peaks()))
                            .unwrap()
                    }
                    "findGridAlignmentOffset" => serde_json::to_value(phase::align(
                        &pcm,
                        rate,
                        n(0),
                        n(1),
                        n(2) as usize,
                        n(3) as usize,
                        false,
                    ))
                    .unwrap(),
                    "findGridAlignmentOffsetAREnergy" => serde_json::to_value(phase::align(
                        &pcm,
                        rate,
                        n(0),
                        n(1),
                        n(2) as usize,
                        n(3) as usize,
                        true,
                    ))
                    .unwrap(),
                    "correctHalfBeatPhaseDirectScan" => {
                        serde_json::to_value(phase::half_beat(&pcm, rate, n(0), n(1))).unwrap()
                    }
                    "rescueSilentFirstBeat" => {
                        serde_json::to_value(phase::rescue_silent(&pcm, rate, n(0), n(1))).unwrap()
                    }
                    "rescueWeakGridPhase" => {
                        serde_json::to_value(phase::rescue_weak(&pcm, rate, n(0), n(1))).unwrap()
                    }
                    "validateBeatGrid" => {
                        serde_json::to_value(phase::validate_grid(&pcm, rate, n(0), n(1))).unwrap()
                    }
                    "validateRiserPosition" => serde_json::to_value(phase::validate_riser(
                        &pcm,
                        rate,
                        n(0),
                        n(1),
                        &peaks(),
                    ))
                    .unwrap(),
                    _ => panic!("Unhandled phase reference {name}"),
                };
                equal(
                    &actual,
                    &row["result"],
                    &format!("{} {name} {args:?}", case["name"]),
                );
                equal(
                    &serde_json::to_value(state.last).unwrap(),
                    &row["tracking"],
                    &format!("{} {name} tracking", case["name"]),
                );
                if [
                    "applied",
                    "appliedCorrection",
                    "usedCalibration",
                    "usedCorrection",
                ]
                .iter()
                .any(|key| row["result"][key].as_bool() == Some(true))
                    || row["result"]["skippedBeats"]
                        .as_u64()
                        .is_some_and(|v| v > 0)
                {
                    changed.insert(name.to_owned());
                }
                if name == "correctFirstBeatFromSubsequentBeats"
                    && row["result"]["appliedCorrection"].as_bool() == Some(true)
                    && row["tracking"].is_null()
                {
                    changed.insert("onset-refinement-fallback".into());
                }
                if let Some(strategy) = row["result"]["strategy"].as_str() {
                    changed.insert(format!("{name}:{strategy}"));
                }
                consumed += 1;
            }
        }
    }
    assert_eq!(consumed, 790);
    for name in [
        "onset-refinement-fallback",
        "correctFirstBeatFromSubsequentBeats",
        "calibrateOnsetFromLaterBeats",
        "calibrateFirstBeatFromDrop",
        "verifyPhaseFromDrop",
        "correctHalfBeatPhaseDirectScan",
        "rescueSilentFirstBeat",
        "rescueWeakGridPhase",
        "validateBeatGrid",
        "validateRiserPosition:A",
        "validateRiserPosition:B",
        "findGridAlignmentOffset",
        "findGridAlignmentOffsetAREnergy",
    ] {
        assert!(
            changed.contains(name),
            "Missing positive source branch {name}"
        );
    }
}

#[test]
fn source_rounding_preserves_negative_ties_and_values_just_below_half() {
    for (value, expected) in [
        (-187.5_f64, -187_f64),
        (-0.5, -0.),
        (-1.5, -1.),
        (0.49999999999999994, 0.),
        (0.5, 1.),
    ] {
        assert_eq!(onset::js_round(value).to_bits(), expected.to_bits());
    }
}

#[test]
fn correction_state_is_per_analysis_and_noop_calls_do_not_erase_prior_corrections() {
    use super::phase;
    let source: Value = serde_json::from_str(include_str!(
        "../../../../tests/fixtures/lexicon-timing/phase-reference.json"
    ))
    .unwrap();
    let case = source["cases"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| {
            c["rows"]
                .as_array()
                .unwrap()
                .iter()
                .any(|r| r["tracking"]["source"] == "sbc")
        })
        .unwrap();
    let row = case["rows"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["tracking"]["source"] == "sbc")
        .unwrap();
    let recipe: fixture::Recipe = serde_json::from_value(case["recipe"].clone()).unwrap();
    let pcm = fixture::signal(&recipe, 44100);
    let args = row["args"].as_array().unwrap();
    let options = args
        .get(3)
        .map(|v| serde_json::from_value(v.clone()).unwrap())
        .unwrap_or_default();
    let (mut first, mut second) = (
        phase::CorrectionState::default(),
        phase::CorrectionState::default(),
    );
    let result = phase::subsequent(
        &pcm,
        44100,
        args[0].as_f64().unwrap(),
        args[1].as_f64().unwrap(),
        &options,
        &mut first,
    );
    assert!(result.applied_correction);
    equal(
        &serde_json::to_value(&first.last).unwrap(),
        &row["tracking"],
        "per-analysis correction",
    );
    assert!(second.last.is_none());
    let previous = serde_json::to_value(&first.last).unwrap();
    assert!(
        !phase::subsequent(&[], 44100, 0., 128., &Default::default(), &mut first)
            .applied_correction
    );
    assert_eq!(serde_json::to_value(&first.last).unwrap(), previous);
    second.note_grid(0.25, -20.);
    assert_eq!(second.last.as_ref().unwrap().source, "gridcorr");
    assert_eq!(serde_json::to_value(&first.last).unwrap(), previous);
    first.reset();
    assert!(first.last.is_none());
    assert!(second.last.is_some());
}

#[test]
fn phase_guards_do_not_turn_empty_or_invalid_coordinates_into_new_anchors() {
    use super::phase;
    let pcm = vec![0.; 44100];
    for (rate, anchor, bpm) in [
        (0, 0.2, 128.),
        (44100, 2., 128.),
        (44100, 0.2, 0.),
        (44100, 0.2, f64::NAN),
    ] {
        let mut state = phase::CorrectionState::default();
        assert!(
            !phase::subsequent(&pcm, rate, anchor, bpm, &Default::default(), &mut state)
                .applied_correction
        );
        assert!(state.last.is_none());
        assert!(!phase::calibrate_drop(&pcm, rate, anchor, bpm, &[]).used_calibration);
        assert!(!phase::verify_drop(&pcm, rate, anchor, bpm, &[]).used_correction);
        assert!(!phase::half_beat(&pcm, rate, anchor, bpm).used_correction);
        assert!(!phase::rescue_silent(&pcm, rate, anchor, bpm).applied);
        assert!(!phase::rescue_weak(&pcm, rate, anchor, bpm).applied);
        assert!(!phase::validate_riser(&pcm, rate, anchor, bpm, &[]).was_in_riser);
        assert!(!phase::align(&pcm, rate, anchor, bpm, 4, 80, false).applied_correction);
        assert_eq!(
            phase::validate_grid(&pcm, rate, anchor, bpm).skipped_beats,
            0
        );
    }
    assert!(!phase::calibrate_onset(&[], 44100, 0., 0., &[]).used_calibration);
    assert!(!phase::rescue_silent(&[], 44100, 0., 128.).applied);
    assert!(!phase::rescue_weak(&[], 44100, 0., 128.).applied);
}

mod anchors;

mod walkback;

mod finish;
mod selection;
