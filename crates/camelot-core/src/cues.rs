//! Experimental beat-aligned novelty suggestions. No energy names are inferred.
use crate::{Cue, TempoEstimate, structure::StructureFeatures};
use anyhow::{Result, ensure};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CueConfig {
    pub max_cues: usize,
    pub min_gap_beats: f64,
    pub context_seconds: f64,
    pub downbeat_bonus: f64,
}

impl Default for CueConfig {
    fn default() -> Self {
        Self {
            max_cues: 8,
            min_gap_beats: 32.0,
            context_seconds: 8.0,
            downbeat_bonus: 0.1,
        }
    }
}

impl CueConfig {
    pub fn validate(&self) -> Result<()> {
        ensure!(
            (1..=8).contains(&self.max_cues),
            "Cue limit must be in 1..=8"
        );
        ensure!(
            self.min_gap_beats.is_finite() && self.min_gap_beats > 0.0,
            "Invalid cue separation"
        );
        ensure!(
            self.context_seconds.is_finite() && self.context_seconds > 0.0,
            "Invalid cue context"
        );
        ensure!(
            self.downbeat_bonus.is_finite() && self.downbeat_bonus >= 0.0,
            "Invalid downbeat bonus"
        );
        Ok(())
    }
}

/// Rank beat times by local loudness/band changes, then suppress nearby cues.
/// Retain the first valid beat. This baseline does not reconstruct vendor cue
/// selection or infer energy labels. Supply actual beat phase, not only BPM.
pub fn suggest(
    structure: &StructureFeatures,
    tempo: &TempoEstimate,
    config: &CueConfig,
) -> Result<Vec<Cue>> {
    config.validate()?;
    let duration = structure.duration_seconds;
    ensure!(
        structure.schema == "centered-rms-six-band-flux-v1",
        "Unknown cue feature schema"
    );
    ensure!(
        duration.is_finite() && duration >= 0.0,
        "Invalid cue duration"
    );
    ensure!(
        tempo.bpm.is_finite() && tempo.bpm > 0.0,
        "Invalid cue tempo"
    );
    let beats = &tempo.beats_seconds;
    let downbeats = tempo.downbeats_seconds.as_deref().unwrap_or(&[]);
    for times in [beats.as_slice(), downbeats] {
        ensure!(
            times.iter().all(|x| x.is_finite()) && times.windows(2).all(|w| w[0] < w[1]),
            "Invalid cue beat times"
        );
    }
    let frames = &structure.frames;
    ensure!(
        frames
            .windows(2)
            .all(|w| w[0].time_seconds < w[1].time_seconds),
        "Unordered cue features"
    );
    let mut prefix = Vec::with_capacity(frames.len() + 1);
    prefix.push([0.0_f64; 8]);
    for frame in frames {
        ensure!(
            frame.time_seconds.is_finite()
                && frame.time_seconds >= 0.0
                && frame.time_seconds < duration
                && frame.rms_db.is_finite()
                && frame.centroid_hz.is_finite()
                && frame.spectral_flux.is_finite()
                && (0.0..=1.0).contains(&frame.spectral_flux)
                && frame
                    .band_energy_fraction
                    .iter()
                    .all(|x| x.is_finite() && *x >= 0.0)
                && frame.band_energy_fraction.iter().sum::<f64>() <= 1.000001,
            "Invalid cue features"
        );
        let mut sum = *prefix.last().expect("initial prefix");
        sum[0] += frame.rms_db / 20.0;
        for (value, band) in sum[1..7].iter_mut().zip(frame.band_energy_fraction) {
            *value += band;
        }
        sum[7] += frame.spectral_flux;
        prefix.push(sum);
    }
    if frames.is_empty() || frames.iter().all(|f| f.rms_db <= -100.0) {
        return Ok(Vec::new());
    }
    let mean = |start: f64, end: f64| -> Option<[f64; 8]> {
        let lo = frames.partition_point(|f| f.time_seconds < start);
        let hi = frames.partition_point(|f| f.time_seconds < end);
        (hi > lo)
            .then(|| std::array::from_fn(|i| (prefix[hi][i] - prefix[lo][i]) / (hi - lo) as f64))
    };
    let mut candidates = Vec::new();
    for &time in beats {
        if !(0.0..duration).contains(&time) {
            continue;
        }
        let mut strength = match (
            mean(time - config.context_seconds, time),
            mean(time, time + config.context_seconds),
        ) {
            (Some(before), Some(after)) => {
                let band_change = (1..7)
                    .map(|i| (after[i] - before[i]).powi(2))
                    .sum::<f64>()
                    .sqrt();
                band_change + (after[0] - before[0]).abs() + after[7]
            }
            _ => 0.0,
        };
        let pos = downbeats.partition_point(|b| *b < time);
        if downbeats[pos.saturating_sub(1)..(pos + 1).min(downbeats.len())]
            .iter()
            .any(|b| (b - time).abs() < 0.04)
        {
            strength += config.downbeat_bonus;
        }
        candidates.push((strength, time));
    }
    let Some(first) = candidates.first() else {
        return Ok(Vec::new());
    };
    let mut selected = vec![first.1];
    candidates.sort_by(|a, b| b.0.total_cmp(&a.0).then_with(|| b.1.total_cmp(&a.1)));
    let beat_period = 60.0 / tempo.bpm;
    for (_, time) in candidates {
        if selected.len() == config.max_cues {
            break;
        }
        // Count musical beat intervals, allowing timestamp quantization at a
        // separation boundary to avoid discarding the actual section downbeat.
        if selected
            .iter()
            .all(|other| ((other - time).abs() / beat_period).round() >= config.min_gap_beats)
        {
            selected.push(time);
        }
    }
    selected.sort_by(f64::total_cmp);
    Ok(selected
        .into_iter()
        .map(|time_seconds| Cue {
            time_seconds,
            name: "Candidate".into(),
        })
        .collect())
}
