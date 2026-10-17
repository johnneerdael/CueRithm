mod power;
pub mod stream;
mod tables;
pub fn lookup(values: &[f32], p: f64) -> Result<f64, String> {
    if !p.is_finite()
        || !(0.0..=1.).contains(&p)
        || values.len() > 100000
        || values
            .iter()
            .any(|v| !v.is_finite() || v.is_sign_negative())
        || values.windows(2).any(|w| w[0] > w[1])
    {
        return Err("sorted nonnegative finite lookup profile required".into());
    }
    if values.is_empty() {
        return Ok(0.);
    }
    if values.len() == 1 || p <= 0. {
        return Ok(f64::from(values[0]));
    }
    let t = p.min(1.) * (values.len() - 1) as f64;
    let floor = t.floor() as usize;
    let i = if values.len() - 1 > floor {
        floor
    } else {
        values.len() - 2
    };
    let fraction = t - i as f64;
    let delta = values[i + 1] - values[i];
    Ok(f64::from(delta).mul_add(fraction, f64::from(values[i])))
}
pub fn inverse_root(x: f64) -> Result<f64, String> {
    if !x.is_finite() || !(0.0001..=1.).contains(&x) {
        return Err("bounded positive inverse-root base required".into());
    }
    power::source_power(x, -0.5)
}
pub struct Bands {
    history: [Vec<f32>; 8],
    sustained: [Vec<f32>; 14],
    transient: [Vec<f32>; 14],
    frames: usize,
    ranges: [(usize, usize); 14],
    gains: [f64; 14],
}
impl Bands {
    pub fn new() -> Result<Self, String> {
        let setup = crate::dsp::energy_setup_rust_v1::default_setup();
        let mut ranges = [(0, 0); 14];
        let mut gains = [0.; 14];
        for (i, (lo, hi)) in setup.bands.into_iter().enumerate() {
            let low = ((lo * 2048.) / 44100.).floor().max(0.) as usize;
            let high = (((hi * 2048.) / 44100.).ceil() as usize).min(1025);
            ranges[i] = if high > low { (low, high) } else { (0, 0) };
            let middle = (lo + hi) * 0.5;
            gains[i] = inverse_root(middle / 16000.)? * 0.5;
        }
        Ok(Self {
            history: std::array::from_fn(|_| Vec::new()),
            sustained: std::array::from_fn(|_| Vec::new()),
            transient: std::array::from_fn(|_| Vec::new()),
            frames: 0,
            ranges,
            gains,
        })
    }
    pub fn push(&mut self, spectrum: &[f32]) -> Result<(), String> {
        if spectrum.len() != 1025
            || spectrum
                .iter()
                .any(|v| !v.is_finite() || v.is_sign_negative() || *v > 65536.)
        {
            return Err("bounded nonnegative1025-bin spectrum required".into());
        }
        if self.frames >= 100000 {
            return Err("band frame bound".into());
        }
        let mut smooth = Vec::with_capacity(1025);
        for i in 0..1025usize {
            let width = (i + 1).min(20).min(1025 - i);
            let half = (width - 1) / 2;
            let mut sorted = spectrum[i - half..i + half + 1].to_vec();
            sorted.sort_by(f32::total_cmp);
            smooth.push(lookup(&sorted, 0.3)? as f32);
        }
        if self.history[0].is_empty() {
            for h in &mut self.history {
                h.resize(1025, 0.);
            }
        }
        let mut difference = Vec::with_capacity(1025);
        for (i, &current) in smooth.iter().enumerate() {
            let oldest = f64::from(self.history[1][i])
                .max(f64::from(self.history[2][i]))
                .max(0.);
            let newest = current.min(self.history[0][i]);
            let value = f64::from(newest) - oldest;
            difference.push(if value > 0. { value as f32 } else { 0. });
        }
        for band in 0..14 {
            let (begin, end) = self.ranges[band];
            let mut original = 0f64;
            let mut transient = 0f64;
            for i in begin..end {
                original += f64::from(spectrum[i]);
                transient += f64::from(difference[i]);
            }
            self.transient[band].push((self.gains[band] * transient) as f32);
            self.sustained[band].push((self.gains[band] * (original - transient.max(0.))) as f32);
        }
        self.history.rotate_right(1);
        self.history[0] = smooth;
        self.frames += 1;
        Ok(())
    }
    pub fn skip(&mut self) -> Result<(), String> {
        if self.frames >= 100000 {
            return Err("band frame bound".into());
        }
        for row in self.sustained.iter_mut().chain(&mut self.transient) {
            row.push(0.);
        }
        self.frames += 1;
        Ok(())
    }
    pub fn state_bytes(&self) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(&0u32.to_le_bytes());
        let history = if self.history[0].is_empty() { 0 } else { 1025 };
        let rows = if self.frames == 0 { 0 } else { 14 };
        for v in [self.frames, history, rows] {
            out.extend_from_slice(&(v as u32).to_le_bytes());
        }
        if self.frames > 0 {
            for &(lo, hi) in &self.ranges {
                out.extend_from_slice(&(lo as u32).to_le_bytes());
                out.extend_from_slice(&(hi as u32).to_le_bytes());
            }
        }
        if history > 0 {
            for row in &self.history {
                for value in row {
                    out.extend_from_slice(&value.to_le_bytes());
                }
            }
        }
        if rows > 0 {
            for row in self.sustained.iter().chain(&self.transient) {
                for value in row {
                    out.extend_from_slice(&value.to_le_bytes());
                }
            }
        }
        let length = (out.len() - 4) as u32;
        out[..4].copy_from_slice(&length.to_le_bytes());
        out
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn lookup_preserves_f32_delta_and_f64_interpolation() {
        assert_eq!(lookup(&[], 0.3).unwrap().to_bits(), 0);
        assert_eq!(lookup(&[1., 33554432.], 1.).unwrap(), 33554433.);
        assert_eq!(lookup(&[0., 10.], 0.3).unwrap(), 3.);
    }
    #[test]
    fn lookup_rejects_unsorted_or_nonfinite() {
        assert!(lookup(&[2., 1.], 0.3).is_err());
        assert!(lookup(&[0., f32::INFINITY], 0.3).is_err());
    }
    #[test]
    fn negative_half_power_keeps_exact_powers() {
        assert_eq!(inverse_root(0.25).unwrap(), 2.);
        assert_eq!(inverse_root(0.0625).unwrap(), 4.);
        assert!(inverse_root(0.).is_err());
    }
    #[test]
    fn steady_spectrum_requires_two_frames_for_transient_then_settles() {
        let mut b = Bands::new().unwrap();
        let mut s = vec![1.; 1025];
        s[0] = 0.;
        b.push(&s).unwrap();
        assert!(b.transient.iter().all(|v| v[0].to_bits() == 0));
        b.push(&s).unwrap();
        assert!(b.transient.iter().all(|v| v[1] > 0.));
        b.push(&s).unwrap();
        assert!(b.transient.iter().all(|v| v[2].to_bits() == 0));
        assert!(b.sustained.iter().all(|v| v[2] > 0.));
    }
    #[test]
    fn skip_appends_zero_without_advancing_history() {
        let mut b = Bands::new().unwrap();
        b.skip().unwrap();
        assert_eq!(b.frames, 1);
        assert!(b.history.iter().all(Vec::is_empty));
        assert!(b.sustained.iter().chain(&b.transient).all(|v| v == &[0.]));
    }
    #[test]
    fn source_sustained_remainder_is_not_clamped_to_zero() {
        let mut b = Bands::new().unwrap();
        let mut s = vec![100.; 1025];
        s[0] = 0.;
        s[17..24].fill(0.);
        s[20] = 1.;
        b.push(&s).unwrap();
        b.push(&s).unwrap();
        assert!(b.sustained[5][1] < 0.);
        assert!(b.transient[5][1] > 0.);
    }
    #[test]
    fn invalid_spectrum_leaves_state_unchanged() {
        let mut b = Bands::new().unwrap();
        assert!(b.push(&vec![-0.; 1025]).is_err());
        assert_eq!(b.frames, 0);
    }
}
