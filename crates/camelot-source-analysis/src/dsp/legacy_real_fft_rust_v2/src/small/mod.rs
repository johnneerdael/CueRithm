//! Selected unit-stride small system complex FFT arithmetic, independent Rust.
mod generated;
fn validate(re: &[f32], im: &[f32]) -> Result<(), String> {
    if re.len() != im.len() || ![8, 16, 32, 64, 128].contains(&re.len()) {
        return Err("equal splitplanes ofN8/16/32/64/128 required".into());
    }
    if re
        .iter()
        .chain(im)
        .any(|x| !x.is_finite() || x.abs() > 1.0e24)
    {
        return Err("finite amplitudes<=1e24 required".into());
    }
    Ok(())
}
fn first4(re: &[f32], im: &[f32]) -> (Vec<f32>, Vec<f32>) {
    let n = re.len();
    let q = n / 4;
    let mut r = vec![0.0; n];
    let mut i = r.clone();
    for j in 0..q {
        let ra = re[j] + re[j + 2 * q];
        let rd = re[j] - re[j + 2 * q];
        let rb = re[j + q] + re[j + 3 * q];
        let re1 = re[j + q] - re[j + 3 * q];
        let ia = im[j] + im[j + 2 * q];
        let id = im[j] - im[j + 2 * q];
        let ib = im[j + q] + im[j + 3 * q];
        let ie = im[j + q] - im[j + 3 * q];
        r[j] = ra + rb;
        r[j + q] = ra - rb;
        r[j + 2 * q] = rd - ie;
        r[j + 3 * q] = rd + ie;
        i[j] = ia + ib;
        i[j + q] = ia - ib;
        i[j + 2 * q] = id + re1;
        i[j + 3 * q] = id - re1;
    }
    (r, i)
}
fn first8(re: &[f32], im: &[f32]) -> (Vec<f32>, Vec<f32>) {
    let n = re.len();
    let q = n / 8;
    let mut r = vec![0.0; n];
    let mut i = r.clone();
    for j in 0..q {
        let a = std::array::from_fn(|k| re[j + k * q]);
        let b = std::array::from_fn(|k| im[j + k * q]);
        let (x, y) = generated::first8(&a, &b);
        for k in 0..8 {
            r[j + k * q] = x[k];
            i[j + k * q] = y[k];
        }
    }
    (r, i)
}
/// Source6-scalar generic row: cos(theta),tan(theta),cos(2theta),tan(2theta),cos(3theta)/cos(theta),tan(3theta).
fn row(theta: f64) -> [f32; 6] {
    let (s1, c1) = theta.sin_cos();
    let (s2, c2) = (theta + theta).sin_cos();
    let (s3, c3) = (theta * 3.0).sin_cos();
    [
        c1 as f32,
        (s1 / c1) as f32,
        c2 as f32,
        (s2 / c2) as f32,
        (c3 / c1) as f32,
        (s3 / c3) as f32,
    ]
}
fn middle_coefficients(n: usize) -> Vec<f32> {
    let mut output = Vec::with_capacity(n / 16 * 6);
    for index in 0..n / 16 {
        let fraction =
            (4_u64 * index as u64).reverse_bits() as f64 * f64::from_bits(0x3bf0000000000000);
        let angle = fraction * f64::from_bits(0x401921fb54442d18);
        output.extend(row(angle));
    }
    output
}
fn final_stage(re: &[f32], im: &[f32]) -> (Vec<f32>, Vec<f32>) {
    let n = re.len();
    let q = n / 4;
    let groups = n / 16;
    let width = groups.trailing_zeros();
    let step = f64::from_bits(0x401921fb54442d18) / n as f64;
    let mut r = vec![0.0; n];
    let mut i = r.clone();
    // Source bitreverse-pair scheduling is a permutation of independent tiles.
    // Read immutable snapshots so every pair has the same inputs as the source's preloads.
    for destination in 0..groups {
        let source = if width == 0 {
            0
        } else {
            destination.reverse_bits() >> (usize::BITS - width)
        };
        let a = std::array::from_fn(|part| {
            std::array::from_fn(|lane| re[part * q + 4 * source + lane])
        });
        let b = std::array::from_fn(|part| {
            std::array::from_fn(|lane| im[part * q + 4 * source + lane])
        });
        let rows: [[f32; 6]; 4] =
            std::array::from_fn(|lane| row(step * (4 * destination + lane) as f64));
        let coeff = std::array::from_fn(|kind| std::array::from_fn(|lane| rows[lane][kind]));
        let (x, y) = generated::final_tile(a, b, coeff);
        for part in 0..4 {
            for lane in 0..4 {
                r[part * q + 4 * destination + lane] = x[part][lane];
                i[part * q + 4 * destination + lane] = y[part][lane];
            }
        }
    }
    (r, i)
}
fn core(re: &[f32], im: &[f32]) -> (Vec<f32>, Vec<f32>) {
    let n = re.len();
    if n == 8 {
        return generated::direct8(re, im);
    }
    let (r, i) = if n == 16 || n == 64 {
        first4(re, im)
    } else {
        first8(re, im)
    };
    let (r, i) = match n {
        64 => generated::middle64(&r, &i, &middle_coefficients(n)),
        128 => generated::middle128(&r, &i, &middle_coefficients(n)),
        _ => (r, i),
    };
    final_stage(&r, &i)
}
/// Forward uses the source negative direction flag: input AND output plane swaps.
/// Inverse is unnormalized. Only the selected size/stride/profile is implemented.
pub fn transform(re: &[f32], im: &[f32], inverse: bool) -> Result<(Vec<f32>, Vec<f32>), String> {
    validate(re, im)?;
    let (r, i) = if inverse {
        core(re, im)
    } else {
        let (i, r) = core(im, re);
        (r, i)
    };
    if r.iter().chain(&i).any(|x| !x.is_finite()) {
        return Err("nonfinite stage output".into());
    }
    Ok((r, i))
}

