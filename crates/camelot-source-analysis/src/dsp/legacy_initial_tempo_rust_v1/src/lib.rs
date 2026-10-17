//! One-shot default initial-tempo reconstruction. Numeric PCM only.
use crate::dsp::legacy_real_fft_rust_v2 as fft;
use crate::dsp::legacy_vector_rust_v1 as vector;
use crate::dsp::legacy_window_rust_v1::{Window, coefficients};

#[derive(Debug, PartialEq)]
pub struct Geometry {
    pub frames: usize,
    pub ac_len: usize,
    pub bounds: [(usize, usize); 3],
}

pub fn geometry(samples: usize) -> Result<Geometry, String> {
    if !(4096..=24 * 44100).contains(&samples) {
        return Err("sample count requires4096..1058400".into());
    }
    let frames = (((samples - 2048) as f32) / 256f32) as usize;
    let hz = 44100f32 / 2048f32;
    let bin = |freq: f32| (f64::from(freq / hz) + 0.5) as usize;
    Ok(Geometry {
        frames,
        ac_len: 2 * frames.next_power_of_two(),
        bounds: [
            (0, bin(200.) + 1),
            (bin(800.) - 1, bin(2200.) + 1),
            (bin(4000.) - 1, bin(15000.) + 1),
        ],
    })
}

pub fn normalized_bpm(mut raw: f32) -> f32 {
    if raw <= 0. {
        return 0.;
    }
    // Infinite positive input is outside the caller's proven finite raw domain.
    if !raw.is_finite() {
        return raw;
    }
    while raw > 180. {
        raw *= 0.5;
    }
    while raw < 50. {
        raw += raw;
    }
    raw
}

#[derive(Debug)]
pub struct Analysis {
    pub samples: usize,
    pub raw_bpm: f32,
    pub bpm: f32,
    pub selected: usize,
    pub start: i32,
    pub intersection: i32,
    pub interval: i32,
    pub qualities: [f32; 3],
    pub geometry: Geometry,
    pub window: [f32; 2048],
    pub parent_fft: Vec<f32>,
    pub last_spectrum: Vec<f32>,
    pub bands: Vec<Band>,
}

#[derive(Debug)]
pub struct Band {
    pub lower: usize,
    pub upper: usize,
    pub curve: Vec<f32>,
    pub histories: [Vec<f32>; 4],
    pub contributions: Vec<f32>,
    pub last_max: f32,
    pub last_min: f32,
    pub ac: Vec<f32>,
    pub packed_power: Vec<f32>,
    pub positions: Vec<i32>,
    pub harmonics: Vec<i32>,
    pub heights: Vec<f32>,
    pub energy: Vec<f32>,
    pub folded: Vec<f32>,
    pub peak_count: usize,
    pub period: i32,
    pub best_lag: i32,
    pub phase: i32,
    pub quality: f32,
}

impl Band {
    fn new(lower: usize, upper: usize, m: usize, l: usize) -> Self {
        Self {
            lower,
            upper,
            curve: vec![0.; m],
            histories: std::array::from_fn(|_| vec![0.; upper - lower]),
            contributions: vec![0.; upper - lower - 2],
            last_max: 0.,
            last_min: 0.,
            ac: vec![0.; l],
            packed_power: vec![0.; l],
            positions: vec![0; m],
            harmonics: vec![0; m],
            heights: vec![0.; m],
            energy: vec![0.; m],
            folded: vec![0.; m],
            peak_count: 0,
            period: 0,
            best_lag: -1,
            phase: 0,
            quality: 0.,
        }
    }

    fn frame(&mut self, spectrum: &[f32], index: usize) -> f32 {
        self.histories.rotate_right(1);
        self.histories[0].copy_from_slice(&spectrum[self.lower..self.upper]);
        if index <= 2 {
            return 0.;
        }
        let [now, prev, old, older] = &self.histories;
        for k in 1..now.len() - 1 {
            let a = if old[k] <= older[k] { older[k] } else { old[k] };
            let b = if old[k + 1] <= old[k - 1] {
                old[k - 1]
            } else {
                old[k + 1]
            };
            self.last_max = if a <= b { b } else { a };
            let a = if now[k - 1] <= now[k] {
                now[k - 1]
            } else {
                now[k]
            };
            self.last_min = if now[k + 1] <= a { now[k + 1] } else { a };
            self.contributions[k - 1] = 0.;
            if self.last_max < prev[k] && self.last_max < self.last_min {
                let high = if prev[k] <= now[k] { now[k] } else { prev[k] };
                self.contributions[k - 1] = high - self.last_max;
            }
        }
        let mut sum = 0.;
        for &v in &self.contributions {
            sum += v;
        }
        sum
    }

