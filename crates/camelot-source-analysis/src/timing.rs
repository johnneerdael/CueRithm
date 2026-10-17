use crate::dsp::{
    key_real_stream_rust_v1 as decoder, legacy_classic_rust_v1 as classic,
    main_timing_composition_rust_v1 as main_timing,
};
use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};
use std::path::Path;

pub(crate) const PROFILE: &str = "Mixed In Key 11.2.6.7446 default: 44100 Hz stereo 16-bit FLAC";

/// Timing computed from the independently implemented source-profile pipeline.
///
/// BPM is the request's float32-narrowed value promoted to binary64. Beat times
/// are the computed final pre-request grid. Complete stored-grid agreement is a
/// separate diagnostic from exact BPM agreement. No confidence is synthesized.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimingAnalysis {
    pub profile: String,
    pub full_source_frames: usize,
    pub analyzed_frames: usize,
    pub bpm: f64,
    pub downbeat_time: f64,
    pub beats_seconds: Vec<f64>,
}

/// Decode and verify a complete44100Hz stereo signed16-bit FLAC and compute its
/// timing, without computing unused key or energy features. Unsupported input
/// and unresolved short/silent timing return errors without a fallback.
pub fn analyze_timing_file(path: &Path) -> Result<TimingAnalysis> {
    let decoded = decoder::decode_profile(path).context("source-profile FLAC decode")?;
    let main = compute(&decoded.interleaved, decoded.full_frames)?;
    Ok(TimingAnalysis {
        profile: PROFILE.into(),
        full_source_frames: decoded.full_frames,
        analyzed_frames: decoded.analyzed_frames,
        bpm: main.wire_tempo,
        downbeat_time: main.wire_downbeat,
        beats_seconds: main.beat_times,
    })
}

pub(crate) fn compute(pcm: &[f32], full_frames: usize) -> Result<main_timing::Output> {
    ensure!(
        pcm.len().is_multiple_of(2),
        "interleaved stereo PCM required"
    );
    let frames = pcm.len() / 2;
    ensure!(
        full_frames >= frames && full_frames <= 64_000_000,
        "full frame count must cover the analyzed prefix within the supported limit"
    );
    ensure!(
        pcm.iter()
            .all(|x| x.is_finite() && (-1.0..1.0).contains(x) && (*x * 32768.).fract() == 0.),
        "finite normalized signed16-bit PCM required"
    );
    ensure!(frames > 0, "timing is unavailable for empty input");
    let mut legacy = classic::Classic::new(2).map_err(anyhow::Error::msg)?;
    for feed in pcm.chunks(1024) {
        let left: Vec<f32> = feed.as_chunks::<2>().0.iter().map(|s| s[0]).collect();
        let right: Vec<f32> = feed.as_chunks::<2>().0.iter().map(|s| s[1]).collect();
        legacy
            .pre_analysis(&[&left, &right])
            .map_err(anyhow::Error::msg)?;
    }
    legacy
        .finish_pre_analysis(true)
        .map_err(anyhow::Error::msg)
        .context("source timing pre-analysis")?;
    legacy
        .process(0., 0, false, false)
        .map_err(anyhow::Error::msg)
        .context("source timing is unresolved for this input")?;
    let initial = legacy.wire_result().map_err(anyhow::Error::msg)?;
    ensure!(
        initial.valid && initial.times.len() >= 2,
        "source timing is unavailable for this input"
    );
    drop(legacy);
    let arrays = crate::waveform::analyze(pcm)?;
    main_timing::analyze(main_timing::Input {
        sample_rate: 44100.,
        hop: 1024,
        audio_frames: frames as u64,
        arrays,
        legacy_beats: initial.times,
        legacy_beat_bits: Vec::new(),
        legacy_tempo: f64::from(initial.tempo),
    })
    .map_err(anyhow::Error::msg)
    .context("Main timing")
}
