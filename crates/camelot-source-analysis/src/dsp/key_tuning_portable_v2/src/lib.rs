use base64::{Engine, engine::general_purpose::STANDARD};
use serde::Deserialize;

mod positive_logarithm;

pub fn hanning(position: f32) -> f32 {
    if position > 0.0 && position < 1.0 {
        let angle = f64::from_bits(0x401921fb54442d18) * f64::from(position);
        angle.cos().mul_add(-0.5, 0.5) as f32
    } else {
        0.0
    }
}

fn a_weight(f: f64) -> f64 {
    let numerator = (((f * 148840000.0) * f) * f) * f;
    let a = f.mul_add(f, 424.36000000000007);
    let b = f.mul_add(f, 11599.29);
    let c = f.mul_add(f, 544496.4099999999);
    numerator / (f.mul_add(f, 148840000.0) * (a * (b * c).sqrt()))
}

pub fn score_frame(peaks: &[[f32; 2]]) -> [f32; 16] {
    let mut result = [0.0_f32; 16];
    for &[bin, amplitude] in peaks {
        let frequency = bin * (44100.0_f32 / 4096.0);
        let frequency64 = f64::from(frequency);
        let denominator = frequency64.powf(0.25);
        let logarithm = positive_logarithm::positive_logarithm(frequency / 440.0)
            .expect("validated positive frequency ratio");
        let semitone = (logarithm / f32::from_bits(0x3f317218)) * 12.0;
        let fraction = f64::from(semitone - semitone.floor());
        let gain = (a_weight(frequency64) * f64::from(amplitude)) / denominator;
        for (i, out) in result.iter_mut().enumerate() {
            let difference = i as f64 / 16.0 - fraction;
            let wrapped = difference - (difference + 0.5).floor();
            let position = (wrapped / 0.8 + 0.5) as f32;
            let value = gain * f64::from(hanning(position));
            let square = value * value;
            *out = (square + f64::from(*out)) as f32;
        }
    }
    result
}

pub fn slice(scores: &[[f32; 16]], frame: i32, phase: f64) -> f64 {
    if frame < 0 || frame as usize >= scores.len() {
        return 0.0;
    }
    let position = 16.0 * phase;
    let index = position.floor() as i32;
    let fraction = position - f64::from(index);
    let lo = f64::from(scores[frame as usize][(index & 15) as usize]);
    let hi = f64::from(scores[frame as usize][((index + 1) & 15) as usize]);
    let product = fraction * hi;
    lo.mul_add(1.0 - fraction, product)
}

pub fn region(scores: &[[f32; 16]], center: i32, radius: i32, curve: [f64; 3]) -> f64 {
    let mut result = 0.0;
    for j in -radius..=radius {
        let x = f64::from(j) / f64::from(radius);
        let a = x * curve[0];
        let b = x * curve[1];
        let phase = a.mul_add(x, b) + curve[2];
        let value = slice(scores, center + j, phase);
        let position = ((f64::from(j) * 0.5) / f64::from(radius + 1)) + 0.5;
        let weight = hanning(position as f32);
        result = value.mul_add(f64::from(weight), result);
    }
    result
}

fn search(
    scores: &[[f32; 16]],
    center: i32,
    seed: [f64; 3],
    widths: [f64; 3],
    counts: [i32; 3],
) -> [f64; 3] {
    let step = std::array::from_fn::<_, 3, _>(|i| {
        let value = if counts[i] > 0 {
            widths[i] / f64::from(counts[i])
        } else {
            widths[i]
        };
        if value <= 0.0 { 1.0 } else { value }
    });
    let mut best_score = -1000000000.0;
    let mut best = seed;
    for a in -counts[0]..=counts[0] {
        for b in -counts[1]..=counts[1] {
            for c in -counts[2]..=counts[2] {
                let candidate = [
                    step[0].mul_add(f64::from(a), seed[0]),
                    step[1].mul_add(f64::from(b), seed[1]),
                    step[2].mul_add(f64::from(c), seed[2]),
                ];
                let score = region(scores, center, 16, candidate);
                if score > best_score {
                    best_score = score;
                    best = candidate;
                }
            }
        }
    }
    best
}

pub fn best_curve(scores: &[[f32; 16]], center: i32) -> [f64; 3] {
    let mut curve = search(scores, center, [0.0; 3], [0.0, 0.0, 0.5], [0, 0, 8]);
    let mut widths = [0.05, 0.4, 0.2];
    for _ in 0..8 {
        curve = search(scores, center, curve, widths, [2; 3]);
        for width in &mut widths {
            *width *= 0.7;
        }
    }
    curve
}

pub fn finish(scores: &[[f32; 16]]) -> Vec<f64> {
    let mut result = vec![0.0_f64; scores.len()];
    let mut previous = [0.0_f64; 3];
    for center in (0..scores.len()).step_by(16) {
        let [a, b, c] = best_curve(scores, center as i32);
        let previous_value = previous[2] + (previous[0] * 0.5).mul_add(0.5, previous[1] * 0.5);
        let next_value = c + (a * -0.5).mul_add(-0.5, b * -0.5);
        let adjusted = c + ((previous_value - next_value) + 0.5).floor();
        for j in -16..=16 {
            let index = center as i32 + j;
            if index < 0 || index as usize >= result.len() {
                continue;
            }
            let old = result[index as usize];
            let x = f64::from(j) * 0.0625;
            let target = adjusted + (a * x).mul_add(x, b * x);
            let position = (f64::from(j + 17) / 34.0).min(0.5);
            let weight = hanning(position as f32).min(1.0);
            result[index as usize] = (target - old).mul_add(f64::from(weight), old);
        }
        previous = [a, b, adjusted];
    }
    result
}

