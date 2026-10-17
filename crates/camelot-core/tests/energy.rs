use camelot_core::energy::EnergyModel;

fn model() -> serde_json::Value {
    serde_json::json!({"gamma":0.1,"classes":[4,6,8],"n_support":[1,1,1],
        "support_vectors":[vec![0.0;183],vec![1.0;183],vec![2.0;183]],
        "dual_coef":[[1.0,-1.0,-1.0],[1.0,1.0,-1.0]],"intercept":[0.0,0.0,0.0]})
}

#[test]
fn numeric_svc_and_duration_mode_do_not_expose_window_labels() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("model.json");
    std::fs::write(&path, model().to_string()).unwrap();
    let m = EnergyModel::load(&path).unwrap();
    assert_eq!(m.predict(&vec![0.0; 183]).unwrap(), 4);
    assert_eq!(m.predict(&vec![1.0; 183]).unwrap(), 6);
    assert_eq!(m.predict(&vec![2.0; 183]).unwrap(), 8);
    let windows = [
        camelot_source_analysis::EnergySegment {
            start_time: 0.,
            end_time: 10.,
            volume_rms_db: -20.,
            features: vec![0.; 183],
        },
        camelot_source_analysis::EnergySegment {
            start_time: 10.,
            end_time: 20.,
            volume_rms_db: -10.,
            features: vec![2.; 183],
        },
    ];
    let rating = m.estimate(&windows).unwrap().unwrap();
    assert_eq!(rating.level, 8); // equal durations: higher energy wins
    let json = serde_json::to_value(rating).unwrap();
    assert!(json.get("segments").is_none());
    assert!(json.get("window_levels").is_none());
    assert!(m.estimate(&[]).unwrap().is_none());
}

#[test]
fn invalid_models_and_nonfinite_features_are_errors() {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path().join("bad.json");
    for bad in [
        serde_json::json!({}),
        {
            let mut x = model();
            x["n_support"] = serde_json::json!([2, 1, 1]);
            x
        },
        {
            let mut x = model();
            x["gamma"] = serde_json::json!(-1.);
            x
        },
    ] {
        std::fs::write(&p, bad.to_string()).unwrap();
        assert!(EnergyModel::load(&p).is_err());
    }
    std::fs::write(&p, model().to_string()).unwrap();
    let m = EnergyModel::load(&p).unwrap();
    assert!(m.predict(&[1.]).is_err());
    assert!(m.predict(&vec![f64::NAN; 183]).is_err());
}

#[test]
#[ignore = "requires explicit private energy SVC and numeric golden inputs"]
fn trained_svc_matches_explicit_numeric_golden() {
    let model = EnergyModel::load(std::path::Path::new(
        &std::env::var_os("CAMELOT_ENERGY_MODEL").expect("model path"),
    ))
    .unwrap();
    let text =
        std::fs::read_to_string(std::env::var_os("CAMELOT_ENERGY_GOLDEN").expect("golden path"))
            .unwrap();
    let cases: Vec<serde_json::Value> = serde_json::from_str(&text).unwrap();
    assert!(!cases.is_empty());
    for (i, case) in cases.iter().enumerate() {
        let features: Vec<f64> = serde_json::from_value(case["features"].clone()).unwrap();
        assert_eq!(
            model.predict(&features).unwrap(),
            case["level"].as_u64().unwrap() as u8,
            "case {i}"
        );
    }
    eprintln!(
        "{} independently generated SVC decisions match",
        cases.len()
    );
}