    fn save_peaks(&mut self, use_fold: bool, span: i32) {
        let found = peaks(if use_fold { &self.folded } else { &self.ac }, span);
        self.peak_count = found.len();
        for (k, (pos, height)) in found.into_iter().enumerate() {
            self.positions[k] = pos;
            self.heights[k] = height;
        }
    }

    fn relative(&mut self) -> Result<f32, String> {
        let mut max = 0.;
        for k in 0..self.peak_count {
            if self.heights[k] > max {
                max = self.heights[k];
                self.best_lag = self.positions[k];
            }
        }
        let lag = self.best_lag;
        if lag != -1 && !(2..self.curve.len() as i32).contains(&lag) {
            return Err("selected lag outside initialized profile".into());
        }
        let p = ((lag as f32) / 25f32).ceil() as i32;
        let nharm = (self.curve.len() as i32 - 1) / lag;
        let mut indices = Vec::new();
        for divisor in (2..=p).rev() {
            indices.push(subharmonic_index(lag, divisor));
        }
        for j in 1..=nharm {
            indices.push(lag * j);
        }
        if indices.len() > self.harmonics.len() {
            return Err("harmonic storage bounds".into());
        }
        let mut sum = 0.;
        for (k, &idx) in indices.iter().enumerate() {
            let index = usize::try_from(idx).map_err(|_| "negative harmonic index")?;
            if index == 0 || index + 1 >= self.ac.len() {
                return Err("AC neighbor bounds".into());
            }
            self.harmonics[k] = idx;
            let left = self.ac[index - 1];
            let right = self.ac[index + 1];
            let edge = if right > left { right } else { left };
            let high = if edge > self.ac[index] {
                edge
            } else {
                self.ac[index]
            };
            sum += high;
        }
        Ok(sum / ((nharm + p - 1) as f32))
    }

    fn process_curve(&mut self) -> Result<(), String> {
        let mut padded = vec![0.; self.ac.len()];
        padded[..self.curve.len()].copy_from_slice(&self.curve);
        let spectrum = fft::forward(&padded)?;
        let conjugate = vector::conjugate(&spectrum)?;
        self.packed_power = vector::packed_multiply(&conjugate, &spectrum)?;
        self.ac = fft::inverse(&self.packed_power)?;
        for &v in &self.curve {
            let square = v * v;
            self.energy[0] += square;
        }
        self.save_peaks(false, self.curve.len() as i32);
        let first = self.relative()?;
        self.period = self.best_lag;
        let mut h = 0usize;
        for k in 0..self.peak_count {
            let q = self.positions[k];
            let harmonic = *self.harmonics.get(h).ok_or("suppression index bounds")?;
            if harmonic < q {
                h += 1;
            }
            let harmonic = *self.harmonics.get(h).ok_or("suppression neighbor bounds")?;
            if (harmonic - 1..=harmonic + 1).contains(&q) {
                self.heights[k] = 0.;
                h += 1;
            }
        }
        let second = self.relative()?;
        let tiny = f64::from_bits(0x3841039d428a8b8f);
        self.quality = ((f64::from(first) + tiny) / (f64::from(second) + tiny)) as f32;
        if self.quality.is_nan() {
            return Err("NaN quality outside bounded profile".into());
        }
        Ok(())
    }

    fn start(&mut self) -> Result<i32, String> {
        let p = self.period;
        if p == 0 {
            return Err("zero period outside initial profile".into());
        }
        let m = self.curve.len() as i32;
        let cycles = m / p;
        if cycles >= 0 {
            for u in 0..m {
                for k in 0..=cycles {
                    self.folded[u as usize] += self.curve[((u + k * p) % m) as usize];
                }
            }
        }
        self.save_peaks(true, p);
        if p > m {
            return Err("phase scan beyond initialized peaks".into());
        }
        let mut max = 0.;
        for k in 0..p.max(0) as usize {
            if self.heights[k] > max {
                max = self.heights[k];
                self.phase = self.positions[k];
            }
        }
        Ok(self.phase)
    }

    fn intersection(&mut self, samples: i32) -> i32 {
        self.period *= 256;
        self.phase *= 256;
        let t = (samples - self.phase) / self.period;
        let a = (self.period - samples) + t * self.period;
        let b = (self.period - samples) + (t - 1) * self.period;
        (if a < 1 { a } else { b }) + samples
    }
}

