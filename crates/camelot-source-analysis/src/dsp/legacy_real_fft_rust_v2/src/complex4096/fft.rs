pub use crate::dsp::legacy_real_fft_rust_v2::complex_first::Complex;

#[derive(Debug, PartialEq, Eq)]
pub enum InputError {
    Length,
    Domain,
}

pub fn fft4096(input: &[Complex]) -> Result<Vec<Complex>, InputError> {
    if input.len() != 4096 {
        return Err(InputError::Length);
    }
    if input.iter().any(|v| {
        !v.re.is_finite() || !v.im.is_finite() || v.re.abs() > 1.0e24 || v.im.abs() > 1.0e24
    }) {
        return Err(InputError::Domain);
    }
    let first = crate::dsp::legacy_real_fft_rust_v2::complex_first::first_stage(input)
        .map_err(|_| InputError::Domain)?;
    let middle = crate::dsp::legacy_real_fft_rust_v2::complex_middle::middle_stage(&first)
        .map_err(|_| InputError::Domain)?;
    Ok(final_stage(&middle))
}

fn final_stage(input: &[Complex]) -> Vec<Complex> {
    let twiddles = crate::dsp::legacy_real_fft_rust_v2::complex_tables::final_twiddles();
    let roots = crate::dsp::legacy_real_fft_rust_v2::complex_tables::final_radix16_coefficients();
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
