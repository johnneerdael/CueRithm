//! Mathematical SDK refinement stage conditioned on explicit waveform and legacy marks.
//! No original component, audio decoder, reference outputs, or network access.
use crate::dsp::beat_refinement_full_rust_v3::tempo::{TempoInput, fix_tempo_range, refine_tempo};
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FullInput {
    pub sample_rate: f64,
    pub hop: i32,
    pub samples: Vec<f32>,
    pub flags: Vec<u32>,
    pub melody_samples: Vec<f32>,
    pub melody_flags: Vec<u32>,
    pub legacy_beats: Vec<f64>,
    #[serde(default)]
    pub legacy_beat_bits: Vec<String>,
    pub legacy_tempo: f64,
    pub duration: f64,
    #[serde(default)]
    pub audio_frames: u64,
}

#[derive(Debug)]
pub struct Beats {
    pub times: Vec<f64>,
    pub tempo: f64,
    pub duration: f64,
    pub downbeat: f64,
    pub origin: f64,
}
impl Beats {
    pub fn new(mut times: Vec<f64>, tempo: f64, duration: f64) -> Result<Self, String> {
        let projected =
            crate::dsp::beat_refinement_full_rust_v3::bounds::projected_normalized_count(
                &times, tempo,
            )?;
        if !duration.is_finite()
            || duration.abs() > crate::dsp::beat_refinement_full_rust_v3::bounds::MAX_ABS_TIME
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
        Ok(Self {
            times,
            tempo,
            duration,
            downbeat,
            origin: 0.0,
        })
    }
    fn half_beats(&self) -> bool {
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
    fn position(&self, t: f64) -> f64 {
        self.position_internal(t) - self.origin
    }
    fn time(&self, coordinate: f64) -> f64 {
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
    fn set_downbeat(&mut self, t: f64) {
        self.downbeat = t;
        self.origin = self.position_internal(t);
    }
}

pub fn uniform_grid(
    period: f64,
    phase: f64,
    fs: f64,
    hop: i32,
    legacy: &[f64],
    mut duration: f64,
) -> Result<Vec<f64>, String> {
    crate::dsp::beat_refinement_full_rust_v3::bounds::validate_waveform(
        fs,
        hop,
        duration,
        &[],
        &[],
    )?;
    crate::dsp::beat_refinement_full_rust_v3::bounds::projected_normalized_count(legacy, 0.0)?;
    if !period.is_finite()
        || period <= 0.0
        || period > crate::dsp::beat_refinement_full_rust_v3::bounds::MAX_ABS_TIME
        || !phase.is_finite()
        || !(0.0..=1.0).contains(&phase)
    {
        return Err("grid period/phase violates bounded contract".into());
    }
    let tempo = 60.0 / period;
    let mut anchor = phase * period;
    if tempo != 0.0 {
        anchor = (1.55 / fs).mul_add(hop as f64, anchor);
    }
    let first = legacy.first().copied().unwrap_or(0.0);
    let q = (((first - anchor) / period) + 0.375).floor();
    anchor = q.mul_add(period, anchor);
    let steps = ((-0.0001 - anchor) / period).ceil().max(0.0);
    if !anchor.is_finite()
        || !steps.is_finite()
        || steps > crate::dsp::beat_refinement_full_rust_v3::bounds::MAX_NORMALIZED_BEATS as f64
    {
        return Err("grid anchor adjustment exceeds bounded count".into());
    }
    let mut advances = 0;
    while anchor < -0.0001 {
        let next = anchor + period;
        advances += 1;
        if !next.is_finite()
            || next <= anchor
            || advances > crate::dsp::beat_refinement_full_rust_v3::bounds::MAX_NORMALIZED_BEATS
        {
            return Err("grid anchor cannot progress within bounded count".into());
        }
        anchor = next;
    }
    if duration <= 0.0 && !legacy.is_empty() {
        duration = legacy[legacy.len() - 1];
    }
    let last = legacy
        .last()
        .copied()
        .unwrap_or_else(|| period.mul_add(-0.5, duration));
    let limit = period.mul_add(0.5, last);
    let end = if duration > 0.0 {
        (duration + 0.0001).min(limit)
    } else {
        limit
    };
    let projected = ((end - anchor) / period).floor().max(0.0) + 3.0;
    if !projected.is_finite()
        || projected > crate::dsp::beat_refinement_full_rust_v3::bounds::MAX_NORMALIZED_BEATS as f64
    {
        return Err("projected grid count exceeds bounded maximum".into());
    }
    let mut out = Vec::new();
    for i in 0..crate::dsp::beat_refinement_full_rust_v3::bounds::MAX_NORMALIZED_BEATS {
        let t = period.mul_add(i as f64, anchor);
        if i >= 2 && ((t + (-0.0001) > duration && duration > 0.0) || t > limit) {
            return Ok(out);
        }
        if !t.is_finite()
            || t.abs() > crate::dsp::beat_refinement_full_rust_v3::bounds::MAX_ABS_TIME
        {
            return Err("generated grid time exceeds bounded numeric contract".into());
        }
        out.push(t);
    }
    Err("bounded stage grid exceeds one million points".into())
}
fn extend(input: Beats) -> Result<Beats, String> {
    if input.times.len() < 5 {
        return Ok(input);
    }
    let tempo = if input.half_beats() {
        input.tempo + input.tempo
    } else {
        input.tempo
    };
    let temp = Beats::new(input.times.clone(), tempo, input.duration)?;
    let mean4 = (temp.time(4.0) - temp.time(0.0)) * 0.25;
    if mean4 <= 0.01 {
        return Ok(input);
    }
    let t0 = temp.time(0.0);
    let projected = (t0 / mean4 + 0.04).floor();
    if !projected.is_finite()
        || projected
            > crate::dsp::beat_refinement_full_rust_v3::bounds::MAX_NORMALIZED_BEATS
                .saturating_sub(temp.times.len()) as f64
    {
        return Err("extension exceeds bounded count".into());
    }
    let count = projected as i32;
    if count < 1 {
        return Ok(input);
    }
    let mut times = Vec::with_capacity(temp.times.len() + count as usize);
    for i in (1..=count).rev() {
        times.push((-(i as f64)).mul_add(mean4, t0));
    }
    times.extend(temp.times);
    let mut out = Beats::new(times, input.tempo, input.duration)?;
    out.set_downbeat(input.downbeat);
    Ok(out)
}
fn upsample_i16(input: &[i16]) -> Vec<i16> {
    let mut out = vec![0; input.len() * 4];
    if input.is_empty() {
        return out;
    }
    for (i, &current) in input.iter().enumerate() {
        out[4 * i] = current;
        let previous = input[i.saturating_sub(1)] as f64;
        let next = input[(i + 1).min(input.len() - 1)] as f64;
        let next_next = input[(i + 2).min(input.len() - 1)] as f64;
        for j in 1..=3 {
            let t = j as f64 * 0.25;
            let t2 = t * t;
            let t3 = t * t2;
            let three = t3 * 3.0;
            let a = ((-2.0_f64).mul_add(t2, t) + t3) * -0.5;
            let b = ((5.0_f64).mul_add(t2, -2.0) - three) * -0.5;
            let c = ((-4.0_f64).mul_add(t2, -t) + three) * -0.5;
            let d = (t2 - t3) * -0.5;
            let y = current as f64 * b;
            let y = previous.mul_add(a, y);
            let y = next.mul_add(c, y);
            let y = next_next.mul_add(d, y);
            out[4 * i + j] = y.clamp(0.0, 32767.0) as i32 as i16;
        }
    }
    out
}
fn envelope(signal: &mut [i16]) {
    let mut anchor = 0.0_f64;
    let mut age = 0.0_f64;
    for value in signal {
        let width = anchor.mul_add(3.0, 20.0).min(90.0) * 0.75;
        let u = age / width + 0.3;
        let decay = if u >= 0.0 {
            (-u).mul_add(u, 1.0).max(0.0).sqrt() * 1.17
        } else {
            1.0
        }
        .min(1.0);
        let sample = *value as f64 * 0.00390625;
        let attack = sample > anchor * decay;
        let chosen = if attack {
            anchor = sample;
            1.0
        } else {
            decay
        };
        let v = anchor * chosen + 0.1;
        *value = ((v * 256.0) as i32).min(32767) as i16;
        age = if attack { 1.0 } else { age + 1.0 };
    }
}
pub fn quiet_start(spans: &[f64], limit: f64) -> f64 {
    for k in (0..=8).rev() {
        if k as f64 <= limit {
            let max = spans[..2 * k].iter().copied().fold(0.0, f64::max);
            if max < 0.1 && spans[2 * k] > 50.0 {
                return k as f64;
            }
        }
    }
    0.0
}
fn loudness(b: &Beats, input: &FullInput, start: f64, end: f64) -> Result<f64, String> {
    let t0 = b.time(start);
    let t1 = b.time(end);
    let start_index = ((t0 * input.sample_rate) / (input.hop as f64)).ceil();
    let end_index = ((t1 * input.sample_rate) / (input.hop as f64)).floor();
    if !start_index.is_finite()
        || !end_index.is_finite()
        || start_index < i32::MIN as f64
        || start_index > i32::MAX as f64
        || end_index < i32::MIN as f64
        || end_index > i32::MAX as f64
    {
        return Err("loudness coordinates exceed bounded i32 conversion".into());
    }
    let first = start_index.max(0.0) as usize;
    let last = (end_index as i32).min(input.melody_samples.len() as i32);
    if last <= first as i32 {
        return Ok(0.0);
    }
    let mut sum = 0.0;
    for &v in &input.melody_samples[first..last as usize] {
        sum += v as f64;
    }
    Ok(sum / (t1 - t0))
}
fn trim_coordinate(b: &Beats, input: &FullInput) -> Result<(f64, usize), String> {
    if input.hop <= 0 || b.times.len() < 2 {
        return Ok((0.0, 0));
    }
    let count = (((input.sample_rate * 20.0) / (input.hop as f64)) as i32)
        .min(input.samples.len() as i32)
        .max(0) as usize;
    let mut raw = vec![0_i16; count];
    for (j, out) in raw.iter_mut().enumerate().skip(64) {
        let i = j - 64;
        let v = ((input.samples[i] as f64 * 256.0) as i32).min(32767);
        *out = if input.flags[i] != 0 { v as i16 } else { 0 };
    }
    let mut signal = upsample_i16(&raw);
    envelope(&mut signal);
    let mut peaks = Vec::new();
    for i in 1..signal.len().saturating_sub(1) {
        if signal[i - 1] < signal[i] && signal[i + 1] <= signal[i] && signal[i] as f64 >= 3276.8 {
            peaks.push((i as f64 + 6.0, signal[i] as f64));
        }
    }
    let subdivision = if b.tempo < 158.0 { 4.0 } else { 2.0 };
    for (index, height) in &mut peaks {
        let time = (((*index * 0.25) + (-64.0)) * input.hop as f64) / input.sample_rate;
        let q = b.position(time);
        let snap = q.mul_add(subdivision, 0.5).floor() / subdivision;
        if (snap - q).abs() > 0.08 {
            *height = 0.0;
        }
        let time = b.time(snap);
        *index = ((input.sample_rate * time) / (input.hop as f64) + 64.0) * 4.0;
    }
    peaks.retain(|p| p.1 > 0.0);
    let first_time = peaks
        .first()
        .map(|p| ((p.0 * 0.25 + (-64.0)) * input.hop as f64) / input.sample_rate)
        .unwrap_or(0.0);
    let q = b.position(first_time);
    let nearest = (q + 0.5).floor();
    if !peaks.is_empty() && first_time <= 18.0 && (nearest - q).abs() <= 0.08 {
        let mut trim = nearest;
        while trim >= 16.0 {
            if loudness(b, input, trim - 15.0, trim - 1.0)? < 10.0 {
                break;
            }
            trim -= 16.0;
        }
        while trim >= 8.0 {
            if loudness(b, input, trim - 7.0, trim - 1.0)? < 20.0 {
                break;
            }
            trim -= 8.0;
        }
        return Ok((trim, peaks.len()));
    }
    let spans = (0..32)
        .map(|i| loudness(b, input, i as f64 * 0.5, (i + 1) as f64 * 0.5))
        .collect::<Result<Vec<_>, String>>()?;
    Ok((
        quiet_start(&spans, if peaks.is_empty() { 512.0 } else { q }),
        peaks.len(),
    ))
}
fn resample_trim(b: &Beats, start: f64) -> Result<Vec<f64>, String> {
    let n = b.times.len();
    let count = (n as f64 - start).floor() as usize;
    if count > crate::dsp::beat_refinement_full_rust_v3::bounds::MAX_NORMALIZED_BEATS {
        return Err("trim resample count exceeds bounded maximum".into());
    }
    let mut out = Vec::with_capacity(count);
    for i in 0..count {
        let x = start + i as f64;
        let j = (x.floor() as i32).min(n as i32 - 2);
        let (frac, lo, hi) = if j >= 0 {
            (x - j as f64, b.times[j as usize], b.times[j as usize + 1])
        } else {
            let lo = b.times[0];
            (
                if j == -1 { x - j as f64 } else { x + 1.0 },
                lo,
                lo + (lo - b.times[1]),
            )
        };
        out.push((hi - lo).mul_add(frac, lo));
    }
    Ok(out)
}
#[derive(Debug, Serialize)]
pub struct FullResult {
    pub scope: &'static str,
    pub branch: &'static str,
    pub tempo: f64,
    pub tempo_bits: String,
    pub beat_times: Vec<f64>,
    pub beat_bits: Vec<String>,
    pub downbeat: f64,
    pub downbeat_bits: String,
    pub trim_coordinate: f64,
    pub surviving_transient_peaks: usize,
    pub initial_normalized_count: usize,
}
pub fn refine_full(input: &FullInput) -> Result<FullResult, String> {
    crate::dsp::beat_refinement_full_rust_v3::bounds::validate_waveform(
        input.sample_rate,
        input.hop,
        input.duration,
        &input.samples,
        &input.flags,
    )?;
    crate::dsp::beat_refinement_full_rust_v3::bounds::validate_waveform(
        input.sample_rate,
        input.hop,
        input.duration,
        &input.melody_samples,
        &input.melody_flags,
    )?;
    crate::dsp::beat_refinement_full_rust_v3::bounds::validate_legacy_entry(
        &input.legacy_beats,
        input.legacy_tempo,
    )?;
    if !input.legacy_beat_bits.is_empty()
        && (input.legacy_beat_bits.len() != input.legacy_beats.len()
            || input
                .legacy_beat_bits
                .iter()
                .zip(&input.legacy_beats)
                .any(|(s, v)| format!("{:016x}", v.to_bits()) != *s))
    {
        return Err("legacy bit witnesses differ".into());
    }
    let stage = TempoInput {
        sample_rate: input.sample_rate,
        hop: input.hop,
        samples: input.samples.clone(),
        flags: input.flags.clone(),
        legacy_beats: input.legacy_beats.clone(),
        legacy_tempo: input.legacy_tempo,
        duration: input.duration,
    };
    let tempo = refine_tempo(&stage)?;
    let legacy = Beats::new(
        input.legacy_beats.clone(),
        input.legacy_tempo,
        input.duration,
    )?;
    let rebuilt = if let (Some(p), Some(f)) = (tempo.fitted_period_frames, tempo.fitted_phase) {
        let seconds = ((1.0 / input.sample_rate) * input.hop as f64) * p;
        let duration = if input.duration <= 0.0 && input.audio_frames != 0 {
            input.audio_frames as f64 / input.sample_rate
        } else {
            input.duration
        };
        Beats::new(
            uniform_grid(
                seconds,
                f,
                input.sample_rate,
                input.hop,
                &legacy.times,
                duration,
            )?,
            tempo.tempo,
            if duration <= 0.0 {
                legacy.times.last().copied().unwrap_or(duration)
            } else {
                duration
            },
        )?
    } else {
        legacy
    };
    let e = extend(rebuilt)?;
    let doubled = if e.half_beats() {
        e.tempo + e.tempo
    } else {
        e.tempo
    };
    let w = Beats::new(e.times.clone(), doubled, e.duration)?;
    let c = Beats::new(w.times.clone(), fix_tempo_range(w.tempo)?, w.duration)?;
    let (trim, peaks) = trim_coordinate(&c, input)?;
    let v = if trim >= -1.0 && trim + 2.0 < c.times.len() as f64 {
        Beats::new(resample_trim(&c, trim)?, w.tempo, c.duration)?
    } else {
        w
    };
    let out = Beats::new(v.times, e.tempo, e.duration)?;
    Ok(FullResult {
        scope: "feature+original-legacy conditioned SDK FixBeatGrid stage; not independent PCM/BPM or wireDownbeat inference",
        branch: tempo.branch,
        tempo: out.tempo,
        tempo_bits: format!("{:016x}", out.tempo.to_bits()),
        beat_bits: out
            .times
            .iter()
            .map(|v| format!("{:016x}", v.to_bits()))
            .collect(),
        downbeat: out.downbeat,
        downbeat_bits: format!("{:016x}", out.downbeat.to_bits()),
        beat_times: out.times,
        trim_coordinate: trim,
        surviving_transient_peaks: peaks,
        initial_normalized_count: tempo.initial_normalized_count,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn constructor_decimates_double_rate_without_changing_tempo() {
        let b = Beats::new((0..9).map(|i| i as f64 * 0.25).collect(), 120.0, 3.0).unwrap();
        assert_eq!(b.times, vec![0., 0.5, 1., 1.5, 2.]);
        assert_eq!(b.tempo, 120.);
    }
    #[test]
    fn zero_candidate_fallback_still_has_latency_correction() {
        let g = uniform_grid(0.46875, 0., 44100., 1024, &[], 1.).unwrap();
        assert_eq!(
            g[0].to_bits(),
            (1.55_f64 / 44100.).mul_add(1024., 0.).to_bits()
        );
    }
    #[test]
    fn quiet_prefix_uses_two_halfbeat_spans_per_beat() {
        let mut spans = vec![0.; 32];
        spans[3] = 1.;
        spans[8] = 60.;
        assert_eq!(quiet_start(&spans, 8.), 0.);
        spans[3] = 0.;
        assert_eq!(quiet_start(&spans, 8.), 4.);
    }
    #[test]
    fn constructor_inserts_midpoints_for_half_rate_marks() {
        let b = Beats::new(vec![0., 1., 2.], 120., 3.).unwrap();
        assert_eq!(b.times, vec![0., 0.5, 1., 1.5, 2.]);
    }
    #[test]
    fn half_beat_rule_excludes_exactly_one_hundred() {
        assert!(Beats::new(vec![], 99.99, 0.).unwrap().half_beats());
        assert!(!Beats::new(vec![], 100., 0.).unwrap().half_beats());
        assert!(!Beats::new(vec![], 0., 0.).unwrap().half_beats());
    }
    #[test]
    fn cubic_i16_preserves_samples_and_truncates_fractional_overshoot() {
        assert_eq!(upsample_i16(&[0, 16]), vec![0, 3, 8, 12, 16, 17, 17, 16]);
        assert!(upsample_i16(&[]).is_empty());
    }
    #[test]
    fn quiet_start_rejects_equal_attack_and_quiet_thresholds() {
        let mut spans = vec![0.; 32];
        spans[8] = 50.;
        assert_eq!(quiet_start(&spans, 8.), 0.);
        spans[8] = 60.;
        spans[1] = 0.1;
        assert_eq!(quiet_start(&spans, 8.), 0.);
    }
    #[test]
    fn time_extrapolates_negative_fraction_using_first_interval() {
        let b = Beats::new(vec![1., 1.5, 2.], 120., 3.).unwrap();
        assert_eq!(b.time(-0.5), 0.75);
        assert_eq!(b.time(3.), 2.5);
        assert_eq!(b.position(2.5), 3.);
    }
    #[test]
    fn extension_restores_original_downbeat_origin() {
        let b = Beats::new(vec![2.1, 2.6, 3.1, 3.6, 4.1], 120., 5.).unwrap();
        let e = extend(b).unwrap();
        assert_eq!(e.times.len(), 9);
        assert_eq!(e.downbeat, 2.1);
        assert_eq!(e.origin, 4.);
        // (4.1-2.1)*.25 is below .5; the fused prefix is above .1.
        assert_eq!(e.times[0].to_bits(), 0x3fb99999999999c0);
    }
    #[test]
    fn uniform_grid_always_emits_first_two_points() {
        assert_eq!(
            uniform_grid(0.5, 0., 44100., 1024, &[], 0.).unwrap().len(),
            2
        );
    }
    fn numeric_input() -> FullInput {
        FullInput {
            sample_rate: 1.,
            hop: 1,
            samples: vec![],
            flags: vec![],
            melody_samples: vec![1., 2., 3., 4., 5.],
            melody_flags: vec![1; 5],
            legacy_beats: vec![],
            legacy_beat_bits: vec![],
            legacy_tempo: 120.,
            duration: 5.,
            audio_frames: 0,
        }
    }
    #[test]
    fn loudness_uses_ceil_start_floor_end_and_time_duration() {
        let b = Beats::new(vec![1.2, 3.8], 60. / 2.6, 5.).unwrap();
        let input = numeric_input();
        assert_eq!(
            loudness(&b, &input, 0., 1.).unwrap().to_bits(),
            (3.0_f64 / (3.8 - 1.2)).to_bits()
        );
    }
    #[test]
    fn worker_rejects_expected_fields() {
        let text = r#"{"sample_rate":44100,"hop":1024,"samples":[],"flags":[],"melody_samples":[],"melody_flags":[],"legacy_beats":[],"legacy_tempo":120,"duration":0,"expected_tempo":120}"#;
        assert!(serde_json::from_str::<FullInput>(text).is_err());
    }
    #[test]
    fn dangerous_tempo_is_rejected_before_any_legacy_normalization() {
        let mut input = numeric_input();
        input.sample_rate = 44100.;
        input.hop = 1024;
        input.legacy_beats = vec![0., 1.];
        input.legacy_tempo = 1e12;
        assert!(refine_full(&input).is_err());
        assert!(Beats::new(vec![0., 1.], 1e12, 1.).is_err());
        let stage = TempoInput {
            sample_rate: 44100.,
            hop: 1024,
            samples: vec![],
            flags: vec![],
            legacy_beats: vec![0., 1.],
            legacy_tempo: 1e12,
            duration: 1.,
        };
        assert!(refine_tempo(&stage).is_err());
    }
    #[test]
    fn public_grid_rejects_invalid_period_and_huge_anchor_step_count() {
        for period in [0., -1., f64::NAN, f64::INFINITY] {
            assert!(uniform_grid(period, 0., 44100., 1024, &[], 1.).is_err());
        }
        assert!(uniform_grid(1e-300, 0., 44100., 1024, &[-1., 0.], 1.).is_err());
    }
    #[test]
    fn extension_checks_prefix_count_before_allocating_or_inserting() {
        let b = Beats::new(
            vec![80000., 80000.02, 80000.04, 80000.06, 80000.08],
            0.,
            86400.,
        )
        .unwrap();
        assert!(extend(b).is_err());
    }
    #[test]
    fn loudness_rejects_out_of_range_indices_before_cast_or_slice() {
        let b = Beats::new(vec![0., 0.5], 120., 1.).unwrap();
        let mut input = numeric_input();
        input.sample_rate = 44100.;
        input.hop = 1024;
        assert!(loudness(&b, &input, 1e30, 1e30 + 1.).is_err());
    }
}
