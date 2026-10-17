pub use crate::dsp::waveform_fft_first_rust_v1::Complex;

#[derive(Debug, PartialEq, Eq)]
pub enum InputError {
    Length,
    Domain,
}

pub fn fft4096(input: &[Complex]) -> Result<Vec<Complex>, InputError> {
    if input.len() != 4096 {
        return Err(InputError::Length);
    }
    if input
        .iter()
        .any(|v| !v.re.is_finite() || !v.im.is_finite() || v.re.abs() > 4.0 || v.im.abs() > 4.0)
    {
        return Err(InputError::Domain);
    }
    let first = crate::dsp::waveform_fft_first_rust_v1::first_stage(input)
        .map_err(|_| InputError::Domain)?;
    let middle = crate::dsp::waveform_fft_middle_rust_v1::middle_stage(&first)
        .map_err(|_| InputError::Domain)?;
    Ok(final_stage(&middle))
}

fn final_stage(input: &[Complex]) -> Vec<Complex> {
    let twiddles = crate::dsp::waveform_fft_tables_rust_v1::final_twiddles();
    let roots = crate::dsp::waveform_fft_tables_rust_v1::final_radix16_coefficients();
    let mut output = vec![Complex::default(); 4096];
    for block in 0..256 {
        let mut rotated = [Complex::default(); 16];
        for (lane, value) in rotated.iter_mut().enumerate() {
            let index = block * 16 + lane;
            let sample = input[index];
            let cosine = twiddles[index];
            let sine = twiddles[4096 + index];
            let imaginary_seed = sample.im * cosine;
            let real_seed = sample.im * sine;
            value.im = (-sample.re).mul_add(sine, imaginary_seed);
            value.re = sample.re.mul_add(cosine, real_seed);
        }
        for frequency in 0..16 {
            let sign = if frequency % 2 == 0 { 1.0 } else { -1.0 };
            let mut imaginary = rotated[8].im.mul_add(sign, rotated[0].im);
            let mut real = rotated[8].re.mul_add(sign, rotated[0].re);
            for q in 1..8 {
                let p = 16 - q;
                let cosine = roots[(q - 1) * 32 + frequency];
                let sine = roots[(q - 1) * 32 + 16 + frequency];
                let a = rotated[q];
                let b = rotated[p];
                imaginary = a.im.mul_add(cosine, imaginary);
                real = a.re.mul_add(cosine, real);
                imaginary = (-a.re).mul_add(sine, imaginary);
                real = a.im.mul_add(sine, real);
                imaginary = b.im.mul_add(cosine, imaginary);
                real = b.re.mul_add(cosine, real);
                imaginary = b.re.mul_add(sine, imaginary);
                real = (-b.im).mul_add(sine, real);
            }
            let index = frequency * 256 + (block as u8).reverse_bits() as usize;
            output[index] = Complex {
                re: real,
                im: imaginary,
            };
        }
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn origin_impulse_is_flat_without_scale_change() {
        let mut input = vec![Complex::default(); 4096];
        input[0].re = 1.0;
        let out = fft4096(&input).unwrap();
        assert_eq!(out.len(), 4096);
        assert!(
            out.iter()
                .all(|v| v.re.to_bits() == 1.0f32.to_bits() && v.im.to_bits() == 0)
        );
    }

    #[test]
    fn constant_retains_exact_dc_and_finite_residuals() {
        let out = fft4096(&vec![
            Complex {
                re: 0.25,
                im: -0.125
            };
            4096
        ])
        .unwrap();
        assert_eq!(
            out[0],
            Complex {
                re: 1024.0,
                im: -512.0
            }
        );
        // System-profile residual, not an analytic exact-zero assumption.
        assert_eq!(out[256].re.to_bits(), 3068133376);
        assert!(out.iter().all(|v| v.re.is_finite() && v.im.is_finite()));
    }

    #[test]
    fn shifted_impulse_retains_final_cosine_before_sine_rounding() {
        let mut input = vec![Complex::default(); 4096];
        input[37].re = 1.0;
        let out = fft4096(&input).unwrap();
        assert_eq!(out[257].re.to_bits(), 3202249635);
    }

    #[test]
    fn zero_preserves_positive_zero_outputs() {
        let out = fft4096(&vec![Complex::default(); 4096]).unwrap();
        assert!(
            out.iter()
                .all(|v| v.re.to_bits() == 0 && v.im.to_bits() == 0)
        );
    }

    #[test]
    fn rejects_invalid_profile_inputs() {
        assert_eq!(fft4096(&[]), Err(InputError::Length));
        let mut input = vec![Complex::default(); 4096];
        input[12].re = f32::NAN;
        assert_eq!(fft4096(&input), Err(InputError::Domain));
        input[12].re = 4.1;
        assert_eq!(fft4096(&input), Err(InputError::Domain));
    }
}
