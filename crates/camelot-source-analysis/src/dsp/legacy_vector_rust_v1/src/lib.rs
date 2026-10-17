//! Fixed source-derived packed vector profile, finite intermediates, default RNE.
//! Magnitude models 16-byte-aligned unit-stride system kernel; no host sqrt.
fn packed(a: &[f32]) -> Result<usize, String> {
    let h = a.len() / 2;
    if a.len() != 2 * h || !(4..=8192).contains(&h) || !h.is_power_of_two() {
        return Err("packed length must be 2*H, power-of-two H in 4..=8192".into());
    }
    finite(a)?;
    Ok(h)
}
fn finite(a: &[f32]) -> Result<(), String> {
    if a.iter().any(|x| !x.is_finite()) {
        return Err("nonfinite numeric input/intermediate".into());
    }
    Ok(())
}
pub fn packed_multiply(a: &[f32], b: &[f32]) -> Result<Vec<f32>, String> {
    let h = packed(a)?;
    if b.len() != a.len() {
        return Err("packed lengths differ".into());
    }
    finite(b)?;
    let mut out = vec![0.0; a.len()];
    out[0] = a[0] * b[0];
    out[h] = a[h] * b[h];
    for k in 1..h {
        let rr = a[k] * b[k];
        let ii = a[h + k] * b[h + k];
        out[k] = rr - ii;
        let ri = b[k] * a[h + k];
        let ir = a[k] * b[h + k];
        out[h + k] = ri + ir;
        finite(&[rr, ii, ri, ir])?;
    }
    finite(&out)?;
    Ok(out)
}
pub fn conjugate(a: &[f32]) -> Result<Vec<f32>, String> {
    let h = packed(a)?;
    let mut out = a.to_vec();
    for v in &mut out[h + 1..] {
        *v *= -1.0;
    }
    Ok(out)
}
pub fn lower_threshold(a: &[f32], threshold: f32, replacement: f32) -> Result<Vec<f32>, String> {
    finite(a)?;
    finite(&[threshold, replacement])?;
    Ok(a.iter()
        .map(|&v| if v < threshold { replacement } else { v })
        .collect())
}
/// Architected 8-bit mantissa estimate, FPCR.AH=0; positive finite input.
/// Integer midpoint interval selection, not a sampled output table.
pub fn reciprocal_sqrt_estimate(x: f32) -> Result<f32, String> {
    if !x.is_finite() || x <= 0.0 {
        return Err("estimate requires positive finite value".into());
    }
    let bits = x.to_bits();
    let mut exp = ((bits >> 23) & 255) as i32;
    let mut frac = bits & 0x7fffff;
    if exp == 0 {
        while frac & 0x400000 == 0 {
            frac <<= 1;
            exp -= 1;
        }
        frac = (frac & 0x3fffff) << 1;
    }
    let scaled = if exp & 1 != 0 {
        128 | (frac >> 16)
    } else {
        256 | (frac >> 15)
    };
    let midpoint = if scaled < 256 {
        2 * scaled + 1
    } else {
        2 * ((scaled & !1) + 1)
    };
    // Least integer k with midpoint*k*k >= 2^28. Output mantissa is floor(k/2).
    let (mut lo, mut hi) = (513u64, 1025u64);
    while lo < hi {
        let k = (lo + hi) / 2;
        if u64::from(midpoint) * k * k < 1 << 28 {
            lo = k + 1;
        } else {
            hi = k;
        }
    }
    let estimate = (lo / 2) as u32;
    let out_exp = ((380 - exp) / 2) as u32;
    Ok(f32::from_bits((out_exp << 23) | ((estimate & 255) << 15)))
}
/// FRSQRTS for finite operands with product equal to0 or in [0.5,1.5]. Exact f32 product has
/// at most48 significant bits; in this domain subtraction from3 fits f64. Halving is exact.
pub fn reciprocal_sqrt_step(a: f32, b: f32) -> Result<f32, String> {
    finite(&[a, b])?;
    let product = f64::from(a) * f64::from(b);
    if product != 0.0 && !(0.5..=1.5).contains(&product) {
        return Err("step product outside bounded Newton profile".into());
    }
    Ok(((-f64::from(a)).mul_add(f64::from(b), 3.0) * 0.5) as f32)
}
pub fn magnitude(a: &[f32]) -> Result<Vec<f32>, String> {
    let h = packed(a)?;
    let mut out = Vec::with_capacity(h);
    for k in 0..h {
        let re = a[k];
        let im = if k == 0 { 0.0 } else { a[h + k] };
        // H8's second4-lane tail group uses TWO rounded squares and FADD.
        let q = if h == 8 && k >= 4 {
            re * re + im * im
        } else {
            im.mul_add(im, re * re)
        };
        finite(&[q])?;
        if q == 0.0 {
            out.push(0.0);
            continue;
        }
        let mut e = reciprocal_sqrt_estimate(q)?;
        let mut t = q * e;
        e *= reciprocal_sqrt_step(e, t)?;
        t = q * e;
        let result = t * reciprocal_sqrt_step(e, t)?;
        finite(&[result])?;
        out.push(result);
    }
    Ok(out)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn dc_nyquist_are_independent_real_products() {
        let a = [2., 3., 0., 0., 5., 4., 0., 0.];
        let b = [7., 2., 0., 0., 11., -1., 0., 0.];
        assert_eq!(
            packed_multiply(&a, &b).unwrap(),
            [14., 10., 0., 0., 55., 5., 0., 0.]
        );
    }
    #[test]
    fn conjugation_preserves_dc_nyquist_and_flips_signed_zero() {
        let a = [2., 3., 0., 0., 5., 4., 0., -0.];
        let o = conjugate(&a).unwrap();
        assert_eq!(
            o.iter().map(|v| v.to_bits()).collect::<Vec<_>>(),
            [
                2f32.to_bits(),
                3f32.to_bits(),
                0,
                0,
                5f32.to_bits(),
                (-4f32).to_bits(),
                0x80000000,
                0
            ]
        );
    }
    #[test]
    fn threshold_replaces_strictly_lower_instead_of_clamping() {
        assert_eq!(
            lower_threshold(&[-1., 0., 1., 2.], 1., 9.).unwrap(),
            [9., 9., 1., 2.]
        );
    }
    #[test]
    fn estimate_one_is_deliberately_approximate() {
        assert_eq!(reciprocal_sqrt_estimate(1.).unwrap().to_bits(), 0x3f7f8000);
    }
    #[test]
    fn step_is_three_minus_product_over_two() {
        assert_eq!(reciprocal_sqrt_step(0.5, 2.).unwrap(), 1.);
    }
    #[test]
    fn magnitude_suppresses_nyquist() {
        let a = [3., 3., 0., 0., 99., 4., 0., 0.];
        let o = magnitude(&a).unwrap();
        assert_eq!(o.len(), 4);
        assert!((o[0] - 3.).abs() < 1e-5);
        assert!((o[1] - 5.).abs() < 1e-5);
        assert_eq!(o[2].to_bits(), 0);
    }
    #[test]
    fn h8_second_group_sums_separately_rounded_squares() {
        let mut a = [0.0; 16];
        a[4] = f32::from_bits(0x3f4a28d0);
        a[12] = f32::from_bits(0x3f45e5ef);
        assert_eq!(magnitude(&a).unwrap()[4].to_bits(), 0x3f8d7323);
    }
    #[test]
    fn invalid_contracts_are_errors() {
        assert!(reciprocal_sqrt_step(0.125, 1.0).is_err());
        assert!(packed_multiply(&[1.; 8], &[1.; 16]).is_err());
        assert!(conjugate(&[1.; 7]).is_err());
        assert!(magnitude(&[f32::MAX; 8]).is_err());
        assert!(reciprocal_sqrt_estimate(0.).is_err());
        assert!(reciprocal_sqrt_estimate(f32::NAN).is_err());
        assert!(lower_threshold(&[f32::NAN], 1., 0.).is_err());
    }
}
