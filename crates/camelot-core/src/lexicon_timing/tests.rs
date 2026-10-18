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
