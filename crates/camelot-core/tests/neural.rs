#![cfg(feature = "neural-beats")]

use camelot_core::neural::{NeuralBeatTracker, tempo_from_beats};

#[test]
fn quantized_beat_times_produce_unbiased_tempo_even_with_occasional_missing_beats() {
    let beats: Vec<f64> = (0..1500)
        .filter(|i| i % 97 != 0)
        .map(|i| {
            let true_time = 0.15 + i as f64 * 60.0 / 128.0;
            (true_time / 0.02).round() * 0.02
        })
        .collect();
    let estimate = tempo_from_beats(&beats, &[]).unwrap();
    assert!((estimate.bpm - 128.0).abs() < 0.01, "{}", estimate.bpm);
    assert_eq!(estimate.beats_seconds, beats);
}

#[test]
fn insufficient_or_invalid_beats_do_not_create_a_tempo() {
    assert!(tempo_from_beats(&[0.1, 0.2], &[]).is_none());
    assert!(tempo_from_beats(&[0.0, 1.0, f64::NAN, 3.0, 4.0, 5.0], &[]).is_none());
    assert!(tempo_from_beats(&[0.0, 1.0, 1.0, 2.0, 3.0, 4.0], &[]).is_none());
}

#[test]
fn spurious_extra_detections_do_not_accumulate_tempo_drift() {
    let mut beats = Vec::new();
    for index in 0..1500 {
        if index % 97 == 0 {
            continue;
        }
        let time = 0.15 + index as f64 * 60.0 / 128.0;
        beats.push((time / 0.02).round() * 0.02);
        if index % 53 == 10 {
            beats.push(((time + 0.18 * 60.0 / 128.0) / 0.02).round() * 0.02);
        }
    }
    beats.sort_by(f64::total_cmp);
    let estimate = tempo_from_beats(&beats, &[]).unwrap();
    assert!((estimate.bpm - 128.0).abs() < 0.01, "{}", estimate.bpm);
}

#[test]
fn missing_models_are_an_initialization_error() {
    assert!(
        NeuralBeatTracker::new(
            std::path::Path::new("missing-mel.onnx"),
            std::path::Path::new("missing-beat.onnx")
        )
        .is_err()
    );
}

#[test]
#[ignore = "Requires explicitly acquired Beat This! model files in CAMELOT_TEST_MODELS"]
fn real_models_run_on_local_pcm_without_network_or_reference_labels() {
    let root = std::path::PathBuf::from(
        std::env::var("CAMELOT_TEST_MODELS").expect("Set model directory"),
    );
    let mut tracker = NeuralBeatTracker::new(
        &root.join("mel_spectrogram.onnx"),
        &root.join("beat_this_small.onnx"),
    )
    .unwrap();
    let samples: Vec<f32> = (0..22050 * 8)
        .map(|i| {
            let time = i as f64 / 22050.0;
            let pulse = (-(time % 0.5) / 0.02).exp();
            (0.2 * (std::f64::consts::TAU * 220.0 * time).sin()
                + 0.3 * pulse * (std::f64::consts::TAU * 900.0 * time).sin()) as f32
        })
        .collect();
    let result = tracker.analyze(&samples, 22050).unwrap();
    if let Some(tempo) = result {
        assert!(tempo.bpm.is_finite());
        assert!(
            tempo
                .beats_seconds
                .iter()
                .all(|x| x.is_finite() && *x >= 0.0 && *x <= 8.0)
        );
    }
    assert_eq!(tracker.model_identity().len(), 2);
}
