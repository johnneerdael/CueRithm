//! Private feature-conditioned diagnostics, not independent PCM/full-engine inference.
//! Arithmetic follows the pinned SDK ARM64 helper assembly documented in the adjacent port map.

#[derive(Debug, PartialEq)]
pub struct Score {
    pub score: f64,
    pub buckets: Vec<f64>,
}

/// Evaluate an explicit waveform-feature vector against a period (feature frames)
/// and phase fraction. Reference labels and audio tags are not inputs.
pub fn calculate_beat_score(samples: &[f64], period: f64, phase: f64) -> Result<Score, String> {
    if !period.is_finite() || period <= 0.0 || !phase.is_finite() {
        return Err("period must be positive and finite; phase must be finite".into());
    }
    if samples.iter().any(|sample| !sample.is_finite()) {
        return Err("samples must be finite".into());
    }
    let mut buckets = Vec::<f64>::new();
    let offset = period * phase;
    for (index, &sample) in samples.iter().enumerate() {
        let q = (index as f64 - offset) / period + 0.5;
        let beat = q.floor();
        if beat < 0.0 {
            continue;
        }
        // Diagnostic resource guard; SDK's actual integer conversion is i32.
        if !beat.is_finite() || beat > 10_000_000.0 {
            return Err("fixture requires more than ten million beat buckets".into());
        }
        let beat_index = beat as usize;
        buckets.resize(beat_index + 1, 0.0);
        let residual = (q - beat) - 0.5;
        let u = residual.mul_add(12.0, 0.5);
        let window = if u > 0.0 && u < 1.0 {
            (u * (std::f64::consts::PI + std::f64::consts::PI))
                .cos()
                .mul_add(-0.5, 0.5)
        } else {
            0.0
        };
        let squared = sample * sample;
        let weighted = squared * window;
        buckets[beat_index] += weighted;
    }
    let mut score = 0.0;
    for &bucket in &buckets {
        score += bucket;
    }
    Ok(Score { score, buckets })
}

/// Fixed2x interpolation with clamped neighbors and observed fused operation order.
/// Overshoot is retained; later native stages perform their own clamping.
pub fn upsample_2x(samples: &[f64]) -> Vec<f64> {
    let mut result = Vec::with_capacity(samples.len() * 2);
    for (index, &current) in samples.iter().enumerate() {
        let previous = samples[index.saturating_sub(1)];
        let next = samples[(index + 1).min(samples.len() - 1)];
        let next_next = samples[(index + 2).min(samples.len() - 1)];
        result.push(current);
        let half = current * 0.5625;
        let half = previous.mul_add(-0.0625, half);
        let half = next.mul_add(0.5625, half);
        let half = next_next.mul_add(-0.0625, half);
        result.push(half);
    }
    result
}

pub mod tempo;

pub mod full;

pub mod bounds;
