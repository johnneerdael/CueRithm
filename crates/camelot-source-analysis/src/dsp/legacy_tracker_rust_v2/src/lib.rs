use crate::dsp::legacy_marks_rust_v1::{Mark, MarkBuffer};
#[derive(Clone, Copy, Debug, Default)]
pub struct Grid {
    pub closest: i32,
    pub anchor: f32,
    pub period: f32,
    pub prediction: f32,
    pub multiplier: f32,
    pub reciprocal: f32,
    pub bpm: f32,
    pub smooth: f32,
    pub instant: f32,
}
#[derive(Clone, Copy, Debug)]
pub struct Winner {
    pub score: f32,
    pub phase: f32,
    pub absolute: f32,
    pub grid: usize,
    pub closest: i32,
}
// Initialized caller words shared only by the source-bound Process pass pair.
type PassScratch = Option<(f32, f32, i32)>;
pub struct Tracker {
    pub grid: [Grid; 7],
    pub exponent: f32,
    pub initial_bpm: f32,
    pub fs: f32,
    pub gain: f32,
    pub energy_ratio: f32,
    pub bpm_adapt: bool,
    pub initialized: bool,
    pub explicit_start: Option<bool>,
    pub steps: i32,
    pub unit: usize,
    pub winning: usize,
    pub one_half: usize,
    pub last_closest: i32,
    pub opaque140: Option<u32>,
    pub output: Vec<Mark>,
    onsets: MarkBuffer,
    poisoned: bool,
}
impl Tracker {
    /// Typed initialized-state projection; masks distinguish native unwritten fields.
    pub fn state_bytes(&self, status: i32) -> Vec<u8> {
        fn word(out: &mut Vec<u8>, v: u32) {
            out.extend_from_slice(&v.to_le_bytes());
        }
        let mut out = Vec::new();
        word(&mut out, (320 + 16 * self.output.len()) as u32);
        let mask =
            u32::from(self.explicit_start.is_some()) | (2 * u32::from(self.opaque140.is_some()));
        for v in [
            status,
            self.steps,
            self.unit as i32,
            self.winning as i32,
            self.one_half as i32,
            self.last_closest,
            i32::from(self.initialized),
            i32::from(self.bpm_adapt),
            mask as i32,
            i32::from(self.explicit_start.unwrap_or(false)),
            self.opaque140.unwrap_or(0) as i32,
            self.output.len() as i32,
        ] {
            word(&mut out, v as u32);
        }
        for v in [
            self.fs,
            self.initial_bpm,
            self.exponent,
            self.gain,
            self.energy_ratio,
        ] {
            word(&mut out, v.to_bits());
        }
        for g in &self.grid {
            word(&mut out, g.closest as u32);
            for v in [
                g.anchor,
                g.period,
                g.prediction,
                g.multiplier,
                g.reciprocal,
                g.bpm,
                g.smooth,
                g.instant,
            ] {
                word(&mut out, v.to_bits());
            }
        }
        for mark in &self.output {
            out.extend_from_slice(&mark.to_le_bytes());
        }
        out
    }
}
impl Tracker {
    pub fn new(marks: &[Mark], bpm: f32) -> Result<Self, String> {
        if !bpm.is_finite() || (bpm != 0. && !(30.0..=400.0).contains(&bpm)) || marks.len() > 100000
        {
            return Err("initial BPM/count outside default prototype domain".into());
        }
        for (i, m) in marks.iter().enumerate() {
            if !(0..=500000000).contains(&m.sample)
                || (i > 0 && m.sample <= marks[i - 1].sample)
                || !m.transition_energy.is_finite()
                || !(0.0..=1.0).contains(&m.transition_energy)
                || !m.local_bpm.is_finite()
                || !m.confidence.is_finite()
            {
                return Err("requires sorted unique bounded numeric onsets".into());
            }
        }
        let mut onsets = MarkBuffer::new();
        onsets.put_buffers(marks).map_err(|e| e.to_string())?;
        Ok(Self {
            grid: [Grid::default(); 7],
            exponent: 1.,
            initial_bpm: bpm,
            fs: 44100.,
            gain: 0.1,
            energy_ratio: 0.,
            bpm_adapt: true,
            initialized: false,
            explicit_start: None,
            steps: 0,
            unit: 0,
            winning: 0,
            one_half: 0,
            last_closest: 0,
            opaque140: None,
            output: Vec::new(),
            onsets,
            poisoned: false,
        })
    }
    fn reset_grid(&mut self, saved: [Grid; 7], bpm: f32, state: f32) -> Result<(), String> {
        self.live()?;
        if !bpm.is_finite()
            || (bpm != 0. && !(30.0..=400.).contains(&bpm))
            || !state.is_finite()
            || !(0.5..=50.).contains(&state)
        {
            return Err("reset parameter domain".into());
        }
        self.grid = saved;
        self.exponent = state;
        if bpm != 0. {
            let base = (self.fs * 30.) / bpm;
            for g in &mut self.grid {
                g.period = g.multiplier * base;
                g.bpm = bpm;
            }
        }
        let mut max = 0.;
        for (i, g) in self.grid.iter().enumerate() {
            if g.multiplier > max {
                max = g.multiplier;
                self.winning = i;
            }
            if g.multiplier == 1. {
                self.unit = i;
            }
            if g.multiplier == 1.5 {
                self.one_half = i;
            }
        }
        self.checked()
    }
    fn auto_anchor(&self, adaptive: bool) -> i32 {
        let n = self.onsets.count();
        let lo = if adaptive {
            ((n as f32) * 0.05).floor() as usize
        } else {
            ((n as f64) * 0.05).floor() as usize
        };
        let hi = if adaptive {
            ((n as f32) * 0.25).floor() as usize
        } else {
            ((n as f64) * 0.25).floor() as usize
        };
        let mut best = 0;
        let mut best_score = 0.;
        for i in lo..hi {
            let prev = i.saturating_sub(1) as i32;
            let a = (self.onsets.get_mark(i as i32) - self.onsets.get_mark(prev)) as f32;
            let b = (self.onsets.get_mark(i as i32 + 1) - self.onsets.get_mark(i as i32)) as f32;
            let t = self.fs * 0.025;
            let denominator = a + 1.;
            let ratio1 = ((b - (t + t)) + 1.) / denominator;
            let weight = if ratio1 > 1. { 0.5 } else { 0.25 };
            let ratio2 = ((t + b) + 1.) / denominator;
            let weight = if ratio2 > 2. { 1. } else { weight };
            let score = self.onsets.transition_energy(i as i32) * weight;
            if score > best_score {
                best_score = score;
                best = i as i32;
            }
        }
        self.onsets.get_mark(best)
    }
    fn anchor(&mut self, start: i32, mode: bool, adaptive: bool) -> Result<i32, String> {
        if start == 0 {
            self.explicit_start = Some(false);
            return Ok(self.auto_anchor(adaptive));
        }
        self.explicit_start = Some(true);
        let closest = self
            .onsets
            .get_closest(0, start)
            .map_err(|e| e.to_string())?;
        if start >= 1 && self.onsets.last_mark() > start && (!mode || adaptive) {
            Ok(start)
        } else {
            Ok(self.onsets.get_mark(closest))
        }
    }
    fn reset_tail(&mut self, anchor: i32, score: Option<f32>) {
        for g in &mut self.grid {
            g.closest = 0;
            g.anchor = anchor as f32;
            g.prediction = g.period + anchor as f32;
            g.instant = 0.;
            if let Some(s) = score {
                g.smooth = s;
            }
        }
        self.steps = 0;
        self.opaque140 = Some(0);
        self.initialized = false;
    }
    pub fn restore(
        &mut self,
        saved: [Grid; 7],
        bpm: f32,
        state: f32,
        start: i32,
        mode: bool,
    ) -> Result<(), String> {
        let result = self.restore_inner(saved, bpm, state, start, mode);
        if result.is_err() {
            self.poisoned = true;
        }
        result
    }
    fn restore_inner(
        &mut self,
        saved: [Grid; 7],
        bpm: f32,
        state: f32,
        start: i32,
        mode: bool,
    ) -> Result<(), String> {
        self.reset_grid(saved, bpm, state)?;
        let anchor = self.anchor(start, mode, false)?;
        let n = self.onsets.count() as i32;
        let mut sum = 0.;
        for i in 0..n {
            let d = self.onsets.get_dist(i);
            let tmp = sum + d as f32;
            if d > 0 {
                sum = tmp;
            }
        }
        let denominator = n.max(1) as f32;
        let mean = sum / denominator;
        let mean_int = mean as i32;
        let mut variance = 0.;
        for i in 0..n {
            let delta = self.onsets.get_dist(i) - mean_int;
            let x = delta as f32;
            let square = x * x;
            variance += square;
        }
        let period = self.grid[self.unit].period.max(1.);
        let mut r = mean / period;
        let exp_branch = if mode {
            let deviation = (variance.sqrt() / denominator) / period;
            if f64::from(deviation) < 0.015 {
                true
            } else {
                let nearest = (r + 0.5).floor();
                let ratio = r / nearest;
                let error = (f64::from(ratio) - 1.).abs();
                error < 0.1
            }
        } else {
            let mut q = 2. - r;
            q += -1.;
            q *= 0.25;
            q += 1.;
            if r < 1. {
                r = q;
            }
            true
        };
        if exp_branch {
            r = r.min(2.);
            let multiplier = if mode { 2. } else { 0.5 };
            self.gain = (crate::dsp::legacy_tracker_math_rust_v1::exp10(r)? * multiplier).max(1.);
        } else {
            self.gain = 5.;
        }
        let rd = f64::from(r);
        let nearest = (rd + 0.5).floor();
        let score = (1. - (rd - nearest).abs()) as f32;
        self.reset_tail(anchor, Some(score.min(f32::from_bits(0x3f733333))));
        self.last_closest = 0;
        self.checked()
    }
    pub fn adaptive_reset(
        &mut self,
        saved: [Grid; 7],
        bpm: f32,
        state: f32,
        start: i32,
        adaptation: f32,
    ) -> Result<(), String> {
        let result = (|| {
            if !adaptation.is_finite() {
                return Err("nonfinite adaptation".into());
            }
            self.reset_grid(saved, bpm, state)?;
            // Canonical positive multipliers exclude source's uninitialized-byte119 bypass.
            let anchor = self.anchor(start, false, true)?;
            self.reset_tail(anchor, None);
            self.gain = adaptation.max(0.01);
            self.checked()
        })();
        if result.is_err() {
            self.poisoned = true;
        }
        result
    }
    fn emit(&mut self, backward: bool, terminal: bool) -> Result<(), String> {
        if self.output.len() >= 1000000 {
            return Err("output count bound".into());
        }
        let g = self.grid[self.unit];
        let index = self.last_closest;
        let amp = if terminal {
            self.onsets.transition_energy(index)
        } else {
            let error = (g.anchor - self.onsets.get_mark(index) as f32).abs();
            if f64::from(error) < f64::from(g.period) * 0.5 {
                let left = (self.onsets.get_mark(index) - self.onsets.get_mark((index - 1).max(0)))
                    as f32
                    + 1.;
                let right = (self.onsets.get_mark(index + 1) - self.onsets.get_mark(index)) as f32;
                let t = self.fs * 0.025;
                let ratio_a = ((t + right) + 1.) / left;
                let ratio_b = ((right - (t + t)) + 1.) / left;
                let weight = if ratio_b > 1. { 0.5 } else { 0.25 };
                let weight = if ratio_a > 2. { 1. } else { weight };
                weight * self.onsets.transition_energy(index)
            } else {
                0.
            }
        };
        let confidence =
            crate::dsp::legacy_tracker_math_rust_v1::confidence(g.smooth, self.exponent)?;
        let mark = Mark {
            sample: floor_coordinate(g.anchor)?,
            transition_energy: amp,
            local_bpm: g.bpm,
            confidence,
        };
        if backward {
            self.output.insert(0, mark);
        } else {
            self.output.push(mark);
        }
        Ok(())
    }
    fn correct(&mut self, w: Winner, backward: bool) -> Result<(f32, f32), String> {
        let h = self.grid[w.grid];
        let unit = self.grid[self.unit];
        let cap = if h.smooth > 0.2 {
            0.05 / h.smooth
        } else {
            0.25
        };
        let raw_abs = f64::from(h.reciprocal) * f64::from(w.absolute);
        let abs_step = if raw_abs.abs() < f64::from_bits(0x3810000000000000) {
            0.
        } else {
            raw_abs.floor() as f32
        };
        let u = 1. - unit.smooth;
        let square = u * u;
        let v = self.gain / square;
        let v = v * 0.1;
        let old = self.exponent * 0.9;
        self.exponent = (old + v).min(50.);
        let a = h.reciprocal * w.phase;
        let z = h.smooth * a;
        let z = h.instant * z;
        let floor = f32::from_bits(0x03480000);
        let delta = if w.phase > 0. {
            if z > floor { z } else { floor }
        } else if z < -floor {
            z
        } else {
            -floor
        };
        let beta = if self.bpm_adapt {
            let h = (160. - h.bpm).max(0.);
            let h = h * f32::from_bits(0x3b7f9724);
            let h = h * h;
            h + 0.7
        } else {
            0.9
        };
        let step = (h.period * h.reciprocal).floor();
        let snap = abs_step > cap * step;
        let (anchor, base) = if snap {
            (unit.prediction.floor(), (self.fs * 30.) / h.bpm)
        } else {
            let anchor = if backward {
                h.anchor - step
            } else {
                h.anchor + step
            };
            (anchor.floor(), step + delta)
        };
        if !base.is_finite()
            || base <= 0.
            || !anchor.is_finite()
            || if backward {
                anchor >= unit.anchor
            } else {
                anchor <= unit.anchor
            }
        {
            return Err("pass nonpositive period/no progress".into());
        }
        let complement = 1. - beta;
        for g in &mut self.grid {
            let period = base * g.multiplier;
            #[allow(clippy::if_same_then_else)]
            let numerator = if snap {
                (self.fs * g.multiplier) * 30.
            } else {
                (g.multiplier * self.fs) * 30.
            };
            let bpm = if snap {
                numerator / period
            } else {
                let old = beta * g.bpm;
                let instant = numerator / period;
                let new = complement * instant;
                old + new
            };
            g.anchor = anchor;
            g.period = period;
            g.bpm = bpm;
            g.prediction = if backward {
                anchor - period
            } else {
                anchor + period
            };
        }
        self.checked()?;
        Ok((delta, abs_step))
    }
    pub fn forward(&mut self) -> Result<i32, String> {
        let result = self.forward_inner(None);
        if result.is_err() {
            self.poisoned = true;
        }
        result
    }
    fn forward_inner(&mut self, mut scratch: PassScratch) -> Result<i32, String> {
        self.checked()?;
        let mut iterations = 0;
        while self.grid[self.unit].prediction < (self.onsets.last_mark() as f32) {
            if iterations >= 1000000 || self.steps >= 1000000 {
                return Err("forward iteration bound".into());
            }
            iterations += 1;
            let previous = self.grid[self.unit].closest;
            self.steps += 1;
            self.emit(false, false)?;
            let winner = match self.best_inner(previous, false)? {
                Some(w) => w,
                None => {
                    let (phase, absolute, closest) =
                        scratch.ok_or("source forward first phase outputs unwritten")?;
                    Winner {
                        score: 0.,
                        phase,
                        absolute,
                        closest,
                        grid: self.winning,
                    }
                }
            };
            let (phase, absolute) = self.correct(winner, false)?;
            scratch = Some((phase, absolute, winner.closest));
        }
        self.emit(false, true)?;
        Ok(0)
    }
    pub fn backward(&mut self) -> Result<i32, String> {
        let result = self.backward_inner().map(|_| 0);
        if result.is_err() {
            self.poisoned = true;
        }
        result
    }
    fn backward_inner(&mut self) -> Result<PassScratch, String> {
        self.checked()?;
        if self.onsets.count() == 0 {
            return Err("empty backward profile unsupported".into());
        }
        self.last_closest = self.onsets.count() as i32 - 1;
        let shift = self.grid[self.unit].period.floor();
        for g in &mut self.grid {
            g.closest = self.last_closest;
            g.anchor -= shift;
            g.prediction -= (g.period * 3.).floor();
        }
        let mut iterations = 0;
        let mut scratch: Option<(f32, f32, i32)> = None;
        while self.grid[self.unit].anchor.floor() >= 0. {
            if iterations >= 1000000 || self.steps >= 1000000 {
                return Err("backward iteration bound".into());
            }
            iterations += 1;
            let previous = self.grid[self.unit].closest;
            self.steps += 1;
            self.emit(true, false)?;
            let winner = match self.best_inner(previous, true)? {
                Some(w) => w,
                None => {
                    let (phase, absolute, closest) =
                        scratch.ok_or("source backward first phase outputs unwritten")?;
                    Winner {
                        score: 0.,
                        phase,
                        absolute,
                        closest,
                        grid: self.winning,
                    }
                }
            };
            let (phase, absolute) = self.correct(winner, true)?;
            scratch = Some((phase, absolute, winner.closest));
        }
        if self.grid[self.unit].anchor.floor() < 0. {
            let anchor = if let Some(m) = self.output.first() {
                m.sample as f32
            } else {
                let mut anchor = self.grid[self.unit].anchor;
                let period = self.grid[self.unit].period;
                let mut iterations = 0;
                while anchor < 0. {
                    let next = anchor + period;
                    if next <= anchor || iterations >= 1000000 {
                        return Err("backward reanchor progress".into());
                    }
                    anchor = next;
                    iterations += 1;
                }
                anchor
            };
            for g in &mut self.grid {
                g.anchor = anchor;
                g.prediction = anchor + g.period;
            }
        }
        self.checked()?;
        Ok(scratch)
    }
    #[allow(clippy::too_many_arguments)]
    pub fn process(
        &mut self,
        saved: [Grid; 7],
        state: f32,
        bpm: f32,
        adaptation: f32,
        start: i32,
        reset_after_backward: bool,
        bpm_adapt: bool,
    ) -> Result<(), String> {
        let result = self.process_inner(
            saved,
            state,
            bpm,
            adaptation,
            start,
            reset_after_backward,
            bpm_adapt,
        );
        if result.is_err() {
            self.poisoned = true;
        }
        result
    }
    #[allow(clippy::too_many_arguments)]
    fn process_inner(
        &mut self,
        saved: [Grid; 7],
        state: f32,
        bpm: f32,
        adaptation: f32,
        start: i32,
        reset_after_backward: bool,
        bpm_adapt: bool,
    ) -> Result<(), String> {
        self.live()?;
        if self.onsets.count() < 7 {
            return Ok(());
        }
        if adaptation == 0. {
            self.restore(saved, bpm, state, start, false)?;
        } else {
            self.adaptive_reset(saved, bpm, state, start, adaptation)?;
        }
        self.output.clear();
        self.bpm_adapt = bpm_adapt;
        let scratch = self.backward_inner()?;
        if reset_after_backward {
            if adaptation == 0. {
                self.restore(saved, bpm, state, start, false)?;
            } else {
                self.adaptive_reset(saved, bpm, state, start, adaptation)?;
            }
        } else {
            self.output.clear();
        }
        // The source's two pass frames alias these words. Both intervening
        // reset routes preserve them in the audited macOS implementation.
        // No words are inherited from another public call or invented as zero.
        self.forward_inner(scratch)?;
        Ok(())
    }
    fn live(&self) -> Result<(), String> {
        if self.poisoned {
            Err("poisoned tracker".into())
        } else {
            Ok(())
        }
    }
    fn checked(&self) -> Result<(), String> {
        self.live()?;
        if self.unit >= 7
            || self.winning >= 7
            || self.one_half >= 7
            || !self.exponent.is_finite()
            || !(0.5..=50.).contains(&self.exponent)
            || !self.energy_ratio.is_finite()
            || !(0.0..=1.).contains(&self.energy_ratio)
            || !self.gain.is_finite()
            || self.gain <= 0.
            || self.fs != 44100.
        {
            return Err("tracker scalar admission".into());
        }
        for g in &self.grid {
            if [
                g.anchor,
                g.period,
                g.prediction,
                g.multiplier,
                g.reciprocal,
                g.bpm,
                g.smooth,
                g.instant,
            ]
            .iter()
            .any(|v| !v.is_finite())
                || g.period <= 0.
                || g.multiplier <= 0.
                || g.reciprocal <= 0.
                || g.bpm <= 0.
                || !(0.0..=1.).contains(&g.smooth)
                || !(0.0..=1.).contains(&g.instant)
            {
                return Err("grid outside finite scoring domain".into());
            }
        }
        Ok(())
    }
    pub fn init(&mut self) -> Result<(), String> {
        let result = self.init_inner();
        if result.is_err() {
            self.poisoned = true;
        }
        result
    }
    fn init_inner(&mut self) -> Result<(), String> {
        self.live()?;
        if self.onsets.count() < 6 {
            return Err("Init requires six onsets".into());
        }
        let (base, smooth) = if self.initial_bpm != 0. {
            ((self.fs * 30.) / self.initial_bpm, 0.4)
        } else {
            let mut base = self.onsets.get_dist(0) as f32;
            let mut tempo = (self.fs * 30.) / base;
            for i in 1..=5 {
                if tempo <= 180. {
                    break;
                }
                base += self.onsets.get_dist(i) as f32;
                tempo = (self.fs * 30.) / base;
            }
            for _ in 0..64 {
                if !base.is_finite() || base <= 0. || !tempo.is_finite() {
                    return Err("invalid fallback interval".into());
                }
                if (80.0..=200.).contains(&tempo) {
                    break;
                }
                if tempo > 200. {
                    base += base;
                } else if tempo < 80. {
                    base = (base * 0.5).floor();
                }
                tempo = (self.fs * 30.) / base;
            }
            if !(80.0..=200.).contains(&tempo) {
                return Err("fallback octave loop".into());
            }
            let ratio = self.onsets.get_dist(0) as f32 / self.onsets.get_dist(1) as f32;
            if !ratio.is_finite() {
                return Err("invalid fallback smooth ratio".into());
            }
            let fractional = ratio - ratio.floor();
            (base, if fractional > 0.4 { 0.4 } else { fractional })
        };
        let first = self.onsets.get_mark(0) as f32;
        for (i, m) in [4., 2., 1., 0.5, 6., 3., 1.5].into_iter().enumerate() {
            let period = if i == 1 { base + base } else { base * m };
            let numerator = if i == 1 {
                (self.fs + self.fs) * 30.
            } else {
                (self.fs * m) * 30.
            };
            self.grid[i] = Grid {
                closest: 0,
                anchor: first,
                period,
                prediction: period + first,
                multiplier: m,
                reciprocal: 1. / m,
                bpm: numerator / period,
                smooth,
                instant: 0.,
            };
        }
        self.unit = 2;
        self.one_half = 6;
        self.winning = 4;
        self.steps = 0;
        self.opaque140 = Some(0);
        self.initialized = true;
        self.checked()
    }
    pub fn find_best(&mut self, previous: i32, backward: bool) -> Result<Option<Winner>, String> {
        let result = self.best_inner(previous, backward);
        if result.is_err() {
            self.poisoned = true;
        }
        result
    }
    fn best_inner(&mut self, previous: i32, backward: bool) -> Result<Option<Winner>, String> {
        self.checked()?;
        if previous < 0 || previous as usize >= self.onsets.count() {
            return Err("previous mark index".into());
        }
        let k = if self.bpm_adapt {
            let z = (140. - self.grid[self.unit].bpm).max(0.);
            let z = z / 40.;
            let t = z * -0.1;
            let z = z * t;
            let z = z + 0.98;
            z.max(0.9)
        } else {
            0.98
        };
        let complement = 1. - f64::from(k);
        let unit_period = self.grid[self.unit].period;
        let mut winner: Option<Winner> = None;
        for i in 0..7 {
            let g = self.grid[i];
            let target = floor_coordinate(g.prediction)?;
            let closest = self
                .onsets
                .get_closest(if backward { 0 } else { previous }, target)
                .map_err(|e| e.to_string())?;
            if closest == -1 {
                continue;
            }
            let x = self.onsets.get_mark(closest) as f32;
            let phase = x - g.prediction;
            let absolute = phase.abs();
            let instant = if absolute >= unit_period || absolute >= g.period {
                0.
            } else {
                let quotient = absolute / g.period;
                let distance = 1. - f64::from(quotient);
                let one_minus = 1. - self.energy_ratio;
                let energy = self.energy_ratio * self.onsets.transition_energy(closest);
                let mix = one_minus + energy;
                crate::dsp::legacy_tracker_math_rust_v1::score(
                    distance * f64::from(mix),
                    self.exponent,
                )?
            };
            let old_term = k * g.smooth;
            let new_term = complement * f64::from(instant);
            let smooth = (new_term + f64::from(old_term)) as f32;
            self.grid[i].instant = instant;
            self.grid[i].smooth = smooth;
            if i == self.unit {
                self.last_closest = closest;
            }
            let eligible = if backward {
                closest < previous
            } else {
                closest > previous
            };
            if eligible && winner.as_ref().map_or(smooth > 0., |w| w.score < smooth) {
                winner = Some(Winner {
                    score: smooth,
                    phase: if backward { -phase } else { phase },
                    absolute,
                    grid: i,
                    closest,
                });
            }
            self.grid[i].closest = closest;
        }
        self.checked()?;
        Ok(winner)
    }
    pub fn preprocess(&mut self) -> Result<i32, String> {
        let result = self.preprocess_inner();
        if result.is_err() {
            self.poisoned = true;
        }
        result
    }
    fn preprocess_inner(&mut self) -> Result<i32, String> {
        self.live()?;
        if !self.initialized {
            if self.onsets.count() > 5 {
                self.init()?;
            }
            return Ok(0);
        }
        self.checked()?;
        let last = self.onsets.last_mark() as f32;
        if last < self.grid[self.winning].prediction {
            return Ok(-1);
        }
        let mut iterations = 0;
        let mut scratch: Option<(f32, f32, i32)> = None;
        while self.grid[self.winning].prediction < last {
            if self.onsets.count() as i32 <= self.grid[self.winning].closest + 1 {
                return Ok(0);
            }
            iterations += 1;
            if iterations > 1000000 || self.steps >= 1000000 {
                return Err("preprocess iteration bound".into());
            }
            let unit = self.grid[self.unit];
            let previous = unit.closest;
            self.steps += 1;
            self.output.push(Mark {
                sample: floor_coordinate(unit.anchor)?,
                transition_energy: self.onsets.transition_energy(self.last_closest),
                local_bpm: unit.bpm,
                confidence: unit.instant,
            });
            let winner = match self.best_inner(previous, false)? {
                Some(w) => w,
                None => {
                    let (phase, absolute, closest) =
                        scratch.ok_or("source preprocessing first phase outputs unwritten")?;
                    Winner {
                        score: 0.,
                        phase,
                        absolute,
                        closest,
                        grid: self.winning,
                    }
                }
            };
            let h = self.grid[winner.grid];
            let s = h.smooth;
            let q = h.instant;
            let r = h.reciprocal;
            let cap = if s > 0.2 { 0.05 / s } else { 0.25 };
            let phase_raw =
                ((f64::from(r) * f64::from(winner.phase)) * f64::from(s)) * f64::from(q);
            let phase = if phase_raw.abs() < f64::from_bits(0x3810000000000000) {
                0.
            } else {
                phase_raw.floor() as f32
            };
            let absolute_raw =
                ((f64::from(r) * f64::from(winner.absolute)) * f64::from(s)) * f64::from(q);
            let absolute = if absolute_raw.abs() < f64::from_bits(0x3810000000000000) {
                0.
            } else {
                absolute_raw as f32
            };
            scratch = Some((phase, absolute, winner.closest));
            let old = self.exponent * 0.9;
            let complement = 1. - s;
            let square = complement * complement;
            let inverse = 1. / square;
            let term = inverse * 0.1;
            self.exponent = (term + old).min(50.);
            let beta = if self.bpm_adapt {
                let h = (160. - h.bpm).max(0.);
                let h = h * f32::from_bits(0x3b7f9724);
                let square = h * h;
                square + 0.7
            } else {
                0.9
            };
            let step = (h.period * r).floor();
            let snap = cap * step < absolute;
            let (anchor, base) = if snap {
                self.exponent = 1.;
                (
                    self.grid[self.unit].prediction.floor(),
                    (self.fs * 30.) / h.bpm,
                )
            } else {
                ((h.anchor + step).floor(), step + phase)
            };
            if !base.is_finite() || base <= 0. || anchor <= unit.anchor {
                return Err("preprocess nonpositive period/no progress".into());
            }
            let complement = 1. - beta;
            for g in &mut self.grid {
                let period = base * g.multiplier;
                // Preserve operand order from the two source blocks.
                #[allow(clippy::if_same_then_else)]
                let numerator = if snap {
                    (self.fs * g.multiplier) * 30.
                } else {
                    (g.multiplier * self.fs) * 30.
                };
                let bpm = if snap {
                    numerator / period
                } else {
                    let old = beta * g.bpm;
                    let instant = numerator / period;
                    let new = complement * instant;
                    old + new
                };
                g.anchor = anchor;
                g.period = period;
                g.bpm = bpm;
                g.prediction = anchor + period;
            }
            self.checked()?;
        }
        Ok(0)
    }
}
fn floor_coordinate(value: f32) -> Result<i32, String> {
    if !value.is_finite() || !(-1000000000.0..=1000000000.0).contains(&value) {
        Err("coordinate conversion domain".into())
    } else {
        Ok(value.floor() as i32)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn marks(n: usize) -> Vec<Mark> {
        (0..n)
            .map(|i| Mark {
                sample: (i * 11025) as i32,
                transition_energy: 1.,
                local_bpm: 0.,
                confidence: 0.,
            })
            .collect()
    }
    #[test]
    fn constructor_and_init_preserve_seven_source_hypotheses() {
        let mut t = Tracker::new(&marks(16), 120.).unwrap();
        assert!(!t.initialized);
        assert_eq!(t.explicit_start, None);
        assert_eq!(t.opaque140, None);
        assert_eq!(t.exponent, 1.);
        t.init().unwrap();
        assert!(t.initialized);
        assert_eq!((t.unit, t.winning, t.one_half), (2, 4, 6));
        for (g, m) in t.grid.iter().zip([4., 2., 1., 0.5, 6., 3., 1.5]) {
            assert_eq!(g.multiplier, m);
            assert_eq!(g.period, 11025. * m);
            assert_eq!(g.prediction, g.period);
            assert_eq!(g.bpm, 120.);
            assert_eq!(g.smooth, 0.4);
            assert_eq!(g.instant, 0.);
        }
    }
    #[test]
    fn best_selection_keeps_first_positive_tie_and_unit_closest() {
        let mut t = Tracker::new(&marks(16), 120.).unwrap();
        t.init().unwrap();
        let w = t.find_best(0, false).unwrap().unwrap();
        assert_eq!((w.grid, w.closest), (0, 4));
        assert_eq!(t.last_closest, 1);
        assert_eq!(w.phase.to_bits(), 0);
        assert!(w.score > 0.4);
        let mut b = Tracker::new(&marks(16), 120.).unwrap();
        b.init().unwrap();
        let w = b.find_best(15, true).unwrap().unwrap();
        assert_eq!(w.phase.to_bits(), 0x80000000);
    }
    #[test]
    fn no_winner_is_exposed_instead_of_inventing_phase_values() {
        let mut t = Tracker::new(&marks(16), 120.).unwrap();
        t.init().unwrap();
        assert!(t.find_best(15, false).unwrap().is_none());
    }
    #[test]
    fn preprocess_first_call_initializes_and_second_advances() {
        let mut t = Tracker::new(&marks(32), 120.).unwrap();
        assert_eq!(t.preprocess().unwrap(), 0);
        assert!(t.initialized);
        assert!(t.output.is_empty());
        assert_eq!(t.preprocess().unwrap(), 0);
        assert!(!t.output.is_empty());
        assert!(t.steps > 0);
        assert!((0.5..=50.).contains(&t.exponent));
        assert!(
            t.output
                .iter()
                .all(|m| m.sample >= 0 && m.local_bpm.is_finite())
        );
        let mut short = Tracker::new(&marks(5), 120.).unwrap();
        assert_eq!(short.preprocess().unwrap(), 0);
        assert!(!short.initialized);
    }
    #[test]
    fn invalid_constructor_values_fail_explicitly() {
        for bpm in [-1., f32::NAN, f32::INFINITY, 401.] {
            assert!(Tracker::new(&marks(7), bpm).is_err());
        }
        let mut a = marks(7);
        a[2].sample = a[1].sample;
        assert!(Tracker::new(&a, 120.).is_err());
        a = marks(7);
        a[2].transition_energy = 1.001;
        assert!(Tracker::new(&a, 120.).is_err());
    }
}

pub fn histogram(marks: &[Mark]) -> Result<(f32, i32), String> {
    if marks.len() > 1000000 {
        return Err("histogram count domain".into());
    }
    let a = 44100f32 * 0.6;
    let hi = (a / 50.) as i32;
    let lo = (a / 240.).floor() as i32;
    let mut counts = vec![0i32; (hi - lo + 1) as usize];
    for m in marks {
        let bpm = m.local_bpm;
        if bpm > 50. && bpm < 240. {
            let a = 44100f32 * 0.6;
            let q = (bpm * 10.).floor() * 0.1;
            let index = (a / q).floor() as i32 - lo;
            let count = counts
                .get_mut(index as usize)
                .ok_or("histogram bin bounds")?;
            *count += 1;
        }
    }
    let mut best = 0;
    let mut maximum = 0.;
    let mut nonempty = 0;
    for (i, &count) in counts.iter().enumerate() {
        let value = count as f32;
        if maximum < value {
            maximum = value;
            best = i;
        }
        if count >= 1 {
            nonempty += 1;
        }
    }
    let denominator = ((best as i32 + lo) as f32) * 100.;
    let numerator = 44100f32 * 60.;
    Ok(((numerator / denominator).floor(), nonempty))
}
#[cfg(test)]
mod pass_tests {
    use super::*;
    fn marks(n: usize) -> Vec<Mark> {
        (0..n)
            .map(|i| Mark {
                sample: (i * 11025) as i32,
                transition_energy: 0.8,
                local_bpm: 0.,
                confidence: 0.,
            })
            .collect()
    }
    #[test]
    fn adaptive_reset_retains_scores_and_last_closest() {
        let mut t = Tracker::new(&marks(64), 120.).unwrap();
        t.init().unwrap();
        let mut saved = t.grid;
        for (i, g) in saved.iter_mut().enumerate() {
            g.smooth = (i + 1) as f32 / 10.;
            g.instant = 0.6;
            g.closest = 12;
        }
        t.last_closest = 17;
        t.adaptive_reset(saved, 120., 2., 0, 1.1).unwrap();
        assert_eq!(t.last_closest, 17);
        assert_eq!(t.exponent, 2.);
        assert_eq!(t.gain, 1.1);
        assert_eq!(t.explicit_start, Some(false));
        for (g, old) in t.grid.iter().zip(saved) {
            assert_eq!(g.smooth, old.smooth);
            assert_eq!(g.instant, 0.);
            assert_eq!(g.closest, 0);
            assert_eq!(g.prediction, g.anchor + g.period);
        }
    }
    #[test]
    fn nonadaptive_reset_recomputes_scores_and_auto_anchor() {
        let mut t = Tracker::new(&marks(64), 120.).unwrap();
        t.init().unwrap();
        let saved = t.grid;
        t.last_closest = 17;
        t.restore(saved, 120., 2., 0, false).unwrap();
        assert_eq!(t.last_closest, 0);
        assert_eq!(t.explicit_start, Some(false));
        assert!(t.gain >= 1.);
        assert_eq!(t.grid[t.unit].anchor, 3. * 11025.);
        assert!(
            t.grid
                .iter()
                .all(|g| g.smooth == f32::from_bits(0x3f733333) && g.instant == 0.)
        );
    }
    #[test]
    fn forward_always_emits_terminal_mark_without_loop() {
        let mut t = Tracker::new(&marks(16), 120.).unwrap();
        t.init().unwrap();
        for g in &mut t.grid {
            g.anchor = 1000.;
            g.prediction = 200000.;
        }
        assert_eq!(t.forward().unwrap(), 0);
        assert_eq!(t.output.len(), 1);
        assert_eq!(t.output[0].sample, 1000);
        assert_eq!(t.output[0].transition_energy, 0.8);
        assert_eq!(t.output[0].confidence, 0.4);
        assert_eq!(t.steps, 0);
    }
    #[test]
    fn adaptive_histogram_lifecycle_runs_backward_then_forward() {
        let mut t = Tracker::new(&marks(64), 120.).unwrap();
        t.preprocess().unwrap();
        t.preprocess().unwrap();
        let saved = t.grid;
        let state = t.exponent;
        t.process(saved, state, 120., 1.1, 0, false, true).unwrap();
        assert!(t.output.len() >= 4);
        assert!(t.output.windows(2).all(|w| w[0].sample < w[1].sample));
        assert!(t.output.iter().all(|m| m.local_bpm == 120.));
    }
    #[test]
    fn histogram_counts_quantized_bins_and_preserves_empty_fallback() {
        assert_eq!(histogram(&[]).unwrap(), (240., 0));
        let mut a = marks(2);
        a[0].local_bpm = 100.;
        a[1].local_bpm = 120.;
        assert_eq!(histogram(&a).unwrap(), (120., 2));
        a[0].local_bpm = 50.;
        a[1].local_bpm = 240.;
        assert_eq!(histogram(&a).unwrap(), (240., 0));
    }
}

#[cfg(test)]
mod admission_tests {
    use super::*;
    fn marks(n: usize) -> Vec<Mark> {
        (0..n)
            .map(|i| Mark {
                sample: (i * 11025) as i32,
                transition_energy: 0.8,
                local_bpm: 0.,
                confidence: 0.,
            })
            .collect()
    }
    #[test]
    fn undefined_first_phase_is_rejected_and_poisoned_tracker_cannot_retry() {
        let mut t = Tracker::new(&marks(16), 120.).unwrap();
        t.init().unwrap();
        let saved = t.grid;
        let error = t.process(saved, 1., 120., 1.1, 0, false, true).unwrap_err();
        assert!(error.contains("first phase outputs unwritten"));
        assert!(t.forward().is_err());
        assert!(t.preprocess().is_err());
    }
    #[test]
    fn poisoned_short_tracker_does_not_bypass_retry_guard() {
        let mut t = Tracker::new(&marks(5), 120.).unwrap();
        assert!(t.init().is_err());
        assert!(
            t.process([Grid::default(); 7], 1., 120., 0., 0, false, false)
                .is_err()
        );
    }
}

#[cfg(test)]
mod context_and_coordinate_tests {
    use super::*;
    fn marks(n: usize, offset: i32) -> Vec<Mark> {
        (0..n)
            .map(|i| Mark {
                sample: offset + (i * 11025) as i32,
                transition_energy: 0.8,
                local_bpm: 0.,
                confidence: 0.,
            })
            .collect()
    }
    #[test]
    fn composed_process_preserves_defined_backward_scratch_for_silent_intro() {
        let mut t = Tracker::new(&marks(64, 1080450), 120.).unwrap();
        t.preprocess().unwrap();
        t.preprocess().unwrap();
        let saved = t.grid;
        let state = t.exponent;
        t.process(saved, state, 120., 1.1, 0, false, true).unwrap();
        assert!(t.output.len() > 64);
        assert!(t.output.windows(2).all(|w| w[0].sample < w[1].sample));
    }
    #[test]
    fn coordinate_promotion_beyond_exact_integer_f32_range_is_source_rounded() {
        let mut t = Tracker::new(&marks(64, 20000001), 120.).unwrap();
        t.init().unwrap();
        assert_eq!(t.grid[t.unit].anchor, 20000000.);
        assert_eq!(t.grid[t.unit].prediction, 20011024.);
    }
    #[test]
    fn extended_coordinate_boundary_remains_checked() {
        assert!(Tracker::new(&marks(6, 499940000), 120.).is_ok());
        assert!(Tracker::new(&marks(2, 500000001), 120.).is_err());
    }
}
