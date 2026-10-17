include!("tables.rs");

/// Reproduce the pinned runtime's positive-finite approximation, including its
/// rounding error. This is intentionally not a correctly rounded generic powf.
pub fn positive_power(base: f32, exponent: f32) -> Result<f32, &'static str> {
    if !base.is_finite() || base.is_sign_negative() || !exponent.is_finite() || exponent <= 0.0 {
        return Err("finite nonnegative base (+0 only) and positive exponent required");
    }
    if base == 1.0 {
        return Ok(1.0);
    }
    if exponent == 1.0 {
        return Ok(base);
    }
    if base == 0.0 {
        return Ok(0.0);
    }
    let mut word = base.to_bits();
    if word < 0x00800000 {
        let shifted = f32::from_bits(word | 0x3f800000) - 1.0;
        word = shifted.to_bits().wrapping_sub(0x3f000000);
    }
    let temporary = word.wrapping_sub(0x3f338000);
    let index = ((temporary >> 16) & 127) as usize;
    let exponent_word = temporary & 0xff800000;
    let mantissa = f64::from(f32::from_bits(word.wrapping_sub(exponent_word)));
    let exponent_scaled = f64::from((exponent_word as i32) >> 16);
    let (inverse_bits, logarithm_bits) = LOG_TABLE[index];
    let residual = mantissa.mul_add(f64::from_bits(inverse_bits), -1.0);
    let base_logarithm = f64::from_bits(logarithm_bits) + exponent_scaled;
    let square = residual * residual;
    let a = residual.mul_add(
        f64::from_bits(LOG_COEFFICIENTS[0]),
        f64::from_bits(LOG_COEFFICIENTS[1]),
    );
    let b = residual.mul_add(
        f64::from_bits(LOG_COEFFICIENTS[2]),
        f64::from_bits(LOG_COEFFICIENTS[3]),
    );
    let polynomial = a.mul_add(square, b);
    let logarithm = polynomial.mul_add(residual, base_logarithm);
    let scaled = (f64::from(exponent) * logarithm).clamp(-32768.0, 32768.0);
    let rounded = scaled.round();
    let integer = rounded as i64;
    let remainder = scaled - rounded;
    let index = (integer & 127) as usize;
    let bits = EXP_TABLE[index].wrapping_add((integer as u64).wrapping_shl(45));
    let factor = f64::from_bits(bits);
    let coefficient =
        f64::from_bits(EXP_COEFFICIENTS[0]).mul_add(remainder, f64::from_bits(EXP_COEFFICIENTS[1]));
    let correction = coefficient * remainder;
    Ok(correction.mul_add(factor, factor) as f32)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn reference_approximation_932_0() {
        assert_eq!(
            positive_power(f32::from_bits(0x3ee113d3), f32::from_bits(0x40403133))
                .unwrap()
                .to_bits(),
            0x3dad8ecc
        );
    }
    #[test]
    fn reference_approximation_1348_1() {
        assert_eq!(
            positive_power(f32::from_bits(0x415c60cd), f32::from_bits(0x3eaa7efa))
                .unwrap()
                .to_bits(),
            0x40194778
        );
    }
    #[test]
    fn reference_approximation_1364_2() {
        assert_eq!(
            positive_power(f32::from_bits(0x416b5400), f32::from_bits(0x3eaa7efa))
                .unwrap()
                .to_bits(),
            0x401caa8f
        );
    }
    #[test]
    fn reference_approximation_1806_3() {
        assert_eq!(
            positive_power(f32::from_bits(0x3a595ca6), f32::from_bits(0x3eaa7efa))
                .unwrap()
                .to_bits(),
            0x3dc0dbb6
        );
    }
    #[test]
    fn reference_approximation_1833_4() {
        assert_eq!(
            positive_power(f32::from_bits(0x43d8454e), f32::from_bits(0x3eaa7efa))
                .unwrap()
                .to_bits(),
            0x40f18449
        );
    }
    #[test]
    fn reference_approximation_1859_5() {
        assert_eq!(
            positive_power(f32::from_bits(0x2f96db79), f32::from_bits(0x3eaa7efa))
                .unwrap()
                .to_bits(),
            0x3a2b9a7a
        );
    }
    #[test]
    fn reference_approximation_1939_6() {
        assert_eq!(
            positive_power(f32::from_bits(0x471bfaae), f32::from_bits(0x3eaa7efa))
                .unwrap()
                .to_bits(),
            0x42083cae
        );
    }
    #[test]
    fn elementary_and_domain_controls() {
        assert_eq!(positive_power(0.0, 0.333).unwrap(), 0.0);
        assert_eq!(positive_power(1.0, 3.003).unwrap(), 1.0);
        assert!(positive_power(-1.0, 0.333).is_err());
        assert!(positive_power(1.0, f32::NAN).is_err());
    }
}
