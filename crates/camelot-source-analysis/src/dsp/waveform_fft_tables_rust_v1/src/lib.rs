//! Source-derived fixed-profile numerical setup; no original runtime dependency.
const N: usize = 4096;
const TAU: f64 = f64::from_bits(0x401921fb54442d18);
const U64_FRACTION: f64 = f64::from_bits(0x3bf0000000000000);

fn reversed_angle(index: u64) -> f64 {
    let fraction = index.reverse_bits() as f64 * U64_FRACTION;
    fraction * TAU
}

/// Fifteen cosine/sine rows per block, each holding sixteen frequency lanes.
pub fn initial_mid_coefficients() -> Vec<f32> {
    let mut values = vec![0.0; (N >> 8) * 480];
    for block in 0..N >> 8 {
        for column in 0..16 {
            let angle = reversed_angle((block * 16 + column) as u64);
            for row in 1..16 {
                let (sine, cosine) = (angle * row as f64).sin_cos();
                let start = block * 480 + (row - 1) * 32;
                values[start + column] = cosine as f32;
                values[start + 16 + column] = sine as f32;
            }
        }
    }
    values
}

/// Seven radix-sixteen roots in natural column order, cosine then sine.
pub fn final_radix16_coefficients() -> Vec<f32> {
    let mut values = vec![0.0; 7 * 32];
    for row in 1..8 {
        let step = TAU * (row as f64 / 16.0);
        for column in 0..16 {
            let (sine, cosine) = (step * column as f64).sin_cos();
            values[(row - 1) * 32 + column] = cosine as f32;
            values[(row - 1) * 32 + 16 + column] = sine as f32;
        }
    }
    values
}

/// Natural radix lanes within each bit-reversed block; separate trig planes.
pub fn final_twiddles() -> Vec<f32> {
    let mut values = vec![0.0; 2 * N];
    for block in 0..N >> 4 {
        let angle = reversed_angle((block * 16) as u64);
        for column in 0..16 {
            let (sine, cosine) = (angle * column as f64).sin_cos();
            let index = block * 16 + column;
            values[index] = cosine as f32;
            values[N + index] = sine as f32;
        }
    }
    values
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn initial_coefficients_preserve_bit_reversed_output_order() {
        let values = initial_mid_coefficients();
        assert_eq!(values.len(), 7680);
        assert_eq!(
            values[..8].iter().map(|v| v.to_bits()).collect::<Vec<_>>(),
            [
                0x3f800000, 0xbf800000, 0x248d3132, 0xa553c9ca, 0x3f3504f3, 0xbf3504f3, 0xbf3504f3,
                0x3f3504f3
            ]
        );
    }

    #[test]
    fn final_radix_coefficients_preserve_natural_column_order() {
        let values = final_radix16_coefficients();
        assert_eq!(values.len(), 224);
        assert_eq!(
            values[..4].iter().map(|v| v.to_bits()).collect::<Vec<_>>(),
            [0x3f800000, 0x3f6c835e, 0x3f3504f3, 0x3ec3ef15]
        );
        assert_eq!(values[16].to_bits(), 0);
    }

    #[test]
    fn final_twiddles_have_separate_cosine_and_sine_planes() {
        let values = final_twiddles();
        assert_eq!(values.len(), 8192);
        assert!(values[..16].iter().all(|v| v.to_bits() == 1.0f32.to_bits()));
        assert!(values[4096..4112].iter().all(|v| v.to_bits() == 0));
        assert_eq!(values[24].to_bits(), 0x248d3132);
    }
}
