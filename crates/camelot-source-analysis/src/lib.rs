//! Independent audio-to-feature calculations for the inspected default profile.
//!
//! These features are classifier inputs. They are not final key, energy or cue
//! decisions.

// Preserve reviewed arithmetic entry points during integration. Some helpers
// serve retained component tests rather than the file-analysis adapter.
// Retained arithmetic uses Clippy lint names that vary by Rust toolchain.
#[allow(dead_code, unknown_lints)]
mod dsp;

mod energy;
mod pcm;
mod pipeline;
mod request;
mod timing;
mod waveform;

pub use pcm::{PcmProfile, analyze_energy_pcm, analyze_timing_pcm};
#[doc(inline)]
pub use pipeline::{analyze_file, analyze_pcm};
pub use request::{AnalysisRequest, EnergySegment, FeatureAnalysis, KeySegment};
#[doc(inline)]
pub use timing::{TimingAnalysis, analyze_timing_file};

/// SHA256 of the compiled runtime source files, their paths and build manifest.
pub fn source_sha256() -> &'static str {
    env!("CAMELOT_SOURCE_ANALYSIS_SHA256")
}

/// Decode the supported complete FLAC profile and compute internal energy inputs only.
pub fn analyze_energy_file(path: &std::path::Path) -> anyhow::Result<Vec<EnergySegment>> {
    let decoded = dsp::key_real_stream_rust_v1::decode_profile(path)
        .map_err(|e| anyhow::anyhow!("energy profile decode: {e:#}"))?;
    energy::analyze(&decoded.interleaved, decoded.full_frames)
}
