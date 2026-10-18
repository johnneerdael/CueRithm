use super::{equal, fixture};
use crate::lexicon_timing::{finish, phase};
use serde_json::Value;
use sha2::{Digest, Sha256};

#[test]
fn original_selected_anchor_to_final_timing_pipeline_matches() {
    let source: Value = serde_json::from_str(include_str!(
        "../../../../../tests/fixtures/lexicon-timing/complete-stage-reference.json"
    ))
    .unwrap();
    let (mut cases, mut changes, mut seeded, mut complete, mut explicit) = (0, 0, 0, 0, 0);
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
        let selected: finish::Candidate = serde_json::from_value(case["selected"].clone()).unwrap();
        let mut state = phase::CorrectionState::default();
        let previous = &case["selected_correction"];
        if !previous.is_null() {
            state.last = Some(phase::AppliedCorrection {
                position: previous["position"].as_f64().unwrap(),
                delta_ms: previous["deltaMs"].as_f64().unwrap(),
                source: match previous["source"].as_str().unwrap() {
                    "sbc" => "sbc",
                    "gridcorr" => "gridcorr",
                    name => panic!("Unknown correction source {name}"),
                },
            });
            seeded += 1;
        }
        let result = finish::finish(&pcm, rate, selected, &mut state);
        equal(
            &serde_json::to_value(result).unwrap(),
            &case["expected"],
            &format!("{} final correction chain", case["name"]),
        );
        if case["selected"]["firstBeat"] != case["expected"]["firstBeat"] {
            changes += 1;
        }
        if case["component_only"].as_bool() == Some(true) {
            explicit += 1;
        } else {
            complete += 1;
        }
        cases += 1;
    }
    assert_eq!(cases, 23);
    assert_eq!((complete, explicit), (15, 8));
    assert!(changes > 0 && seeded > 0);
}
