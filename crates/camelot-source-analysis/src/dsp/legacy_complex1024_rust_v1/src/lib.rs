#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Complex {
    pub re: f32,
    pub im: f32,
}

fn validate(input: &[Complex]) -> Result<(), String> {
    if input.len() != 1024 {
        return Err("exactly1024 complex values required".into());
    }
    if input.iter().any(|v| {
        !v.re.is_finite() || !v.im.is_finite() || v.re.abs() > 1.0e24 || v.im.abs() > 1.0e24
    }) {
        return Err("outside finite profile".into());
    }
    Ok(())
}

pub fn transform(input: &[Complex], inverse: bool) -> Result<Vec<Complex>, String> {
    validate(input)?;
    let swapped: Vec<_> = input
        .iter()
        .map(|&x| {
            if inverse {
                Complex { re: x.im, im: x.re }
            } else {
                x
            }
        })
        .collect();
    let first = first4(&swapped)?;
    let middle = middle256(&first);
    let mut out = final16(&middle);
    if inverse {
        for x in &mut out {
            std::mem::swap(&mut x.re, &mut x.im);
        }
    }
    Ok(out)
}

/// Exact source initial radix4; retain coefficient-zero fused operations.
pub fn first4(input: &[Complex]) -> Result<Vec<Complex>, String> {
    validate(input)?;
    let mut out = vec![Complex::default(); 1024];
    for n in 0..256 {
        for slot in 0..4 {
            let sign = [1.0, 1.0, -1.0, -1.0][slot];
            let cosine = [1.0, -1.0, 0.0, 0.0][slot];
            let sine = [0.0, 0.0, 1.0, -1.0][slot];
            let a = input[n];
            let b = input[256 + n];
            let c = input[512 + n];
            let d = input[768 + n];
            let mut im = c.im.mul_add(sign, a.im);
            let mut re = c.re.mul_add(sign, a.re);
            im = b.im.mul_add(cosine, im);
            re = b.im.mul_add(sine, re);
            im = (-b.re).mul_add(sine, im);
            re = b.re.mul_add(cosine, re);
            im = d.im.mul_add(cosine, im);
            re = (-d.im).mul_add(sine, re);
            im = d.re.mul_add(sine, im);
            re = d.re.mul_add(cosine, re);
            out[slot * 256 + n] = Complex { re, im };
        }
    }
    Ok(out)
}

fn middle256(input: &[Complex]) -> Vec<Complex> {
    let coefficients = tables::initial_mid_coefficients();
    let mut out = vec![Complex::default(); 1024];
    for block in 0..4 {
        for lane in 0..16 {
            for slot in 0..16 {
                let mut im = input[block * 256 + lane].im;
                let mut re = input[block * 256 + lane].re;
                for q in 1..16 {
                    let x = input[block * 256 + q * 16 + lane];
                    let index = block * 480 + (q - 1) * 32 + slot;
                    let cosine = coefficients[index];
                    let sine = coefficients[index + 16];
                    im = x.im.mul_add(cosine, im);
                    re = x.im.mul_add(sine, re);
                    im = (-x.re).mul_add(sine, im);
                    re = x.re.mul_add(cosine, re);
                }
                out[block * 256 + slot * 16 + lane] = Complex { re, im };
            }
        }
    }
    out
}

fn final16(input: &[Complex]) -> Vec<Complex> {
    let twiddles = tables::final_twiddles();
    let roots = tables::final_radix16_coefficients();
    let mut out = vec![Complex::default(); 1024];
    for block in 0..64 {
        let mut rotated = [Complex::default(); 16];
        for (lane, value) in rotated.iter_mut().enumerate() {
            let index = block * 16 + lane;
            let x = input[index];
            let cosine = twiddles[index];
            let sine = twiddles[1024 + index];
            let imag_seed = x.im * cosine;
            let real_seed = x.im * sine;
            value.im = (-x.re).mul_add(sine, imag_seed);
            value.re = x.re.mul_add(cosine, real_seed);
        }
        for frequency in 0..16 {
            let sign = if frequency % 2 == 0 { 1.0 } else { -1.0 };
            let mut im = rotated[8].im.mul_add(sign, rotated[0].im);
            let mut re = rotated[8].re.mul_add(sign, rotated[0].re);
            for q in 1..8 {
                let p = 16 - q;
                let cosine = roots[(q - 1) * 32 + frequency];
                let sine = roots[(q - 1) * 32 + 16 + frequency];
                let a = rotated[q];
                let b = rotated[p];
                im = a.im.mul_add(cosine, im);
                re = a.re.mul_add(cosine, re);
                im = (-a.re).mul_add(sine, im);
                re = a.im.mul_add(sine, re);
                im = b.im.mul_add(cosine, im);
                re = b.re.mul_add(cosine, re);
                im = b.re.mul_add(sine, im);
                re = (-b.im).mul_add(sine, re);
            }
            let index = frequency * 64 + ((block as u8).reverse_bits() >> 2) as usize;
            out[index] = Complex { re, im };
        }
    }
    out
}
pub mod tables;