pub fn analyze(pcm: &[f32]) -> Result<Analysis, String> {
    let geometry = geometry(pcm.len())?;
    if pcm.iter().any(|v| !v.is_finite() || v.abs() > 2.) {
        return Err("requires finite numeric PCM abs<=2".into());
    }
    let mut bands: Vec<_> = geometry
        .bounds
        .iter()
        .map(|&(lo, hi)| Band::new(lo, hi, geometry.frames, geometry.ac_len))
        .collect();
    let window = coefficients(Window::Daloopa7);
    let mut parent_fft = Vec::new();
    let mut last_spectrum = Vec::new();
    for frame in 0..geometry.frames {
        let offset = frame * 256;
        let weighted: Vec<_> = pcm[offset..offset + 2048]
            .iter()
            .zip(window)
            .map(|(&v, w)| v * w)
            .collect();
        parent_fft = fft::forward(&weighted)?;
        let mag = vector::magnitude(&parent_fft)?;
        last_spectrum = vector::lower_threshold(&mag, f32::from_bits(0x02081cea), 0.)?;
        if frame != 0 {
            for band in &mut bands {
                band.curve[frame - 1] = band.frame(&last_spectrum, frame);
            }
        }
    }
    for band in &mut bands {
        band.process_curve()?;
    }
    let qualities = std::array::from_fn(|i| bands[i].quality);
    let mut selected = 0;
    let mut quality = 0.;
    for (i, &q) in qualities.iter().enumerate() {
        if quality <= q {
            quality = q;
            selected = i;
        }
    }
    let start = bands[selected].start()? * 256;
    let intersection = bands[selected].intersection(pcm.len() as i32);
    let interval = bands[selected].period;
    // onsGetBeatInterval 0x2e134: CMP period,0; CSINV period,wzr,GE.
    // Stored negative sample periods remain negative, but getter returns -1.
    let beat_interval = if interval < 0 { -1 } else { interval };
    let raw_bpm = (((1u32 << selected) as f32 * 120.) * 44100.) / beat_interval as f32;
    if !raw_bpm.is_finite() {
        return Err("nonfinite raw BPM".into());
    }
    let bpm = normalized_bpm(raw_bpm);
    Ok(Analysis {
        samples: pcm.len(),
        raw_bpm,
        bpm,
        selected,
        start,
        intersection,
        interval,
        qualities,
        geometry,
        window,
        parent_fft,
        last_spectrum,
        bands,
    })
}

impl Analysis {
    /// Initialized semantic state only; no native pointers, padding or dead tails.
    pub fn state_bytes(&self) -> Vec<u8> {
        fn word(out: &mut Vec<u8>, value: u32) {
            out.extend_from_slice(&value.to_le_bytes());
        }
        fn floats(out: &mut Vec<u8>, values: &[f32]) {
            for v in values {
                word(out, v.to_bits());
            }
        }
        let mut out = b"ITM1".to_vec();
        for v in [
            self.samples as i32,
            self.geometry.frames as i32,
            self.geometry.ac_len as i32,
            self.selected as i32,
            self.start,
            self.intersection,
            self.interval,
        ] {
            word(&mut out, v as u32);
        }
        floats(&mut out, &[self.raw_bpm, self.bpm, 44100f32 / 2048f32]);
        floats(&mut out, &self.qualities);
        floats(&mut out, &self.window);
        floats(&mut out, &self.parent_fft);
        floats(&mut out, &self.last_spectrum);
        for b in &self.bands {
            for v in [
                b.lower as i32,
                b.upper as i32,
                (b.upper - b.lower) as i32,
                b.curve.len() as i32,
                b.ac.len() as i32,
                b.peak_count as i32,
                b.period,
                b.best_lag,
                b.phase,
            ] {
                word(&mut out, v as u32);
            }
            floats(&mut out, &[b.last_max, b.last_min, b.quality]);
            floats(&mut out, &b.curve);
            for hist in &b.histories {
                floats(&mut out, hist);
            }
            floats(&mut out, &b.contributions);
            floats(&mut out, &b.ac);
            floats(&mut out, &b.packed_power);
            for &v in &b.positions {
                word(&mut out, v as u32);
            }
            for &v in &b.harmonics {
                word(&mut out, v as u32);
            }
            floats(&mut out, &b.heights);
            floats(&mut out, &b.energy);
            floats(&mut out, &b.folded);
        }
        out
    }
}

