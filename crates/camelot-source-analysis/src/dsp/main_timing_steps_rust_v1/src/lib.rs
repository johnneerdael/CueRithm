//! Deterministic Main timing steps, numeric features and explicit component anchors only.
pub mod bounds;
pub const MAX_GRID_POINTS: usize = 100_000;
const MAX_COORDINATE: f64 = 1_000_000.0;
#[derive(Clone, Debug, serde::Serialize)]
pub struct Grid {
    pub times: Vec<f64>,
    pub tempo: f64,
    pub duration: f64,
    pub downbeat: f64,
    pub origin: f64,
}
impl Grid {
    pub fn new(mut times: Vec<f64>, tempo: f64, duration: f64) -> Result<Self, String> {
        let projected = crate::dsp::main_timing_steps_rust_v1::bounds::projected_normalized_count(
            &times, tempo,
        )?;
        if times.len() < 2
            || !(2..=MAX_GRID_POINTS).contains(&projected)
            || !(10.0..=1000.0).contains(&tempo)
        {
            return Err(
                "ordinary timing grid requires2..100000 normalized marks andtempo10..1000".into(),
            );
        }
        if !duration.is_finite()
            || duration.abs() > crate::dsp::main_timing_steps_rust_v1::bounds::MAX_ABS_TIME
        {
            return Err("duration exceeds bounded contract".into());
        }
        let downbeat = times.first().copied().unwrap_or(0.0);
        if times.len() >= 2 && tempo > 0.0 {
            let intervals = (times.len() - 1) as f64;
            let span = times[times.len() - 1] - times[0];
            let ratio = (((60.0 / span) * intervals) / tempo).ln();
            let mut factor = (ratio / std::f64::consts::LN_2 + 0.5).floor().exp2();
            while factor > 1.75 {
                times = times.into_iter().step_by(2).collect();
                factor *= 0.5;
            }
            while factor < 0.6 {
                let mut dense = Vec::with_capacity(times.len() * 2);
                for pair in times.windows(2) {
                    dense.push(pair[0]);
                    dense.push((pair[0] + pair[1]) * 0.5);
                }
                if let Some(last) = times.last() {
                    dense.push(*last);
                }
                times = dense;
                factor += factor;
            }
        }
        debug_assert_eq!(times.len(), projected);
        if times.windows(2).any(|p| p[1] <= p[0]) {
            return Err("normalization loses increasing geometry".into());
        }
        Ok(Self {
            times,
            tempo,
            duration,
            downbeat,
            origin: 0.0,
        })
    }
    pub fn half_beats(&self) -> bool {
        self.tempo > 0.0 && self.tempo < 100.0
    }
    fn position_internal(&self, t: f64) -> f64 {
        let n = self.times.len();
        if n < 2 {
            return 0.0;
        }
        let b = &self.times;
        if t < b[0] {
            let p = b[1] - b[0];
            return if p > 0.0 { (t - b[0]) / p } else { 0.0 };
        }
        if t > b[n - 1] && b[n - 1] - b[n - 2] > 0.0 {
            return ((t - b[n - 1]) / (b[n - 1] - b[n - 2]) + n as f64) + (-1.0);
        }
        for i in 0..n - 1 {
            if b[i + 1] >= t {
                let p = b[i + 1] - b[i];
                let frac = if p > 0.0 { (t - b[i]) / p } else { 0.0 };
                return frac + i as f64;
            }
        }
        0.0
    }
    fn position_unchecked(&self, t: f64) -> f64 {
        self.position_internal(t) - self.origin
    }
    fn time_unchecked(&self, coordinate: f64) -> f64 {
        let q = coordinate / 1.0 + self.origin;
        let n = self.times.len();
        if n < 2 {
            return 0.0;
        }
        let i = q as i32;
        let b = &self.times;
        if i < 0 {
            return q.mul_add(b[1] - b[0], b[0]);
        }
        if i as usize + 1 >= n {
            return ((q - n as f64) + 1.0).mul_add(b[n - 1] - b[n - 2], b[n - 1]);
        }
        let i = i as usize;
        (q - i as f64).mul_add(b[i + 1] - b[i], b[i])
    }
    pub fn position(&self, time: f64) -> Result<f64, String> {
        self.query_ready()?;
        if !time.is_finite() || time.abs() > bounds::MAX_ABS_TIME {
            return Err("time outside bounded domain".into());
        }
        let value = self.position_unchecked(time);
        if !value.is_finite() || value.abs() > MAX_COORDINATE {
            return Err("coordinate outside bounded domain".into());
        }
        Ok(value)
    }
    pub fn time(&self, coordinate: f64) -> Result<f64, String> {
        self.query_ready()?;
        if !coordinate.is_finite()
            || coordinate.abs() > MAX_COORDINATE
            || !(coordinate + self.origin).is_finite()
        {
            return Err("coordinate outside bounded domain".into());
        }
        let value = self.time_unchecked(coordinate);
        if !value.is_finite() || value.abs() > bounds::MAX_ABS_TIME {
            return Err("interpolated time outside bounded domain".into());
        }
        Ok(value)
    }
    pub fn set_downbeat(&mut self, time: f64) -> Result<(), String> {
        self.query_ready()?;
        if !time.is_finite() || time.abs() > bounds::MAX_ABS_TIME {
            return Err("downbeat outside bounded domain".into());
        }
        let origin = self.position_internal(time);
        if !origin.is_finite() || origin.abs() > MAX_COORDINATE {
            return Err("downbeat origin outside bounded domain".into());
        }
        self.downbeat = time;
        self.origin = origin;
        Ok(())
    }
    fn query_ready(&self) -> Result<(), String> {
        if !(2..=MAX_GRID_POINTS).contains(&self.times.len())
            || !(10.0..=1000.0).contains(&self.tempo)
            || !self.origin.is_finite()
            || self.origin.abs() > MAX_COORDINATE
        {
            return Err("invalid bounded grid query state".into());
        }
        Ok(())
    }
    fn checked(&self) -> Result<(), String> {
        self.query_ready()?;
        bounds::projected_normalized_count(&self.times, self.tempo)?;
        if self.times.len() < 2
            || self.times.len() > MAX_GRID_POINTS
            || !self.duration.is_finite()
            || self.duration.abs() > bounds::MAX_ABS_TIME
            || !self.downbeat.is_finite()
            || !self.origin.is_finite()
        {
            return Err("invalid bounded grid state".into());
        }
        Ok(())
    }
}

