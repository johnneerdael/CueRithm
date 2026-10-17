const SIZES: [usize; 3] = [512, 2048, 8192];
const TAU: f64 = f64::from_bits(0x4019_21fb_5444_2d18);
const U64_FRACTION: f64 = f64::from_bits(0x3bf0_0000_0000_0000);
type Planes = (Vec<f32>, Vec<f32>);

pub fn transform(re: &[f32], im: &[f32], inverse: bool) -> Result<Planes, String> {
    let n = re.len();
    if !SIZES.contains(&n) || im.len() != n {
        return Err("requires equal planes of512/2048/8192 words".into());
    }
    if re
        .iter()
        .chain(im)
        .any(|x| !x.is_finite() || x.abs() > 1e24)
    {
        return Err("finite componentabs<=1e24 required".into());
    }
    let (a, b) = if inverse { (im, re) } else { (re, im) };
    let (mut r, mut i) = if n == 2048 {
        crate::dsp::legacy_initial2_8_rust_v1::first8(a, b)?
    } else {
        crate::dsp::legacy_initial2_8_rust_v1::first2(a, b)?
    };
    let tables = coefficients(n)?;
    let middle_words = n / 256 * 480;
    let middle = &tables[..middle_words];
    if n == 8192 {
        (r, i) = middle_stage(&r, &i, 4096, &middle[..960]);
    }
    (r, i) = middle_stage(&r, &i, 256, middle);
    let roots = &tables[middle_words..middle_words + 224];
    let twiddles = &tables[middle_words + 224..];
    (r, i) = final_stage(&r, &i, roots, twiddles);
    if inverse {
        std::mem::swap(&mut r, &mut i);
    }
    if r.iter().chain(&i).any(|x| !x.is_finite()) {
        return Err("nonfinite output outside prototype profile".into());
    }
    Ok((r, i))
}

// Source two-block calls have the same per-block recipe at both middle sizes.
// Reading immutable planes preserves the source's preloaded in-place tiles.
fn middle_stage(re: &[f32], im: &[f32], block_size: usize, coefficients: &[f32]) -> Planes {
    let n = re.len();
    let lanes = block_size / 16;
    let mut r = vec![0.; n];
    let mut i = vec![0.; n];
    for block in 0..n / block_size {
        for lane in 0..lanes {
            for frequency in 0..16 {
                let seed = block * block_size + lane;
                let (mut rr, mut ii) = (re[seed], im[seed]);
                for q in 1..16 {
                    let k = block * block_size + q * lanes + lane;
                    let t = block * 480 + (q - 1) * 32 + frequency;
                    let (c, s) = (coefficients[t], coefficients[t + 16]);
                    ii = im[k].mul_add(c, ii);
                    rr = im[k].mul_add(s, rr);
                    ii = (-re[k]).mul_add(s, ii);
                    rr = re[k].mul_add(c, rr);
                }
                let dst = block * block_size + frequency * lanes + lane;
                r[dst] = rr;
                i[dst] = ii;
            }
        }
    }
    (r, i)
}

fn final_stage(re: &[f32], im: &[f32], roots: &[f32], twiddles: &[f32]) -> Planes {
    let n = re.len();
    let blocks = n / 16;
    let width = blocks.trailing_zeros();
    let mut r = vec![0.; n];
    let mut i = vec![0.; n];
    for block in 0..blocks {
        let mut rotated_r = [0f32; 16];
        let mut rotated_i = [0f32; 16];
        for lane in 0..16 {
            let k = block * 16 + lane;
            let (c, s) = (twiddles[k], twiddles[n + k]);
            let imag_seed = im[k] * c;
            let real_seed = im[k] * s;
            rotated_i[lane] = (-re[k]).mul_add(s, imag_seed);
            rotated_r[lane] = re[k].mul_add(c, real_seed);
        }
        for frequency in 0..16 {
            let sign = if frequency % 2 == 0 { 1. } else { -1. };
            let mut rr = rotated_r[8].mul_add(sign, rotated_r[0]);
            let mut ii = rotated_i[8].mul_add(sign, rotated_i[0]);
            for q in 1..8 {
                let p = 16 - q;
                let c = roots[(q - 1) * 32 + frequency];
                let s = roots[(q - 1) * 32 + 16 + frequency];
                ii = rotated_i[q].mul_add(c, ii);
                rr = rotated_r[q].mul_add(c, rr);
                ii = (-rotated_r[q]).mul_add(s, ii);
                rr = rotated_i[q].mul_add(s, rr);
                ii = rotated_i[p].mul_add(c, ii);
                rr = rotated_r[p].mul_add(c, rr);
                ii = rotated_r[p].mul_add(s, ii);
                rr = (-rotated_i[p]).mul_add(s, rr);
            }
            let dst = frequency * blocks + (block.reverse_bits() >> (usize::BITS - width));
            r[dst] = rr;
            i[dst] = ii;
        }
    }
    (r, i)
}

fn angle(index: usize) -> f64 {
    ((index as u64).reverse_bits() as f64 * U64_FRACTION) * TAU
}

/// Setup layout: middle rows, final seven roots, final cosine/sine planes.
pub fn coefficients(n: usize) -> Result<Vec<f32>, String> {
    if !SIZES.contains(&n) {
        return Err("unsupported setup size".into());
    }
    let middle_words = n / 256 * 480;
    let mut values = vec![0.; middle_words + 224 + 2 * n];
    for block in 0..n / 256 {
        for column in 0..16 {
            let theta = angle(block * 16 + column);
            for row in 1..16 {
                let (s, c) = (theta * row as f64).sin_cos();
                let t = block * 480 + (row - 1) * 32 + column;
                values[t] = c as f32;
                values[t + 16] = s as f32;
            }
        }
    }
    for row in 1..8 {
        let step = TAU * (row as f64 / 16.);
        for column in 0..16 {
            let (s, c) = (step * column as f64).sin_cos();
            let t = middle_words + (row - 1) * 32 + column;
            values[t] = c as f32;
            values[t + 16] = s as f32;
        }
    }
    for block in 0..n / 16 {
        let theta = angle(block * 16);
        for column in 0..16 {
            let (s, c) = (theta * column as f64).sin_cos();
            let t = middle_words + 224 + block * 16 + column;
            values[t] = c as f32;
            values[t + n] = s as f32;
        }
    }
    Ok(values)
}
