pub use crate::dsp::legacy_real_fft_rust_v2::complex_first::Complex;

#[derive(Debug, PartialEq, Eq)]
pub enum InputError {
    Length,
    Domain,
}

/// Compose sixteen radix-sixteen blocks while retaining physical region order.
/// The separate output preserves the original kernel's read-before-store behavior.
pub fn middle_stage(input: &[Complex]) -> Result<Vec<Complex>, InputError> {
    if input.len() != 4096 {
        return Err(InputError::Length);
    }
    if input.iter().any(|v| {
        !v.re.is_finite() || !v.im.is_finite() || v.re.abs() > 1.0e26 || v.im.abs() > 1.0e26
    }) {
        return Err(InputError::Domain);
    }
    let coefficients =
        crate::dsp::legacy_real_fft_rust_v2::complex_tables::initial_mid_coefficients();
    let mut out = vec![Complex::default(); 4096];
    for block in 0..16 {
        for lane in 0..16 {
            for slot in 0..16 {
                let mut imaginary = input[block * 256 + lane].im;
                let mut real = input[block * 256 + lane].re;
                for q in 1..16 {
                    let sample = input[block * 256 + q * 16 + lane];
                    let index = block * 480 + (q - 1) * 32 + slot;
                    let cosine = coefficients[index];
                    let sine = coefficients[index + 16];
                    imaginary = sample.im.mul_add(cosine, imaginary);
                    real = sample.im.mul_add(sine, real);
                    imaginary = (-sample.re).mul_add(sine, imaginary);
                    real = sample.re.mul_add(cosine, real);
                }
                out[block * 256 + slot * 16 + lane] = Complex {
                    re: real,
                    im: imaginary,
                };
            }
        }
    }
    Ok(out)
}
