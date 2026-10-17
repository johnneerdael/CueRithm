//! Source-derived Darwin cosine/window numeric profile, positive angles only.
pub const N: usize = 2048;
pub const MAX_ANGLE_BITS: u32 = 0x4149_0fdb;

#[derive(Clone, Copy)]
pub enum Window {
    Onset4,
    Daloopa7,
}

// Own-process libsystem_m coefficient bytes; see legacy-cosf-source-v1.
const COS: [f64; 4] = [
    f64::from_bits(0x3ef9_9169_fc93_8f4a),
    f64::from_bits(0xbf56_c07f_1694_5df4),
    f64::from_bits(0x3fa5_5553_c789_9699),
    f64::from_bits(0xbfdf_ffff_fcb8_2e97),
];
const SIN: [f64; 3] = [
    f64::from_bits(0xbf29_943e_0fc4_818f),
    f64::from_bits(0x3f81_1073_afd1_45eb),
    f64::from_bits(0xbfc5_5554_5268_a02a),
];

fn cosine_polynomial(reduced: f64) -> f64 {
    let square = reduced * reduced;
    let upper = square.mul_add(COS[0], COS[1]);
    let lower = square.mul_add(COS[2], COS[3]);
    let fourth = square * square;
    square.mul_add(fourth.mul_add(upper, lower), 1.0)
}

fn sine_polynomial(reduced: f64) -> f64 {
    let square = reduced * reduced;
    let cube = reduced * square;
    let polynomial = square.mul_add(SIN[0], SIN[1]);
    let fifth = square * cube;
    let lower = cube.mul_add(SIN[2], reduced);
    fifth.mul_add(polynomial, lower)
}

/// Preserve the captured result profile, not arbitrary IEEE cosf behavior.
/// Reject negative signs (including -0), nonfinite values and angles above4pi_f32.
pub fn positive_cosine(x: f32) -> Result<f32, String> {
    let bits = x.to_bits();
    if bits >> 31 != 0 || bits > MAX_ANGLE_BITS {
        return Err("angle requires +0 or finite positive value<=4pi_f32".into());
    }
    if bits < 0x3980_0000 {
        // Original f32 tiny path, 0xc0..0xcc; result1 but source ordering retained.
        return Ok((f32::from_bits(0x4c80_0000) - x) * f32::from_bits(0x3280_0000));
    }
    let x = f64::from(x);
    if bits < 0x3f49_0fdb {
        return Ok(cosine_polynomial(x) as f32);
    }
    let quotient = x * f64::from_bits(0x3fe4_5f30_6dc9_c883);
    let nearest = quotient.round_ties_even();
    let phase = nearest as u64 + 1;
    // FMSUB at0x11c is x-nearest*pi/2, one fused operation.
    let reduced = (-nearest).mul_add(f64::from_bits(0x3ff9_21fb_5444_2d18), x);
    let value = if phase & 1 != 0 {
        cosine_polynomial(reduced)
    } else {
        sine_polynomial(reduced)
    };
    let sign = ((phase >> 1) & 1) << 63;
    Ok(f64::from_bits(value.to_bits() ^ sign) as f32)
}

/// Fixed2048 coefficient generation with source f32 angle and coefficient grouping.
pub fn coefficients(window: Window) -> [f32; N] {
    let two_pi = f32::from_bits(0x40c9_0fdb);
    let four_pi = f32::from_bits(MAX_ANGLE_BITS);
    std::array::from_fn(|index| match window {
        Window::Onset4 => {
            let angle1 = (index as f32 * two_pi) / (N - 1) as f32;
            let angle2 = ((2 * index) as f32 * two_pi) / (N - 1) as f32;
            let first = positive_cosine(angle1).expect("source angle in domain") * -0.5;
            let first = first + f32::from_bits(0x3ed7_0a3d);
            let second = positive_cosine(angle2).expect("source angle in domain")
                * f32::from_bits(0x3da3_d70a);
            second + first
        }
        Window::Daloopa7 => {
            let angle1 = (index as f32 * two_pi) / N as f32;
            let angle2 = (index as f32 * four_pi) / N as f32;
            let first = f64::from(positive_cosine(angle1).expect("source angle in domain")) * -0.5;
            let first = first + 0.42;
            let second = f64::from(positive_cosine(angle2).expect("source angle in domain")) * 0.08;
            (second + first) as f32
        }
    })
}
