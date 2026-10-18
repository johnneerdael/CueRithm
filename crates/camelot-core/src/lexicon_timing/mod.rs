//! Independent local tempo and constant-grid anchor analysis.
//! Desktop selection additionally requires decoder, clock and cache validation.

use anyhow::{Result, ensure};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub const METHOD: &str = "lexicon-timing-v1";

// Keep this list in step with the production modules below. Length prefixes
// distinguish component boundaries; test/fixture changes are not identities.
pub(crate) fn implementation_sha256() -> String {
    let mut hash = Sha256::new();
    for source in [
        include_bytes!("mod.rs").as_slice(),
        include_bytes!("tempo.rs").as_slice(),
        include_bytes!("onset.rs").as_slice(),
        include_bytes!("dp.rs").as_slice(),
        include_bytes!("phase.rs").as_slice(),
        include_bytes!("phase/calibration.rs").as_slice(),
        include_bytes!("phase/grid.rs").as_slice(),
        include_bytes!("phase/rescue.rs").as_slice(),
        include_bytes!("phase/riser.rs").as_slice(),
        include_bytes!("phase/subsequent.rs").as_slice(),
        include_bytes!("intro.rs").as_slice(),
        include_bytes!("anchors.rs").as_slice(),
        include_bytes!("walkback.rs").as_slice(),
        include_bytes!("walkback/negative.rs").as_slice(),
        include_bytes!("walkback/positive.rs").as_slice(),
        include_bytes!("finish.rs").as_slice(),
        include_bytes!("selection.rs").as_slice(),
        include_bytes!("selection/drop.rs").as_slice(),
        include_bytes!("selection/early.rs").as_slice(),
        include_bytes!("selection/immediate.rs").as_slice(),
        include_bytes!("selection/long_intro.rs").as_slice(),
    ] {
        hash.update((source.len() as u64).to_le_bytes());
        hash.update(source);
    }
    format!("{:x}", hash.finalize())
}

mod anchors;
mod dp;
mod finish;
mod intro;
mod onset;
mod phase;
mod selection;
mod tempo;
mod walkback;

#[derive(Debug, Clone, Serialize, Deserialize)]
/// Candidate-search bounds. Source-compatible refinement can round slightly
/// outside a supplied noninteger bound; these are not hard output clamps.
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

/// A single constant-tempo grid anchor. Confidence is the source heuristic,
/// not a calibrated probability or a separate musical downbeat annotation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimingEvidence {
    pub bpm: f64,
    pub first_beat_seconds: f64,
    pub confidence: f64,
}

pub fn analyze(
    pcm: &[f32],
    sample_rate: u32,
    options: &LexiconTempoOptions,
) -> Result<Option<TimingEvidence>> {
    // The inspected renderer's minimum analysis duration is ten seconds.
    // Validate even short inputs so malformed PCM/options remain errors.
    if (pcm.len() as u64) < u64::from(sample_rate) * 10 {
        validate(pcm, sample_rate, options)?;
        return Ok(None);
    }
    let mut state = phase::CorrectionState::default();
    let Some(selected) = selection::select(pcm, sample_rate, options, &mut state)? else {
        return Ok(None);
    };
    let result = finish::finish(pcm, sample_rate, selected, &mut state);
    ensure!(
        result.first_beat.is_finite()
            && result.first_beat >= 0.
            && result.first_beat <= pcm.len() as f64 / sample_rate as f64,
        "Timing analysis produced an invalid anchor"
    );
    Ok(Some(TimingEvidence {
        bpm: result.bpm,
        first_beat_seconds: result.first_beat,
        confidence: result.confidence,
    }))
}

pub fn analyze_tempo(
    pcm: &[f32],
    sample_rate: u32,
    options: &LexiconTempoOptions,
) -> Result<Option<TempoEvidence>> {
    validate(pcm, sample_rate, options)?;
    Ok(tempo::analyze(pcm, sample_rate, options))
}

fn validate(pcm: &[f32], sample_rate: u32, options: &LexiconTempoOptions) -> Result<()> {
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
    Ok(())
}

#[cfg(test)]
mod tests;
