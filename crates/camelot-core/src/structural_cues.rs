//! Checked integration boundary for the unchanged Lexicon-compatible DSP.
use crate::lexicon_cues::{
    LexiconConfig, StructuralCueResult, generate_structural_cues,
    generate_structural_cues_with_filtered,
};
use anyhow::{Result, ensure};

impl LexiconConfig {
    pub fn validate(&self) -> Result<()> {
        ensure!(
            (1..=4096).contains(&self.min_cue_length_beats),
            "Breakdown length must be in 1..=4096 beats"
        );
        Ok(())
    }
}

pub fn try_generate_structural_cues(
    pcm: &[f32],
    sample_rate: u32,
    bpm: f64,
    first_beat: f64,
    config: &LexiconConfig,
) -> Result<StructuralCueResult> {
    checked_generate(pcm, sample_rate, bpm, first_beat, config, false)
}

/// Apply the shipped renderer's Web Audio lowpass defaults before cue detection.
pub fn try_generate_renderer_cues(
    pcm: &[f32],
    sample_rate: u32,
    bpm: f64,
    first_beat: f64,
    config: &LexiconConfig,
) -> Result<StructuralCueResult> {
    checked_generate(pcm, sample_rate, bpm, first_beat, config, true)
}

fn checked_generate(
    pcm: &[f32],
    sample_rate: u32,
    bpm: f64,
    first_beat: f64,
    config: &LexiconConfig,
    renderer_frontend: bool,
) -> Result<StructuralCueResult> {
    config.validate()?;
    ensure!(
        (8000..=192000).contains(&sample_rate),
        "Invalid cue sample rate"
    );
    ensure!(
        bpm.is_finite() && (1.0..=1000.0).contains(&bpm),
        "Invalid cue BPM"
    );
    ensure!(
        pcm.len() <= 64_000_000 && pcm.iter().all(|x| x.is_finite()),
        "Invalid cue PCM"
    );
    let duration = pcm.len() as f64 / sample_rate as f64;
    ensure!(
        first_beat.is_finite() && first_beat >= -60.0 / bpm && first_beat <= duration,
        "Invalid first beat"
    );
    if pcm.is_empty() || pcm.iter().all(|x| *x == 0.0) {
        return Ok(StructuralCueResult {
            cues: Vec::new(),
            emergency_loop: None,
        });
    }
    let mut result = if renderer_frontend {
        let filtered = crate::lexicon_cue_frontend::filter(pcm, sample_rate)?;
        generate_structural_cues_with_filtered(
            pcm,
            sample_rate,
            bpm,
            first_beat,
            config,
            Some(&filtered),
        )
    } else {
        generate_structural_cues(pcm, sample_rate, bpm, first_beat, config)
    };
    // A beat grid may begin just before a cropped file. Keep that anchor for
    // every DSP calculation; only clip cues at that anchor to the boundary.
    for cue in &mut result.cues {
        if cue.time_seconds == first_beat && first_beat < 0.0 {
            cue.time_seconds = 0.0;
        }
    }
    ensure!(
        result.cues.iter().all(|c| c.time_seconds.is_finite()
            && c.time_seconds >= 0.0
            && c.time_seconds < duration),
        "Cue outside audio"
    );
    if let Some(loop_) = &result.emergency_loop {
        ensure!(
            loop_.time_seconds.is_finite()
                && loop_.time_seconds >= 0.0
                && loop_.time_seconds < duration,
            "Loop outside audio"
        );
    }
    Ok(result)
}
