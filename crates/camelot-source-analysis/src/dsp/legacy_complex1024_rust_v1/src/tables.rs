//! Source-derived fixed-profile numerical setup; no original runtime dependency.
const N: usize = 1024;
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
