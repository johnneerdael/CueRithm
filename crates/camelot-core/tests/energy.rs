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

fn window(start: f64, features: f64) -> camelot_source_analysis::EnergySegment {
    camelot_source_analysis::EnergySegment {
        start_time: start,
        end_time: start + 10.,
        volume_rms_db: -20.,
        features: vec![features; 183],
    }
}

#[test]
fn overall_level_is_the_80th_percentile_of_window_levels_not_their_duration_mode() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("model.json");
    std::fs::write(&path, model().to_string()).unwrap();
    let m = EnergyModel::load(&path).unwrap();
    // Window levels 6,6,6,6,8: the duration mode is 6, the 80th percentile (linear interpolation, position 3.2)
    // is 6 + 0.2 * (8 - 6) = 6.4, which rounds to 6; five windows of 8 after four of 6 move it to 7.4 -> 7.
    let mostly_six: Vec<_> = (0..4)
        .map(|i| window(i as f64 * 10., 1.))
        .chain([window(40., 2.)])
        .collect();
    assert_eq!(m.estimate_overall(&mostly_six).unwrap().unwrap().level, 6);
    let mut four_six_five_eight: Vec<_> = (0..4).map(|i| window(i as f64 * 10., 1.)).collect();
    four_six_five_eight.extend((4..9).map(|i| window(i as f64 * 10., 2.)));
    // levels sorted: 6,6,6,6,8,8,8,8,8; position 6.4 is between 8 and 8 -> 8.
    assert_eq!(
        m.estimate_overall(&four_six_five_eight)
            .unwrap()
            .unwrap()
            .level,
        8
    );
    // Ten windows, two of them 8: position 7.2 is 6 + 0.2 * (8 - 6) = 6.4 -> 6 (the mode is also 6).
    let two_high: Vec<_> = (0..8)
        .map(|i| window(i as f64 * 10., 1.))
        .chain((8..10).map(|i| window(i as f64 * 10., 2.)))
        .collect();
    assert_eq!(m.estimate_overall(&two_high).unwrap().unwrap().level, 6);
    // Three of ten are 8: position 7.2 falls between sorted values 8 (index 7) and 8 (index 8) -> 8, where the mode stays 6.
    let three_high: Vec<_> = (0..7)
        .map(|i| window(i as f64 * 10., 1.))
        .chain((7..10).map(|i| window(i as f64 * 10., 2.)))
        .collect();
    assert_eq!(m.estimate(&three_high).unwrap().unwrap().level, 6);
    assert_eq!(m.estimate_overall(&three_high).unwrap().unwrap().level, 8);
    assert_eq!(
        m.estimate_overall(&three_high).unwrap().unwrap().method,
        "svc-percentile80-v1"
    );
    assert!(m.estimate_overall(&[]).unwrap().is_none());
}
