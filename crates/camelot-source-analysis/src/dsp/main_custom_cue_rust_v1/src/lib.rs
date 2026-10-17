//! Source-default Main custom pass0 arithmetic, separate from the SDK final curve.
use serde::{Deserialize, Serialize};
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ArrayInput {
    pub amplitudes: Vec<f32>,
    pub amplitude_bits: Option<Vec<String>>,
}
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WaveformInput {
    pub sample_rate: f64,
    pub hop: f64,
    pub arrays: Vec<ArrayInput>,
}
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BeatInput {
    pub beat_times_seconds: Vec<f64>,
    pub beat_time_bits: Option<Vec<String>>,
    pub tempo: f64,
    pub tempo_bits: Option<String>,
    pub duration_seconds: f64,
    pub duration_bits: Option<String>,
    pub downbeat_time: f64,
    pub downbeat_bits: Option<String>,
}
pub type Granule = [f64; 5];

#[derive(Debug, Serialize)]
pub struct CustomOutput {
    pub scope: &'static str,
    pub origin: i32,
    pub end: i32,
    pub onset_index: usize,
    pub minimum_start_index: usize,
    pub raw_score_bits: Vec<String>,
    pub phase_cost_bits: Vec<String>,
    pub selected_phase: usize,
    pub selected_normalized_coordinate: f64,
    pub selected_time: f64,
    pub selected_time_bits: String,
}
struct Context {
    times: Vec<f64>,
    duration: f64,
    offset: f64,
}
fn scalar(value: f64, witness: &Option<String>) -> Result<f64, String> {
    if let Some(s) = witness {
        if s.len() != 16 || !s.bytes().all(|x| x.is_ascii_hexdigit()) {
            return Err("invalid binary64 witness".into());
        }
        let raw = u64::from_str_radix(s, 16).map_err(|e| e.to_string())?;
        if raw != value.to_bits() {
            return Err("numeric/binary64 witness differs".into());
        }
    }
    Ok(value)
}
impl Context {
    fn new(b: &BeatInput) -> Result<Self, String> {
        let times = b.beat_times_seconds.clone();
        if times.len() < 2
            || times.len() > 100000
            || times.iter().any(|x| !x.is_finite() || x.abs() > 86400.0)
            || times.windows(2).any(|p| p[0] >= p[1])
        {
            return Err("bounded increasing beats required".into());
        }
        if let Some(bits) = &b.beat_time_bits {
            if bits.len() != times.len() {
                return Err("beat bit count differs".into());
            }
            for (value, witness) in times.iter().zip(bits) {
                scalar(*value, &Some(witness.clone()))?;
            }
        }
        let tempo = scalar(b.tempo, &b.tempo_bits)?;
        let duration = scalar(b.duration_seconds, &b.duration_bits)?;
        let downbeat = scalar(b.downbeat_time, &b.downbeat_bits)?;
        if !tempo.is_finite()
            || !(0.0..=1000.0).contains(&tempo)
            || tempo == 0.0
            || !duration.is_finite()
            || !(0.0..=86400.0).contains(&duration)
            || !downbeat.is_finite()
            || downbeat.abs() > 86400.0
        {
            return Err("bounded finite beat scalars required".into());
        }
        let mut out = Self {
            times,
            duration,
            offset: 0.0,
        };
        out.offset = out.position_internal(downbeat);
        if !out.offset.is_finite() || out.offset.abs() > 1000000.0 {
            return Err("normalized origin outside bounded domain".into());
        }
        Ok(out)
    }
    fn position_internal(&self, t: f64) -> f64 {
        let b = &self.times;
        let n = b.len();
        if t < b[0] {
            return (t - b[0]) / (b[1] - b[0]);
        }
        if t > b[n - 1] {
            return ((t - b[n - 1]) / (b[n - 1] - b[n - 2]) + n as f64) + (-1.0);
        }
        let i = b.partition_point(|x| *x < t).saturating_sub(1);
        (t - b[i]) / (b[i + 1] - b[i]) + i as f64
    }
    fn position(&self, t: f64) -> f64 {
        self.position_internal(t) - self.offset
    }
    fn time(&self, coordinate: f64) -> f64 {
        let q = coordinate / 1.0 + self.offset;
        let i = q.trunc() as isize;
        let b = &self.times;
        let n = b.len();
        if i < 0 {
            return q.mul_add(b[1] - b[0], b[0]);
        }
        if i as usize + 1 >= n {
            return ((q - n as f64) + 1.0).mul_add(b[n - 1] - b[n - 2], b[n - 1]);
        }
        let i = i as usize;
        (q - i as f64).mul_add(b[i + 1] - b[i], b[i])
    }
}
pub fn predict(wave: &WaveformInput, beats: &BeatInput) -> Result<CustomOutput, String> {
    let context = Context::new(beats)?;
    if wave.arrays.len() < 4 || wave.arrays.len() > 8 {
        return Err("four to eight Main channels required".into());
    }
    let shape = channel(wave, 0)?.len();
    if shape > 100000 || (1..4).any(|i| channel(wave, i).map_or(true, |a| a.len() != shape)) {
        return Err("Main channel shape differs".into());
    }
    let position = context.position(context.duration);
    if !position.is_finite() || position.abs() > 999000.0 {
        return Err("duration coordinate outside bounded domain".into());
    }
    let origin = -32;
    let last = (position + 0.5).floor() as i32;
    let end = ((last + 16) / 32) * 32;
    if end - origin > 100000 {
        return Err("Main granule count outside bounded domain".into());
    }
    if end <= origin || last < 16 {
        return Ok(CustomOutput {
            scope: "source short-range Main result: curve/anchor computation skipped",
            origin,
            end,
            onset_index: 0,
            minimum_start_index: 0,
            raw_score_bits: vec![],
            phase_cost_bits: vec![],
            selected_phase: 0,
            selected_normalized_coordinate: 0.0,
            selected_time: 0.0,
            selected_time_bits: "0000000000000000".into(),
        });
    }
    let mut granules = Vec::with_capacity((end - origin + 32) as usize);
    for i in origin..end {
        granules.push(granule(
            wave,
            context.time(i as f64),
            context.time(i as f64 + 1.0),
        )?);
    }
    granules.extend([[0.0; 5]; 32]);
    let (onset, minimum) = onset_index(&granules, &context, origin)?;
    let mut scores = vec![0.0; granules.len()];
    if onset + 96 < scores.len() {
        for (i, value) in scores
            .iter_mut()
            .enumerate()
            .take(granules.len() - 80)
            .skip(onset + 16)
        {
            *value = score_at(&granules, i)?;
        }
    }
    let (phase, costs) = phase_scores(&scores)?;
    let selected = context.time(phase as f64);
    if !selected.is_finite() || costs.iter().any(|x| !x.is_finite()) {
        return Err("nonfinite Main phase result".into());
    }
    Ok(CustomOutput {
        scope: "independent Main custom pass0 from supplied waveform/grid; not complete timing or service cue selection",
        origin,
        end,
        onset_index: onset,
        minimum_start_index: minimum,
        raw_score_bits: scores
            .iter()
            .map(|x| format!("{:016x}", x.to_bits()))
            .collect(),
        phase_cost_bits: costs
            .iter()
            .map(|x| format!("{:016x}", x.to_bits()))
            .collect(),
        selected_phase: phase,
        selected_normalized_coordinate: phase as f64,
        selected_time: selected,
        selected_time_bits: format!("{:016x}", selected.to_bits()),
    })
}

