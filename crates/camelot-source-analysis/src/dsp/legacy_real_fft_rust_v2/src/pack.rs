#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Direction {
    Forward,
    Inverse,
}

#[derive(Debug, PartialEq, Eq)]
pub enum InputError {
    Length,
    Domain,
}

/// Reproduce the stride-one real-transform SIMD pre/post stage.
///
/// `packed` contains equal real/imaginary planes; coefficients contain
/// N/4 cosine words followed by N/4 sine words. The full transform must
/// generate these coefficients independently. This function does no FFT.
pub fn combine(
    packed: &[f32],
    coefficients: &[f32],
    direction: Direction,
) -> Result<Vec<f32>, InputError> {
    let n = packed.len();
    if !(16..=16384).contains(&n) || !n.is_power_of_two() || coefficients.len() != n / 2 {
        return Err(InputError::Length);
    }
    if packed.iter().any(|x| !x.is_finite() || x.abs() > 1.0e24)
        || coefficients.iter().any(|x| !x.is_finite() || x.abs() > 1.0)
    {
        return Err(InputError::Domain);
    }
    let half = n / 2;
    let quarter = n / 4;
    let mut output = vec![0.0; n];
    let (dc_real, dc_imag) = match direction {
        Direction::Forward => (packed[0] + packed[0], packed[half] + packed[half]),
        Direction::Inverse => (packed[0], packed[half]),
    };
    output[0] = dc_real + dc_imag;
    output[half] = dc_real - dc_imag;
    for k in 1..=quarter {
        let mirror = half - k;
        let a_real = packed[k];
        let a_imag = packed[half + k];
        let b_real = packed[mirror];
        let b_imag = packed[half + mirror];
        let real_sum = a_real + b_real;
        let imag_difference = a_imag - b_imag;
        let imag_sum = a_imag + b_imag;
        let real_difference = match direction {
            Direction::Forward => b_real - a_real,
            Direction::Inverse => a_real - b_real,
        };
        let cosine = coefficients[k - 1];
        let sine = coefficients[quarter + k - 1];
        // Source SIMD uses four separate products; no implicit FMA.
        let real_first = cosine * imag_sum;
        let real_second = sine * real_difference;
        let imag_first = cosine * real_difference;
        let imag_second = sine * imag_sum;
        let real_rotation = real_first + real_second;
        let imag_rotation = imag_first - imag_second;
        let (low_real, high_real) = match direction {
            Direction::Forward => (real_sum + real_rotation, real_sum - real_rotation),
            Direction::Inverse => (real_sum - real_rotation, real_sum + real_rotation),
        };
        let low_imag = imag_rotation + imag_difference;
        let high_imag = imag_rotation - imag_difference;
        output[k] = low_real;
        output[half + k] = low_imag;
        // Both sides address the central bin; source high-side store is last.
        output[mirror] = high_real;
        output[half + mirror] = high_imag;
    }
    Ok(output)
}
