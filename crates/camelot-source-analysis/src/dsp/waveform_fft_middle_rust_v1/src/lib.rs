pub use crate::dsp::waveform_fft_first_rust_v1::Complex;

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
    if input
        .iter()
        .any(|v| !v.re.is_finite() || !v.im.is_finite() || v.re.abs() > 128.0 || v.im.abs() > 128.0)
    {
        return Err(InputError::Domain);
    }
    let coefficients = crate::dsp::waveform_fft_tables_rust_v1::initial_mid_coefficients();
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zero_stays_zero() {
        let out = middle_stage(&vec![Complex::default(); 4096]).unwrap();
        assert_eq!(out.len(), 4096);
        assert!(
            out.iter()
                .all(|v| v.re.to_bits() == 0 && v.im.to_bits() == 0)
        );
    }

    #[test]
    fn each_block_origin_spreads_over_physical_slots() {
        let mut input = vec![Complex::default(); 4096];
        for block in 0..16 {
            input[block * 256].re = 1.0;
        }
        let out = middle_stage(&input).unwrap();
        for block in 0..16 {
            for slot in 0..16 {
                assert_eq!(out[block * 256 + slot * 16].re, 1.0);
                assert_eq!(out[block * 256 + slot * 16].im, 0.0);
            }
        }
    }

    #[test]
    fn rejects_missing_or_nonfinite_stage_data() {
        assert_eq!(middle_stage(&[]), Err(InputError::Length));
        let mut input = vec![Complex::default(); 4096];
        input[13].re = f32::NAN;
        assert_eq!(middle_stage(&input), Err(InputError::Domain));
    }
}
