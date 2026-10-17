use super::tables::{EXP_COEFFICIENT_BITS, EXP_ROWS, LOG_COEFFICIENT_BITS, LOG_ROWS};
const ONE: u64 = 0x3ff0000000000000;
const FRACTION: u64 = 0x000fffffffffffff;
pub(super) fn source_power(base: f64, exponent: f64) -> Result<f64, String> {
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
