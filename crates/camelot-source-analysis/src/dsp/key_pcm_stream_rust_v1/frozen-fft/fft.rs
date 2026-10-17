//! Private independent FFT numerical prototype; no proprietary runtime dependency.
use std::{env, fs, io};

#[derive(Clone, Copy, Debug, Default)]
pub struct Complex {
    pub re: f32,
    pub im: f32,
}

pub fn multiply(a: Complex, b: Complex) -> Complex {
    let rr = a.re * b.re;
    let ii = a.im * b.im;
    let ri = a.re * b.im;
    let ir = a.im * b.re;
    Complex {
        re: rr - ii,
        im: ir + ri,
    }
}
pub fn fft(values: &mut [Complex]) {
    let n = values.len();
    assert!(n.is_power_of_two());
    if n > 2 {
        let mut target = n / 2;
        for source in 1..n - 1 {
            if source < target {
                values.swap(source, target);
            }
            let mut bit = n / 2;
            while bit <= target {
                target -= bit;
                bit >>= 1;
            }
            target += bit;
        }
    }
    for stage in 0..n.trailing_zeros() {
        let half = 1usize << stage;
        let (sin, cos) = (std::f64::consts::PI / half as f64).sin_cos();
        let root = Complex {
            re: cos as f32,
            im: -(sin as f32),
        };
        let mut twiddle = Complex { re: 1.0, im: 0.0 };
        for column in 0..half {
            for first in (column..n).step_by(2 * half) {
                let product = multiply(twiddle, values[first + half]);
                let original = values[first];
                values[first + half] = Complex {
                    re: original.re - product.re,
                    im: original.im - product.im,
                };
                values[first] = Complex {
                    re: product.re + original.re,
                    im: product.im + original.im,
                };
            }
            twiddle = multiply(twiddle, root);
        }
    }
}
pub fn dual(a: &[f32], b: &[f32]) -> (Vec<Complex>, Vec<Complex>) {
    assert_eq!(a.len(), b.len());
    let n = a.len();
    assert!(n >= 2 && n.is_power_of_two());
    let mut packed: Vec<_> = a
        .iter()
        .zip(b)
        .map(|(&re, &im)| Complex { re, im })
        .collect();
    fft(&mut packed);
    let mut first = vec![Complex::default(); n / 2];
    let mut second = vec![Complex::default(); n / 2];
    first[0].re = packed[0].re + packed[0].re;
    second[0].im = packed[0].im * -2.0;
    for bin in 1..n / 2 {
        let forward = packed[bin];
        let reverse = packed[n - bin];
        first[bin] = Complex {
            re: forward.re + reverse.re,
            im: forward.im - reverse.im,
        };
        second[bin] = Complex {
            re: reverse.re - forward.re,
            im: -forward.im - reverse.im,
        };
    }
    (first, second)
}

fn main() -> io::Result<()> {
    let args: Vec<_> = env::args().collect();
    if args.len() != 3 {
        return Err(io::Error::other("usage: fft INPUT OUTPUT"));
    }
    let bytes = fs::read(&args[1])?;
    if bytes.len() % 8 != 0 {
        return Err(io::Error::other("invalid complex32 byte count"));
    }
    let mut values: Vec<_> = bytes
        .chunks_exact(8)
        .map(|b| Complex {
            re: f32::from_le_bytes(b[..4].try_into().unwrap()),
            im: f32::from_le_bytes(b[4..].try_into().unwrap()),
        })
        .collect();
    fft(&mut values);
    let bytes: Vec<_> = values
        .iter()
        .flat_map(|v| [v.re.to_le_bytes(), v.im.to_le_bytes()].concat())
        .collect();
    fs::write(&args[2], bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn bits(v: &[Complex]) -> Vec<(u32, u32)> {
        v.iter().map(|c| (c.re.to_bits(), c.im.to_bits())).collect()
    }
    #[test]
    fn complex_product_uses_full_real_and_imaginary_parts() {
        let p = multiply(Complex { re: 1.0, im: 2.0 }, Complex { re: 3.0, im: 4.0 });
        assert_eq!((p.re, p.im), (-5.0, 10.0));
    }
    #[test]
    fn two_point_transform_is_unnormalized() {
        let mut x = [Complex { re: 1.0, im: 2.0 }, Complex { re: 3.0, im: 4.0 }];
        fft(&mut x);
        assert_eq!(
            bits(&x),
            bits(&[Complex { re: 4.0, im: 6.0 }, Complex { re: -2.0, im: -2.0 }])
        );
    }
    #[test]
    fn impulse_generates_flat_spectrum() {
        let mut x = [Complex::default(); 8];
        x[0].re = 1.0;
        fft(&mut x);
        assert!(x.iter().all(|c| c.re == 1.0 && c.im == 0.0));
    }
    #[test]
    fn dual_real_transform_retains_double_amplitude_and_second_rotation() {
        let (a, b) = dual(&[1.0, 0.0, 0.0, 0.0], &[2.0, 0.0, 0.0, 0.0]);
        for c in &a[..2] {
            assert_eq!((c.re, c.im), (2.0, 0.0));
        }
        for c in &b[..2] {
            assert_eq!((c.re, c.im), (0.0, -4.0));
        }
    }
}
