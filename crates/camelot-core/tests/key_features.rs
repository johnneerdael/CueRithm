use camelot_core::KeyEstimate;

#[test]
fn legacy_dsp_features_round_trip_and_neural_results_can_omit_them() {
    let mut json = serde_json::json!({"key":{"tonic":0,"mode":"major"},"camelot":"8B",
        "margin":0.2,"candidates":[],"tuning_cents":0.0,"chroma":vec![0.0;12],"bass_chroma":vec![0.0;12]});
    let old: KeyEstimate = serde_json::from_value(json.clone()).unwrap();
    assert!(old.features.is_some());
    assert_eq!(serde_json::to_value(old).unwrap(), json);
    for field in ["chroma", "bass_chroma", "tuning_cents"] {
        json.as_object_mut().unwrap().remove(field);
    }
    let neural: KeyEstimate = serde_json::from_value(json.clone()).unwrap();
    assert!(neural.features.is_none());
    assert_eq!(serde_json::to_value(neural).unwrap(), json);
    json["chroma"] = serde_json::json!(vec![0.0; 12]);
    assert!(
        serde_json::from_value::<KeyEstimate>(json.clone()).is_err(),
        "Partial diagnostics must not silently disappear"
    );
    json["chroma"] = serde_json::json!("broken");
    assert!(serde_json::from_value::<KeyEstimate>(json).is_err());
}
