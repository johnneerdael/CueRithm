//! Portable audio analysis. Reference labels belong to the evaluator, not this API.

pub mod analysis;
pub mod audio;
pub mod cues;
pub mod engine;
pub mod lexicon_cues;
pub mod evaluation;
pub mod key;
#[cfg(feature = "neural-beats")]
pub mod neural;
#[cfg(feature = "neural-key")]
pub mod neural_key;
#[cfg(feature = "openkeyscan-key")]
pub mod openkeyscan;
pub mod profile;
pub mod rekordbox;
mod rhythm;
mod spectral;
pub mod structure;
mod tonal;

pub use analysis::{Analysis, AnalysisConfig, Cue, analyze_pcm};
pub use camelot_source_analysis::{
    AnalysisRequest, EnergySegment as EnergyRequestSegment, FeatureAnalysis,
    KeySegment as KeyRequestSegment, analyze_file as analyze_file_features,
    analyze_pcm as analyze_pcm_features,
};
pub use engine::{Engine, EngineSettings, FileAnalysis, SourceTimingMetadata};
pub use key::{Mode, MusicalKey};
pub use rhythm::{TempoCandidate, TempoEstimate};
pub use tonal::{KeyCandidate, KeyEstimate, TonalFeatures};
