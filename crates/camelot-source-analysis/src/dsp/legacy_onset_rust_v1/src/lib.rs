pub use crate::dsp::legacy_marks_rust_v1::Mark;
pub mod math;
#[derive(Clone, Debug)]
pub struct Configuration {
    pub fft: usize,
    pub hop: usize,
    pub history: usize,
    pub delay: usize,
    pub prefix: usize,
    pub exclusion: usize,
    pub merge_samples: i32,
    pub fir: Vec<f32>,
}

pub struct OnsetStream {
    channels: usize,
    config: Configuration,
    window: [f32; 2048],
    suppression: Vec<f32>,
    input: Vec<f32>,
    oldest: Vec<f32>,
    previous: Vec<f32>,
    work: Vec<f32>,
    fir_history: Vec<f32>,
    history: Vec<f32>,
    raw_amplitude: Vec<f32>,
    raw_sample: Vec<i32>,
    frames: usize,
    received: usize,
    stats_tick: i32,
    baseline: f32,
    minimum: f32,
    positive_sum: f32,
    positive_count: f32,
    negative_sum: f32,
    negative_count: f32,
    previous_amplitude: f32,
    candidate_sample: i32,
    candidate_amplitude: f32,
    last_strength: f32,
    last_peak_hop: i32,
    last_peak_center: f32,
    last_mean: f32,
    marks: Vec<Mark>,
    finished: bool,
    failed: bool,
}

