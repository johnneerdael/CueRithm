use super::{equal, fixture};
use crate::lexicon_timing::{intro::IntroKind, walkback};
use serde_json::Value;
use sha2::{Digest, Sha256};

#[test]
fn original_first_beat_walkback_results_match_with_optional_strength_and_intro_kind() {
    let source: Value = serde_json::from_str(include_str!(
        "../../../../../tests/fixtures/lexicon-timing/walkback-reference.json"
    ))
    .unwrap();
    let mut consumed = 0;
    let mut returned_earlier = 0;
    let mut half_steps = 0;
    for case in source["cases"].as_array().unwrap() {
        let rate = case["sample_rate"].as_u64().unwrap() as u32;
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
            let anchor = args[0].as_f64().unwrap();
            let period = args[1].as_f64().unwrap();
            let peaks: Vec<f64> = serde_json::from_value(args[2].clone()).unwrap();
            let kind: IntroKind = serde_json::from_value(args[4].clone()).unwrap();
            let actual =
                walkback::walk_back(&pcm, rate, anchor, period, &peaks, args[3].as_f64(), kind);
            equal(
                &serde_json::to_value(actual).unwrap(),
                &row["result"],
                &format!("{} walkback {args:?}", case["name"]),
            );
            if row["result"]["position"].as_f64().unwrap() < anchor {
                returned_earlier += 1;
            }
            if row["result"]["beatsWalkedBack"].as_f64().unwrap().fract() == 0.5 {
                half_steps += 1;
            }
            consumed += 1;
        }
    }
    assert_eq!(consumed, 1224);
    assert!(returned_earlier > 0 && half_steps > 0);
}

#[test]
fn first_beat_walkback_bounds_invalid_inputs_without_panicking_or_looping() {
    for (rate, anchor, period) in [
        (0, 1., 0.5),
        (44100, 0., 0.),
        (44100, 0., f64::NAN),
        (44100, f64::NAN, 0.5),
    ] {
        let result = walkback::walk_back(&[], rate, anchor, period, &[], None, IntroKind::None);
        assert_eq!(result.beats_walked_back, 0.);
    }
}
