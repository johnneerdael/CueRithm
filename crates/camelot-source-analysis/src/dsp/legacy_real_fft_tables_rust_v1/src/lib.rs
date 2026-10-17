//! Fixed real FFT setup tables, source cosf and double-sin result profiles.
pub use crate::dsp::legacy_window_rust_v1::positive_cosine;
pub const MIN_LOG2: u32 = 4;
pub const MAX_LOG2: u32 = 14;
pub const MAX_ANGLE_BITS: u32 = 0x3fc9_0fdb;
const SIN: [f64; 6] = [
    f64::from_bits(0x3de5_d8fd_1fd1_9ccd),
    f64::from_bits(0xbe5a_e5e5_a929_1f5d),
    f64::from_bits(0x3ec7_1de3_567d_48a1),
    f64::from_bits(0xbf2a_01a0_19bf_df03),
    f64::from_bits(0x3f81_1111_1110_f7d0),
    f64::from_bits(0xbfc5_5555_5555_5548),
];
const COS: [f64; 7] = [
    f64::from_bits(0xbda8_fa49_a086_1a9b),
    f64::from_bits(0x3e21_ee9d_7b4e_3f05),
    f64::from_bits(0xbe92_7e4f_7eac_4bc6),
    f64::from_bits(0x3efa_01a0_19c8_44f5),
    f64::from_bits(0xbf56_c16c_16c1_4f91),
    f64::from_bits(0x3fa5_5555_5555_554b),
    f64::from_bits(0xbfe0_0000_0000_0000),
];

/// Own-system double sin profile on widened f32 angles +0..=pi/2_f32.
/// No claim about errno, floating exceptions, negative signs or other domains.
pub fn positive_double_sine(x: f32) -> Result<f64, String> {
    let bits = x.to_bits();
    if bits >> 31 != 0 || bits > MAX_ANGLE_BITS {
        return Err("requires +0 or positive finite angle<=pi/2_f32".into());
    }
    if bits == 0 {
        return Ok(0.0);
    }
    let x = f64::from(x);
    if x <= f64::from_bits(0x3fe9_21fb_5444_2d18) {
        // sin+0x6f8..0x738: six coefficients; final square*P is separate.
        let square = x * x;
        let mut polynomial = SIN[0];
        for coefficient in &SIN[1..] {
            polynomial = square.mul_add(polynomial, *coefficient);
        }
        let correction = square * polynomial;
        return Ok(x.mul_add(correction, x) + 0.0);
    }
    // sin+0x108..0x148: all accepted reduced angles have nearest==1.
    let nearest = (x * f64::from_bits(0x3fe4_5f30_6dc9_c883)).round_ties_even();
    let first = nearest * f64::from_bits(0x3ff9_21fb_5444_0000);
    let second = nearest * f64::from_bits(0x3d86_8c23_4c4c_0000);
    let third = nearest * f64::from_bits(0x3b29_8a2e_0370_7345);
    let remainder = x - first;
    let high = remainder - second;
    let error = remainder - high;
    let error = error - second;
    let low = error - third;
    // sin+0x808..0x814 then0x7ac..0x7e4: retain the rounded cross product.
    let cross = high * low;
    let twice_cross = cross + cross;
    let square = high.mul_add(high, twice_cross);
    let mut polynomial = COS[0];
    for coefficient in &COS[1..] {
        polynomial = square.mul_add(polynomial, *coefficient);
    }
    Ok(square.mul_add(polynomial, 1.0))
}

/// Source setup+0x40 table at byte offset2*N: COSf plane then narrowed SINd.
/// Both planes contain index1..=N/4; angle=(two_pi_f32/N)*index in f32.
pub fn real_table(n: usize) -> Result<Vec<f32>, String> {
    if !n.is_power_of_two() || !(1 << MIN_LOG2..=1 << MAX_LOG2).contains(&n) {
        return Err("power-of-two N16..16384 required".into());
    }
    let step = f32::from_bits(0x40c9_0fdb) / n as f32;
    let quarter = n / 4;
    let mut table = vec![0.0; n / 2];
    for index in 1..=quarter {
        let angle = step * index as f32;
        table[index - 1] = positive_cosine(angle)?;
        table[quarter + index - 1] = positive_double_sine(angle)? as f32;
    }
    Ok(table)
}
