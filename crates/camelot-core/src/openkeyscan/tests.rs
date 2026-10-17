use super::*;

#[test]
fn all_twenty_four_classes_follow_the_pinned_camelot_order() {
    let expected = [
        "1A", "2A", "3A", "4A", "5A", "6A", "7A", "8A", "9A", "10A", "11A", "12A", "1B", "2B",
        "3B", "4B", "5B", "6B", "7B", "8B", "9B", "10B", "11B", "12B",
    ];
    for (class, want) in expected.iter().enumerate() {
        let mut logits = [-2.0; 24];
        logits[class] = 3.0;
        let result = key_from_logits(&logits).unwrap();
        assert_eq!(&result.camelot, want);
        assert_eq!(result.candidates.len(), 24);
        assert_eq!(result.candidates[0].score, 3.0);
        assert_eq!(result.margin, 5.0);
    }
    assert_eq!(key_from_logits(&[0.0; 24]).unwrap().camelot, "1A");
}

#[test]
fn malformed_logits_are_errors_rather_than_a_key() {
    assert!(key_from_logits(&[1.0; 23]).is_err());
    for value in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
        let mut scores = [0.0; 24];
        scores[11] = value;
        assert!(key_from_logits(&scores).is_err());
    }
}

#[test]
fn short_or_inaudible_audio_is_unknown_but_invalid_input_is_an_error() {
    assert!(preprocess(&[], 44100).unwrap().is_none());
    assert!(
        preprocess(&vec![0.1; 44100 * 3 - 1], 44100)
            .unwrap()
            .is_none()
    );
    assert!(preprocess(&vec![0.0; 44100 * 3], 44100).unwrap().is_none());
    assert!(preprocess(&vec![1e-6; 44100 * 3], 44100).unwrap().is_none());
    assert!(preprocess(&[f32::NAN], 44100).is_err());
    assert!(preprocess(&[0.0], 22050).is_err());
    assert!(preprocess(&vec![f32::MAX; 44100 * 3], 44100).is_err());
}

fn floats(bytes: &[u8]) -> Vec<f32> {
    bytes
        .as_chunks::<4>()
        .0
        .iter()
        .map(|v| f32::from_le_bytes(*v))
        .collect()
}

#[test]
fn odd_length_generated_noise_matches_independent_upstream_cqt() {
    let samples = floats(include_bytes!(
        "../../tests/fixtures/openkeyscan/noise5-pcm.f32le"
    ));
    let expected = floats(include_bytes!(
        "../../tests/fixtures/openkeyscan/noise5-cqt.f32le"
    ));
    let tensor = preprocess(&samples, 44100).unwrap().unwrap();
    assert_eq!(tensor.columns, 24);
    assert_eq!(tensor.data.len(), expected.len());
    for (i, (actual, want)) in tensor.data.iter().zip(expected).enumerate() {
        assert!(
            (actual - want).abs() <= 1e-4 + 1e-4 * want.abs(),
            "CQT[{i}] {actual} != {want}"
        );
    }
}

#[test]
fn half_rate_resampling_keeps_odd_lengths_across_input_chunks() {
    for n in [1, 3, 191999, 192001] {
        let mut impulse = vec![0.0; n];
        impulse[n / 2] = 0.5;
        let result = half(&impulse).unwrap();
        assert_eq!(result.len(), n.div_ceil(2));
        assert!(result.iter().all(|x| x.is_finite()));
    }
}

#[test]
fn changed_model_is_rejected_before_parsing() {
    let file = tempfile::NamedTempFile::new().unwrap();
    std::fs::write(file.path(), b"not the pinned graph").unwrap();
    let error = OpenKeyScanModel::load(file.path())
        .err()
        .expect("wrong graph must fail");
    assert!(error.to_string().contains("checksum"), "{error:#}");
}

#[test]
#[ignore = "requires CAMELOT_OPENKEYSCAN_MODEL pointing at the pinned optional graph"]
fn pinned_network_runs_on_pcm_with_unknown_and_reuse_semantics() {
    let path = std::env::var_os("CAMELOT_OPENKEYSCAN_MODEL").expect("model path required");
    let mut model = OpenKeyScanModel::load(Path::new(&path)).unwrap();
    assert!(
        model
            .analyze(&vec![0.0; 44100 * 3], 44100)
            .unwrap()
            .is_none()
    );
    let samples = floats(include_bytes!(
        "../../tests/fixtures/openkeyscan/noise5-pcm.f32le"
    ));
    for _ in 0..2 {
        let result = model.analyze(&samples, 44100).unwrap().unwrap();
        assert_eq!(result.camelot, "6A");
        assert!(result.candidates.iter().all(|x| x.score.is_finite()));
    }
}

#[test]
fn signed_zero_ties_preserve_the_upstream_first_class_rule() {
    let mut logits = [-1.0; 24];
    logits[0] = -0.0;
    logits[1] = 0.0;
    assert_eq!(key_from_logits(&logits).unwrap().camelot, "1A");
}

#[test]
fn native_adapter_rejects_too_small_output_without_writing_past_it() {
    let input = [0.5_f32; 3];
    let mut output = [123.0_f32; 2];
    let mut written = 99;
    let mut message = [0 as c_char; 256];
    let code = unsafe {
        cw_soxr_half(
            input.as_ptr(),
            input.len(),
            output.as_mut_ptr(),
            1,
            &mut written,
            message.as_mut_ptr(),
            message.len(),
        )
    };
    assert_ne!(code, 0);
    assert_eq!(written, 0);
    assert_eq!(output, [123.0, 123.0]);
}
