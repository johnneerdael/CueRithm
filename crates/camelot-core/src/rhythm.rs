use crate::{AnalysisConfig, spectral::Spectrum};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TempoCandidate {
    pub bpm: f64,
    pub score: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TempoEstimate {
    pub bpm: f64,
    /// Periodicity score, not a calibrated confidence probability; absent when unavailable.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub strength: Option<f64>,
    pub candidates: Vec<TempoCandidate>,
    pub beats_seconds: Vec<f64>,
    pub downbeats_seconds: Option<Vec<f64>>,
    /// Single source timing anchor; does not imply a complete metrical grid.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub downbeat_time: Option<f64>,
    pub method: String,
}

fn autocorrelation(values: &[f64], lag: usize) -> f64 {
    if lag >= values.len() {
        return 0.0;
    }
    let mut sum = 0.0;
    let mut left = 0.0;
    let mut right = 0.0;
    for i in lag..values.len() {
        sum += values[i] * values[i - lag];
        left += values[i] * values[i];
        right += values[i - lag] * values[i - lag];
    }
    sum / (left * right).sqrt().max(1e-12)
}

fn refine_period(peaks: &[f64], period: f64) -> f64 {
    if peaks.len() < 6 {
        return period;
    }
    let first = peaks[0];
    let pairs: Vec<_> = peaks
        .iter()
        .filter_map(|time| {
            let index = ((time - first) / period).round();
            ((time - first - index * period).abs() < period * 0.12).then_some((index, *time))
        })
        .collect();
    if pairs.len() < 6 {
        return period;
    }
    let count = pairs.len() as f64;
    let mx = pairs.iter().map(|(x, _)| x).sum::<f64>() / count;
    let my = pairs.iter().map(|(_, y)| y).sum::<f64>() / count;
    let variance = pairs.iter().map(|(x, _)| (x - mx).powi(2)).sum::<f64>();
    if variance < 1.0 {
        return period;
    }
    let slope = pairs.iter().map(|(x, y)| (x - mx) * (y - my)).sum::<f64>() / variance;
    if (slope / period - 1.0).abs() < 0.03 {
        slope
    } else {
        period
    }
}

pub(crate) fn estimate(
    samples: &[f32],
    rate: u32,
    config: &AnalysisConfig,
) -> Option<TempoEstimate> {
    let size = config.rhythm_frame_size;
    if samples.len() < size * 3 {
        return None;
    }
    let hop = config.rhythm_hop_size;
    let frame_seconds = hop as f64 / rate as f64;
    let fps = 1.0 / frame_seconds;
    let mut fft = Spectrum::new(size);
    let mut magnitudes = vec![0.0; size / 2 + 1];
    let mut previous = vec![0.0; magnitudes.len()];
    let mut novelty = Vec::new();
    for offset in (0..=samples.len() - size).step_by(hop) {
        fft.magnitudes(&samples[offset..offset + size], &mut magnitudes);
        let mut flux = 0.0;
        for bin in 1..magnitudes.len() {
            let value = (1.0 + 10.0 * magnitudes[bin]).ln();
            if bin as f64 * rate as f64 / size as f64 > 40.0 {
                flux += (value - previous[bin]).max(0.0);
            }
            previous[bin] = value;
        }
        novelty.push(flux);
    }
    let radius = (fps * 0.1).round().max(1.0) as usize;
    let envelope: Vec<f64> = novelty
        .iter()
        .enumerate()
        .map(|(i, value)| {
            let start = i.saturating_sub(radius);
            let end = (i + radius + 1).min(novelty.len());
            (value - novelty[start..end].iter().sum::<f64>() / (end - start) as f64).max(0.0)
        })
        .collect();
    let maximum = envelope.iter().copied().fold(0.0_f64, f64::max);
    if maximum < 1e-8 {
        return None;
    }
    let min_lag = (60.0 * fps / config.max_bpm).floor() as usize;
    let max_lag = ((60.0 * fps / config.min_bpm).ceil() as usize).min(envelope.len() / 3);
    if max_lag <= min_lag + 2 {
        return None;
    }
    let correlations: Vec<_> = (0..=max_lag + 1)
        .map(|lag| autocorrelation(&envelope, lag))
        .collect();
    let peaks: Vec<f64> = (1..envelope.len() - 1)
        .filter(|&i| {
            envelope[i] > maximum * 0.12
                && envelope[i] > envelope[i - 1]
                && envelope[i] >= envelope[i + 1]
        })
        .map(|i| (i * hop + size / 2) as f64 / rate as f64)
        .collect();
    let mut candidates = Vec::new();
    for lag in min_lag.max(2)..=max_lag {
        let center = correlations[lag];
        if center < correlations[lag - 1] || center <= correlations[lag + 1] {
            continue;
        }
        let denominator = correlations[lag - 1] - 2.0 * center + correlations[lag + 1];
        let correction = if denominator.abs() > 1e-12 {
            (0.5 * (correlations[lag - 1] - correlations[lag + 1]) / denominator).clamp(-0.5, 0.5)
        } else {
            0.0
        };
        let period = refine_period(&peaks, (lag as f64 + correction) / fps);
        let bpm = 60.0 / period;
        if bpm < config.min_bpm || bpm > config.max_bpm {
            continue;
        }
        let prior = (-0.5 * ((bpm / 125.0).log2() / 0.8).powi(2)).exp();
        candidates.push(TempoCandidate {
            bpm,
            score: center * (0.9 + 0.1 * prior),
        });
    }
    candidates.sort_by(|a, b| b.score.total_cmp(&a.score));
    let best = candidates.first()?;
    if best.score < 0.08 {
        return None;
    }
    let bpm = best.bpm;
    let strength = best.score;
    let period = 60.0 / bpm;
    let mut x = 0.0;
    let mut y = 0.0;
    for (i, value) in envelope.iter().enumerate() {
        let time = (i * hop + size / 2) as f64 / rate as f64;
        let angle = std::f64::consts::TAU * time / period;
        x += value * angle.cos();
        y += value * angle.sin();
    }
    let phase = (y.atan2(x) / std::f64::consts::TAU).rem_euclid(1.0) * period;
    let duration = samples.len() as f64 / rate as f64;
    let beats_seconds = (0..)
        .map(|i| phase + i as f64 * period)
        .take_while(|t| *t < duration)
        .collect();
    candidates.truncate(8);
    Some(TempoEstimate {
        bpm,
        strength: Some(strength),
        candidates,
        beats_seconds,
        downbeats_seconds: None,
        downbeat_time: None,
        method: "spectral-flux-autocorrelation-v1".into(),
    })
}