#[derive(Debug, serde::Serialize)]
pub struct AnchorDiagnostics {
    pub selected_anchor: f64,
    pub shift: f64,
    pub attempted_coordinates: Vec<f64>,
}
pub fn align_anchor(
    grid: &mut Grid,
    anchor: f64,
    force: bool,
) -> Result<AnchorDiagnostics, String> {
    grid.checked()?;
    let mut q = grid.position(anchor)?;
    let max_attempts = ((q + 256.0) / 16.0).ceil().max(1.0) + 2.0;
    if max_attempts > MAX_GRID_POINTS as f64 {
        return Err("anchor backtracking exceeds bounded count".into());
    }
    let mut selected = anchor;
    let mut attempted = Vec::new();
    loop {
        let next = q + (-16.0);
        if !next.is_finite() || next >= q {
            return Err("anchor backtracking cannot progress".into());
        }
        q = next;
        let candidate = grid.time(q)?;
        attempted.push(q);
        if candidate >= -0.2 {
            selected = if q < -256.0 { anchor } else { candidate };
        }
        if q < -256.0 || candidate < -0.2 {
            break;
        }
        if attempted.len() >= MAX_GRID_POINTS {
            return Err("anchor backtracking exceeds bounded count".into());
        }
    }
    let mut shift = 0.0;
    if force {
        let q = grid.position(selected)?;
        let u = q + 0.01;
        let residual = (u - u.floor()) + (-0.01);
        let target = grid.time(q + residual)?;
        let old = grid.time(q)?;
        shift = target - old;
        let times = grid.times.iter().map(|&t| shift + t).collect();
        let mut out = Grid::new(times, grid.tempo, grid.duration)?;
        out.set_downbeat(selected)?;
        *grid = out;
    } else {
        grid.set_downbeat(selected)?;
    }
    Ok(AnchorDiagnostics {
        selected_anchor: selected,
        shift,
        attempted_coordinates: attempted,
    })
}
pub fn pass2_tiny_shift(grid: &Grid) -> Result<Grid, String> {
    grid.checked()?;
    let delta = f64::from_bits(0xbc7ffffffffffffc);
    Grid::new(
        grid.times.iter().map(|&t| delta + t).collect(),
        grid.tempo,
        grid.duration,
    )
}
pub fn final_resample(grid: &Grid) -> Result<Grid, String> {
    grid.checked()?;
    let mut q = grid.position(grid.downbeat)?;
    let end = grid.duration.max(*grid.times.last().unwrap());
    let projected = (grid.position(end)? - q).ceil().max(0.0) + 2.0;
    if !projected.is_finite() || projected > MAX_GRID_POINTS as f64 {
        return Err("tail resampling exceeds bounded count".into());
    }
    let mut times = Vec::new();
    loop {
        let time = grid.time(q)?;
        if time >= end {
            break;
        }
        if times.last().is_some_and(|&last| time <= last) {
            return Err("tail time cannot progress".into());
        }
        if times.len() >= MAX_GRID_POINTS {
            return Err("tail resampling exceeds bounded count".into());
        }
        times.push(time);
        let next = q + 1.0;
        if next <= q {
            return Err("tail coordinate cannot progress".into());
        }
        q = next;
    }
    let first = times.first().copied().ok_or("resampled tail is empty")?;
    let mut out = Grid::new(times, grid.tempo, grid.duration)?;
    out.set_downbeat(first)?;
    Ok(out)
}
const MAX_WEIGHTED_VISITS: usize = 20_000_000;
#[derive(Clone, Debug)]
pub struct Waveform {
    pub sample_rate: f64,
    pub hop: i32,
    pub arrays: Vec<Vec<f32>>,
}
impl Waveform {
    pub fn new(sample_rate: f64, hop: i32, arrays: Vec<Vec<f32>>) -> Result<Self, String> {
        let out = Self {
            sample_rate,
            hop,
            arrays,
        };
        out.checked()?;
        Ok(out)
    }
    fn checked(&self) -> Result<(), String> {
        if !(1000.0..=384000.0).contains(&self.sample_rate)
            || !(2..=65536).contains(&self.hop)
            || !(2..=8).contains(&self.arrays.len())
        {
            return Err("bounded waveform geometry/array count required".into());
        }
        let frames = self.arrays[0].len();
        if frames > bounds::MAX_FEATURE_FRAMES
            || self.arrays.iter().any(|a| {
                (!a.is_empty() && a.len() != frames) || a.iter().any(|v| !v.is_finite() || *v < 0.0)
            })
        {
            return Err(
                "bounded finite nonnegative waveform arrays of matching nonempty lengths required"
                    .into(),
            );
        }
        Ok(())
    }
    fn indices(&self, e: [f64; 4]) -> Result<(usize, usize), String> {
        if e.iter()
            .any(|x| !x.is_finite() || x.abs() > bounds::MAX_ABS_TIME + 1024.0)
            || e.windows(2).any(|p| p[1] < p[0])
        {
            return Err("invalid weighted window endpoints".into());
        }
        let start = ((self.sample_rate * e[0]) / self.hop as f64).floor();
        let end = ((self.sample_rate * e[3]) / self.hop as f64).ceil();
        if start < i32::MIN as f64
            || start > i32::MAX as f64
            || end < i32::MIN as f64
            || end > i32::MAX as f64
        {
            return Err("weighted coordinates exceed bounded i32 conversion".into());
        }
        let start = start.max(0.0) as usize;
        let end = (end as i32).min(self.arrays[0].len() as i32).max(0) as usize;
        Ok((start, end.max(start)))
    }
    pub fn weighted_average(
        &self,
        channel: usize,
        endpoints: [f64; 4],
        minimum: f64,
        power: f64,
    ) -> Result<f64, String> {
        if !(1000.0..=384000.0).contains(&self.sample_rate)
            || !(2..=65536).contains(&self.hop)
            || !(2..=8).contains(&self.arrays.len())
        {
            return Err("invalid bounded waveform state".into());
        }
        if !minimum.is_finite()
            || minimum < 0.0
            || !power.is_finite()
            || power <= 0.0
            || power > 64.0
        {
            return Err("invalid weighted minimum/power".into());
        }
        let channel = channel.min(self.arrays.len() - 1);
        if self.arrays[channel].is_empty() {
            return Ok(0.0);
        }
        if self.arrays[0].len() > bounds::MAX_FEATURE_FRAMES
            || self.arrays[channel].len() != self.arrays[0].len()
        {
            return Err("invalid bounded selected feature length".into());
        }
        let [t1, t2, t3, t4] = endpoints;
        let (begin, end) = self.indices(endpoints)?;
        let mut accumulated = 0.0;
        for index in begin..end {
            let mut value = (self.arrays[channel][index] as f64).max(0.0);
            if value < minimum {
                value = 0.0;
            }
            let time = ((index as f64 + 0.5) / self.sample_rate) * self.hop as f64;
            let mut weight = 0.0;
            if t1 < time && time < t2 {
                let u = ((time - t1) * 0.5) / (t2 - t1);
                if u > 0.0 && u < 1.0 {
                    weight = (u * (std::f64::consts::PI + std::f64::consts::PI))
                        .cos()
                        .mul_add(-0.5, 0.5);
                }
            }
            if t2 <= time && time <= t3 {
                weight = 1.0;
            }
            if t3 < time && time < t4 {
                let u = ((time - t3) * 0.5) / (t4 - t3) + 0.5;
                weight = 0.0;
                if u > 0.0 && u < 1.0 {
                    weight = (u * (std::f64::consts::PI + std::f64::consts::PI))
                        .cos()
                        .mul_add(-0.5, 0.5);
                }
            }
            accumulated = weight.mul_add(value.powf(power), accumulated);
        }
        let edge = ((t2 - t1) + t4) - t3;
        let mut area = (edge.mul_add(0.5, t3 - t2) * self.sample_rate) / self.hop as f64;
        if area <= 0.0 {
            area = 1.0;
        }
        let mean = (accumulated / area).powf(1.0 / power);
        let rate_scale = ((self.sample_rate * 0.5) / self.hop as f64).powf(0.8333333333333334);
        let result = rate_scale * mean;
        if !result.is_finite() {
            return Err("weighted result exceeds finite contract".into());
        }
        Ok(result)
    }
}
fn endpoints(time: f64, width: f64) -> [f64; 4] {
    let lo = time - width;
    let hi = time + width;
    let span = hi - lo;
    [
        (-span).mul_add(0.12, lo),
        (-span).mul_add(0.02, lo),
        (-span).mul_add(0.12, hi),
        (-span).mul_add(0.02, hi),
    ]
}
fn score_times(wave: &Waveform, times: &[f64], width: f64, channel: usize) -> Result<f64, String> {
    let mut visits = 0usize;
    for &t in times {
        let (a, b) = wave.indices(endpoints(t, width))?;
        visits = visits
            .checked_add(b - a)
            .filter(|&n| n <= MAX_WEIGHTED_VISITS)
            .ok_or("weighted work exceeds bounded budget")?;
    }
    let mut sum = 0.0;
    for &t in times {
        sum += wave.weighted_average(channel, endpoints(t, width), 0.0, 1.2)?;
    }
    if !sum.is_finite() {
        return Err("phase score exceeds finite domain".into());
    }
    Ok(sum)
}
pub fn special_tempo(tempo: f64) -> bool {
    (82.5..=89.0).contains(&tempo)
}
#[derive(Debug, serde::Serialize)]
pub struct Pass1Diagnostics {
    pub original_score: f64,
    pub shifted_score: f64,
    pub shift: f64,
    pub accepted: bool,
    pub skipped_special: bool,
}
pub fn pass1_half_shift(grid: &Grid, wave: &Waveform) -> Result<(Grid, Pass1Diagnostics), String> {
    grid.checked()?;
    wave.checked()?;
    if special_tempo(grid.tempo) {
        return Ok((
            grid.clone(),
            Pass1Diagnostics {
                original_score: 0.0,
                shifted_score: 0.0,
                shift: 0.0,
                accepted: false,
                skipped_special: true,
            },
        ));
    }
    let shift = (60.0 / grid.tempo) * 0.5;
    let shifted: Vec<f64> = grid.times.iter().map(|&t| shift + t).collect();
    let original_score = score_times(wave, &grid.times, 0.1, 1)?;
    let shifted_score = score_times(wave, &shifted, 0.1, 1)?;
    let accepted = shifted_score * 1.0 > original_score;
    let mut out = Grid::new(
        if accepted {
            shifted
        } else {
            grid.times.clone()
        },
        grid.tempo,
        grid.duration,
    )?;
    let used_shift = if accepted { shift } else { 0.0 };
    out.set_downbeat(used_shift + grid.downbeat)?;
    Ok((
        out,
        Pass1Diagnostics {
            original_score,
            shifted_score,
            shift: used_shift,
            accepted,
            skipped_special: false,
        },
    ))
}
fn select_residue(scores: &[f64], candidates: &[usize], stride: usize) -> (usize, Vec<f64>) {
    let mut best = -1_000_000_000.0;
    let mut selected = 0;
    let mut sums = Vec::with_capacity(candidates.len());
    for &residue in candidates {
        let mut sum = 0.0;
        for i in 0..scores.len() / stride {
            sum += scores[residue + i * stride];
        }
        sums.push(sum);
        if sum > best {
            best = sum;
            selected = residue;
        }
    }
    (selected, sums)
}
#[derive(Debug, serde::Serialize)]
pub struct PhaseStageDiagnostics {
    pub name: &'static str,
    pub feature_index: usize,
    pub selected_residue: usize,
    pub shift: f64,
    pub phase_sums: Vec<f64>,
    pub features: Vec<f64>,
    pub pattern_scores: Vec<f64>,
}
#[derive(Debug, serde::Serialize)]
pub struct PatternDiagnostics {
    pub special: bool,
    pub stages: Vec<PhaseStageDiagnostics>,
}
const PATTERNS: [&str; 16] = [
    "10010000000100011001000000010101",
    "00000000000010001001000010010000",
    "1000100010010010",
    "11000001001000001100000100100000",
    "10100001101000001010000110100000",
    "10100001101000100010000110100010",
    "1001001001000101",
    "10000000001000001010000000100010",
    "1000100010001000",
    "00000000000000001_______________",
    "000000001___1___1___1___1___1___",
    "00000000100010001_______________",
    "1___0___1___0___1_______________",
    "1_0_1_0_1_0_1_0_________________",
    "1_0_____________________________",
    "10",
];
#[derive(Clone, Copy)]
enum PhaseKind {
    Quarter,
    Half,
    Whole,
}
impl PhaseKind {
    fn name(self) -> &'static str {
        match self {
            Self::Quarter => "quarter",
            Self::Half => "half",
            Self::Whole => "whole",
        }
    }
    fn weight(self, index: usize) -> f64 {
        match self {
            Self::Quarter => {
                if (8..=11).contains(&index) {
                    1.0
                } else {
                    0.0
                }
            }
            Self::Half => match index {
                13 => f64::from_bits(0x3fc9ba5e353f7cee),
                14 => f64::from_bits(0x3fd09374bc6a7efa),
                _ => 0.0,
            },
            Self::Whole => 0.0,
        }
    }
    fn candidates(self) -> &'static [usize] {
        match self {
            Self::Quarter => &[0, 1],
            Self::Half => &[0, 2],
            Self::Whole => &[0, 4],
        }
    }
    fn stride(self) -> usize {
        match self {
            Self::Quarter => 2,
            Self::Half => 4,
            Self::Whole => 8,
        }
    }
}
fn phase_features(grid: &Grid, wave: &Waveform, channel: usize) -> Result<Vec<f64>, String> {
    let count = grid
        .times
        .len()
        .checked_mul(4)
        .and_then(|n| n.checked_add(129))
        .filter(|&n| n <= 4 * MAX_GRID_POINTS + 129)
        .ok_or("phase feature count exceeds bounded maximum")?;
    let width = f64::from_bits(0x3fba7fef8c25882b);
    let mut q = -16.0;
    let mut visits = 0usize;
    for _ in 0..count {
        let t = grid.time(q)?;
        let (a, b) = wave.indices(endpoints(t, width))?;
        visits = visits
            .checked_add(b - a)
            .filter(|&n| n <= MAX_WEIGHTED_VISITS)
            .ok_or("phase weighted work exceeds bounded budget")?;
        q += 0.25;
    }
    let mut values = Vec::with_capacity(count);
    q = -16.0;
    for _ in 0..count {
        let t = grid.time(q)?;
        values.push(wave.weighted_average(channel, endpoints(t, width), 0.0, 1.2)?);
        q += 0.25;
    }
    Ok(values)
}
fn phase_stage(
    grid: &Grid,
    wave: &Waveform,
    kind: PhaseKind,
    feature_index: usize,
) -> Result<(Grid, PhaseStageDiagnostics), String> {
    let features = phase_features(grid, wave, feature_index)?;
    let mut maximum = 0.0;
    for &v in &features {
        if v > maximum {
            maximum = v;
        }
    }
    let threshold = maximum * f64::from_bits(0x3fe604189374bc6a);
    let mut scores = Vec::with_capacity(features.len() - 32);
    for i in 0..features.len() - 32 {
        let mut best = 0.0;
        for (j, pattern) in PATTERNS.iter().enumerate() {
            let mut sum = 0.0;
            for (k, c) in pattern.bytes().enumerate() {
                if c == b'_' {
                    continue;
                }
                match kind {
                    PhaseKind::Half => {
                        sum += if c == b'1' {
                            features[i + k]
                        } else {
                            -features[i + k]
                        };
                    }
                    _ => {
                        sum += if (c == b'1') == (features[i + k] > threshold) {
                            1.0
                        } else {
                            -1.0
                        };
                    }
                }
            }
            if !matches!(kind, PhaseKind::Half) && sum <= 0.0 {
                sum = 0.0;
            }
            let value = sum * kind.weight(j);
            if value > best {
                best = value;
            }
        }
        if best <= 0.0 {
            best = 0.0;
        }
        scores.push(best);
    }
    let (selected_residue, phase_sums) = select_residue(&scores, kind.candidates(), kind.stride());
    if phase_sums.iter().any(|v| !v.is_finite()) {
        return Err("phase residue sum exceeds finite domain".into());
    }
    let modulo = kind.stride();
    let signed = if selected_residue > modulo / 2 {
        selected_residue as i32 - modulo as i32
    } else {
        selected_residue as i32
    };
    let delta = ((60.0 / grid.tempo) * (signed as f64)) * 0.25;
    let mut out = Grid::new(
        grid.times.iter().map(|&t| delta + t).collect(),
        grid.tempo,
        grid.duration,
    )?;
    out.set_downbeat(delta + grid.downbeat)?;
    Ok((
        out,
        PhaseStageDiagnostics {
            name: kind.name(),
            feature_index,
            selected_residue,
            shift: delta,
            phase_sums,
            features,
            pattern_scores: scores,
        },
    ))
}
pub fn pattern_shifts(grid: &Grid, wave: &Waveform) -> Result<(Grid, PatternDiagnostics), String> {
    grid.checked()?;
    wave.checked()?;
    let special = special_tempo(grid.tempo);
    let (mut out, quarter) =
        phase_stage(grid, wave, PhaseKind::Quarter, if special { 3 } else { 1 })?;
    let mut stages = vec![quarter];
    if !special {
        let (next, half) = phase_stage(&out, wave, PhaseKind::Half, 1)?;
        out = next;
        stages.push(half);
        let (next, whole) = phase_stage(&out, wave, PhaseKind::Whole, 1)?;
        out = next;
        stages.push(whole);
    }
    Ok((out, PatternDiagnostics { special, stages }))
}
#[derive(Debug, serde::Serialize)]
pub struct PreparedDiagnostics {
    pub alignment: AnchorDiagnostics,
    pub patterns: PatternDiagnostics,
}
pub fn prepare_pass2(
    grid: &Grid,
    wave: &Waveform,
    sdk_anchor: f64,
) -> Result<(Grid, PreparedDiagnostics), String> {
    let mut out = pass2_tiny_shift(grid)?;
    let alignment = align_anchor(
        &mut out,
        sdk_anchor + f64::from_bits(0xbc7ffffffffffffc),
        true,
    )?;
    let (out, patterns) = pattern_shifts(&out, wave)?;
    Ok((
        out,
        PreparedDiagnostics {
            alignment,
            patterns,
        },
    ))
}
#[cfg(test)]
mod tests {
    use super::*;
    fn grid() -> Grid {
        Grid::new(vec![0., 0.5, 1., 1.5], 120., 2.1).unwrap()
    }
    #[test]
    fn uniform_coordinate_interpolation_is_fractional() {
        let g = grid();
        assert_eq!(g.time(0.5).unwrap(), 0.25);
        assert_eq!(g.position(0.25).unwrap(), 0.5);
    }
    #[test]
    fn setting_downbeat_moves_origin_without_moving_marks() {
        let mut g = grid();
        let original = g.times.clone();
        g.set_downbeat(0.25).unwrap();
        assert_eq!(g.origin, 0.5);
        assert_eq!(g.times, original);
        assert_eq!(g.time(0.).unwrap(), 0.25);
    }
    #[test]
    fn negative_fraction_uses_truncating_first_interval() {
        assert_eq!(grid().time(-0.25).unwrap(), -0.125);
    }
    #[test]
    fn anchor_backtracks_sixteen_and_accepts_exact_negative_point_two() {
        let mut g = Grid::new(vec![-0.2, 0.8], 60., 20.).unwrap();
        let anchor = g.time(16.).unwrap();
        let d = align_anchor(&mut g, anchor, false).unwrap();
        assert_eq!(d.selected_anchor, -0.2);
        assert_eq!(d.attempted_coordinates, vec![0., -16.]);
    }
    #[test]
    fn anchor_below_negative_point_two_is_not_accepted() {
        let first = f64::from_bits(0xbfc999999999999b);
        let mut g = Grid::new(vec![first, first + 1.], 60., 20.).unwrap();
        let anchor = g.time(16.).unwrap();
        let d = align_anchor(&mut g, anchor, false).unwrap();
        assert_eq!(d.selected_anchor, anchor);
    }
    #[test]
    fn anchor_negative_coordinate_cap_restores_supplied_anchor() {
        let mut g = Grid::new(vec![200., 200.5, 201., 201.5], 120., 202.).unwrap();
        let d = align_anchor(&mut g, 200., false).unwrap();
        assert_eq!(d.selected_anchor, 200.);
        assert_eq!(d.attempted_coordinates.last(), Some(&-272.));
    }
    #[test]
    fn force_alignment_uses_point_zero_one_bias_not_nearest_half() {
        let mut g = grid();
        let d = align_anchor(&mut g, 0.375, true).unwrap();
        assert_eq!(d.shift, 0.375);
        assert_eq!(g.times[0], 0.375);
    }
    #[test]
    fn tiny_shift_keeps_exact_negative_literal_bits() {
        assert_eq!(
            pass2_tiny_shift(&grid()).unwrap().times[0].to_bits(),
            0xbc7ffffffffffffc
        );
    }
    #[test]
    fn final_resampling_extends_to_duration_with_strict_endpoint() {
        let g = grid();
        assert_eq!(
            final_resample(&g).unwrap().times,
            vec![0., 0.5, 1., 1.5, 2.]
        );
        let g = Grid::new(g.times, 120., 2.).unwrap();
        assert_eq!(final_resample(&g).unwrap().times.len(), 4);
    }
    #[test]
    fn nonuniform_tail_uses_actual_last_interval_not_tempo_period() {
        let g = Grid::new(vec![0., 0.4, 1., 1.7], 120., 2.5).unwrap();
        assert_eq!(
            final_resample(&g).unwrap().times,
            vec![0., 0.4, 1., 1.7, 2.4]
        );
    }
    fn wave(value: f32) -> Waveform {
        Waveform::new(
            1000.,
            500,
            vec![vec![0.; 8], vec![value; 8], vec![0.; 8], vec![value; 8]],
        )
        .unwrap()
    }
    #[test]
    fn plateau_generalized_mean_of_one_with_unit_rate_scale_is_one() {
        let w = wave(1.);
        assert_eq!(
            w.weighted_average(1, [0., 0., 2., 2.], 0., 1.2).unwrap(),
            1.
        );
    }
    #[test]
    fn pass1_accepts_source_half_beat_when_synthetic_onsets_land_there() {
        let (g, d) = pass1_half_shift(&grid(), &wave(1.)).unwrap();
        assert!(d.accepted);
        assert_eq!(d.shift, 0.25);
        assert_eq!(g.times[0], 0.25);
    }
    #[test]
    fn pass1_zero_scores_preserve_unshifted_marks_on_tie() {
        let original = grid();
        let (g, d) = pass1_half_shift(&original, &wave(0.)).unwrap();
        assert!(!d.accepted);
        assert_eq!(g.times, original.times);
    }
    #[test]
    fn residue_search_selects_only_source_candidates_and_keeps_first_tie() {
        assert_eq!(select_residue(&[1., 10., 1., 10.], &[0, 1], 2).0, 1);
        assert_eq!(select_residue(&[0., 100., 10., 100.], &[0, 2], 4).0, 2);
        assert_eq!(select_residue(&[1.; 8], &[0, 1], 2).0, 0);
    }
    #[test]
    fn zero_pattern_features_keep_grid_and_earliest_residues() {
        let original = grid();
        let (g, d) = pattern_shifts(&original, &wave(0.)).unwrap();
        assert_eq!(g.times, original.times);
        assert_eq!(d.stages.len(), 3);
        assert!(
            d.stages
                .iter()
                .all(|s| s.selected_residue == 0 && s.shift == 0.)
        );
    }
    #[test]
    fn special_tempo_skips_pass1_and_half_whole_but_uses_feature_three() {
        let p = 60. / 88.;
        let g = Grid::new((0..4).map(|i| i as f64 * p).collect(), 88., 3.).unwrap();
        assert!(pass1_half_shift(&g, &wave(1.)).unwrap().1.skipped_special);
        let (_, d) = pattern_shifts(&g, &wave(0.)).unwrap();
        assert!(d.special);
        assert_eq!(d.stages.len(), 1);
        assert_eq!(d.stages[0].feature_index, 3);
    }
    #[test]
    fn default_whole_patterns_are_zero_even_with_nonzero_features() {
        let (_, d) = pattern_shifts(&grid(), &wave(1.)).unwrap();
        let s = &d.stages[2];
        assert_eq!(s.shift, 0.);
        assert!(s.pattern_scores.iter().all(|v| *v == 0.));
    }
    #[test]
    fn constructors_reject_invalid_or_excessively_normalized_geometry() {
        assert!(Grid::new(vec![0., 1.], 1e12, 1.).is_err());
        assert!(Waveform::new(44100., 1024, vec![]).is_err());
        assert!(Waveform::new(44100., 1024, vec![vec![0.; 2], vec![0.; 1]]).is_err());
        assert!(Waveform::new(44100., 0, vec![vec![], vec![]]).is_err());
    }
    #[test]
    fn final_extension_rejects_projected_excess_before_loop_allocation() {
        let g = Grid::new(vec![0., 0.5, 1., 1.5, 1.500000000001], 120., 2.1).unwrap();
        assert!(final_resample(&g).is_err());
    }
    #[test]
    fn weighted_budget_rejects_clustered_grid_before_score_computation() {
        let mut times = (0..2999).map(|i| i as f64 / 3000.).collect::<Vec<_>>();
        times.push(1499.5);
        let g = Grid::new(times, 120., 1500.).unwrap();
        let w = Waveform::new(96000., 2, vec![vec![0.; 100000], vec![0.; 100000]]).unwrap();
        assert!(pass1_half_shift(&g, &w).is_err());
    }
    #[test]
    fn public_mutated_shape_and_nonfinite_queries_return_errors() {
        let mut g = grid();
        assert!(align_anchor(&mut g, f64::NAN, true).is_err());
        g.times.clear();
        assert!(g.time(0.).is_err());
        let mut w = wave(1.);
        w.arrays.clear();
        assert!(w.weighted_average(1, [0., 0., 1., 1.], 0., 1.2).is_err());
    }
}
