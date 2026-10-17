pub const BINS: usize = 207;
pub const CHANNELS: usize = 6;
pub type Magnitudes = [f32; BINS];
pub type Features = [f32; CHANNELS];

#[derive(Debug, PartialEq, Eq)]
pub enum ContextError {
    InputLength,
    InputDomain,
    Finished,
    Math,
}

pub struct Context {
    magnitudes: std::collections::VecDeque<Magnitudes>,
    geometric: std::collections::VecDeque<Magnitudes>,
    taper: Magnitudes,
    bands: [Vec<f64>; 2],
    emitted: usize,
    finished: bool,
}
impl Default for Context {
    fn default() -> Self {
        Self::new()
    }
}
impl Context {
    pub fn new() -> Self {
        let basis = frequency_basis();
        Self {
            magnitudes: std::collections::VecDeque::new(),
            geometric: std::collections::VecDeque::new(),
            taper: melody_taper(),
            bands: [
                basis
                    .iter()
                    .map(|&f| band_weight(f, 0.0, 0.0, 150.0, 250.0))
                    .collect(),
                basis
                    .iter()
                    .map(|&f| band_weight(f, 250.0, 500.0, 99999.0, 99999.0))
                    .collect(),
            ],
            emitted: 0,
            finished: false,
        }
    }

    /// Accept default-profile nonnegative magnitudes. The upper bound is a
    /// prototype contract enclosing normalized PCM, not an original error ABI.
    pub fn feed(&mut self, row: &[f32]) -> Result<Option<Features>, ContextError> {
        if self.finished {
            return Err(ContextError::Finished);
        }
        if row.len() != BINS {
            return Err(ContextError::InputLength);
        }
        if row
            .iter()
            .any(|v| !v.is_finite() || v.is_sign_negative() || *v > 1e10)
        {
            return Err(ContextError::InputDomain);
        }
        self.push(row.try_into().unwrap())
    }

    pub fn finish(&mut self) -> Result<Vec<Features>, ContextError> {
        if self.finished {
            return Ok(Vec::new());
        }
        self.finished = true;
        let mut result = Vec::new();
        if !self.magnitudes.is_empty() {
            for _ in 0..6 {
                if let Some(row) = self.push([0.0; BINS])? {
                    result.push(row);
                }
            }
        }
        Ok(result)
    }

    pub fn emitted_frames(&self) -> usize {
        self.emitted
    }

    fn push(&mut self, row: Magnitudes) -> Result<Option<Features>, ContextError> {
        if self.magnitudes.len() > 12 {
            self.magnitudes.pop_front();
        }
        self.magnitudes.push_back(row);
        let count = self.magnitudes.len();
        if count >= 6 {
            if self.geometric.len() > 2 {
                self.geometric.pop_front();
            }
            let h = frequency_geometric(&self.magnitudes[count - 6])?;
            self.geometric.push_back(h);
        }
        if count < 7 {
            return Ok(None);
        }
        let result = self.frame()?;
        self.emitted += 1;
        Ok(Some(result))
    }

    fn frame(&self) -> Result<Features, ContextError> {
        let zero = [0.0; BINS];
        let mut m = [&zero; 13];
        for (i, row) in self.magnitudes.iter().enumerate() {
            m[13 - self.magnitudes.len() + i] = row;
        }
        let mut h = [&zero; 3];
        for (i, row) in self.geometric.iter().enumerate() {
            h[3 - self.geometric.len() + i] = row;
        }
        let a = std::array::from_fn(|j| arithmetic11(std::array::from_fn(|i| m[i + 1][j])));
        let b = frequency_arithmetic(&a);
        let mut g = [0.0; BINS];
        for (j, out) in g.iter_mut().enumerate() {
            *out = geometric11(std::array::from_fn(|i| m[i + 1][j]))?;
        }
        let c = (0.89_f64 as f32) * f32::from_bits(0x3e9cb924);
        let mut onset = 0.0_f32;
        let mut energy = 0.0_f32;
        let mut bands = [0.0_f32; 2];
        for j in 1..BINS {
            let local_h = (h[0][j] + h[2][j]).mul_add(0.0625, h[1][j] * 0.125);
            let residual = (-b[j]).mul_add(c, local_h);
            onset += ((residual * 1200.0) / (j as f32)).max(0.0);
            let value = energy_term(local_h, b[j], c, j)?;
            energy += value;
            for (band, sum) in bands.iter_mut().enumerate() {
                *sum += (f64::from(value) * self.bands[band][j]) as f32;
            }
        }
        let mut melody = 0.0_f32;
        for j in 3..BINS - 2 {
            let smooth = (g[j - 1] + g[j + 1]).mul_add(0.25, g[j] * 0.5);
            let residual = b[j].mul_add(f32::from_bits(0xbfb05048), smooth).max(0.0);
            let z = self.taper[j] * residual;
            melody = z.mul_add(z, melody);
        }
        let melody = (melody * f32::from_bits(0x27800000))
            .sqrt()
            .abs()
            .sqrt()
            .abs()
            * 6.0;
        let result = [
            0.0,
            amplitude(onset),
            melody,
            amplitude(energy),
            amplitude(bands[0]),
            amplitude(bands[1]),
        ];
        if result.iter().any(|v| !v.is_finite()) {
            return Err(ContextError::Math);
        }
        Ok(result)
    }
}

