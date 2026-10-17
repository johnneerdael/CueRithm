#[path = "complex4096/first.rs"]
mod complex_first;
#[path = "complex4096/middle.rs"]
mod complex_middle;
#[path = "complex4096/tables.rs"]
mod complex_tables;
#[path = "complex4096/fft.rs"]
mod complex_transform;
mod pack;
mod small;
mod special256;

fn validate(input: &[f32], bound: f32) -> Result<usize, String> {
    let n = input.len();
    if !(16..=16384).contains(&n) || !n.is_power_of_two() {
        return Err("requires poweroftwo16..16384 words".into());
    }
    if input.iter().any(|x| !x.is_finite() || x.abs() > bound) {
        return Err("input outside finite direction-specific domain".into());
    }
    Ok(n)
}

fn complex(re: &[f32], im: &[f32], inverse: bool) -> Result<(Vec<f32>, Vec<f32>), String> {
    match re.len() {
        8 | 16 | 32 | 64 | 128 => small::transform(re, im, inverse),
        256 => special256::transform(re, im, inverse),
        512 | 2048 | 8192 => crate::dsp::legacy_aligned_complex_rust_v1::transform(re, im, inverse),
        1024 => {
            use crate::dsp::legacy_complex1024_rust_v1::Complex;
            let input: Vec<_> = re
                .iter()
                .zip(im)
                .map(|(&re, &im)| Complex { re, im })
                .collect();
            let out = crate::dsp::legacy_complex1024_rust_v1::transform(&input, inverse)?;
            Ok((
                out.iter().map(|v| v.re).collect(),
                out.iter().map(|v| v.im).collect(),
            ))
        }
        4096 => {
            use complex_transform::{Complex, fft4096};
            let input: Vec<_> = re
                .iter()
                .zip(im)
                .map(|(&r, &i)| {
                    if inverse {
                        Complex { re: i, im: r }
                    } else {
                        Complex { re: r, im: i }
                    }
                })
                .collect();
            let out = fft4096(&input).map_err(|_| "complex4096 domain")?;
            if inverse {
                Ok((
                    out.iter().map(|v| v.im).collect(),
                    out.iter().map(|v| v.re).collect(),
                ))
            } else {
                Ok((
                    out.iter().map(|v| v.re).collect(),
                    out.iter().map(|v| v.im).collect(),
                ))
            }
        }
        _ => Err("unsupported complex size".into()),
    }
}

fn finite(output: Vec<f32>) -> Result<Vec<f32>, String> {
    if output.iter().any(|x| !x.is_finite()) {
        Err("nonfinite result outside prototype domain".into())
    } else {
        Ok(output)
    }
}

pub fn forward(input: &[f32]) -> Result<Vec<f32>, String> {
    let n = validate(input, 1e12)?;
    let re: Vec<_> = input.iter().step_by(2).copied().collect();
    let im: Vec<_> = input.iter().skip(1).step_by(2).copied().collect();
    let (r, i) = complex(&re, &im, false)?;
    let packed: Vec<_> = r.into_iter().chain(i).collect();
    let mut out = pack::combine(
        &packed,
        &crate::dsp::legacy_real_fft_tables_rust_v1::real_table(n)?,
        pack::Direction::Forward,
    )
    .map_err(|_| "real packing domain")?;
    for x in &mut out {
        *x *= 0.5;
    }
    finite(out)
}

pub fn inverse(input: &[f32]) -> Result<Vec<f32>, String> {
    let n = validate(input, 1e20)?;
    let pre = pack::combine(
        input,
        &crate::dsp::legacy_real_fft_tables_rust_v1::real_table(n)?,
        pack::Direction::Inverse,
    )
    .map_err(|_| "real unpacking domain")?;
    let (r, i) = complex(&pre[..n / 2], &pre[n / 2..], true)?;
    let out: Vec<_> = r.into_iter().zip(i).flat_map(|(r, i)| [r, i]).collect();
    finite(out)
}