impl OnsetStream {
    pub fn new(channels: usize) -> Result<Self, String> {
        if !(1..=8).contains(&channels) {
            return Err("channels must be1..8".into());
        }
        let fs = 44100f32;
        let hop = 256usize;
        let history = ((fs * 384.) / (hop as f32 * 1000.) + 1.) as usize;
        let delay = history >> 1;
        let prefix = ((fs * 50.) / 1000.) as usize / hop;
        let exclusion = ((fs * 120.) / (hop as f32 * 1000.)) as usize;
        let length = ((fs * 96.) / (hop as f32 * 1000.)) as usize;
        let mut fir = Vec::with_capacity(length);
        for k in 0..length {
            let a = ((k + 1) as f32 * std::f32::consts::PI) / (length + 1) as f32;
            let s = math::positive_sine(a)?;
            fir.push(s * s);
        }
        let mut suppression = vec![1.; exclusion];
        for (k, v) in suppression.iter_mut().enumerate().skip(exclusion >> 1) {
            let a = ((k + 1) as f32 * std::f32::consts::PI) / (exclusion + 1) as f32;
            let s = math::positive_sine(a)?;
            *v = s * s;
        }
        let config = Configuration {
            fft: 2048,
            hop,
            history,
            delay,
            prefix,
            exclusion,
            merge_samples: ((fs * 50.) / 1000.) as i32,
            fir,
        };
        Ok(Self {
            channels,
            config,
            window: crate::dsp::legacy_window_rust_v1::coefficients(
                crate::dsp::legacy_window_rust_v1::Window::Onset4,
            ),
            suppression,
            input: Vec::new(),
            oldest: vec![0.; 1024],
            previous: vec![0.; 1024],
            work: vec![0.; 1024],
            fir_history: vec![0.; length],
            history: vec![0.; history],
            raw_amplitude: vec![0.; delay + prefix],
            raw_sample: vec![0; delay + prefix],
            frames: 0,
            received: 0,
            stats_tick: 0,
            baseline: 0.,
            minimum: 0.,
            positive_sum: 0.,
            positive_count: 0.,
            negative_sum: 0.,
            negative_count: 0.,
            previous_amplitude: 1.,
            candidate_sample: 0,
            candidate_amplitude: 0.,
            last_strength: 0.,
            last_peak_hop: 0,
            last_peak_center: 0.,
            last_mean: 0.,
            marks: Vec::new(),
            finished: false,
            failed: false,
        })
    }
    pub fn config(&self) -> Configuration {
        self.config.clone()
    }
    pub fn frame_count(&self) -> usize {
        self.frames
    }
    pub fn buffered(&self) -> usize {
        self.input.len()
    }
    pub fn raw_history(&self) -> &[f32] {
        &self.raw_amplitude
    }
    pub fn sample_history(&self) -> &[i32] {
        &self.raw_sample
    }
    pub fn last_mean(&self) -> f32 {
        self.last_mean
    }
    pub fn latest_spectrum(&self) -> &[f32] {
        &self.previous
    }
    pub fn marks(&self) -> &[Mark] {
        &self.marks
    }
    /// Private experiment projection of initialized source state, not a native ABI.
    pub fn state_bytes(&self) -> Vec<u8> {
        let mut out = Vec::new();
        for x in [
            self.frames as u32,
            self.input.len() as u32,
            self.stats_tick as u32,
            (self.frames * 256) as u32,
            self.marks.len() as u32,
        ] {
            out.extend(x.to_le_bytes());
        }
        for x in [
            self.baseline,
            self.minimum,
            self.last_strength,
            self.last_peak_center,
            self.previous_amplitude,
            self.candidate_amplitude,
            self.positive_sum,
            self.positive_count,
            self.negative_sum,
            self.negative_count,
        ] {
            out.extend(x.to_le_bytes());
        }
        out.extend(self.candidate_sample.to_le_bytes());
        out.extend(self.last_peak_hop.to_le_bytes());
        for part in [
            &self.config.fir,
            &self.suppression,
            &self.fir_history,
            &self.history,
            &self.raw_amplitude,
        ] {
            for x in part {
                out.extend(x.to_le_bytes());
            }
        }
        for x in &self.raw_sample {
            out.extend(x.to_le_bytes());
        }
        for part in [&self.oldest, &self.previous, &self.work, &self.input] {
            for x in part {
                out.extend(x.to_le_bytes());
            }
        }
        for mark in &self.marks {
            out.extend(mark.to_le_bytes());
        }
        out
    }
    fn active(&self) -> Result<(), String> {
        if self.finished || self.failed {
            Err("stream already finished or failed".into())
        } else {
            Ok(())
        }
    }
    pub fn process(&mut self, channels: &[&[f32]]) -> Result<(), String> {
        self.active()?;
        if channels.len() != self.channels {
            return Err("channel count mismatch".into());
        }
        let n = channels[0].len();
        if n > 32768
            || self
                .received
                .checked_add(n)
                .is_none_or(|v| v > 1_000_000_000)
        {
            return Err("outside bounded feed/sample domain".into());
        }
        if channels
            .iter()
            .any(|c| c.len() != n || c.iter().any(|x| !x.is_finite() || x.abs() > 8.))
        {
            return Err("equal finiteabs<=8 PCM planes required".into());
        }
        let divisor = self.channels as f32;
        for j in 0..n {
            let mut x = channels[0][j] / divisor;
            for c in &channels[1..] {
                x += c[j] / divisor;
            }
            self.input.push(x);
        }
        self.received += n;
        match self.run_frames() {
            Ok(()) => Ok(()),
            Err(e) => {
                self.failed = true;
                Err(e)
            }
        }
    }
    fn run_frames(&mut self) -> Result<(), String> {
        let mut local_index = 0usize;
        while self.input.len() >= 2048 {
            if self.frames >= 3_906_200 {
                return Err("source sample coordinate bound".into());
            }
            self.frames += 1;
            let mut centered = self.input[..2048].to_vec();
            let mut amplitude = 0f32;
            let mut sum = 0f32;
            for (j, &x) in centered.iter().enumerate() {
                if j < 256 && amplitude < x.abs() {
                    amplitude = x.abs();
                    local_index = j;
                }
                sum += x;
            }
            let mean = sum / 2048.;
            self.last_mean = if mean >= 1e-9 { mean } else { 0. };
            for x in &mut centered {
                *x -= self.last_mean;
            }
            let len = self.raw_amplitude.len();
            self.raw_amplitude.copy_within(1.., 0);
            self.raw_amplitude[len - 1] = amplitude;
            self.raw_sample.copy_within(1.., 0);
            self.raw_sample[len - 1] = ((self.frames - 1) * 256 + local_index) as i32;
            for (x, w) in centered.iter_mut().zip(&self.window) {
                *x *= *w;
            }
            let packed = crate::dsp::legacy_real_fft_rust_v2::forward(&centered)?;
            self.work = crate::dsp::legacy_vector_rust_v1::magnitude(&packed)?;
            if self.stats_tick > 264600 {
                if self.positive_count != 0. {
                    self.positive_sum /= self.positive_count;
                    self.positive_count = 1.;
                }
                if self.negative_count != 0. {
                    self.negative_sum /= self.negative_count;
                    self.negative_count = 1.;
                }
                self.stats_tick = 0;
            }
            let distance = goto_distance(&self.oldest, &self.previous, &self.work)?;
            let last = self.fir_history.len() - 1;
            self.fir_history.copy_within(..last, 1);
            self.fir_history[0] = distance;
            let mut filtered = self.fir_history[0] * self.config.fir[0];
            for k in 1..self.fir_history.len() {
                filtered += self.fir_history[k] * self.config.fir[k];
            }
            let reciprocal = 1f32 / self.config.history as f32;
            self.baseline -= reciprocal * self.history[0];
            self.baseline += filtered * reciprocal;
            self.history.copy_within(1.., 0);
            let end = self.history.len() - 1;
            self.history[end] = filtered;
            let strength = self.peak_pick()?;
            if strength != 0. && self.frames > self.config.delay && self.candidate_amplitude > 0. {
                if self.marks.len() >= 1_000_000 {
                    return Err("mark capacity bound".into());
                }
                self.marks.push(Mark {
                    sample: self.candidate_sample,
                    transition_energy: strength,
                    local_bpm: 0.,
                    confidence: 0.,
                });
                self.previous_amplitude = self.candidate_amplitude;
            }
            std::mem::swap(&mut self.oldest, &mut self.previous);
            std::mem::swap(&mut self.previous, &mut self.work);
            self.input.drain(..256);
            self.stats_tick += 256;
        }
        Ok(())
    }
    fn peak_pick(&mut self) -> Result<f32, String> {
        let k = self.config.delay;
        let center = self.history[k];
        if center < self.history[k - 1] && center < self.history[k + 1] {
            self.minimum = center;
        }
        if center <= self.history[k - 1] || center <= self.history[k + 1] {
            return Ok(0.);
        }
        let threshold = f32::from_bits(0x3fa3d70a) * self.baseline;
        let negative_mean = if self.negative_count != 0. {
            self.negative_sum / self.negative_count
        } else {
            0.
        };
        let relative = if threshold != 0. {
            center / threshold
        } else {
            0.
        };
        let positive_mean = if self.positive_count != 0. {
            self.positive_sum / self.positive_count
        } else {
            0.
        };
        let contrast = center - self.minimum;
        self.candidate_amplitude = 0.;
        self.candidate_sample = self.raw_sample[self.config.prefix];
        let mut amplitude = self.raw_amplitude[0].max(0.);
        self.candidate_amplitude = amplitude;
        let mut rise = 0f32;
        for j in 1..self.config.prefix {
            if amplitude < self.raw_amplitude[j] {
                amplitude = self.raw_amplitude[j];
                self.candidate_amplitude = amplitude;
            }
            let delta = self.raw_amplitude[j] - self.raw_amplitude[j - 1];
            if rise < delta {
                rise = delta;
                self.candidate_sample = self.raw_sample[j];
            }
        }
        let amplitude_ratio = (amplitude / self.previous_amplitude)
            * (self.last_peak_center / (center + f32::from_bits(0x2b8cbccc)));
        if (threshold < center && negative_mean < contrast && amplitude_ratio < 4.)
            || (f32::from_bits(0x3f4ccccd) < relative
                && positive_mean * f32::from_bits(0x3f333333) < contrast)
        {
            let strength = (contrast * f32::from_bits(0x3f4ccccd) + center) * 0.5;
            let hop = self.candidate_sample / 256;
            let delta = hop - self.last_peak_hop;
            if delta <= 0 && (self.last_strength != 0. || strength < 0.) {
                return Err(
                    "ambiguous nonpositive peak coordinate outside prototype domain".into(),
                );
            }
            if delta > 0
                && delta <= self.config.exclusion as i32
                && strength < self.suppression[delta as usize - 1] * self.last_strength
            {
                return Ok(0.);
            }
            self.last_strength = strength;
            self.last_peak_hop = hop;
            self.last_peak_center = center;
            if relative > 1. {
                self.positive_sum += contrast;
                self.positive_count += 1.;
            }
            Ok(strength)
        } else {
            self.negative_sum += contrast;
            self.negative_count += 1.;
            Ok(0.)
        }
    }
    pub fn finish(&mut self) -> Result<Vec<Mark>, String> {
        self.active()?;
        let target = self.frames + self.config.delay;
        while self.frames < target {
            self.input.resize(self.input.len() + 2304, 0.);
            if let Err(e) = self.run_frames() {
                self.failed = true;
                return Err(e);
            }
        }
        match finish_clusters(&self.marks) {
            Ok(out) => {
                self.marks = out;
                self.finished = true;
                Ok(self.marks.clone())
            }
            Err(e) => {
                self.failed = true;
                Err(e)
            }
        }
    }
}

