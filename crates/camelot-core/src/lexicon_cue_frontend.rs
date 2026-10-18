//! Seven renderer lowpass stages with the Web Audio default Q of 1 dB.
use anyhow::{Result, ensure};
use sha2::{Digest, Sha256};

pub(crate) fn implementation_sha256() -> String {
    let mut hash = Sha256::new();
    for source in [
        include_bytes!("lexicon_cues.rs").as_slice(),
        include_bytes!("structural_cues.rs").as_slice(),
        include_bytes!("lexicon_cue_frontend.rs").as_slice(),
    ] {
        hash.update((source.len() as u64).to_le_bytes());
        hash.update(source);
    }
    format!("{:x}", hash.finalize())
}

pub(crate) fn filter(pcm: &[f32], rate: u32) -> Result<Vec<f32>> {
    ensure!(
        (8000..=192000).contains(&rate),
        "Invalid cue frontend sample rate"
    );
    ensure!(
        pcm.iter().all(|sample| sample.is_finite()),
        "Nonfinite cue frontend PCM"
    );
    let mut output = pcm.to_vec();
    let resonance = 10f64.powf(1. / 20.);
    for frequency in [800., 400., 400., 200., 200., 200., 200.] {
        // Match Blink's normalized-cutoff and reciprocal-normalization order.
        let cutoff = frequency / (rate as f64 / 2.);
        let theta = std::f64::consts::PI * cutoff;
        let alpha = theta.sin() / (2. * resonance);
        let beta = (1. - theta.cos()) / 2.;
        let inverse = 1. / (1. + alpha);
        let (b0, b1, b2) = (beta * inverse, 2. * beta * inverse, beta * inverse);
        let (a1, a2) = (-2. * theta.cos() * inverse, (1. - alpha) * inverse);
        let (mut x1, mut x2, mut y1, mut y2) = (0., 0., 0., 0.);
        for sample in &mut output {
            let x = *sample as f64;
            let y = b0 * x + b1 * x1 + b2 * x2 - a1 * y1 - a2 * y2;
            (x2, x1, y2, y1) = (x1, x, y1, y);
            *sample = y as f32;
        }
    }
    ensure!(
        output.iter().all(|sample| sample.is_finite()),
        "Cue frontend produced nonfinite samples"
    );
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::filter;
    use serde::Deserialize;
    use sha2::{Digest, Sha256};

    #[derive(Deserialize)]
    struct Reference {
        cases: Vec<Case>,
    }
    #[derive(Deserialize)]
    struct Case {
        sample_rate: u32,
        frames: usize,
        offset_frames: usize,
        burst_frames: usize,
        input_sha256: String,
        output_f32_bits: Vec<u32>,
    }
    #[test]
    fn webaudio_default_q_cascade_agrees_within_float32_rounding() {
        let source: Reference = serde_json::from_str(include_str!(
            "../../../tests/fixtures/lexicon-cue-frontend/reference.json"
        ))
        .unwrap();
        let mut consumed = 0;
        let mut summaries = Vec::new();
        for case in source.cases {
            let mut pcm = vec![0f32; case.frames];
            for i in 0..case.burst_frames.min(case.frames - case.offset_frames) {
                let sign = if i % 512 < 256 { 1. } else { -1. };
                pcm[case.offset_frames + i] =
                    ((case.burst_frames - i) as f64 / case.burst_frames as f64 * sign) as f32;
            }
            let mut sha = Sha256::new();
            for sample in &pcm {
                sha.update(sample.to_le_bytes());
            }
            assert_eq!(format!("{:x}", sha.finalize()), case.input_sha256);
            let actual = filter(&pcm, case.sample_rate).unwrap();
            assert_eq!(actual.len(), case.output_f32_bits.len());
            let mut mismatches = 0;
            let mut max_error = 0f64;
            for (&actual, &expected) in actual.iter().zip(&case.output_f32_bits) {
                mismatches += usize::from(actual.to_bits() != expected);
                max_error = max_error.max((actual as f64 - f32::from_bits(expected) as f64).abs());
            }
            // Web Audio audio buses store Float32 between stages. This conformance
            // fixture permits one Float32 epsilon of measured platform rounding;
            // it does not assert byte-identical output across browser versions.
            assert!(
                max_error <= f32::EPSILON as f64,
                "rate{}: {mismatches} differing bit patterns; max error {max_error}",
                case.sample_rate
            );
            let fallback = crate::lexicon_cues::cascaded_lowpass(&pcm, case.sample_rate);
            let fallback_error = fallback
                .iter()
                .zip(&case.output_f32_bits)
                .map(|(&actual, &expected)| (actual as f64 - f32::from_bits(expected) as f64).abs())
                .fold(0f64, f64::max);
            assert!(
                fallback_error > 0.01,
                "Fixture must distinguish WebAudio from Butterworth Q"
            );
            summaries.push((case.sample_rate, mismatches, max_error));
            consumed += 1;
        }
        assert_eq!(consumed, 3);
        assert_eq!(summaries.len(), 3);
    }
    #[test]
    fn frontend_bounds_invalid_inputs_and_keeps_empty_audio_empty() {
        assert!(filter(&[], 44100).unwrap().is_empty());
        assert!(filter(&vec![f32::MAX; 4096], 44100).is_err());
        for rate in [0, 7999, 192001] {
            assert!(filter(&[0.], rate).is_err());
        }
        for sample in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
            assert!(filter(&[sample], 44100).is_err());
        }
    }
    #[test]
    fn original_single_pole_iir_matches_float64_feedback_and_float32_stores() {
        let reference: serde_json::Value = serde_json::from_str(include_str!(
            "../../../tests/fixtures/lexicon-cue-frontend/iir-reference.json"
        ))
        .unwrap();
        let mut consumed = 0;
        for case in reference["cases"].as_array().unwrap() {
            let rate = case["sample_rate"].as_u64().unwrap() as u32;
            let frames = case["frames"].as_u64().unwrap() as usize;
            let offset = case["offset_frames"].as_u64().unwrap() as usize;
            let burst = case["burst_frames"].as_u64().unwrap() as usize;
            let mut pcm = vec![0f32; frames];
            for i in 0..burst.min(frames - offset) {
                let sign = if i % 512 < 256 { 1. } else { -1. };
                pcm[offset + i] = ((burst - i) as f64 / burst as f64 * sign) as f32;
            }
            let mut hash = Sha256::new();
            for sample in &pcm {
                hash.update(sample.to_le_bytes());
            }
            assert_eq!(
                format!("{:x}", hash.finalize()),
                case["input_sha256"].as_str().unwrap()
            );
            for row in case["rows"].as_array().unwrap() {
                let cutoff = row["cutoff_hz"].as_f64().unwrap();
                let passes = row["passes"].as_u64().unwrap() as u32;
                let actual = crate::lexicon_cues::low_pass_iir(&pcm, rate, cutoff, passes);
                let expected: Vec<u32> =
                    serde_json::from_value(row["output_f32_bits"].clone()).unwrap();
                assert_eq!(actual.len(), expected.len());
                for (i, (actual, expected)) in actual.iter().zip(expected).enumerate() {
                    assert_eq!(
                        actual.to_bits(),
                        expected,
                        "rate{rate} cutoff{cutoff} passes{passes} sample{i}"
                    );
                }
                consumed += 1;
            }
        }
        assert_eq!(consumed, 12);
    }
}
