//! Private finite-domain reproduction of one observed system double-pow profile.

mod tables;

use tables::{EXP_COEFFICIENT_BITS, EXP_ROWS, LOG_COEFFICIENT_BITS, LOG_ROWS};

const ONE: u64 = 0x3ff0000000000000;
const FRACTION: u64 = 0x000fffffffffffff;
const EXPONENT: f64 = f64::from_bits(0x3fa8618618618618);

#[derive(Debug, PartialEq, Eq)]
pub struct DomainError;

/// Reproduce the captured Darwin double-pow profile for exponent bits3fa8618618618618.
///
/// Accept positive finite bases and positive zero. Reject negativezero, negative,
/// infinite and NaN bases. This is not an arbitrary-exponent or full-IEEE pow API.
pub fn geometric_power(base: f64) -> Result<f64, DomainError> {
    if base.is_sign_negative() || !base.is_finite() {
        return Err(DomainError);
    }
    if base == 0.0 {
        return Ok(0.0);
    }

    let mut word = base.to_bits();
    if word < 0x0010000000000000 {
        let normalized = f64::from_bits(word | ONE) - 1.0;
        word = normalized.to_bits().wrapping_add(0xc020000000000000);
    }
    let fraction = word & FRACTION;
    if fraction == 0 {
        if word == ONE {
            return Ok(1.0);
        }
        let signed_carrier = word.wrapping_sub(ONE) as i64;
        let scaled_integer = (signed_carrier as f64) * f64::from_bits((1023 - 45) << 52);
        let high = scaled_integer * EXPONENT;
        let low = scaled_integer.mul_add(EXPONENT, -high);
        return Ok(reconstruct(high, low));
    }

    let index = ((fraction.wrapping_add(1 << 44) & 0x001fe00000000000) >> 45) as usize;
    let exponent_carrier = (word.wrapping_add(0xc018100000000000) as i64) >> 52;
    let [packed, high_word] = LOG_ROWS[index];
    let inverse = f64::from_bits((packed & 0xffffffff) << 32);
    let table_high = f64::from_bits(high_word);
    let table_low = f64::from_bits(packed & 0xffffffff00000000);
    let mantissa_word = fraction | ONE;
    let mantissa = f64::from_bits(mantissa_word);
    let product = inverse * mantissa;
    let mantissa_high = f64::from_bits(mantissa_word & 0xffffffffffe00000);
    let mantissa_low = mantissa - mantissa_high;
    let error = inverse.mul_add(mantissa_high, -product);
    let r = product - 1.0;
    let r_low = inverse.mul_add(mantissa_low, error);

    let c = LOG_COEFFICIENT_BITS.map(f64::from_bits);
    let polynomial = r.mul_add(c[0], c[1]);
    let polynomial = r.mul_add(polynomial, c[2]);
    let polynomial = r.mul_add(polynomial, c[3]);
    let polynomial = r.mul_add(polynomial, c[4]);
    let square = r * r;
    let fourth = square * square;
    let polynomial = fourth * polynomial;

    let a = c[5].mul_add(r, c[7]);
    let b = c[6].mul_add(r, c[8]);
    let error = c[5].mul_add(r, -(a - c[7]));
    let b = c[5].mul_add(r_low, b);
    let b = error + b;
    let c_high = a.mul_add(r, c[9]);
    let c_low = b.mul_add(r, c[10]);
    let error = a.mul_add(r, -(c_high - c[9]));
    let c_low = a.mul_add(r_low, c_low);
    let c_low = error + c_low;
    let linear = c_high * r;
    let c_low = c_low * r;
    let error = c_high.mul_add(r, -linear);
    let c_low = c_high.mul_add(r_low, c_low);
    let tail = error + c_low;

    let exponent_carrier = exponent_carrier as f64;
    let high = ((exponent_carrier + table_high) + linear) + polynomial;
    let high = f64::from_bits(high.to_bits() & !1);
    let low = (((exponent_carrier - high) + table_high) + linear) + polynomial;
    let low = table_low + low;
    let low = low + tail;
    let scaled_y = EXPONENT * 128.0;
    let scaled_high = scaled_y * high;
    let scaled_low = scaled_y.mul_add(high, -scaled_high);
    let scaled_low = scaled_y.mul_add(low, scaled_low);
    Ok(reconstruct(scaled_high, scaled_low))
}

fn reconstruct(high: f64, low: f64) -> f64 {
    let floor = high.floor();
    let integer = floor as i64;
    let fraction = high - floor;
    let [factor_word, correction] = EXP_ROWS[(integer & 127) as usize];
    let corrected = low - f64::from_bits(correction);
    let corrected = corrected + fraction;
    let e = EXP_COEFFICIENT_BITS.map(f64::from_bits);
    let polynomial = fraction.mul_add(e[0], e[1]);
    let polynomial = fraction.mul_add(polynomial, e[2]);
    let polynomial = corrected.mul_add(polynomial, e[3]);
    let polynomial = corrected.mul_add(polynomial, e[4]);
    let polynomial = corrected * polynomial;

    let first_scale = (integer as u64).wrapping_shl(44) & 0xfff0000000000000;
    let second_scale = (integer as u64).wrapping_shl(45) & 0xfff0000000000000;
    let second_scale = second_scale.wrapping_sub(first_scale).wrapping_add(ONE);
    let factor = f64::from_bits(first_scale.wrapping_add(factor_word));
    let value = factor.mul_add(polynomial, factor);
    value * f64::from_bits(second_scale)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn exact(base: u64, result: u64, narrow: u32) {
        let value = geometric_power(f64::from_bits(base)).unwrap();
        assert_eq!(
            (value.to_bits(), (value as f32).to_bits()),
            (result, narrow)
        );
    }

    #[test]
    fn positive_zero() {
        exact(0, 0, 0);
    }

    #[test]
    fn unit() {
        exact(0x3ff0000000000000, 0x3ff0000000000000, 0x3f800000);
    }

    #[test]
    fn two_power_shortcut() {
        exact(0x4000000000000000, 0x3ff08973e2c8a9c6, 0x3f844b9f);
    }

    #[test]
    fn smallest_subnormal_power() {
        exact(1, 0x3cbcfbb031a741b5, 0x25e7dd82);
    }

    #[test]
    fn smallest_normal_power() {
        exact(0x0010000000000000, 0x3ce428a2f98d7296, 0x27214518);
    }

    #[test]
    fn log_table_edge() {
        exact(0x3ff0100000000000, 0x3ff000c2af8bccda, 0x3f800615);
    }

    #[test]
    fn ordinary_normal() {
        exact(0x3171b85dda3c9dab, 0x3f3f1ca7d4fcb189, 0x39f8e53f);
    }

    #[test]
    fn rejects_out_of_domain() {
        for word in [
            0x8000000000000000,
            0xbff0000000000000,
            0x7ff0000000000000,
            0x7ff8000000000001,
        ] {
            assert_eq!(geometric_power(f64::from_bits(word)), Err(DomainError));
        }
    }
}
