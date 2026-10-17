mod table;
pub fn log10(x: f64) -> Result<f64, String> {
    if !x.is_finite() || !(1e-50..=1e9).contains(&x) {
        return Err("positive finite bounded log10 profile required".into());
    }
    let word = x.to_bits();
    let n = ((word.wrapping_add(0xc018100000000000) as i64) >> 52) as f64;
    let fraction = word & 0x000fffffffffffff;
    let j = ((fraction + 0x100000000000) >> 45) as usize;
    let m = f64::from_bits(fraction | 0x3ff0000000000000);
    let inv = f64::from_bits(table::TABLE[j][0]);
    let center = f64::from_bits(table::TABLE[j][1]);
    let r = m.mul_add(inv, -1.);
    let c = table::C.map(f64::from_bits);
    let a = r.mul_add(c[1], c[0]);
    let b = r.mul_add(r + c[3], c[2]);
    let cc = r.mul_add(r + c[5], c[4]);
    let polynomial = (((r * r) * a) * b) * cc;
    let result = if n != 0. {
        let high_center = f64::from_bits(center.to_bits() & 0xfffffffff8000000);
        let low_center = center - high_center;
        let low_center = low_center + polynomial;
        let high = n.mul_add(f64::from_bits(0x3fd3441350000000), high_center);
        let low = n * f64::from_bits(0x3e03ef3fde623e25);
        let linear = r * f64::from_bits(0x3fdbcb7b1526e50e);
        let low = low + linear;
        let low = low + low_center;
        low + high
    } else {
        let product = m * inv;
        let d = product - 1.;
        let error = inv.mul_add(m, -product);
        let low_term = r * f64::from_bits(0xbe26c8d78e6acaa4);
        let rh = f64::from_bits(r.to_bits() & 0xfffffffff8000000);
        let delta = d - rh;
        let high_term = rh * f64::from_bits(0x3fdbcb7b18000000);
        let delta = error + delta;
        let sum = center + high_term;
        let recovered = sum - center;
        let low_term = delta.mul_add(f64::from_bits(0x3fdbcb7b18000000), low_term);
        let compensation = high_term - recovered;
        let low_term = low_term + compensation;
        let low_term = low_term + polynomial;
        low_term + sum
    };
    Ok(result)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn source_ten_powers_have_exact_endpoints() {
        assert_eq!(log10(1.).unwrap().to_bits(), 0);
        assert_eq!(log10(10.).unwrap(), 1.);
        assert_eq!(log10(0.1).unwrap(), -1.);
        assert_eq!(log10(1e-50).unwrap(), -50.);
    }
    #[test]
    fn excludes_zero_and_nonfinite_inputs() {
        assert!(log10(0.).is_err());
        assert!(log10(-1.).is_err());
        assert!(log10(f64::INFINITY).is_err());
    }
}
