use base64::{Engine, engine::general_purpose::STANDARD};
use serde::{Deserialize, Serialize};

/// Finite-cache path reconstructed from retained ARM64 instruction evidence.
pub fn range(
    cache: &[[f32; 88]],
    sample_rate: f64,
    fft: i32,
    hop: i32,
    start: f64,
    end: f64,
) -> [f64; 88] {
    let coordinate = |t| ((sample_rate * t - f64::from(fft / 2)) / f64::from(hop)) - 1.0;
    let mut lo = coordinate(start);
    let mut hi = coordinate(end);
    if hi - lo < 1.0 {
        lo = (lo + hi).mul_add(0.5, -0.5);
        hi = lo + 1.0;
    }
    let first = lo.floor() as i32;
    let last = hi.floor() as i32;
    let mut result = [0.0; 88];
    if last < first {
        return result;
    }
    let begin = first.max(0) as usize;
    let stop = ((last + 1).max(0) as usize).min(cache.len());
    let reciprocal = 1.0 / (hi - lo);
    let first_weight = (f64::from(first) - lo) + 1.0;
    // Preserve both operations, rather than replacing this with hi - last.
    let last_weight = (hi - f64::from(last + 1)) + 1.0;
    for i in begin..stop {
        let mut boundary = if i == first as usize {
            first_weight
        } else {
            1.0
        };
        if i == last as usize {
            boundary = last_weight;
        }
        let weight = reciprocal * boundary;
        for (j, out) in result.iter_mut().enumerate() {
            let a = if i == 0 {
                0.0
            } else {
                f64::from(cache[i - 1][j])
            };
            let b = f64::from(cache[i][j]);
            let c = if i + 1 < cache.len() {
                f64::from(cache[i + 1][j])
            } else {
                0.0
            };
            let median = a.min(b).max(a.max(b).min(c));
            *out = weight.mul_add(median, *out);
        }
    }
    result
}

fn to_beat(grid: &[f64], t: f64) -> f64 {
    let mut i = 0;
    // Preserve the original comparison direction; this is not a binary search.
    if grid.len() >= 3 && t < grid[1] {
        let mut j = 2;
        while j < grid.len() - 1 && t < grid[j] {
            j += 1;
        }
        i = j - 1;
    }
    i as f64 + (t - grid[i]) / (grid[i + 1] - grid[i])
}