fn amplitude(sum: f32) -> f32 {
    let value = (sum * f32::from_bits(0x33800000)).sqrt().abs();
    let value = value * 3.0;
    let value = value * f32::from_bits(0x3e4ccccd);
    value * 6.0
}

fn power32(base: f32, exponent: u32) -> Result<f32, ContextError> {
    crate::dsp::darwin_powf_port_v1::positive_power(base, f32::from_bits(exponent))
        .map_err(|_| ContextError::Math)
}

fn energy_term(h: f32, b: f32, c: f32, j: usize) -> Result<f32, ContextError> {
    let baseline = c * b;
    let negative_baseline = -baseline;
    let negative_square = baseline * negative_baseline;
    let negative_cube = baseline * negative_square;
    let square = h * h;
    let residual = square.mul_add(h, negative_cube).max(0.0);
    let q = power32(residual, 0x3eaaaaab)?;
    let q = q * f32::from_bits(0x3e4ccccd);
    let q = power32(q, 0x3e99999a)?;
    let baseline = power32(b * f32::from_bits(0x3d0f5c29), 0x3f333333)?;
    let value = baseline * q;
    let value = value * f32::from_bits(0x45778000);
    Ok((value / (j as f32)).max(0.0))
}

fn arithmetic11(x: [f32; 11]) -> f32 {
    let p0 = (x[1] + x[9]) * 3.0;
    let p0 = (x[0] + x[10]).mul_add(2.0, p0);
    let p1 = (x[3] + x[7]) * 5.0;
    let p1 = (x[2] + x[8]).mul_add(4.0, p1);
    let p2 = x[5] * 7.0;
    let p2 = (x[4] + x[6]).mul_add(6.0, p2);
    ((p0 + p1) + p2) * f32::from_bits(0x3cae4c41)
}

fn frequency_arithmetic(row: &Magnitudes) -> Magnitudes {
    std::array::from_fn(|j| {
        if (5..BINS - 5).contains(&j) {
            return arithmetic11(std::array::from_fn(|i| row[j + i - 5]));
        }
        let (v1, v2, v3, v4, far) = if j < 5 {
            let mut v1 = row[j + 1];
            let mut v2 = row[j + 2];
            let mut v3 = row[j + 3];
            let mut v4 = row[j + 4];
            if j >= 1 {
                v1 += row[j - 1];
            }
            if j >= 2 {
                v2 += row[j - 2];
            }
            if j >= 3 {
                v3 += row[j - 3];
            }
            if j == 4 {
                v4 += row[j - 4];
            }
            (v1, v2, v3, v4, row[j + 5])
        } else {
            let mut v1 = row[j - 1];
            let mut v2 = row[j - 2];
            let mut v3 = row[j - 3];
            let mut v4 = row[j - 4];
            if j < BINS - 1 {
                v1 += row[j + 1];
            }
            if j < BINS - 2 {
                v2 += row[j + 2];
            }
            if j < BINS - 3 {
                v3 += row[j + 3];
            }
            if j < BINS - 4 {
                v4 += row[j + 4];
            }
            (v1, v2, v3, v4, row[j - 5])
        };
        let sum = v4 * 3.0;
        let sum = far.mul_add(2.0, sum);
        let sum = v3.mul_add(4.0, sum);
        let sum = v2.mul_add(5.0, sum);
        let sum = v1.mul_add(6.0, sum);
        row[j].mul_add(7.0, sum) * f32::from_bits(0x3cae4c41)
    })
}

