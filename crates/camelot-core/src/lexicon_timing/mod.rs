//! Independent local timing components. The complete anchor detector is still
//! being integrated; tempo evidence alone is not a desktop timing backend.

use anyhow::{Result, ensure};
use serde::{Deserialize, Serialize};

mod tempo;
// Removed when the complete anchor pipeline consumes these internal helpers.
#[allow(dead_code)]
mod dp;
#[allow(dead_code)]
mod onset;
#[allow(dead_code, unused_imports)]
mod phase;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LexiconTempoOptions {
    pub min_tempo: f64,
    pub max_tempo: f64,
}
impl Default for LexiconTempoOptions {
    fn default() -> Self {
        Self {
            min_tempo: 98.,
            max_tempo: 195.,
        }
    }
}
/// Raw tempo-helper evidence, not a calibrated probability or a beat anchor.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TempoEvidence {
    pub bpm: f64,
    pub confidence: f64,
    pub peaks: Vec<usize>,
}

pub fn analyze_tempo(
    pcm: &[f32],
    sample_rate: u32,
    options: &LexiconTempoOptions,
) -> Result<Option<TempoEvidence>> {
    ensure!(
        (8000..=192000).contains(&sample_rate),
        "Unsupported timing sample rate"
    );
    ensure!(
        options.min_tempo.is_finite()
            && options.max_tempo.is_finite()
            && options.min_tempo >= 50.
            && options.max_tempo <= 300.
            && options.min_tempo < options.max_tempo,
        "Invalid timing tempo bounds"
    );
    ensure!(
        pcm.iter().all(|v| v.is_finite()),
        "Timing PCM contains nonfinite samples"
    );
    Ok(tempo::analyze(pcm, sample_rate, options))
}

#[cfg(test)]
mod tests;