pub fn weighted_average(
    fs: f64,
    hop: f64,
    values: &[f32],
    ends: [f64; 4],
    power: f64,
) -> Result<f64, String> {
    if !fs.is_finite()
        || !(0.0..=384000.0).contains(&fs)
        || fs == 0.0
        || !hop.is_finite()
        || !(1.0..=65536.0).contains(&hop)
        || !power.is_finite()
        || !(0.01..=64.0).contains(&power)
        || ends.iter().any(|x| !x.is_finite() || x.abs() > 86400.0)
        || ends.windows(2).any(|p| p[0] > p[1])
        || values.len() > 100000
        || values.iter().any(|x| !x.is_finite() || x.abs() > 1e10)
    {
        return Err("weighted window exceeds finite bounded domain".into());
    }
    if values.is_empty() {
        return Ok(0.0);
    }
    let [t1, t2, t3, t4] = ends;
    let begin = ((fs * t1) / hop).floor().max(0.0);
    let end = ((fs * t4) / hop).ceil().min(values.len() as f64);
    let mut total = 0.0;
    for (i, value) in values
        .iter()
        .enumerate()
        .take(end.max(0.0) as usize)
        .skip(begin as usize)
    {
        let value = value.max(0.0) as f64;
        let time = ((i as f64 + 0.5) / fs) * hop;
        let mut weight = 0.0;
        if t1 < time && time < t2 {
            let u = ((time - t1) * 0.5) / (t2 - t1);
            if 0.0 < u && u < 1.0 {
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
            if 0.0 < u && u < 1.0 {
                weight = (u * (std::f64::consts::PI + std::f64::consts::PI))
                    .cos()
                    .mul_add(-0.5, 0.5);
            }
        }
        total = weight.mul_add(value.powf(power), total);
    }
    let edge = ((t2 - t1) + t4) - t3;
    let mut area = (edge.mul_add(0.5, t3 - t2) * fs) / hop;
    if area <= 0.0 {
        area = 1.0;
    }
    let result = ((fs * 0.5) / hop).powf(5.0 / 6.0) * (total / area).powf(1.0 / power);
    if !result.is_finite() {
        return Err("nonfinite weighted result".into());
    }
    Ok(result)
}
fn channel(wave: &WaveformInput, index: usize) -> Result<std::borrow::Cow<'_, [f32]>, String> {
    let a = wave
        .arrays
        .get(index)
        .ok_or("missing Main feature channel")?;
    if let Some(bits) = &a.amplitude_bits {
        if bits.len() > 100000 || (!a.amplitudes.is_empty() && a.amplitudes.len() != bits.len()) {
            return Err("Main channel bit count differs".into());
        }
        let v = bits
            .iter()
            .map(|x| {
                u32::from_str_radix(x, 16)
                    .map(f32::from_bits)
                    .map_err(|e| e.to_string())
            })
            .collect::<Result<Vec<_>, _>>()?;
        if !a.amplitudes.is_empty()
            && !a
                .amplitudes
                .iter()
                .zip(&v)
                .all(|(a, b)| a.to_bits() == b.to_bits())
        {
            return Err("Main channel numeric witness differs".into());
        }
        Ok(std::borrow::Cow::Owned(v))
    } else {
        Ok(std::borrow::Cow::Borrowed(&a.amplitudes))
    }
}
pub fn granule(wave: &WaveformInput, start: f64, end: f64) -> Result<Granule, String> {
    if !start.is_finite() || !end.is_finite() || end < start {
        return Err("invalid granule interval".into());
    }
    let width = end - start;
    let first = [
        width.mul_add(-0.12, start),
        width.mul_add(-0.02, start),
        width.mul_add(-0.12, end),
        width.mul_add(-0.02, end),
    ];
    let second = [
        width.mul_add(-0.18, start),
        width.mul_add(-0.12, start),
        width.mul_add(-0.18, end),
        width.mul_add(-0.12, end),
    ];
    let a = channel(wave, 2)?;
    let b = channel(wave, 3)?;
    let g0 = weighted_average(wave.sample_rate, wave.hop, &a, first, 1.07)?;
    let g2 = weighted_average(wave.sample_rate, wave.hop, &a, first, 4.543)?;
    let g1 = weighted_average(wave.sample_rate, wave.hop, &b, second, 2.636)?;
    let g3 = weighted_average(wave.sample_rate, wave.hop, &b, second, 5.435)?;
    Ok([g0, g1, g2, g3, g0 + g1])
}
fn difference(
    values: &[f64; 128],
    left: usize,
    spacing: usize,
    width: usize,
    negative_gain: f64,
) -> f64 {
    let shifts = width.max(spacing).div_ceil(width);
    let mut result = 0.0;
    for shift in 0..shifts {
        let base = 64 + shift * width;
        let (mut before, mut after) = (0.0, 0.0);
        let mut count = 0;
        for segment in 0..left / spacing {
            for sub in 0..width {
                let p = base + segment * spacing + sub;
                before += values[p - left];
                after += values[p];
                count += 1;
            }
        }
        let delta = (after - before) / count as f64;
        let mut value = (delta * if delta >= 0.0 { 1.0 } else { negative_gain }).abs();
        if delta < 0.0 && negative_gain < 0.0 {
            value = -value;
        }
        result += value;
    }
    result / shifts as f64
}
pub fn score_at(granules: &[Granule], index: usize) -> Result<f64, String> {
    if granules.len() > 100000
        || index > 100000
        || granules
            .iter()
            .flatten()
            .any(|x| !x.is_finite() || x.abs() > 1e100)
    {
        return Err("invalid bounded granules".into());
    }
    let mut primary = [0.0; 128];
    let mut melody = [0.0; 128];
    let mut secondary = [0.0; 128];
    for i in 0..128 {
        let j = index as isize - 64 + i as isize;
        if j >= 0 && (j as usize) < granules.len() {
            let g = granules[j as usize];
            primary[i] = g[0];
            melody[i] = g[3];
            secondary[i] = g[1];
        }
    }
    let mut short = 0.0;
    for ((left, gain), coefficient) in [32, 16, 8, 4]
        .into_iter()
        .zip([0.8, 0.8, 0.9, 0.9])
        .zip([1.0, 1.0, 1.0, 0.0])
    {
        short = (coefficient * 96.0_f64).mul_add(difference(&melody, left, left, 1, gain), short);
    }
    for (values, scale) in [(&primary, 98.304_f64), (&secondary, 245.76)] {
        for (left, coefficient) in [32, 16, 8, 4].into_iter().zip([1.0, 0.0, 0.0, 0.0]) {
            short = (coefficient * scale).mul_add(difference(values, left, 4, 1, 0.4), short);
        }
    }
    let mut long = 0.0;
    for (left, coefficient) in [32, 16, 8, 4].into_iter().zip([3.2_f64, 1.6, 0.8, 0.0]) {
        long = coefficient.mul_add(difference(&melody, left, left, left, -1.0), long);
    }
    if long < 0.0 {
        long *= 0.0;
    }
    Ok(short + long)
}
fn onset_index(
    granules: &[Granule],
    beats: &Context,
    origin: i32,
) -> Result<(usize, usize), String> {
    if granules.len() < 64 {
        return Err("onset stage requires at least64 explicit granules".into());
    }
    let maximum_primary = granules[32..40]
        .iter()
        .map(|g| g[0])
        .fold(f64::NEG_INFINITY, f64::max);
    let maximum_combined = granules[32..40]
        .iter()
        .map(|g| g[4])
        .fold(f64::NEG_INFINITY, f64::max);
    let mut index = 32;
    if maximum_primary < 5.0 && maximum_combined < 7.0 {
        for offset in 0..64.min(granules.len() - 33) {
            let current = 32 + offset;
            if granules[current][0] > maximum_primary.mul_add(1.6, 5.0)
                || granules[current][4] > 60.0
            {
                index = if granules[current + 1][0] > granules[current][0] * 5.0 {
                    current + 1
                } else {
                    current
                };
                break;
            }
        }
    }
    let minimum = ((beats.position(-0.018).ceil() as i32) - origin).max(4) as usize;
    if index + 20 > granules.len() {
        return Err("insufficient onset neighborhood".into());
    }
    let mut dramatic = false;
    for start in [index + 4, index + 12] {
        let window = &granules[start..start + 8];
        let maximum = window[..4]
            .iter()
            .map(|g| g[3])
            .fold(f64::NEG_INFINITY, f64::max);
        dramatic = dramatic || maximum.mul_add(3.0, 40.0) <= window[4][3];
    }
    if beats.time((index as i32 + origin) as f64) >= -0.0001 {
        let current = granules[index];
        let previous = granules[index - 1];
        let previous2 = granules[index - 2];
        let threshold = current[0].mul_add(0.4, 7.0);
        let flag = previous[0] >= threshold
            && current[0] * 0.1 > previous2[0]
            && !(current[3] >= 155.0 && previous[3] < current[3] * 0.5);
        if previous[0] < threshold + threshold || current[0] * 0.1 <= previous2[0] {
            if flag {
                index -= 1;
            }
        } else if current[3] < 165.0 || flag || current[3] * 0.35 <= previous[3] {
            index -= 1;
        }
    }
    let primary_threshold = if dramatic { 70.0 } else { 6.0 };
    while index > minimum {
        let neighborhood = &granules[index - 1..index + 5];
        if neighborhood.iter().any(|g| g[0] < primary_threshold) {
            break;
        }
        let min_melody = neighborhood
            .iter()
            .map(|g| g[3])
            .fold(f64::INFINITY, f64::min);
        let threshold = min_melody.mul_add(1.24, 44.0).max(60.0);
        if granules[index..index + 5].iter().any(|g| g[3] >= threshold) {
            break;
        }
        index -= 1;
    }
    Ok((index, minimum))
}

