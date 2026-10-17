use crate::{
    AnalysisRequest, FeatureAnalysis, KeySegment,
    dsp::{
        key_pcm_stream_rust_v1 as key_stream, key_real_stream_rust_v1 as decoder,
        key_request_aggregation_rust_v1 as key_bank,
    },
};
use anyhow::{Context, Result};
use base64::{Engine, engine::general_purpose::STANDARD};
use std::path::Path;

/// Decode and integrity-verify a 44100 Hz stereo signed16-bit FLAC, then compute
/// the inspected default profile's analysis-request features.
///
/// The decoder determines the reader-returned analyzed prefix and retains the
/// full decoded source frame count separately. Use this entry point when starting
/// from a file. Unsupported formats and unresolved timing return an error.
pub fn analyze_file(path: &Path) -> Result<FeatureAnalysis> {
    let decoded = decoder::decode_profile(path).context("source-profile FLAC decode")?;
    analyze_pcm(&decoded.interleaved, decoded.full_frames)
}

/// Compute the inspected default profile's request features from decoded PCM.
///
/// # Input contract
///
/// Supply the reader-returned **analyzed prefix**, interleaved left/right at
/// exactly **44100 Hz**, normalized from signed16-bit samples by dividing by
/// 32768. Values must be finite, quantized to that scale and within `[-1, 1)`.
/// `full_frames` is the separately verified count of stereo frames in the entire
/// decoded source, including any tail excluded from `pcm`. It must cover the
/// supplied prefix and be at most 64000000 frames.
///
/// Request duration uses `pcm.len() / 2 / 44100`; energy normalization uses the
/// full source duration. Correct sample rate, source integrity, prefix boundary
/// and full frame count are caller obligations: this slice alone cannot establish
/// them. Prefer [`analyze_file`] for file input so the decoder supplies them.
///
/// Invalid sample shapes/values, unsupported sizes and unresolved short or silent
/// timing return an error. Successful output contains classifier inputs; final
/// key, energy ratings and selected cues require a separate decision stage.
pub fn analyze_pcm(pcm: &[f32], full_frames: usize) -> Result<FeatureAnalysis> {
    let main = crate::timing::compute(pcm, full_frames)?;
    let frames = pcm.len() / 2;
    let mut key = key_stream::Stream::default();
    for feed in pcm.chunks(1024) {
        key.feed(feed, 2).map_err(anyhow::Error::msg)?;
    }
    key.finish().map_err(anyhow::Error::msg)?;
    let cache = std::mem::take(&mut key.cache);
    drop(key);
    let duration = frames as f64 / 44100.;
    // Retain the validated Rust aggregation entry point; its numeric transport
    // stays in memory and receives only freshly computed cache and timing.
    let bytes: Vec<u8> = cache
        .iter()
        .flatten()
        .flat_map(|v| v.to_le_bytes())
        .collect();
    let input = serde_json::json!({"cache_f32le_base64":STANDARD.encode(&bytes),"frame_count":cache.len(),"beat_time_bits":main.beat_bits,"downbeat_bits":main.wire_downbeat_bits,"duration_bits":format!("{:016x}",duration.to_bits())});
    let bank = key_bank::process_json(&input.to_string()).map_err(anyhow::Error::msg)?;
    let bank: serde_json::Value = serde_json::from_str(&bank)?;
    let features = bank["feature_bits"]
        .as_array()
        .context("key bank vector missing")?
        .iter()
        .map(|v| {
            let text = v.as_str().context("key feature bits missing")?;
            Ok(f64::from_bits(u64::from_str_radix(text, 16)?))
        })
        .collect::<Result<Vec<f64>>>()?;
    let energy = crate::energy::analyze(pcm, full_frames)?;
    let request = AnalysisRequest {
        segments: vec![KeySegment {
            start_time: 0.,
            end_time: duration,
            pitch_probabilities: [0.; 12],
            features,
        }],
        energy_segment_data: energy,
        duration_in_seconds: duration,
        tempo: main.wire_tempo,
        downbeat_time: main.wire_downbeat,
        cue_point_start_beat: main.cue_point_start_beat,
        cue_point_data: main.cue_point_data,
        key_algorithm_version: 94,
        energy_algorithm_version: "2".into(),
        cue_point_algorithm_version: "3".into(),
    };
    Ok(FeatureAnalysis {
        profile: crate::timing::PROFILE.into(),
        full_source_frames: full_frames,
        analyzed_frames: frames,
        analysis: request,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_odd_channel_shape_before_analysis() {
        let error = analyze_pcm(&[0.0], 1).unwrap_err();
        assert!(error.to_string().contains("stereo"));
    }

    #[test]
    fn rejects_nonquantized_and_nonfinite_pcm() {
        for value in [0.1, 1.0, f32::NAN, f32::INFINITY] {
            let error = analyze_pcm(&[value, value], 1).unwrap_err();
            assert!(error.to_string().contains("16-bit"));
        }
    }

    #[test]
    fn full_count_cannot_be_shorter_than_analyzed_prefix() {
        let error = analyze_pcm(&[0.0; 4], 1).unwrap_err();
        assert!(error.to_string().contains("full frame count"));
    }

    #[test]
    fn unsupported_file_rate_is_explicit() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/stereo16-48000.flac");
        let error = analyze_file(&path).unwrap_err();
        assert!(format!("{error:#}").contains("44100"));
    }

    #[test]
    fn short_silence_does_not_invent_timing() {
        let error = analyze_pcm(&[0.0; 128], 64).unwrap_err();
        assert!(format!("{error:#}").contains("timing"));
    }
    #[test]
    fn synthetic_file_produces_classifier_inputs_without_final_decisions() {
        let path =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/pulses64-stereo16.flac");
        let output = analyze_file(&path).unwrap();
        assert_eq!(output.full_source_frames, 64 * 44100);
        assert!(output.analyzed_frames <= output.full_source_frames);
        assert_eq!(output.analysis.segments.len(), 1);
        assert!(!output.analysis.energy_segment_data.is_empty());
        assert!(
            output
                .analysis
                .energy_segment_data
                .iter()
                .all(|s| s.features.len() == 183)
        );
        assert!(output.analysis.tempo.is_finite() && output.analysis.tempo > 0.);
        let json = serde_json::to_value(output).unwrap();
        assert!(json["analysis"].get("Key").is_none());
        assert!(json["analysis"].get("CuePoints").is_none());
    }
}
