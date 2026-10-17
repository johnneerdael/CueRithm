#![cfg(feature = "neural-key")]
use camelot_core::neural_key::{NeuralKeyModel, key_from_scores};

#[test]
fn source_class_order_maps_all_twenty_four_keys_without_a_second_softmax() {
    let expected = [
        "11B", "6B", "1B", "8B", "3B", "10B", "5B", "12B", "7B", "2B", "9B", "4B", "10A", "5A",
        "12A", "7A", "2A", "9A", "4A", "11A", "6A", "1A", "8A", "3A",
    ];
    for (index, expected) in expected.iter().enumerate() {
        let mut scores = [0.0_f32; 24];
        scores[index] = 1.0;
        let estimate = key_from_scores(&scores).unwrap();
        assert_eq!(estimate.camelot, *expected);
        assert_eq!(estimate.candidates.len(), 24);
        assert_eq!(estimate.candidates[0].score, 1.0);
        assert_eq!(estimate.margin, 1.0);
        assert!(estimate.features.is_none());
    }
}

#[test]
fn malformed_scores_and_unrecognized_model_bytes_are_rejected() {
    assert!(key_from_scores(&[0.0; 23]).is_err());
    assert!(key_from_scores(&[0.0; 24]).is_err());
    assert!(key_from_scores(&[f32::NAN; 24]).is_err());
    let mut scores = [0.0; 24];
    scores[0] = 1.5;
    scores[1] = -0.5;
    assert!(key_from_scores(&scores).is_err());
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("wrong.onnx");
    std::fs::write(&path, b"not the pinned key model").unwrap();
    assert!(NeuralKeyModel::load(&path).is_err());
}

#[test]
#[ignore = "Requires explicit pinned S-KEY ONNX file in CAMELOT_TEST_KEY_MODEL"]
fn pinned_model_runs_offline_and_rejects_short_silent_or_invalid_pcm() {
    let mut model = NeuralKeyModel::load(std::path::Path::new(
        &std::env::var("CAMELOT_TEST_KEY_MODEL").unwrap(),
    ))
    .unwrap();
    assert!(model.analyze(&[0.1; 100], 22050).unwrap().is_none());
    assert!(
        model
            .analyze(&vec![0.0; 22050 * 3], 22050)
            .unwrap()
            .is_none()
    );
    assert!(model.analyze(&[f32::NAN], 22050).is_err());
    assert!(model.analyze(&[0.0], 11025).is_err());
    let samples: Vec<f32> = (0..22050 * 3)
        .map(|i| {
            let t = i as f64 / 22050.0;
            (0.2 * ([440.0, 523.251, 659.255]
                .iter()
                .map(|f| (std::f64::consts::TAU * f * t).sin())
                .sum::<f64>())) as f32
        })
        .collect();
    let result = model.analyze(&samples, 22050).unwrap().unwrap();
    assert_eq!(result.candidates.len(), 24);
    assert!(result.features.is_none());
}
