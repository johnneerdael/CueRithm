use base64::{Engine, engine::general_purpose::STANDARD};
use serde::Deserialize;

mod positive_logarithm;
mod positive_power;

const BIN_HZ: f32 = 44100.0 / 4096.0;
const POWER_A: f32 = f32::from_bits(0x40403133);
const POWER_B: f32 = f32::from_bits(0x3eaa7efa);

pub fn window(position: f32) -> f32 {
    if position > 0.0 && position < 1.0 {
        let angle = f64::from(position) * f64::from_bits(0x401921fb54442d18);
        angle.cos().mul_add(-0.5, 0.5) as f32
    } else {
        0.0
    }
}

fn a_weight(f: f64) -> f32 {
    let constant = 148840000.0;
    let numerator = (((f * constant) * f) * f) * f;
    let first = f.mul_add(f, 424.36000000000007);
    let second = f.mul_add(f, 11599.29);
    let third = f.mul_add(f, 544496.4099999999);
    let middle = first * (second * third).sqrt();
    (numerator / (f.mul_add(f, constant) * middle)) as f32
}

pub struct Kernel {
    a_power: [f32; 180],
    bin_frequency: [f32; 88],
}
impl Default for Kernel {
    fn default() -> Self {
        let mut a_power = [0.0; 180];
        for (i, out) in a_power.iter_mut().enumerate() {
            let exponent = (i as f32).mul_add(f32::from_bits(0x3d2aaaab), -4.0);
            let frequency = exponent.exp2() * 440.0;
            *out = positive_power::positive_power(a_weight(f64::from(frequency)), POWER_A)
                .expect("finite positive A-weight table");
        }
        let mut bin_frequency = [0.0; 88];
        for (i, out) in bin_frequency.iter_mut().enumerate() {
            let exponent = (i as f32).mul_add(f32::from_bits(0x3daaaaab), -4.0);
            *out = (exponent.exp2() * 440.0) / BIN_HZ;
        }
        Self {
            a_power,
            bin_frequency,
        }
    }
}
impl Kernel {
    pub fn frame(&self, peaks: &[[f32; 2]], tuning: f64) -> [f32; 88] {
        let mut integer = [0.0_f32; 89];
        let mut half = [0.0_f32; 89];
        let fraction = tuning - (tuning + 0.5).floor();
        for &[bin, amplitude] in peaks {
            let ratio = (BIN_HZ * bin) / 440.0;
            let logarithm = positive_logarithm::positive_logarithm(ratio)
                .expect("finite positive spectral ratio");
            let initial = (logarithm / f32::from_bits(0x3f317218) + 4.0) * 12.0;
            let pitch = (f64::from(initial) - fraction) as f32;
            let index = (f64::from(pitch) + 0.5) as i32;
            if index < 0 {
                continue;
            }
            let position = (pitch - index as f32) / 0.8 + 0.5;
            let weight = window(position);
            let power = amplitude * amplitude;
            if index <= 88 {
                let candidate = power * weight;
                if integer[index as usize] < candidate {
                    integer[index as usize] = candidate;
                }
            }
            let half_pitch = pitch - 0.5;
            let index = (f64::from(half_pitch) + 0.5) as i32;
            if index < 0 {
                continue;
            }
            let position = (half_pitch - index as f32) / 0.8 + 0.5;
            let weight = window(position);
            if index <= 88 {
                let candidate = power * weight;
                if half[index as usize] < candidate {
                    half[index as usize] = candidate;
                }
            }
        }
        // Guard cells88 remain in raw power units, as in the original path.
        for i in 0..88 {
            integer[i] = positive_power::positive_power(integer[i] * self.a_power[i * 2], POWER_B)
                .expect("finite nonnegative raster value")
                .sqrt()
                * 6000000.0;
            half[i] = positive_power::positive_power(half[i] * self.a_power[i * 2 + 1], POWER_B)
                .expect("finite nonnegative raster value")
                .sqrt()
                * 6000000.0;
        }
        let mut result = [0.0; 88];
        for n in 2..88 {
            let mut neighborhood = half[n - 2] + integer[n - 1];
            neighborhood += half[n - 1];
            neighborhood += half[n];
            neighborhood += integer[n + 1];
            neighborhood += half[n + 1];
            let threshold = neighborhood * 0.4;
            let retained = if integer[n] < threshold {
                0.0
            } else {
                integer[n]
            };
            result[n] = retained / self.bin_frequency[n];
        }
        result
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Input {
    peaks_u32_f32pair_le_base64: String,
    tuning_bits: Vec<String>,
}

pub fn process_json(text: &str) -> Result<Vec<u8>, String> {
    let input: Input = serde_json::from_str(text).map_err(|e| e.to_string())?;
    if input.tuning_bits.len() > 100_000 {
        return Err("frame count exceeds limit".into());
    }
    let bytes = STANDARD
        .decode(input.peaks_u32_f32pair_le_base64)
        .map_err(|e| e.to_string())?;
    let mut position = 0;
    fn word(bytes: &[u8], position: &mut usize) -> Result<[u8; 4], String> {
        let part = bytes
            .get(*position..*position + 4)
            .ok_or("truncated peak data")?;
        *position += 4;
        Ok(part.try_into().unwrap())
    }
    let count = u32::from_le_bytes(word(&bytes, &mut position)?) as usize;
    if count != input.tuning_bits.len() {
        return Err("peak/tuning frame counts differ".into());
    }
    let kernel = Kernel::default();
    let mut result = Vec::with_capacity(count * 352);
    for bits in input.tuning_bits {
        if bits.len() != 16 {
            return Err("expected16 tuning hex digits".into());
        }
        let tuning = f64::from_bits(u64::from_str_radix(&bits, 16).map_err(|e| e.to_string())?);
        if !tuning.is_finite() || tuning.abs() > 1000.0 {
            return Err("unsupported tuning value".into());
        }
        let peak_count = u32::from_le_bytes(word(&bytes, &mut position)?) as usize;
        if peak_count > 4096 {
            return Err("too many peaks in frame".into());
        }
        let mut peaks = Vec::with_capacity(peak_count);
        for _ in 0..peak_count {
            let bin = f32::from_le_bytes(word(&bytes, &mut position)?);
            let amplitude = f32::from_le_bytes(word(&bytes, &mut position)?);
            if !bin.is_finite()
                || !(0.0..=1024.0).contains(&bin)
                || bin == 0.0
                || ((BIN_HZ * bin) / 440.0) == 0.0
                || !amplitude.is_finite()
                || !(0.0..=1e9).contains(&amplitude)
            {
                return Err("unsupported spectral peak".into());
            }
            peaks.push([bin, amplitude]);
        }
        for value in kernel.frame(&peaks, tuning) {
            if !value.is_finite() {
                return Err("nonfinite cache result".into());
            }
            result.extend_from_slice(&value.to_le_bytes());
        }
    }
    if position != bytes.len() {
        return Err("unexpected trailing peak bytes".into());
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn peak(amplitude: f32) -> [[f32; 2]; 1] {
        [[440.0 / (44100.0 / 4096.0), amplitude]]
    }

    #[test]
    fn empty_peak_frames_remain_silent() {
        assert_eq!(Kernel::default().frame(&[], 0.25), [0.0; 88]);
    }

    #[test]
    fn isolated_440hz_peak_survives_in_bin48() {
        let values = Kernel::default().frame(&peak(100.0), 0.0);
        assert!(values[48] > 0.0);
        assert!(values.iter().enumerate().all(|(i, &v)| i == 48 || v == 0.0));
    }

    #[test]
    fn stronger_isolated_peak_has_larger_retained_value() {
        let kernel = Kernel::default();
        assert!(kernel.frame(&peak(200.0), 0.0)[48] > kernel.frame(&peak(100.0), 0.0)[48]);
    }

    #[test]
    fn tuning_compensation_uses_only_the_wrapped_fraction() {
        let kernel = Kernel::default();
        assert_eq!(
            kernel.frame(&peak(100.0), -0.25),
            kernel.frame(&peak(100.0), 1.75)
        );
    }

    #[test]
    fn upper_guard_cell_stays_in_raw_power_units() {
        let kernel = Kernel::default();
        let bin =
            |note: f32| (note.mul_add(f32::from_bits(0x3daaaaab), -4.0).exp2() * 440.0) / BIN_HZ;
        let main = [bin(87.0), 100.0];
        let baseline = kernel.frame(&[main], 0.0);
        let with_guard = kernel.frame(&[main, [bin(88.0), 2000.0]], 0.0);
        assert!(baseline[87] > 0.0);
        assert_eq!(baseline[87].to_bits(), with_guard[87].to_bits());
    }

    #[test]
    fn raised_cosine_has_open_edges_and_unit_center() {
        assert_eq!(window(0.0), 0.0);
        assert_eq!(window(1.0), 0.0);
        assert_eq!(window(0.5), 1.0);
    }

    fn numeric_input(peaks: &[u8], tuning: &[&str]) -> serde_json::Value {
        serde_json::json!({ "peaks_u32_f32pair_le_base64": STANDARD.encode(peaks), "tuning_bits": tuning })
    }

    #[test]
    fn input_contract_excludes_expected_caches() {
        let mut input = numeric_input(&0_u32.to_le_bytes(), &[]);
        assert_eq!(process_json(&input.to_string()).unwrap(), Vec::<u8>::new());
        input["expected_cache"] = "00000000".into();
        assert!(process_json(&input.to_string()).is_err());
    }

    #[test]
    fn malformed_peak_payloads_are_rejected() {
        assert!(process_json(&numeric_input(&[], &[]).to_string()).is_err());
        assert!(process_json(&numeric_input(&1_u32.to_le_bytes(), &[]).to_string()).is_err());
        let bytes = [0, 0, 0, 0, 1];
        assert!(process_json(&numeric_input(&bytes, &[]).to_string()).is_err());
    }

    #[test]
    fn nonfinite_tuning_and_peak_values_are_rejected() {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&1_u32.to_le_bytes());
        bytes.extend_from_slice(&0_u32.to_le_bytes());
        assert!(process_json(&numeric_input(&bytes, &["7ff8000000000000"]).to_string()).is_err());
        bytes[4..8].copy_from_slice(&1_u32.to_le_bytes());
        bytes.extend_from_slice(&f32::NAN.to_le_bytes());
        bytes.extend_from_slice(&1.0_f32.to_le_bytes());
        assert!(process_json(&numeric_input(&bytes, &["0000000000000000"]).to_string()).is_err());
    }
}

#[cfg(test)]
mod positive_ratio_contract {
    use super::*;
    #[test]
    fn positive_peak_ratio_underflow_is_rejected() {
        let words = [1_u32, 1_u32, 1_u32, 100.0_f32.to_bits()];
        let bytes: Vec<u8> = words.into_iter().flat_map(u32::to_le_bytes).collect();
        let input = serde_json::json!({
            "peaks_u32_f32pair_le_base64": STANDARD.encode(bytes),
            "tuning_bits": ["0000000000000000"]
        });
        assert!(process_json(&input.to_string()).is_err());
    }
}
