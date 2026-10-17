//! Portable audio analysis. Reference labels belong to the evaluator, not this API.

pub mod analysis;
pub mod audio;
pub mod energy;
pub mod engine;
pub mod evaluation;
pub mod key;
// Preserve the validated port verbatim; validate/lint integration separately.
#[allow(clippy::all, unused_variables, unused_assignments, unused_parens)]
#[rustfmt::skip]
pub mod lexicon_cues;
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
mod structural_cues;
pub mod structure;
mod tonal;

pub use analysis::{Analysis, AnalysisConfig, Cue, analyze_pcm};
pub use engine::{Engine, EngineSettings, FileAnalysis, SourceTimingMetadata};
pub use key::{Mode, MusicalKey};
pub use rhythm::{TempoCandidate, TempoEstimate};
pub use tonal::{KeyCandidate, KeyEstimate, TonalFeatures};
