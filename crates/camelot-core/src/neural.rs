//! Optional offline Beat This! inference; model files must be supplied explicitly.
use crate::rhythm::{TempoCandidate, TempoEstimate};
use anyhow::{Result, ensure};
use beat_this::{BeatThis, RtenRuntime, Runtime};
use sha2::{Digest, Sha256};
use std::{collections::BTreeMap, fs::File, io::Read, path::Path};

pub struct NeuralBeatTracker {
    tracker: BeatThis<<RtenRuntime as Runtime>::Model>,
    model_identity: BTreeMap<String, String>,
}

fn file_sha256(path: &Path) -> Result<String> {
    let mut file = File::open(path)?;
    let mut hash = Sha256::new();
    let mut buffer = [0_u8; 65536];
    loop {
        let count = file.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        hash.update(&buffer[..count]);
    }
    Ok(format!("{:x}", hash.finalize()))
}

impl NeuralBeatTracker {
    pub fn new(mel_model: &Path, beat_model: &Path) -> Result<Self> {
        let model_identity = BTreeMap::from([
            ("beat-this-mel".into(), file_sha256(mel_model)?),
            ("beat-this-beats".into(), file_sha256(beat_model)?),
        ]);
        let tracker = BeatThis::new(&RtenRuntime, mel_model, beat_model)?;
        Ok(Self {
            tracker,
            model_identity,
        })
    }

    pub fn model_identity(&self) -> &BTreeMap<String, String> {
        &self.model_identity
    }

    pub fn analyze(&mut self, samples: &[f32], rate: u32) -> Result<Option<TempoEstimate>> {
        ensure!(
            (8000..=192000).contains(&rate),
            "Invalid neural input sample rate"
        );
        ensure!(
            samples.iter().all(|x| x.is_finite()),
            "Non-finite neural PCM input"
        );
        if samples.len() < rate as usize * 2 || samples.iter().all(|x| x.abs() < 1e-6) {
            return Ok(None);
        }
        let result = self.tracker.analyze_audio(samples, rate)?;
        let beats: Vec<f64> = result.beats.iter().map(|x| *x as f64).collect();
        let downbeats: Vec<f64> = result.downbeats.iter().map(|x| *x as f64).collect();
        Ok(tempo_from_beats(&beats, &downbeats))
    }
}

fn median(values: &mut [f64]) -> f64 {
    values.sort_by(f64::total_cmp);
    let mid = values.len() / 2;
    if values.len().is_multiple_of(2) {
        (values[mid - 1] + values[mid]) * 0.5
    } else {
        values[mid]
    }
}

fn phase_coherence(beats: &[f64], period: f64, center: f64) -> (f64, f64) {
    let mut x = 0.0;
    let mut y = 0.0;
    let mut count = 0;
    // Bound the search cost for long recordings without shortening its time span.
    for time in beats.iter().step_by((beats.len() / 2048).max(1)) {
        let angle = std::f64::consts::TAU * (time - center) / period;
        let (sin, cos) = angle.sin_cos();
        x += cos;
        y += sin;
        count += 1;
    }
    (
        y.atan2(x) / std::f64::consts::TAU * period,
        x.hypot(y) / count as f64,
    )
}

/// Fit a periodic time lattice without assuming every detection advances a beat.
/// Extra detections are outliers, and missed beats do not shift later indices.
pub fn tempo_from_beats(beats: &[f64], downbeats: &[f64]) -> Option<TempoEstimate> {
    if beats.len() < 6
        || beats.iter().chain(downbeats).any(|x| !x.is_finite())
        || beats.windows(2).any(|w| w[1] <= w[0])
    {
        return None;
    }
    let span = 32.min(beats.len() / 2);
    let mut periods: Vec<f64> = beats
        .windows(span + 1)
        .map(|w| (w[span] - w[0]) / span as f64)
        .collect();
    let rough = median(&mut periods);
    if rough <= 0.0 {
        return None;
    }
    let rough_bpm = 60.0 / rough;
    if !(30.0..=300.0).contains(&rough_bpm) {
        return None;
    }
    let center = beats[beats.len() / 2];
    let duration = beats[beats.len() - 1] - beats[0];
    let low = (rough_bpm * 0.94).max(30.0);
    let high = (rough_bpm * 1.06).min(300.0);
    let step = (15.0 / duration).min(0.05);
    let steps = ((high - low) / step).ceil().clamp(64.0, 4096.0) as usize;
    let mut period = rough;
    let mut best_coherence = -1.0;
    for i in 0..=steps {
        let candidate = 60.0 / (low + (high - low) * i as f64 / steps as f64);
        let (_, coherence) = phase_coherence(beats, candidate, center);
        if coherence > best_coherence {
            best_coherence = coherence;
            period = candidate;
        }
    }
    for _ in 0..5 {
        let (phase, _) = phase_coherence(beats, period, center);
        let pairs: Vec<(f64, f64)> = beats
            .iter()
            .filter_map(|time| {
                let relative = time - center;
                let index = ((relative - phase) / period).round();
                ((relative - phase - index * period).abs() <= period * 0.2)
                    .then_some((index, relative))
            })
            .collect();
        if pairs.len() < 6 {
            break;
        }
        let count = pairs.len() as f64;
        let mx = pairs.iter().map(|p| p.0).sum::<f64>() / count;
        let my = pairs.iter().map(|p| p.1).sum::<f64>() / count;
        let variance = pairs.iter().map(|p| (p.0 - mx).powi(2)).sum::<f64>();
        if variance < 1.0 {
            break;
        }
        let fitted = pairs.iter().map(|p| (p.0 - mx) * (p.1 - my)).sum::<f64>() / variance;
        if (fitted / rough - 1.0).abs() > 0.08 {
            break;
        }
        let change = (fitted - period).abs();
        period = fitted;
        if change < 1e-10 {
            break;
        }
    }
    let bpm = 60.0 / period;
    if !bpm.is_finite() || !(30.0..=300.0).contains(&bpm) {
        return None;
    }
    let (_, strength) = phase_coherence(beats, period, center);
    let candidates = [1.0, 0.5, 2.0]
        .into_iter()
        .filter_map(|factor| {
            let alternative = bpm * factor;
            (30.0..=300.0)
                .contains(&alternative)
                .then_some(TempoCandidate {
                    bpm: alternative,
                    score: if factor == 1.0 {
                        strength
                    } else {
                        strength * 0.5
                    },
                })
        })
        .collect();
    Some(TempoEstimate {
        bpm,
        strength: Some(strength),
        candidates,
        beats_seconds: beats.to_vec(),
        downbeats_seconds: Some(downbeats.to_vec()),
        downbeat_time: None,
        method: "beat-this-rten-1.1.0-lattice-v2".into(),
    })
}