pub fn goto_distance(a: &[f32], b: &[f32], c: &[f32]) -> Result<f32, String> {
    let n = a.len();
    if n < 2
        || b.len() != n
        || c.len() != n
        || a.iter()
            .chain(b)
            .chain(c)
            .any(|x| !x.is_finite() || *x < 0.)
    {
        return Err("equal nonnegative finite spectra oflength>=2 required".into());
    }
    let mut total = 0f32;
    for k in 0..n {
        let mut old = a[k];
        if k > 0 && old < a[k - 1] {
            old = a[k - 1];
        }
        if k + 1 < n && old < a[k + 1] {
            old = a[k + 1];
        }
        let minimum = if c[k] <= b[k] { c[k] } else { b[k] };
        let maximum = if b[k] <= c[k] { c[k] } else { b[k] };
        if old < minimum {
            total += maximum - old;
        }
    }
    if !total.is_finite() {
        return Err("nonfinite distance".into());
    }
    Ok(total)
}

pub fn finish_clusters(marks: &[Mark]) -> Result<Vec<Mark>, String> {
    if marks.len() > 1_000_000
        || marks.iter().any(|m| {
            m.sample < 0
                || m.sample > 1_000_000_000
                || !m.transition_energy.is_finite()
                || m.transition_energy <= 0.
        })
        || marks.windows(2).any(|w| w[0].sample > w[1].sample)
    {
        return Err("sorted finite positive raw marks required".into());
    }
    let mut values: Vec<_> = marks.iter().map(|m| m.transition_energy).collect();
    let mut remaining = marks.len() as isize;
    let mut maximum = 0f32;
    let mut clusters = 0;
    while remaining > 0 {
        let mut best = 0f32;
        let mut selected = 0;
        for (j, &v) in values.iter().enumerate() {
            if best < v {
                best = v;
                selected = j;
            }
        }
        if best <= 0. {
            return Err("source cluster progress domain".into());
        }
        let mut sum = best;
        for j in (0..selected).rev() {
            if marks[selected].sample - marks[j].sample >= 2205 {
                break;
            }
            sum += values[j];
            values[j] = 0.;
            remaining -= 1;
        }
        for j in selected + 1..marks.len() {
            if marks[j].sample - marks[selected].sample >= 2205 {
                break;
            }
            sum += values[j];
            values[j] = 0.;
            remaining -= 1;
        }
        if maximum < sum {
            maximum = sum;
        }
        values[selected] = -sum;
        clusters += 1;
        remaining -= 1;
    }
    let mut out = Vec::with_capacity(clusters);
    for (m, &v) in marks.iter().zip(&values) {
        if v < 0. {
            let q = v / maximum;
            let argument = q * 3.;
            let energy = 1f32 - math::negative_exp(argument)?;
            out.push(Mark {
                sample: m.sample,
                transition_energy: energy,
                local_bpm: 0.,
                confidence: 0.,
            });
        }
    }
    if out.len() != clusters {
        return Err("source cluster count outside bounded profile".into());
    }
    Ok(out)
}

#[cfg(test)]
mod admission_tests {
    use super::*;
    #[test]
    fn ambiguous_zero_previous_strength_and_negative_proposal_rejects() {
        // A scalar state contract witness, not a claim of reachable PCM mismatch.
        let mut stream = OnsetStream::new(1).unwrap();
        let k = stream.config.delay;
        stream.history[k - 1] = 0.5;
        stream.history[k] = 1.;
        stream.history[k + 1] = 0.5;
        stream.minimum = 10.;
        stream.baseline = 0.1;
        stream.negative_sum = -10.;
        stream.negative_count = 1.;
        stream.raw_sample.fill(10000);
        stream.raw_amplitude.fill(1.);
        stream.last_peak_hop = 39;
        stream.last_strength = 0.;
        assert!(stream.peak_pick().is_err());
    }
}
