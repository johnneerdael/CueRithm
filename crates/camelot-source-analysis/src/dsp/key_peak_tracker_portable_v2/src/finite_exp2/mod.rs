include!("table.rs");

/// Pinned runtime's finite-argument binary32 exp2 approximation.
/// Preserves that approximation's rounding, including overflow/underflow.
pub fn finite_exp2(value: f32) -> Result<f32, &'static str> {
    if !value.is_finite() {
        return Err("finite argument required");
    }
    if value <= -256.0 {
        return Ok(0.0);
    }
    if value >= 256.0 {
        return Ok(f32::INFINITY);
    }
    let value = f64::from(value);
    // FADD/FSUB magic shift rounds to nearest multiples of1/128, ties even.
    let biased = value + SHIFT;
    let rounded = biased - SHIFT;
    let word = biased.to_bits();
    let residual = value - rounded;
    let scale_word = TABLE[(word & 127) as usize].wrapping_add(word.wrapping_shl(45));
    let scale = f64::from_bits(scale_word);
    let polynomial = C2.mul_add(residual, C1);
    let product = polynomial * residual;
    Ok(product.mul_add(scale, scale) as f32)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_integer_powers_and_signed_zero() {
        for (argument, expected) in [
            (0.0, 1.0_f32),
            (-0.0, 1.0),
            (1.0, 2.0),
            (-1.0, 0.5),
            (-149.0, f32::from_bits(1)),
        ] {
            assert_eq!(finite_exp2(argument).unwrap().to_bits(), expected.to_bits());
        }
    }

    #[test]
    fn native_peak_rounding_regressions() {
        for (argument, expected) in [
            (0xbf102e7b, 0x3f2d426f),
            (0xbed7535c, 0x3f3f4449),
            (0xbe38a9a9, 0x3f61eb67),
            (0xbf3664a8, 0x3f1c3ad9),
            (0xbec46246, 0x3f443c21),
            (0xbed948ac, 0x3f3ec2a5),
            (0xbf313991, 0x3f1e6e77),
            (0xbf429fa7, 0x3f172419),
            (0xbdd350d6, 0x3f6e548d),
        ] {
            assert_eq!(
                finite_exp2(f32::from_bits(argument)).unwrap().to_bits(),
                expected
            );
        }
    }

    #[test]
    fn finite_extremes_preserve_underflow_and_overflow() {
        assert_eq!(finite_exp2(-256.0).unwrap().to_bits(), 0);
        assert_eq!(finite_exp2(-f32::MAX).unwrap().to_bits(), 0);
        assert_eq!(finite_exp2(256.0).unwrap(), f32::INFINITY);
        assert_eq!(finite_exp2(f32::MAX).unwrap(), f32::INFINITY);
    }

    #[test]
    fn rejects_nonfinite_arguments() {
        for value in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
            assert!(finite_exp2(value).is_err());
        }
    }
}