fn peaks(values: &[f32], span: i32) -> Vec<(i32, f32)> {
    if span < 4 {
        return Vec::new();
    }
    let n = span as usize;
    let mut start = n - 2;
    if n >= 5 {
        for k in 2..=n - 3 {
            if values[k] < values[k - 1] && values[k] < values[k + 1] {
                start = k;
                break;
            }
        }
    } else {
        start = 2;
    }
    (start..=n - 2)
        .filter_map(|k| {
            if values[k - 1] < values[k] && values[k + 1] < values[k] {
                Some((k as i32, values[k]))
            } else {
                None
            }
        })
        .collect()
}

fn subharmonic_index(lag: i32, divisor: i32) -> i32 {
    ((1f32 / divisor as f32) * lag as f32) as i32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn construction_matches_bands_and_ac_sizes() {
        let first = geometry(4096).unwrap();
        assert_eq!(first.frames, 8);
        assert_eq!(first.ac_len, 16);
        assert_eq!(first.bounds, [(0, 10), (36, 103), (185, 698)]);
        let last = geometry(24 * 44100).unwrap();
        assert_eq!((last.frames, last.ac_len), (4126, 16384));
        assert_eq!(geometry(4351).unwrap().frames, 8);
        assert_eq!(geometry(4352).unwrap().frames, 9);
    }

    #[test]
    fn silence_retains_negative_raw_sentinel_but_getter_returns_zero() {
        let a = analyze(&vec![0.; 4096]).unwrap();
        assert_eq!(a.raw_bpm, -21168000.);
        assert_eq!(a.bpm.to_bits(), 0);
        assert_eq!(a.selected, 2);
        assert_eq!(a.start, 0);
        assert_eq!(a.interval, -256);
        assert_eq!(a.intersection, 3840);
        assert_eq!(a.qualities, [1.; 3]);
    }

    #[test]
    fn getter_uses_inclusive_bounds_and_repeated_halving_then_doubling() {
        for (raw, expected) in [
            (-12f32, 0f32),
            (-0., 0.),
            (0., 0.),
            (360., 180.),
            (181., 90.5),
            (45., 90.),
            (49.5, 99.),
            (50., 50.),
            (180., 180.),
        ] {
            assert_eq!(normalized_bpm(raw).to_bits(), expected.to_bits());
        }
    }

    #[test]
    fn peaks_start_at_first_strict_minimum_and_ignore_plateaus() {
        let a = [9., 8., 1., 5., 3., 6., 2., 8., 0.];
        assert_eq!(peaks(&a, 9), vec![(3, 5.), (5, 6.), (7, 8.)]);
        assert!(peaks(&[0., 1., 2., 2., 1., 0.], 6).is_empty());
        // No early strict minimum: fallback still checks N-2.
        assert_eq!(peaks(&[0., 1., 2., 3., 4., 0.], 6), vec![(4, 4.)]);
        assert_eq!(peaks(&[0., 0., 2., 0.], 4), vec![(2, 2.)]);
        assert!(peaks(&[0., 4., 0.], 3).is_empty());
    }

    #[test]
    fn subharmonic_indices_require_reciprocal_before_multiply() {
        assert_eq!(subharmonic_index(1066, 41), 25);
        assert_eq!((1066f32 / 41f32) as i32, 26);
    }

    #[test]
    fn rejects_input_outside_the_declared_pcm_domain() {
        for n in [0, 4095, 24 * 44100 + 1] {
            assert!(analyze(&vec![0.; n]).is_err());
        }
        for value in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY, 2.0001, -2.0001] {
            let mut input = vec![0.; 4096];
            input[30] = value;
            assert!(analyze(&input).is_err());
        }
    }

    #[test]
    fn numeric_pcm_is_deterministic_without_track_metadata() {
        let pcm: Vec<_> = (0..8192)
            .map(|i| if i % 1024 < 16 { 1. } else { 0. })
            .collect();
        let a = analyze(&pcm).unwrap();
        let b = analyze(&pcm).unwrap();
        assert!(a.raw_bpm.is_finite());
        assert!(a.bpm == 0. || (50. ..=180.).contains(&a.bpm));
        assert_eq!(a.raw_bpm.to_bits(), b.raw_bpm.to_bits());
        assert_eq!(
            (a.selected, a.start, a.intersection, a.interval),
            (b.selected, b.start, b.intersection, b.interval)
        );
    }
}
