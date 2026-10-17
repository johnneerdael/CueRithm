//! Independent audio-to-feature calculations for the inspected default profile.
//!
//! These features are classifier inputs. They are not final key, energy or cue
//! decisions.

// Preserve reviewed arithmetic entry points during integration. Some helpers
// serve retained component tests rather than the file-analysis adapter.
#[allow(dead_code)]
mod dsp;

mod energy;
mod pipeline;
mod request;
mod timing;
mod waveform;

#[doc(inline)]
pub use pipeline::{analyze_file, analyze_pcm};
pub use request::{AnalysisRequest, EnergySegment, FeatureAnalysis, KeySegment};
#[doc(inline)]
pub use timing::{TimingAnalysis, analyze_timing_file};

/// SHA256 of the compiled runtime source files, their paths and build manifest.
pub fn source_sha256() -> &'static str {
    env!("CAMELOT_SOURCE_ANALYSIS_SHA256")
}