fn to_time(grid: &[f64], beat: f64) -> f64 {
    let i = (beat.floor() as i64).clamp(0, grid.len() as i64 - 2) as usize;
    (beat - i as f64).mul_add(grid[i + 1] - grid[i], grid[i])
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Input {
    cache_f32le_base64: String,
    frame_count: usize,
    beat_time_bits: Vec<String>,
    downbeat_bits: String,
    duration_bits: String,
}

#[derive(Serialize)]
struct Output {
    first_bar: i32,
    last_bar_exclusive: i32,
    feature_bits: Vec<String>,
}

fn number(bits: &str) -> Result<f64, String> {
    if bits.len() != 16 {
        return Err("expected 16 hexadecimal digits".into());
    }
    let value = f64::from_bits(u64::from_str_radix(bits, 16).map_err(|e| e.to_string())?);
    if !value.is_finite() {
        return Err("nonfinite input".into());
    }
    Ok(value)
}

pub fn process_json(text: &str) -> Result<String, String> {
    let input: Input = serde_json::from_str(text).map_err(|e| e.to_string())?;
    if input.frame_count > 1_000_000
        || input.beat_time_bits.len() < 2
        || input.beat_time_bits.len() > 100_000
    {
        return Err("unsupported input dimensions".into());
    }
    let bytes = STANDARD
        .decode(input.cache_f32le_base64)
        .map_err(|e| e.to_string())?;
    if bytes.len() != input.frame_count * 88 * 4 {
        return Err("cache length differs from frame count".into());
    }
    let mut cache = Vec::with_capacity(input.frame_count);
    for frame in bytes.as_chunks::<352>().0 {
        let mut values = [0.0; 88];
        for (j, chunk) in frame.as_chunks::<4>().0.iter().enumerate() {
            let value = f32::from_le_bytes(*chunk);
            if !value.is_finite() || value < 0.0 {
                return Err("cache must contain finite nonnegative values".into());
            }
            values[j] = value;
        }
        cache.push(values);
    }
    let grid = input
        .beat_time_bits
        .iter()
        .map(|s| number(s))
        .collect::<Result<Vec<_>, _>>()?;
    if grid.windows(2).any(|w| w[1] <= w[0]) {
        return Err("grid must increase strictly".into());
    }
    let downbeat = number(&input.downbeat_bits)?;
    let duration = number(&input.duration_bits)?;
    if !(0.0..=100_000.0).contains(&duration)
        || grid.iter().any(|t| t.abs() > 100_000.0)
        || downbeat.abs() > 100_000.0
    {
        return Err("unsupported time range".into());
    }
    let anchor = to_beat(&grid, downbeat);
    let first_float = ((to_beat(&grid, 0.0) - anchor) * 0.25 + 1e-9).floor();
    let last_float = ((to_beat(&grid, duration) - anchor) * 0.25 - 1e-9).ceil();
    if !first_float.is_finite()
        || !last_float.is_finite()
        || first_float.abs() > 100_000.0
        || last_float.abs() > 100_000.0
        || last_float < first_float
    {
        return Err("unsupported bar range".into());
    }
    let first_bar = first_float as i32;
    let last_bar_exclusive = last_float as i32;
    let mut features: Vec<f64> = Vec::new();
    for bar in first_bar..last_bar_exclusive {
        let start = to_time(&grid, anchor + f64::from(bar * 4));
        let end = to_time(&grid, anchor + f64::from((bar + 1) * 4));
        if !start.is_finite()
            || !end.is_finite()
            || start.abs() > 100_000.0
            || end.abs() > 100_000.0
        {
            return Err("unsupported extrapolated range".into());
        }
        let raw = range(&cache, 44100.0, 4096, 2048, start, end);
        let offset = (bar & 3) as usize * 88;
        features.resize(features.len().max(offset + 88), 0.0);
        for (j, value) in raw.into_iter().enumerate() {
            features[offset + j] += value;
        }
    }
    if features.iter().any(|v| !v.is_finite()) {
        return Err("nonfinite result".into());
    }
    serde_json::to_string(&Output {
        first_bar,
        last_bar_exclusive,
        feature_bits: features
            .into_iter()
            .map(|v| format!("{:016x}", v.to_bits()))
            .collect(),
    })
    .map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frames() -> Vec<[f32; 88]> {
        [2.0, 10.0, 4.0].into_iter().map(|v| [v; 88]).collect()
    }

    #[test]
    fn median_removes_an_isolated_peak() {
        assert_eq!(range(&frames(), 1.0, 2, 1, 2.0, 4.0), [3.0; 88]);
    }

    #[test]
    fn clipping_keeps_original_denominator() {
        assert_eq!(range(&frames(), 1.0, 2, 1, 1.5, 2.5), [1.0; 88]);
    }

    #[test]
    fn short_window_expands_around_midpoint() {
        assert_eq!(range(&frames(), 1.0, 2, 1, 2.25, 2.5), [1.75; 88]);
    }

    #[test]
    fn absent_neighbors_are_zero_and_disjoint_ranges_are_empty() {
        assert_eq!(range(&[[42.0; 88]], 1.0, 2, 1, 2.0, 3.0), [0.0; 88]);
        assert_eq!(range(&frames(), 1.0, 2, 1, -10.0, -8.0), [0.0; 88]);
    }

    #[test]
    fn irregular_grid_preserves_original_extrapolation_quirk() {
        let grid = [1.0, 2.0, 4.0, 8.0];
        assert_eq!(to_beat(&grid, 0.0), 1.0);
        assert_eq!(to_beat(&grid, 8.0), 7.0);
    }

    #[test]
    fn time_interpolation_requires_fused_rounding() {
        let epsilon = 2.0_f64.powi(-27);
        let time = to_time(&[-1.0, -epsilon], 1.0 + epsilon);
        assert_eq!(time.to_bits(), (-2.0_f64.powi(-54)).to_bits());
        let product = (1.0 + epsilon) * (1.0 - epsilon);
        assert_eq!(product - 1.0, 0.0);
    }

    fn empty_input() -> serde_json::Value {
        serde_json::json!({
            "cache_f32le_base64": "", "frame_count": 0,
            "beat_time_bits": ["0000000000000000", "3ff0000000000000"],
            "downbeat_bits": "0000000000000000", "duration_bits": "0000000000000000"
        })
    }

    #[test]
    fn json_contract_rejects_reference_fields() {
        let mut input = empty_input();
        assert!(process_json(&input.to_string()).is_ok());
        input["Features"] = serde_json::json!([42.0]);
        assert!(process_json(&input.to_string()).is_err());
    }

    #[test]
    fn json_contract_rejects_invalid_numeric_context() {
        let mut input = empty_input();
        input["duration_bits"] = "7ff8000000000000".into();
        assert!(process_json(&input.to_string()).is_err());
        let mut input = empty_input();
        input["frame_count"] = 1.into();
        assert!(process_json(&input.to_string()).is_err());
        let mut input = empty_input();
        input["beat_time_bits"][1] = "0000000000000000".into();
        assert!(process_json(&input.to_string()).is_err());
    }
}
