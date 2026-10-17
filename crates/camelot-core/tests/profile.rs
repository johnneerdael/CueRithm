use camelot_core::{
    AnalysisConfig, KeyEstimate, Mode, MusicalKey, TonalFeatures, profile::KeyProfileModel,
};

fn model_json() -> serde_json::Value {
    let mut major = vec![0.0; 24];
    let mut minor = vec![0.0; 24];
    major[0] = 4.0;
    major[4] = 1.0;
    major[7] = 1.0;
    minor[0] = 4.0;
    minor[3] = 1.0;
    minor[7] = 1.0;
    serde_json::json!({"schema_version": 1, "feature_schema": "relative-centered-unit-chroma-bass-v1", "feature_weights": [1.0, 0.0], "coefficients": [major, minor], "intercepts": [0.0, 0.0], "sample_rate": 11025, "key_frame_size": 4096, "key_hop_size": 1024})
}

#[test]
fn profiles_are_equivariant_under_all_twelve_transpositions_in_both_modes() {
    let model = KeyProfileModel::from_json(&model_json().to_string()).unwrap();
    let mut base = KeyEstimate {
        key: MusicalKey::new(0, Mode::Major).unwrap(),
        camelot: "8B".into(),
        margin: 0.0,
        features: Some(TonalFeatures {
            tuning_cents: 0.0,
            chroma: [0.0; 12],
            bass_chroma: [0.0; 12],
        }),
        candidates: Vec::new(),
    };
    for tonic in 0..12 {
        for (mode, third) in [(Mode::Major, 4), (Mode::Minor, 3)] {
            let features = base.features.as_mut().unwrap();
            features.chroma = [0.0; 12];
            features.chroma[tonic] = 1.0;
            features.chroma[(tonic + third) % 12] = 0.6;
            features.chroma[(tonic + 7) % 12] = 0.6;
            model.apply(&mut base).unwrap();
            assert_eq!(base.key, MusicalKey::new(tonic as u8, mode).unwrap());
            assert_eq!(base.candidates.len(), 24);
        }
    }
}

#[test]
fn malformed_models_and_feature_configuration_mismatch_are_rejected() {
    let mut value = model_json();
    value["coefficients"] = serde_json::json!([[1.0], [2.0]]);
    assert!(KeyProfileModel::from_json(&value.to_string()).is_err());
    let mut value = model_json();
    value["feature_schema"] = serde_json::json!("unknown-features");
    assert!(KeyProfileModel::from_json(&value.to_string()).is_err());
    let model = KeyProfileModel::from_json(&model_json().to_string()).unwrap();
    assert!(
        model
            .validate_for(22050, &AnalysisConfig::default())
            .is_err()
    );
    let config = AnalysisConfig {
        key_frame_size: 8192,
        ..Default::default()
    };
    assert!(model.validate_for(11025, &config).is_err());
    assert!(
        model
            .validate_for(11025, &AnalysisConfig::default())
            .is_ok()
    );
}