fn phrase_weight(beat: i32, weights: [f64; 5]) -> f64 {
    let mut factors = [0.0; 5];
    for (i, period) in [32, 16, 8, 4, 2].into_iter().enumerate() {
        let remainder = beat & (period - 1);
        let distance = remainder.min(period - remainder) as f64 * (2.0 / period as f64);
        let adjusted = (weights[i] - 1.0).mul_add(1.0, 1.0);
        factors[i] = (adjusted - 1.0).mul_add(distance.clamp(0.0, 1.0), 1.0);
    }
    ((factors[2] * (factors[0] * factors[1])) * factors[3]) * factors[4]
}
fn hanning_mean(values: &[f64], index: usize, width: f64) -> f64 {
    let span = width + 1.0;
    let begin = (index as f64 - span * 0.5).ceil() as isize;
    let end = (index as f64 + span * 0.5).ceil() as isize;
    let (mut total, mut denominator) = (0.0, 0.0);
    for p in begin..end {
        let u = (p as f64 - index as f64) / span + 0.5;
        let weight = if 0.0 < u && u < 1.0 {
            (u * (std::f64::consts::PI + std::f64::consts::PI))
                .cos()
                .mul_add(-0.5, 0.5)
        } else {
            0.0
        };
        if p >= 0 && (p as usize) < values.len() {
            total = values[p as usize].mul_add(weight, total);
        }
        denominator += weight;
    }
    if denominator > 0.0 {
        total / denominator
    } else {
        0.0
    }
}
pub fn phase_scores(scores: &[f64]) -> Result<(usize, Vec<f64>), String> {
    if scores.len() > 100000 || scores.iter().any(|x| !x.is_finite() || x.abs() > 1e100) {
        return Err("invalid bounded custom curve".into());
    }
    let mut costs = vec![0.0; 32];
    if scores.len() < 96 {
        return Ok((0, costs));
    }
    let end = ((scores.len() - 64) / 32) * 32;
    let mut smooth = Vec::with_capacity(scores.len());
    for i in 0..scores.len() {
        let mut v = 0.0;
        for (width, gain) in [(1.0, 1.5), (3.0, -0.5), (32.0, -1.0)] {
            v = hanning_mean(scores, i, width).mul_add(gain, v);
        }
        smooth.push(if v > 0.0 { v.powf(1.1) } else { 0.0 });
    }
    let bias = [
        0x3feccccccccccccd,
        0x3fed916872b020c5,
        0x3feeb851eb851eb8,
        0x3fef22d0e5604189,
        0x3fef126e978d4fdf,
    ]
    .map(|x| f64::from_bits(x).powf(1.15));
    let mut best = 0;
    let mut best_value = 0.0;
    for (phase, cost) in costs.iter_mut().enumerate() {
        let mut sum = 0.0;
        for (i, value) in smooth.iter().take(end).enumerate() {
            sum = value.mul_add(
                phrase_weight(i as i32 - phase as i32, [0.4, 0.3, 0.1, 0.0, 0.0]),
                sum,
            );
        }
        let w = phrase_weight(phase as i32, bias);
        *cost = sum * (w * w);
        if *cost > best_value {
            best_value = *cost;
            best = phase;
        }
    }
    Ok((best, costs))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn box_window_constant_one_has_exact_unit_generalized_mean() {
        assert_eq!(
            weighted_average(2.0, 1.0, &[1.0; 4], [0.0, 0.0, 1.0, 1.0], 1.07)
                .unwrap()
                .to_bits(),
            1.0_f64.to_bits()
        );
    }
    #[test]
    fn complete_zero_window_has_positive_zero() {
        assert_eq!(
            weighted_average(2.0, 1.0, &[0.0; 4], [0.0, 0.0, 1.0, 1.0], 5.435)
                .unwrap()
                .to_bits(),
            0
        );
    }
    #[test]
    fn custom_granule_zero_input_has_five_zero_features() {
        let w = WaveformInput {
            sample_rate: 2.0,
            hop: 1.0,
            arrays: (0..8)
                .map(|_| ArrayInput {
                    amplitudes: vec![0.0; 4],
                    amplitude_bits: None,
                })
                .collect(),
        };
        assert_eq!(granule(&w, 0.0, 1.0).unwrap(), [0.0; 5]);
    }
    #[test]
    fn complete_zero_granule_window_scores_zero() {
        assert_eq!(score_at(&[[0.0; 5]; 128], 64).unwrap().to_bits(), 0);
    }
    #[test]
    fn positive_melody_step_has_analytic_contrast_score() {
        let mut g = [[0.0; 5]; 128];
        for row in &mut g[64..] {
            row[3] = 1.0;
        }
        assert_eq!(score_at(&g, 64).unwrap().to_bits(), 293.6_f64.to_bits());
    }
    #[test]
    fn zero_phase_costs_retain_earliest_phase() {
        let (phase, costs) = phase_scores(&vec![0.0; 256]).unwrap();
        assert_eq!(phase, 0);
        assert_eq!(costs, vec![0.0; 32]);
    }
    #[test]
    fn isolated_nonzero_phase_is_computed_from_raw_curve() {
        let mut scores = vec![0.0; 256];
        scores[67] = 100.0;
        let (phase, costs) = phase_scores(&scores).unwrap();
        assert_eq!(phase, 3);
        assert!(costs[3] > costs[2] && costs[3] > costs[4]);
    }
    #[test]
    fn short_curve_returns_phase_zero_without_cost_search() {
        assert_eq!(phase_scores(&vec![1.0; 95]).unwrap(), (0, vec![0.0; 32]));
    }
    fn simple_beats() -> BeatInput {
        BeatInput {
            beat_times_seconds: vec![0.0, 0.5, 1.0, 1.5, 2.0],
            beat_time_bits: None,
            tempo: 120.0,
            tempo_bits: None,
            duration_seconds: 2.0,
            duration_bits: None,
            downbeat_time: 0.25,
            downbeat_bits: None,
        }
    }
    #[test]
    fn long_zero_generator_keeps_origin_and_context_downbeat() {
        let w = WaveformInput {
            sample_rate: 2.0,
            hop: 1.0,
            arrays: (0..8)
                .map(|_| ArrayInput {
                    amplitudes: vec![0.0; 8],
                    amplitude_bits: None,
                })
                .collect(),
        };
        let mut b = simple_beats();
        b.duration_seconds = 20.0;
        let r = predict(&w, &b).unwrap();
        assert_eq!(r.origin, -32);
        assert_eq!(r.end, 32);
        assert_eq!(r.raw_score_bits.len(), 96);
        assert!(r.raw_score_bits.iter().all(|x| x == "0000000000000000"));
        assert_eq!(r.selected_phase, 0);
        assert_eq!(r.selected_time.to_bits(), 0.25_f64.to_bits());
    }
    #[test]
    fn complete_generator_rejects_nonincreasing_grid() {
        let w = WaveformInput {
            sample_rate: 2.0,
            hop: 1.0,
            arrays: (0..8)
                .map(|_| ArrayInput {
                    amplitudes: vec![0.0; 8],
                    amplitude_bits: None,
                })
                .collect(),
        };
        let mut b = simple_beats();
        b.beat_times_seconds[2] = b.beat_times_seconds[1];
        assert!(predict(&w, &b).is_err());
    }
    #[test]
    fn short_duration_returns_empty_curve_and_default_zero_anchor() {
        let w = WaveformInput {
            sample_rate: 2.0,
            hop: 1.0,
            arrays: (0..8)
                .map(|_| ArrayInput {
                    amplitudes: vec![0.0; 8],
                    amplitude_bits: None,
                })
                .collect(),
        };
        let r = predict(&w, &simple_beats()).unwrap();
        assert!(r.raw_score_bits.is_empty());
        assert_eq!(r.selected_time_bits, "0000000000000000");
    }
}
