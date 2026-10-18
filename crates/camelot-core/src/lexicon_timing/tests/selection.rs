use super::{equal, fixture};
use crate::lexicon_timing::{LexiconTempoOptions, phase, selection};
use serde_json::Value;
use sha2::{Digest, Sha256};

#[test]
fn original_audio_to_selected_anchor_and_correction_state_match() {
    let cases: Vec<Value> = [
        include_str!("../../../../../tests/fixtures/lexicon-timing/complete-stage-reference.json"),
        include_str!("../../../../../tests/fixtures/lexicon-timing/complete-offset-reference.json"),
    ]
    .into_iter()
    .flat_map(|text| {
        let source: Value = serde_json::from_str(text).unwrap();
        source["cases"].as_array().unwrap().clone()
    })
    .collect();
    let mut consumed = 0;
    for case in &cases {
        if case["component_only"].as_bool() == Some(true) {
            continue;
        }
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
        let options: LexiconTempoOptions = serde_json::from_value(case["options"].clone()).unwrap();
        // A preceding invocation must not leak its correction bookkeeping.
        let mut state = phase::CorrectionState::default();
        state.note_grid(99., 99.);
        let result = selection::select(&pcm, rate, &options, &mut state)
            .unwrap()
            .unwrap();
        equal(
            &serde_json::to_value(result).unwrap(),
            &case["selected"],
            &format!("{} initial selection", case["name"]),
        );
        equal(
            &serde_json::to_value(state.last).unwrap(),
            &case["selected_correction"],
            &format!("{} selection bookkeeping", case["name"]),
        );
        consumed += 1;
    }
    assert_eq!(consumed, 62);
}

#[test]
fn original_selection_branches_with_explicit_tempo_evidence_match() {
    let source: Value = serde_json::from_str(include_str!(
        "../../../../../tests/fixtures/lexicon-timing/selection-component-reference.json"
    ))
    .unwrap();
    let mut consumed = 0;
    let mut branches = std::collections::BTreeSet::new();
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
        let evidence = crate::lexicon_timing::TempoEvidence {
            bpm: case["input_tempo"]["tempo"].as_f64().unwrap(),
            confidence: case["input_tempo"]["confidence"].as_f64().unwrap(),
            peaks: serde_json::from_value(case["input_tempo"]["peaks"].clone()).unwrap(),
        };
        let mut state = phase::CorrectionState::default();
        let actual = selection::from_tempo(&pcm, rate, evidence, &mut state).unwrap();
        equal(
            &serde_json::to_value(actual).unwrap(),
            &case["selected"],
            &format!("{} selection component", case["name"]),
        );
        equal(
            &serde_json::to_value(state.last).unwrap(),
            &case["selected_correction"],
            &format!("{} component bookkeeping", case["name"]),
        );
        branches.insert(case["branch"].as_u64().unwrap());
        consumed += 1;
    }
    assert_eq!(consumed, 291);
    assert_eq!(branches, (0..20).collect());
}
