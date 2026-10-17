//! Selected aligned system N256 operation recipe, independently reconstructed.
//! Exact fixed size, unit stride, no normalization; finite inputs |x|<=1e24.
use std::sync::OnceLock;
const N: usize = 256;
struct Tables {
    matrix: Vec<f32>,
    cosines: Vec<f32>,
    sines: Vec<f32>,
}
fn tables() -> &'static Tables {
    static T: OnceLock<Tables> = OnceLock::new();
    T.get_or_init(|| {
        // Source twiddle-angle expression is independent of transform length.
        // Prefixes are generated mathematically by the unchanged source-profile crate.
        let all = crate::dsp::waveform_fft_tables_rust_v1::final_twiddles();
        Tables {
            matrix: crate::dsp::waveform_fft_tables_rust_v1::final_radix16_coefficients(),
            cosines: all[..N].to_vec(),
            sines: all[4096..4096 + N].to_vec(),
        }
    })
}
/// Source-generated numeric setup output for separate host-side validation.
#[allow(dead_code)] // Retain the unchanged source diagnostic alongside copied arithmetic.
pub fn coefficient_words() -> Vec<f32> {
    let t = tables();
    [
        t.matrix.as_slice(),
        t.cosines.as_slice(),
        t.sines.as_slice(),
    ]
    .concat()
}
fn dense(ii: &[f32; 16], rr: &[f32; 16], frequency: usize, t: &[f32]) -> (f32, f32) {
    let d = if frequency & 1 == 0 { 1.0 } else { -1.0 };
    let mut i = ii[8].mul_add(d, ii[0]);
    let mut r = rr[8].mul_add(d, rr[0]);
    for q in 1..8 {
        let p = 16 - q;
        let c = t[(2 * q - 2) * 16 + frequency];
        let s = t[(2 * q - 1) * 16 + frequency];
        i = ii[q].mul_add(c, i);
        i = (-rr[q]).mul_add(s, i);
        // BOTH source N256 dense passes add the real sine term BEFORE cosine.
        r = ii[q].mul_add(s, r);
        r = rr[q].mul_add(c, r);
        i = ii[p].mul_add(c, i);
        i = rr[p].mul_add(s, i);
        r = (-ii[p]).mul_add(s, r);
        r = rr[p].mul_add(c, r);
    }
    (i, r)
}
fn forward(re: &[f32], im: &[f32]) -> (Vec<f32>, Vec<f32>) {
    let t = tables();
    let mut first_i = [0.0; N];
    let mut first_r = [0.0; N];
    for n in 0..16 {
        let ii = std::array::from_fn(|q| im[q * 16 + n]);
        let rr = std::array::from_fn(|q| re[q * 16 + n]);
        for j in 0..16 {
            let (i, r) = dense(&ii, &rr, j, &t.matrix);
            first_i[n * 16 + j] = i;
            first_r[n * 16 + j] = r;
        }
    }
    let mut oi = vec![0.0; N];
    let mut or = vec![0.0; N];
    for j in 0..16 {
        let mut ii = [0.0; 16];
        let mut rr = [0.0; 16];
        ii[0] = first_i[j];
        rr[0] = first_r[j];
        // n0 is a bit copy; do not insert even algebraically neutral twiddle arithmetic.
        for n in 1usize..16 {
            let k = (n.reverse_bits() >> (usize::BITS - 4)) * 16 + j;
            let i = first_i[n * 16 + j];
            let r = first_r[n * 16 + j];
            let c = t.cosines[k];
            let s = t.sines[k];
            ii[n] = (-r).mul_add(s, i * c);
            rr[n] = r.mul_add(c, i * s);
        }
        for j2 in 0..16 {
            let (i, r) = dense(&ii, &rr, j2, &t.matrix);
            oi[j2 * 16 + j] = i;
            or[j2 * 16 + j] = r;
        }
    }
    (or, oi)
}
pub fn transform(re: &[f32], im: &[f32], inverse: bool) -> Result<(Vec<f32>, Vec<f32>), String> {
    if re.len() != N || im.len() != N {
        return Err("requires exactly256 real and imaginary words".into());
    }
    if re
        .iter()
        .chain(im)
        .any(|x| !x.is_finite() || x.abs() > 1.0e24)
    {
        return Err("requires finite inputs with magnitude<=1e24".into());
    }
    if inverse {
        let (i, r) = forward(im, re);
        Ok((r, i))
    } else {
        Ok(forward(re, im))
    }
}
