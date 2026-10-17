//! Fixed tracker scoring contract; source f64 pow DAG, not host pow.
mod tables;
use tables::{EXP_COEFFICIENT_BITS, EXP_ROWS, LOG_COEFFICIENT_BITS, LOG_ROWS};
const ONE: u64 = 0x3ff0000000000000;
const FRACTION: u64 = 0x000fffffffffffff;
const FLOOR: f64 = f64::from_bits(0x3869000000000000);

/// frexp exponent only; the tracker discards the returned mantissa.
pub fn frexp_exponent(base: f64) -> Result<i32, String> {
    if !base.is_finite() || base.is_sign_negative() {
        return Err("nonnegative finite source base required".into());
    }
    let bits = base.to_bits();
    if bits == 0 {
        return Ok(0);
    }
    let raw = (bits >> 52) as i32;
    if raw != 0 {
        Ok(raw - 1022)
    } else {
        Ok((63 - bits.leading_zeros()) as i32 - 1073)
    }
}

/// Source frexp guard, pow/f64 floor, then one f32 conversion.
/// Exponent comes from tracker f32 state; +0 base has source pow result+0.
pub fn score(base: f64, exponent: f32) -> Result<f32, String> {
    let e = frexp_exponent(base)?;
    if base > 1. || !exponent.is_finite() || !(0.5..=50.).contains(&exponent) {
        return Err("requires base[+0,1] and finite exponent[.5,50]".into());
    }
    if exponent * e.abs() as f32 >= 1000. {
        return Ok(FLOOR as f32);
    }
    let raw = if base == 0. {
        0.
    } else {
        positive_power(base, f64::from(exponent))?
    };
    if !raw.is_finite() || raw < 0. {
        return Err("unexpected pow result".into());
    }
    Ok((if raw > FLOOR { raw } else { FLOOR }) as f32)
}

pub fn confidence(smooth: f32, state: f32) -> Result<f32, String> {
    if !smooth.is_finite()
        || smooth.is_sign_negative()
        || smooth > 1.
        || !state.is_finite()
        || !(0.5..=50.).contains(&state)
    {
        return Err("confidence requires smooth[+0,1],state[.5,50]".into());
    }
    if smooth == 0. {
        return Ok(0.);
    }
    let exponent = 1.0 / f64::from(state);
    Ok(positive_power(f64::from(smooth), exponent)? as f32)
}

fn positive_power(base: f64, exponent: f64) -> Result<f64, String> {
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
        let high = scaled_integer * exponent;
        let low = scaled_integer.mul_add(exponent, -high);
        return reconstruct(high, low);
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
    let scaled_y = exponent * 128.0;
    let scaled_high = scaled_y * high;
    let scaled_low = scaled_y.mul_add(high, -scaled_high);
    let scaled_low = scaled_y.mul_add(low, scaled_low);
    reconstruct(scaled_high, scaled_low)
}

fn reconstruct(high: f64, low: f64) -> Result<f64, String> {
    if !high.is_finite()
        || !(f64::from_bits(0xc100cc0000000000)..=f64::from_bits(0x4100040000000000))
            .contains(&high)
    {
        return Err("pow reconstruction outside source normal range".into());
    }
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
    Ok(value * f64::from_bits(second_scale))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn frexp_zero_normal_and_subnormal_boundaries() {
        for (x, e) in [
            (0., 0),
            (1., 1),
            (0.5, 0),
            (0.25, -1),
            (f64::from_bits(1), -1073),
            (f64::MIN_POSITIVE, -1021),
        ] {
            assert_eq!(frexp_exponent(x).unwrap(), e);
        }
    }
    #[test]
    fn exact_powers_and_variable_exponents() {
        assert_eq!(score(1., 50.).unwrap(), 1.);
        assert_eq!(score(0.25, 2.).unwrap(), 0.0625);
        assert_eq!(score(0.25, 0.5).unwrap(), 0.5);
        assert_eq!(score(0.5, 3.).unwrap(), 0.125);
    }
    #[test]
    fn caller_guard_and_double_clamp_produce_the_source_float_floor() {
        let floor = f64::from_bits(0x3869000000000000) as f32;
        assert_eq!(score(0., 1.).unwrap().to_bits(), floor.to_bits());
        assert_eq!(
            score(f64::from_bits(1), 50.).unwrap().to_bits(),
            floor.to_bits()
        );
        assert_eq!(
            score(2f64.powi(-30), 40.).unwrap().to_bits(),
            floor.to_bits()
        );
        assert_eq!(
            score(2f64.powi(-100), 2.).unwrap().to_bits(),
            floor.to_bits()
        );
    }
    #[test]
    fn invalid_inputs_are_rejected() {
        for x in [-0., -1., 1.0001, f64::INFINITY, f64::NAN] {
            assert!(score(x, 1.).is_err());
        }
        for y in [0., 0.499, 50.001, f32::INFINITY, f32::NAN] {
            assert!(score(0.5, y).is_err());
        }
    }
}

#[cfg(test)]
mod confidence_tests {
    use super::*;
    #[test]
    fn emitted_confidence_uses_double_reciprocal_state() {
        assert_eq!(confidence(0.25, 2.).unwrap(), 0.5);
        assert_eq!(confidence(0.5, 0.5).unwrap(), 0.25);
        assert_eq!(confidence(1., 50.).unwrap(), 1.);
        assert_eq!(confidence(0., 1.).unwrap().to_bits(), 0);
    }
    #[test]
    fn confidence_domain_is_explicit() {
        for x in [-0., -1., 1.001, f32::NAN, f32::INFINITY] {
            assert!(confidence(x, 1.).is_err());
        }
        for e in [0., 0.499, 50.001, f32::NAN, f32::INFINITY] {
            assert!(confidence(0.5, e).is_err());
        }
    }
}

mod decimal_exp {
    include!("exp_table.rs");
    pub fn calculate(value: f32) -> Result<f32, String> {
        let bits = value.to_bits();
        if bits >> 31 != 0 || bits > 0x40000000 {
            return Err("decimal exp requires +0..2".into());
        }
        let z = f64::from(value) * f64::from_bits(0x407a934f0979a371);
        let rounded = z.round();
        let n = rounded as u64;
        let r = z - rounded;
        let scale = f64::from_bits(TABLE[(n & 127) as usize].wrapping_add(n.wrapping_shl(45)));
        let p = f64::from_bits(0x3eeebfbdff30d656).mul_add(r, f64::from_bits(0x3f762e4453e10dae));
        let q = p * r;
        Ok(q.mul_add(scale, scale) as f32)
    }
}
pub fn exp10(value: f32) -> Result<f32, String> {
    decimal_exp::calculate(value)
}
#[cfg(test)]
mod exp10_tests {
    use super::*;
    #[test]
    fn decimal_exp_has_exact_endpoints() {
        assert_eq!(exp10(0.).unwrap(), 1.);
        assert_eq!(exp10(1.).unwrap(), 10.);
        assert_eq!(exp10(2.).unwrap(), 100.);
    }
    #[test]
    fn decimal_exp_domain_is_explicit() {
        for x in [-0., -1., 2.001, f32::NAN, f32::INFINITY] {
            assert!(exp10(x).is_err());
        }
    }
}