fn geometric_combine(a: f64, b: f64, c: f64) -> Result<f32, ContextError> {
    let b = b * a;
    let c = c * b;
    let c = b * c;
    let c = a * c;
    crate::dsp::waveform_pow_rust_v1::geometric_power(c)
        .map(|v| v as f32)
        .map_err(|_| ContextError::Math)
}

fn geometric11(x: [f32; 11]) -> Result<f32, ContextError> {
    let x = x.map(f64::from);
    let c = ((x[0] * x[1]) * x[9]) * x[10];
    let b = ((x[2] * x[3]) * x[7]) * x[8];
    let a = (x[4] * x[5]) * x[6];
    geometric_combine(a, b, c)
}

fn frequency_geometric(row: &Magnitudes) -> Result<Magnitudes, ContextError> {
    let mut result = [0.0; BINS];
    let x = row.map(f64::from);
    for j in 0..BINS {
        let (a, b, c) = if j < 5 {
            let mut a = x[j] * x[j + 1];
            let mut b = x[j + 2] * x[j + 3];
            let mut c = x[j + 4] * x[j + 5];
            if j == 4 {
                c *= x[j - 4];
            }
            if j >= 3 {
                b *= x[j - 3];
            }
            if j >= 2 {
                b *= x[j - 2];
            }
            if j >= 1 {
                a *= x[j - 1];
            }
            (a, b, c)
        } else if j < BINS - 5 {
            (
                ((x[j - 1] * x[j]) * x[j + 1]),
                ((x[j - 3] * x[j - 2]) * x[j + 2]) * x[j + 3],
                ((x[j - 5] * x[j - 4]) * x[j + 4]) * x[j + 5],
            )
        } else {
            let mut a = x[j - 1] * x[j];
            if j < BINS - 1 {
                a *= x[j + 1];
            }
            let mut b = x[j - 3] * x[j - 2];
            if j < BINS - 2 {
                b *= x[j + 2];
            }
            if j < BINS - 3 {
                b *= x[j + 3];
            }
            let mut c = x[j - 5] * x[j - 4];
            if j < BINS - 4 {
                c *= x[j + 4];
            }
            (a, b, c)
        };
        result[j] = geometric_combine(a, b, c)?;
    }
    Ok(result)
}

fn melody_taper() -> Magnitudes {
    std::array::from_fn(|j| {
        let rise = (((j as i32 - 16) as f32) * 0.0625).clamp(0.0, 1.0) * 16.0;
        let fall = (((j as i32 - 70) as f32) / -40.0 + 1.0).clamp(0.0, 1.0);
        rise * fall
    })
}

fn frequency_basis() -> Vec<f64> {
    let mut start = 0_u64;
    let mut width = 1_u64;
    let mut result = Vec::new();
    while start <= 4096 {
        for _ in 0..=40 {
            let next = start + width;
            let half = (start + next - 1) as f64 * 0.5;
            result.push((44100.0_f64 / 4096.0) * half);
            if start > 4096 {
                break;
            }
            start = next;
        }
        width *= 2;
    }
    result
}

