use crate::{AnalysisConfig, Mode, MusicalKey, spectral::Spectrum};
use serde::{Deserialize, Serialize};

// Published Krumhansl-Kessler pitch-class profiles; C-relative semitone order.
const MAJOR: [f64; 12] = [
    6.35, 2.23, 3.48, 2.33, 4.38, 4.09, 2.52, 5.19, 2.39, 3.66, 2.29, 2.88,
];
const MINOR: [f64; 12] = [
    6.33, 2.68, 3.52, 5.38, 2.60, 3.53, 2.54, 4.75, 3.98, 2.69, 3.34, 3.17,
];
const BINS_PER_NOTE: usize = 36;
const HIST_SIZE: usize = BINS_PER_NOTE * 12;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyCandidate {
    pub key: MusicalKey,
    pub camelot: String,
    pub score: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TonalFeatures {
    pub tuning_cents: f64,
    pub chroma: [f64; 12],
    pub bass_chroma: [f64; 12],
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(try_from = "KeyEstimateFields")]
pub struct KeyEstimate {
    pub key: MusicalKey,
    pub camelot: String,
    /// Ranking margin, not a calibrated probability.
    pub margin: f64,
    /// Optional DSP diagnostics; not fabricated for a neural-only estimate.
    /// Flattening preserves existing DSP result JSON and saved feature records.
    #[serde(flatten)]
    pub features: Option<TonalFeatures>,
    pub candidates: Vec<KeyCandidate>,
}

// Deserialize optional diagnostics as an all-or-none group. Option+flatten on
// its own would silently discard malformed or partially present DSP features.
#[derive(Deserialize)]
struct KeyEstimateFields {
    key: MusicalKey,
    camelot: String,
    margin: f64,
    candidates: Vec<KeyCandidate>,
    tuning_cents: Option<f64>,
    chroma: Option<[f64; 12]>,
    bass_chroma: Option<[f64; 12]>,
}

impl TryFrom<KeyEstimateFields> for KeyEstimate {
    type Error = anyhow::Error;
    fn try_from(value: KeyEstimateFields) -> anyhow::Result<Self> {
        let features = match (value.tuning_cents, value.chroma, value.bass_chroma) {
            (Some(tuning_cents), Some(chroma), Some(bass_chroma)) => Some(TonalFeatures {
                tuning_cents,
                chroma,
                bass_chroma,
            }),
            (None, None, None) => None,
            _ => anyhow::bail!("Partial DSP key diagnostics"),
        };
        Ok(Self {
            key: value.key,
            camelot: value.camelot,
            margin: value.margin,
            candidates: value.candidates,
            features,
        })
    }
}

fn normalize(values: &mut [f64]) {
    let norm = values.iter().map(|x| x * x).sum::<f64>().sqrt();
    if norm > 0.0 {
        for value in values {
            *value /= norm;
        }
    }
}

fn correlation(chroma: &[f64; 12], profile: &[f64; 12], tonic: usize) -> f64 {
    let a_mean = chroma.iter().sum::<f64>() / 12.0;
    let b_mean = profile.iter().sum::<f64>() / 12.0;
    let mut product = 0.0;
    let mut a_norm = 0.0;
    let mut b_norm = 0.0;
    for (i, value) in chroma.iter().enumerate() {
        let a = value - a_mean;
        let b = profile[(i + 12 - tonic) % 12] - b_mean;
        product += a * b;
        a_norm += a * a;
        b_norm += b * b;
    }
    product / (a_norm * b_norm).sqrt().max(1e-12)
}

pub(crate) fn estimate(samples: &[f32], rate: u32, config: &AnalysisConfig) -> Option<KeyEstimate> {
    let size = config.key_frame_size;
    if samples.len() < size {
        return None;
    }
    let mut fft = Spectrum::new(size);
    let mut magnitudes = vec![0.0; size / 2 + 1];
    let mut chroma_hist = [0.0; HIST_SIZE];
    let mut bass_hist = [0.0; HIST_SIZE];
    let mut tuning_x = 0.0;
    let mut tuning_y = 0.0;
    let mut total_weight = 0.0;
    for offset in (0..=samples.len() - size).step_by(config.key_hop_size) {
        fft.magnitudes(&samples[offset..offset + size], &mut magnitudes);
        let max = magnitudes.iter().copied().fold(0.0_f64, f64::max);
        if max < 1e-6 {
            continue;
        }
        let mut frame = [0.0; HIST_SIZE];
        let mut bass = [0.0; HIST_SIZE];
        for bin in 2..magnitudes.len() - 1 {
            let magnitude = magnitudes[bin];
            if magnitude < max * 0.01
                || magnitude < magnitudes[bin - 1]
                || magnitude <= magnitudes[bin + 1]
            {
                continue;
            }
            let left = magnitudes[bin - 1].max(1e-12).ln();
            let center = magnitude.max(1e-12).ln();
            let right = magnitudes[bin + 1].max(1e-12).ln();
            let denominator = left - 2.0 * center + right;
            let correction = if denominator.abs() > 1e-12 {
                (0.5 * (left - right) / denominator).clamp(-0.5, 0.5)
            } else {
                0.0
            };
            let frequency = (bin as f64 + correction) * rate as f64 / size as f64;
            if !(55.0..=5000.0).contains(&frequency) {
                continue;
            }
            let note = 69.0 + 12.0 * (frequency / 440.0).log2();
            let radius = (bin / 8).max(4);
            let start = bin.saturating_sub(radius);
            let end = (bin + radius + 1).min(magnitudes.len());
            let local_mean = magnitudes[start..end].iter().sum::<f64>() / (end - start) as f64;
            let weight = (magnitude / local_mean.max(1e-12)).sqrt() * (magnitude / max).powf(0.35);
            let index =
                ((note.rem_euclid(12.0) * BINS_PER_NOTE as f64).round() as usize) % HIST_SIZE;
            frame[index] += weight;
            if frequency < 400.0 {
                bass[index] += weight;
            }
            if (150.0..=2500.0).contains(&frequency) {
                let angle = std::f64::consts::TAU * (note - note.round());
                tuning_x += angle.cos() * weight;
                tuning_y += angle.sin() * weight;
                total_weight += weight;
            }
        }
        normalize(&mut frame);
        normalize(&mut bass);
        for i in 0..HIST_SIZE {
            chroma_hist[i] += frame[i];
            bass_hist[i] += bass[i];
        }
    }
    if total_weight == 0.0 {
        return None;
    }
    let concentration = tuning_x.hypot(tuning_y) / total_weight;
    let tuning = if concentration > 0.15 {
        tuning_y.atan2(tuning_x) / std::f64::consts::TAU
    } else {
        0.0
    };
    let mut chroma = [0.0; 12];
    let mut bass_chroma = [0.0; 12];
    for i in 0..HIST_SIZE {
        let note = i as f64 / BINS_PER_NOTE as f64 - tuning;
        let pitch_class = (note.round() as i32).rem_euclid(12) as usize;
        let distance = (note - note.round()).abs();
        let weight = (std::f64::consts::PI * distance).cos().powi(2);
        chroma[pitch_class] += chroma_hist[i] * weight;
        bass_chroma[pitch_class] += bass_hist[i] * weight;
    }
    normalize(&mut chroma);
    normalize(&mut bass_chroma);
    let mut candidates = Vec::with_capacity(24);
    for tonic in 0..12 {
        for (mode, profile) in [(Mode::Major, &MAJOR), (Mode::Minor, &MINOR)] {
            let key = MusicalKey::new(tonic as u8, mode).expect("bounded tonic");
            let score = 0.85 * correlation(&chroma, profile, tonic)
                + 0.15 * correlation(&bass_chroma, profile, tonic);
            candidates.push(KeyCandidate {
                key,
                camelot: key.camelot(),
                score,
            });
        }
    }
    candidates.sort_by(|a, b| b.score.total_cmp(&a.score));
    if candidates[0].score < 0.15 {
        return None;
    }
    let margin = candidates[0].score - candidates[1].score;
    Some(KeyEstimate {
        key: candidates[0].key,
        camelot: candidates[0].camelot.clone(),
        margin,
        features: Some(TonalFeatures {
            tuning_cents: tuning * 100.0,
            chroma,
            bass_chroma,
        }),
        candidates,
    })
}
