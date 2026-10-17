//! Source AMX initial radix2/radix8 scalar recipes; no native APIs or tables.
const H: f32 = f32::from_bits(0x3f35_04f3);
// Literal source coefficient rows at libvDSP+ee840,+ee880,+ee8c0.
const COS: [[f32; 8]; 3] = [
    [1.0, -1.0, 0.0, 0.0, H, -H, -H, H],
    [1.0, 1.0, -1.0, -1.0, 0.0, 0.0, 0.0, 0.0],
    [1.0, -1.0, 0.0, 0.0, -H, H, H, -H],
];
const SIN: [[f32; 8]; 3] = [
    [0.0, 0.0, 1.0, -1.0, H, -H, H, -H],
    [0.0, 0.0, 0.0, 0.0, 1.0, 1.0, -1.0, -1.0],
    [0.0, 0.0, -1.0, 1.0, H, -H, H, -H],
];

fn inputs(re: &[f32], im: &[f32], sizes: &[usize]) -> Result<usize, String> {
    if re.len() != im.len() || !sizes.contains(&re.len()) {
        return Err("unsupported stage length/profile or unequal plane lengths".into());
    }
    if re.iter().chain(im).any(|x| !x.is_finite()) {
        return Err("finite input planes required".into());
    }
    Ok(re.len())
}

fn finite_pair(r: f32, i: f32) -> Result<(), String> {
    if !r.is_finite() || !i.is_finite() {
        return Err("nonfinite stage intermediate outside prototype profile".into());
    }
    Ok(())
}

/// Selected aligned initial radix2 for complexN512 or8192; return(real,imag).
pub fn first2(re: &[f32], im: &[f32]) -> Result<(Vec<f32>, Vec<f32>), String> {
    let n = inputs(re, im, &[512, 8192])?;
    let m = n / 2;
    let mut out_re = vec![0.0; n];
    let mut out_im = vec![0.0; n];
    for sample in 0..m {
        for (j, sign) in [1.0, -1.0].into_iter().enumerate() {
            // Source skip3 copies followed by FMA, including signed zeros.
            let i = im[m + sample].mul_add(sign, im[sample]);
            let r = re[m + sample].mul_add(sign, re[sample]);
            finite_pair(r, i)?;
            out_re[j * m + sample] = r;
            out_im[j * m + sample] = i;
        }
    }
    Ok((out_re, out_im))
}

/// Selected aligned initial radix8 for complexN2048; return(real,imag).
pub fn first8(re: &[f32], im: &[f32]) -> Result<(Vec<f32>, Vec<f32>), String> {
    let n = inputs(re, im, &[2048])?;
    let m = n / 8;
    let mut out_re = vec![0.0; n];
    let mut out_im = vec![0.0; n];
    for sample in 0..m {
        for j in 0..8 {
            // Bit copies, not zero-initialized accumulators plus input.
            let (mut r, mut i) = (re[sample], im[sample]);
            let diagonal = if j < 4 { 1.0 } else { -1.0 };
            i = im[4 * m + sample].mul_add(diagonal, i);
            r = re[4 * m + sample].mul_add(diagonal, r);
            finite_pair(r, i)?;
            // Source branch order0,4,1,7,2,6,3,5; eight ordered FMAs per pair.
            for q in 1..4 {
                let p = 8 - q;
                let (c, s) = (COS[q - 1][j], SIN[q - 1][j]);
                let (iq, rq) = (im[q * m + sample], re[q * m + sample]);
                let (ip, rp) = (im[p * m + sample], re[p * m + sample]);
                i = iq.mul_add(c, i);
                r = iq.mul_add(s, r);
                i = (-rq).mul_add(s, i);
                r = rq.mul_add(c, r);
                finite_pair(r, i)?;
                i = ip.mul_add(c, i);
                r = (-ip).mul_add(s, r);
                i = rp.mul_add(s, i);
                r = rp.mul_add(c, r);
                finite_pair(r, i)?;
            }
            // Physical slotj, frequencybitreverse3(j); retain zero-coefficient FMAs.
            out_re[j * m + sample] = r;
            out_im[j * m + sample] = i;
        }
    }
    Ok((out_re, out_im))
}
