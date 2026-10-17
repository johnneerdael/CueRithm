include!("exp_table.rs");
fn sine_polynomial(x: f64) -> f64 {
    let s = x * x;
    let c = x * s;
    let p = s.mul_add(
        f64::from_bits(0xbf29943e0fc4818f),
        f64::from_bits(0x3f811073afd145eb),
    );
    let f = s * c;
    let l = c.mul_add(f64::from_bits(0xbfc555545268a02a), x);
    f.mul_add(p, l)
}
fn cosine_polynomial(x: f64) -> f64 {
    let s = x * x;
    let u = s.mul_add(
        f64::from_bits(0x3ef99169fc938f4a),
        f64::from_bits(0xbf56c07f16945df4),
    );
    let l = s.mul_add(
        f64::from_bits(0x3fa55553c7899699),
        f64::from_bits(0xbfdffffffcb82e97),
    );
    let f = s * s;
    s.mul_add(f.mul_add(u, l), 1.)
}
/// Selected positive-angle source sinf, including its f32 tiny path.
pub fn positive_sine(x: f32) -> Result<f32, String> {
    let bits = x.to_bits();
    if bits >> 31 != 0 || bits > 0x40490fdb {
        return Err("sine requires +0..pi_f32".into());
    }
    if bits < 0x39800000 {
        return Ok(x.mul_add(f32::from_bits(0x4c800000), x) * f32::from_bits(0x32800000));
    }
    let x = f64::from(x);
    if bits < 0x3f490fdb {
        return Ok(sine_polynomial(x) as f32);
    }
    let q = (x * f64::from_bits(0x3fe45f306dc9c883)).round_ties_even();
    let r = (-q).mul_add(f64::from_bits(0x3ff921fb54442d18), x);
    let v = if q as u64 & 1 != 0 {
        cosine_polynomial(r)
    } else {
        sine_polynomial(r)
    };
    Ok(f64::from_bits(v.to_bits() ^ ((((q as u64) >> 1) & 1) << 63)) as f32)
}
/// Selected source expf on the complete mode0 normalization argument range.
pub fn negative_exp(x: f32) -> Result<f32, String> {
    if !x.is_finite() || !(-3.0..=0.0).contains(&x) {
        return Err("exp requires finite[-3,0]".into());
    }
    let z = f64::from(x) * f64::from_bits(0x40671547652b82fe);
    let rounded = z.round();
    let n = rounded as i64 as u64;
    let r = z - rounded;
    let scale = f64::from_bits(TABLE[(n & 127) as usize].wrapping_add(n.wrapping_shl(45)));
    let p = f64::from_bits(0x3eeebfbdff30d656).mul_add(r, f64::from_bits(0x3f762e4453e10dae));
    let t = p * r;
    Ok(t.mul_add(scale, scale) as f32)
}