/// Source descriptor tables; final rows follow the source bitreverse-pair visit order.
#[allow(dead_code)] // Retain the unchanged source diagnostic alongside copied arithmetic.
pub fn coefficient_tables(n: usize) -> Result<(Vec<f32>, Vec<f32>), String> {
    if ![16, 32, 64, 128].contains(&n) {
        return Err("descriptorN16/32/64/128required".into());
    }
    let groups = n / 16;
    let width = groups.trailing_zeros();
    let reverse = |x: usize| {
        if width == 0 {
            0
        } else {
            x.reverse_bits() >> (usize::BITS - width)
        }
    };
    let mut order = Vec::with_capacity(groups);
    let mut low = 0;
    let mut high = 0;
    loop {
        order.push(high);
        let next = low + 1;
        high = reverse(next);
        if low >= high {
            break;
        }
        low = next;
        if low != high {
            let mut paired = low;
            loop {
                order.push(paired);
                order.push(high);
                high += 1;
                paired = reverse(high);
                if high == paired {
                    break;
                }
            }
        }
    }
    let mut sorted = order.clone();
    sorted.sort_unstable();
    if sorted != (0..groups).collect::<Vec<_>>() {
        return Err("invalid bounded source row schedule".into());
    }
    let step = f64::from_bits(0x401921fb54442d18) / n as f64;
    let mut final_rows = Vec::with_capacity(groups * 24);
    for group in order {
        let rows: [[f32; 6]; 4] = std::array::from_fn(|lane| row(step * (group * 4 + lane) as f64));
        for kind in 0..6 {
            for item in &rows {
                final_rows.push(item[kind]);
            }
        }
    }
    let middle = middle_coefficients(n);
    if middle.iter().chain(&final_rows).any(|x| !x.is_finite()) {
        return Err("nonfinite generatedcoefficient".into());
    }
    Ok((middle, final_rows))
}
