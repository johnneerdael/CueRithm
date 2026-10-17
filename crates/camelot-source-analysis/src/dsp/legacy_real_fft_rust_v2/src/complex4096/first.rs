#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Complex {
    pub re: f32,
    pub im: f32,
}

#[derive(Debug, PartialEq, Eq)]
pub enum InputError {
    Length,
    Domain,
}

/// Fixed N4096 first radix-sixteen stage of the observed aligned FFT profile.
///
/// Return physical bit-reversed regions; later stages perform the final ordering.
/// Domain rejection is this prototype's explicit boundary, not an original error ABI.
pub fn first_stage(input: &[Complex]) -> Result<Vec<Complex>, InputError> {
    const N: usize = 4096;
    const M: usize = N / 16;
    if input.len() != N {
        return Err(InputError::Length);
    }
    if input.iter().any(|v| {
        !v.re.is_finite() || !v.im.is_finite() || v.re.abs() > 1.0e24 || v.im.abs() > 1.0e24
    }) {
        return Err(InputError::Domain);
    }
    let coefficients =
        crate::dsp::legacy_real_fft_rust_v2::complex_tables::initial_mid_coefficients();
    let mut output = vec![Complex::default(); N];
    for n in 0..M {
        for slot in 0..16 {
            let mut imaginary = input[n].im;
            let mut real = input[n].re;
            let sign = coefficients[14 * 16 + slot];
            imaginary = input[8 * M + n].im.mul_add(sign, imaginary);
            real = input[8 * M + n].re.mul_add(sign, real);
            for q in 1..8 {
                let p = 16 - q;
                let cosine = coefficients[(q - 1) * 32 + slot];
                let sine = coefficients[(q - 1) * 32 + 16 + slot];
                let a = input[q * M + n];
                let b = input[p * M + n];
                imaginary = a.im.mul_add(cosine, imaginary);
                real = a.im.mul_add(sine, real);
                imaginary = (-a.re).mul_add(sine, imaginary);
                real = a.re.mul_add(cosine, real);
                imaginary = b.im.mul_add(cosine, imaginary);
                real = (-b.im).mul_add(sine, real);
                imaginary = b.re.mul_add(sine, imaginary);
                real = b.re.mul_add(cosine, real);
            }
            output[slot * M + n] = Complex {
                re: real,
                im: imaginary,
            };
        }
    }
    Ok(output)
}
