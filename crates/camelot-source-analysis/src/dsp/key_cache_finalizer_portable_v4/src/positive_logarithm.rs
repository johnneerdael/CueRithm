include!("table.rs");

/// Positive-finite path of the pinned runtime's binary32 logarithm.
pub fn positive_logarithm(value: f32) -> Result<f32, &'static str> {
    if !value.is_finite() || value <= 0.0 {
        return Err("finite positive input required");
    }
    let mut word = value.to_bits();
    if word < 0x00800000 {
        let shifted = f32::from_bits(word | 0x3f800000) - 1.0;
        word = shifted.to_bits().wrapping_add(0xc1000000);
    }
    let exponent = (word.wrapping_add(0xc0c10000) as i32) >> 23;
    let mantissa_word = word & 0x007fffff;
    let index = ((mantissa_word + 0x00010000) >> 17) as usize;
    let mantissa = f64::from(f32::from_bits(mantissa_word | 0x3f800000));
    let (inverse_bits, logarithm_bits) = TABLE[index];
    let residual = mantissa.mul_add(f64::from_bits(inverse_bits), -1.0);
    let a = residual.mul_add(
        f64::from_bits(COEFFICIENTS[0]),
        f64::from_bits(COEFFICIENTS[1]),
    );
    let b = residual.mul_add(a, f64::from_bits(COEFFICIENTS[2]));
    let product = residual * b;
    let polynomial = residual.mul_add(product, residual);
    let leading = f64::from(exponent).mul_add(
        f64::from_bits(COEFFICIENTS[3]),
        f64::from_bits(logarithm_bits),
    );
    Ok((leading + polynomial) as f32)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn power_of_two() {
        assert_eq!(
            positive_logarithm(f32::from_bits(0x40000000))
                .unwrap()
                .to_bits(),
            0x3f317218
        );
    }
    #[test]
    fn rounding_regression() {
        assert_eq!(
            positive_logarithm(f32::from_bits(0x408be333))
                .unwrap()
                .to_bits(),
            0x3fbcd02a
        );
    }
    #[test]
    fn smallest_subnormal() {
        assert_eq!(
            positive_logarithm(f32::from_bits(0x00000001))
                .unwrap()
                .to_bits(),
            0xc2ce8ed0
        );
    }
    #[test]
    fn exact_one() {
        assert_eq!(
            positive_logarithm(f32::from_bits(0x3f800000))
                .unwrap()
                .to_bits(),
            0x00000000
        );
    }
    #[test]
    fn invalid_domain() {
        for x in [0.0, -0.0, -1.0, f32::INFINITY, f32::NAN] {
            assert!(positive_logarithm(x).is_err());
        }
    }
}
