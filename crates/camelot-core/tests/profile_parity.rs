use camelot_core::{KeyEstimate, profile::KeyProfileModel};
use std::{collections::HashMap, path::PathBuf};

#[test]
#[ignore = "Requires explicit private key model, frozen tuning features and independent NumPy outputs"]
fn rust_profile_inference_matches_independent_training_runtime() {
    let model = KeyProfileModel::load(&PathBuf::from(
        std::env::var("CAMELOT_KEY_MODEL").expect("Set trained profile model"),
    ))
    .unwrap();
    let predictions = std::fs::read_to_string(
        std::env::var("CAMELOT_KEY_PREDICTIONS").expect("Set tuning predictions"),
    )
    .unwrap();
    let golden: serde_json::Value = serde_json::from_slice(
        &std::fs::read(std::env::var("CAMELOT_KEY_GOLDEN").expect("Set NumPy inference output"))
            .unwrap(),
    )
    .unwrap();
    assert_eq!(golden["model_sha256"], model.source_sha256());
    let by_id: HashMap<String, serde_json::Value> = golden["rows"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| (v["id"].as_str().unwrap().to_string(), v.clone()))
        .collect();
    let mut count = 0;
    for line in predictions.lines() {
        let row: serde_json::Value = serde_json::from_str(line).unwrap();
        assert_eq!(row["split"], "tuning");
        if row["result"].is_null() {
            continue;
        }
        let id = row["id"].as_str().unwrap();
        let mut estimate: KeyEstimate =
            serde_json::from_value(row["result"]["analysis"]["key"].clone()).unwrap();
        model.apply(&mut estimate).unwrap();
        assert_eq!(estimate.camelot, by_id[id]["camelot"], "{id}");
        let expected_score = by_id[id]["score"].as_f64().unwrap();
        assert!(
            (estimate.candidates[0].score - expected_score).abs() < 1e-9,
            "{id}"
        );
        count += 1;
    }
    assert_eq!(count, by_id.len());
    assert_eq!(
        count, 277,
        "Frozen baseline tuning-feature regression corpus"
    );
}