#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
enum Stage {
    Scores,
    Tuning,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Input {
    peaks_u32_f32pair_le_base64: String,
    stage: Stage,
}

pub fn process_json(text: &str) -> Result<Vec<u8>, String> {
    let input: Input = serde_json::from_str(text).map_err(|e| e.to_string())?;
    let bytes = STANDARD
        .decode(input.peaks_u32_f32pair_le_base64)
        .map_err(|e| e.to_string())?;
    let mut position = 0;
    fn word(bytes: &[u8], position: &mut usize) -> Result<[u8; 4], String> {
        let part = bytes
            .get(*position..*position + 4)
            .ok_or("truncated peak bytes")?;
        *position += 4;
        Ok(part.try_into().unwrap())
    }
    let count = u32::from_le_bytes(word(&bytes, &mut position)?) as usize;
    if count > 100_000 {
        return Err("frame count exceeds limit".into());
    }
    let mut scores = Vec::with_capacity(count);
    for _ in 0..count {
        let peak_count = u32::from_le_bytes(word(&bytes, &mut position)?) as usize;
        if peak_count > 4096 {
            return Err("too many peaks per frame".into());
        }
        let mut peaks = Vec::with_capacity(peak_count);
        for _ in 0..peak_count {
            let bin = f32::from_le_bytes(word(&bytes, &mut position)?);
            let amplitude = f32::from_le_bytes(word(&bytes, &mut position)?);
            if !bin.is_finite()
                || bin <= 0.0
                || bin > 1024.0
                || ((bin * (44100.0_f32 / 4096.0)) / 440.0) == 0.0
                || !amplitude.is_finite()
                || !(0.0..=1e9).contains(&amplitude)
            {
                return Err("unsupported peak value".into());
            }
            peaks.push([bin, amplitude]);
        }
        let row = score_frame(&peaks);
        if row.iter().any(|v| !v.is_finite()) {
            return Err("nonfinite score".into());
        }
        scores.push(row);
    }
    if position != bytes.len() {
        return Err("unexpected trailing peak bytes".into());
    }
    let mut result = Vec::new();
    match input.stage {
        Stage::Scores => {
            for row in scores {
                for value in row {
                    result.extend_from_slice(&value.to_le_bytes());
                }
            }
        }
        Stage::Tuning => {
            for value in finish(&scores) {
                if !value.is_finite() {
                    return Err("nonfinite tuning".into());
                }
                result.extend_from_slice(&value.to_le_bytes());
            }
        }
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn isolated_pitch_scores_the_matching_phase() {
        let peak = [[440.0 / (44100.0 / 4096.0), 100.0]];
        let row = score_frame(&peak);
        assert!(row[0] > row[1]);
        assert!(row[1] > 0.0);
        assert_eq!(row[8], 0.0);
    }

    #[test]
    fn negative_slice_phase_uses_floor_before_wrap() {
        let mut row = [0.0; 16];
        row[0] = 1.0;
        row[15] = 3.0;
        assert_eq!(slice(&[row], 0, -1.0 / 32.0), 2.0);
        assert_eq!(slice(&[row], 0, 1.0), 1.0);
        assert_eq!(slice(&[row], -1, 0.0), 0.0);
    }

    #[test]
    fn region_keeps_outside_frames_zero_without_rescaling() {
        let rows = [[1.0; 16]; 3];
        assert_eq!(region(&rows, 0, 1, [0.0; 3]), 1.5);
        assert_eq!(region(&rows, 1, 1, [0.0; 3]), 2.0);
    }

    #[test]
    fn empty_frame_stays_zero() {
        assert_eq!(score_frame(&[]), [0.0; 16]);
    }

    #[test]
    fn zero_scores_preserve_the_observed_optimizer_tie_result() {
        let tuning = finish(&[[0.0; 16]]);
        assert_eq!(tuning[0].to_bits(), 0xbfc069fe4b74f370);
    }

    fn numeric_input(bytes: &[u8], stage: &str) -> serde_json::Value {
        serde_json::json!({"peaks_u32_f32pair_le_base64": STANDARD.encode(bytes), "stage": stage})
    }

    #[test]
    fn worker_contract_excludes_original_tuning_inputs() {
        let mut input = numeric_input(&0_u32.to_le_bytes(), "tuning");
        assert_eq!(process_json(&input.to_string()).unwrap(), Vec::<u8>::new());
        input["original_tuning"] = serde_json::json!([0.25]);
        assert!(process_json(&input.to_string()).is_err());
    }

    #[test]
    fn worker_rejects_truncated_and_trailing_peak_bytes() {
        assert!(process_json(&numeric_input(&[], "scores").to_string()).is_err());
        assert!(process_json(&numeric_input(&1_u32.to_le_bytes(), "tuning").to_string()).is_err());
        assert!(process_json(&numeric_input(&[0, 0, 0, 0, 1], "scores").to_string()).is_err());
    }

    #[test]
    fn worker_rejects_nonfinite_or_underflowed_frequency() {
        for bin in [f32::NAN, f32::from_bits(1)] {
            let mut bytes = Vec::new();
            bytes.extend_from_slice(&1_u32.to_le_bytes());
            bytes.extend_from_slice(&1_u32.to_le_bytes());
            bytes.extend_from_slice(&bin.to_le_bytes());
            bytes.extend_from_slice(&1.0_f32.to_le_bytes());
            assert!(process_json(&numeric_input(&bytes, "scores").to_string()).is_err());
        }
    }
}