fn band_weight(f: f64, a: f64, b: f64, c: f64, d: f64) -> f64 {
    if f < a || f > d {
        0.0
    } else if f >= b && f <= c {
        1.0
    } else if f < b {
        (f - a) / (b - a)
    } else {
        (f - d) / (c - d)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_finish_emits_nothing() {
        assert!(Context::new().finish().unwrap().is_empty());
    }

    #[test]
    fn one_nonzero_frame_retains_centered_onset_and_energy_after_finish() {
        let mut context = Context::new();
        assert!(context.feed(&[1.0; BINS]).unwrap().is_none());
        let rows = context.finish().unwrap();
        assert_eq!(rows.len(), 1);
        let row = rows[0];
        assert!(row[1] > 0.0 && row[3] > 0.0 && row[4] > 0.0 && row[5] > 0.0);
        assert_eq!(row[2].to_bits(), 0);
        assert!(row[4] <= row[3] && row[5] <= row[3]);
    }

    #[test]
    fn isolated_frequency_activates_melody_only_with_complete_temporal_context() {
        let mut context = Context::new();
        let mut input = [0.0; BINS];
        input[40] = 1.0;
        let mut rows = Vec::new();
        for _ in 0..17 {
            if let Some(row) = context.feed(&input).unwrap() {
                rows.push(row);
            }
        }
        rows.extend(context.finish().unwrap());
        assert_eq!(rows.len(), 17);
        for (index, row) in rows.iter().enumerate() {
            assert_eq!(row[2] > 0.0, (5..=11).contains(&index));
            for channel in [0, 1, 3, 4, 5] {
                assert_eq!(row[channel].to_bits(), 0);
            }
        }
    }

    #[test]
    fn delayed_zero_rows_and_finish_preserve_every_input_frame() {
        for count in 1..=20 {
            let mut context = Context::new();
            let mut rows = Vec::new();
            for i in 0..count {
                let row = context.feed(&[0.0; BINS]).unwrap();
                assert_eq!(row.is_some(), i >= 6);
                if let Some(row) = row {
                    rows.push(row);
                }
            }
            rows.extend(context.finish().unwrap());
            assert_eq!(rows.len(), count);
            assert_eq!(context.emitted_frames(), count);
            assert!(rows.iter().flatten().all(|v| v.to_bits() == 0));
        }
    }

    #[test]
    fn finish_is_idempotent_and_later_feed_is_rejected() {
        let mut context = Context::new();
        context.feed(&[0.0; BINS]).unwrap();
        assert_eq!(context.finish().unwrap().len(), 1);
        assert!(context.finish().unwrap().is_empty());
        assert_eq!(context.feed(&[0.0; BINS]), Err(ContextError::Finished));
    }

    #[test]
    fn invalid_rows_fail_before_input_history_changes() {
        let mut context = Context::new();
        assert_eq!(context.feed(&[0.0; 2]), Err(ContextError::InputLength));
        for bad in [-0.0, -1.0, f32::NAN, f32::INFINITY, 1.1e10] {
            let mut row = [0.0; BINS];
            row[10] = bad;
            assert_eq!(context.feed(&row), Err(ContextError::InputDomain));
        }
        assert!(context.finish().unwrap().is_empty());
    }

    #[test]
    fn arithmetic_impulse_uses_source_weights_without_edge_renormalization() {
        let mut row = [0.0; BINS];
        row[0] = 1.0;
        let out = frequency_arithmetic(&row);
        assert_eq!(out[0], 7.0 * (1.0_f32 / 47.0));
        assert_eq!(out[5], 2.0 * (1.0_f32 / 47.0));
        assert_eq!(out[6], 0.0);
    }

    #[test]
    fn geometric_unit_rows_omit_missing_frequency_factors() {
        let values = frequency_geometric(&[1.0; BINS]).unwrap();
        assert!(values.iter().all(|v| v.to_bits() == 1.0_f32.to_bits()));
    }

    #[test]
    fn taper_uses_frequency_ramp_plateau_and_fall() {
        let w = melody_taper();
        for (i, expected) in [
            (16, 0.0),
            (24, 8.0),
            (32, 16.0),
            (70, 16.0),
            (90, 8.0),
            (110, 0.0),
        ] {
            assert_eq!(w[i], expected);
        }
    }

    #[test]
    fn band_basis_retains_inclusive_41_value_groups() {
        let f = frequency_basis();
        assert_eq!(f.len(), 271);
        assert_eq!(f[40], 430.6640625);
        assert_eq!(f[41], 446.81396484375);
        assert_eq!(f[80], 1286.60888671875);
    }

    #[test]
    fn trapezoid_keeps_coincident_low_edges_without_division_by_zero() {
        assert_eq!(band_weight(0.0, 0.0, 0.0, 150.0, 250.0), 1.0);
        assert_eq!(band_weight(200.0, 0.0, 0.0, 150.0, 250.0), 0.5);
        assert_eq!(band_weight(250.0, 0.0, 0.0, 150.0, 250.0), 0.0);
        assert_eq!(band_weight(375.0, 250.0, 500.0, 99999.0, 99999.0), 0.5);
    }
}
