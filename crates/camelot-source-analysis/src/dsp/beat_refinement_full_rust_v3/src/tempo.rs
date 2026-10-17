//! Feature-conditioned SDK tempo-stage reconstruction; not independent PCM inference.
//! Only explicit waveform/legacy intermediates are accepted. No expected outputs are read.
use crate::dsp::beat_refinement_full_rust_v3::{calculate_beat_score, upsample_2x};
use serde::{Deserialize, Serialize};

const BASE_PERIOD: f64 = 0.31088082901554404;
const PERIOD_STEP: f64 = 0.001;
const AC_LENGTH: usize = 4096;

#[derive(Debug, Deserialize)]
pub struct TempoInput {
    pub sample_rate: f64,
    pub hop: i32,
    pub samples: Vec<f32>,
    pub flags: Vec<u32>,
    pub legacy_beats: Vec<f64>,
    pub legacy_tempo: f64,
    pub duration: f64,
}

#[derive(Debug, Serialize)]
pub struct TempoResult {
    pub tempo: f64,
    pub branch: &'static str,
    pub initial_normalized_count: usize,
    pub median_tempo: f64,
    pub variation: f64,
    pub selected_ac_block: Option<usize>,
    pub coarse_peak_index: Option<usize>,
    pub waveform_bpm: Option<f64>,
    pub adjusted_bpm: Option<f64>,
    pub fitted_period_frames: Option<f64>,
    pub fitted_phase: Option<f64>,
}

fn normalized_legacy(input: &TempoInput) -> Vec<f64> {
    let mut beats = input.legacy_beats.clone();
    if beats.len() < 2 || input.legacy_tempo <= 0.0 {
        return beats;
    }
    let span = beats[beats.len() - 1] - beats[0];
    let intervals = (beats.len() - 1) as f64;
    // Original constructor uses FRINTM after adding0.5, not truncation or round().
    let ratio = (((60.0 / span) * intervals) / input.legacy_tempo).ln();
    let mut factor = (ratio / std::f64::consts::LN_2 + 0.5).floor().exp2();
    while factor > 1.75 {
        beats = beats.into_iter().step_by(2).collect();
        factor *= 0.5;
    }
    while factor < 0.6 {
        let mut dense = Vec::with_capacity(beats.len() * 2);
        for pair in beats.windows(2) {
            dense.push(pair[0]);
            dense.push((pair[0] + pair[1]) * 0.5);
        }
        if let Some(last) = beats.last() {
            dense.push(*last);
        }
        beats = dense;
        factor += factor;
    }
    beats
}

fn regularity(beats: &[f64]) -> Option<(f64, f64)> {
    if beats.len() <= 64 {
        return Some((0.0, 0.0));
    }
    let intervals: Vec<f64> = beats.windows(2).map(|x| x[1] - x[0]).collect();
    if intervals.iter().any(|x| *x <= 1e-5) {
        return None;
    }
    let mut blocks = Vec::new();
    for block in intervals.as_chunks::<32>().0 {
        let mut sum = 0.0;
        for pair in block.as_chunks::<2>().0 {
            sum = (sum + pair[0]) + pair[1];
        }
        blocks.push(sum * 0.03125);
    }
    let mut sorted = blocks.clone();
    sorted.sort_by(f64::total_cmp);
    if sorted.is_empty() {
        return Some((0.0, 0.0));
    }
    let position = (sorted.len() - 1) as f64 * 0.5;
    let i = (position.floor() as usize).min(sorted.len().saturating_sub(2));
    let median = if sorted.len() == 1 {
        sorted[0]
    } else {
        (position - i as f64).mul_add(sorted[i + 1] - sorted[i], sorted[i])
    };
    let bpm = if median >= 0.0 { 60.0 / median } else { 0.0 };
    let mut variation: f64 = -1.0;
    for period in blocks {
        let r = period / median;
        variation = variation.max(r.max(1.0 / r));
    }
    Some((bpm, variation))
}

fn peak_signal(samples: &[f64]) -> Vec<f32> {
    let up = upsample_2x(samples);
    let mut peaks = vec![0.0; up.len()];
    for (index, triple) in up.windows(3).enumerate() {
        let left = triple[0];
        let center = triple[1];
        let right = triple[2];
        if center <= left || right <= 0.0 || center < right || left <= 0.0 {
            continue;
        }
        let denominator = right.mul_add(-2.0, center.mul_add(4.0, left * -2.0));
        if denominator <= 0.0 {
            continue;
        }
        let delta = (right - left) / denominator;
        let quadratic = (left + right).mul_add(0.5, -center);
        let right_term = (right - quadratic) - center;
        let height = center + (quadratic * delta).mul_add(delta, delta * right_term);
        let center_index = index + 1;
        if delta >= 0.0 {
            peaks[center_index] = height.mul_add(1.0 - delta, peaks[center_index]);
            peaks[index + 2] = height.mul_add(delta, peaks[index + 2]);
        } else {
            peaks[center_index] = height.mul_add(delta + 1.0, peaks[center_index]);
            peaks[index] = height.mul_add(-delta, peaks[index]);
        }
    }
    for _ in 0..2 {
        let mut smooth = vec![0.0; peaks.len()];
        for (index, triple) in peaks.windows(3).enumerate() {
            smooth[index + 1] = ((triple[0] + triple[1]) + triple[2]) / 3.0;
        }
        peaks = smooth;
    }
    peaks
        .into_iter()
        .map(|x| {
            let y = x as f32;
            if y < 0.0 { 0.0 } else { y }
        })
        .collect()
}

fn lag_dot(signal: &[f32], start: usize, lag: usize) -> f64 {
    let remaining = signal.len() - start;
    if lag >= remaining {
        return 0.0;
    }
    let count = (remaining - lag).min(AC_LENGTH);
    let mut value = 0.0;
    for j in 0..count {
        value = (signal[start + j] as f64).mul_add(signal[start + j + lag] as f64, value);
    }
    value.max(0.0)
}

fn autocorrelation(signal: &[f32]) -> (Vec<f32>, usize) {
    let mut best_score = -1.0;
    let mut best = 0;
    for block in 0..signal.len().div_ceil(AC_LENGTH) {
        let start = block * AC_LENGTH;
        let mut score = 0.0;
        for lag in 0..AC_LENGTH {
            let value = (lag_dot(signal, start, lag).sqrt() * 0.000244140625) * 50.0;
            score = value.mul_add(value, score);
        }
        if block == 0 || score > best_score {
            best = block;
            best_score = score;
        }
    }
    let start = best * AC_LENGTH;
    let divisor = (signal.len() - start).min(AC_LENGTH) as f64;
    let ac = (0..AC_LENGTH)
        .map(|lag| ((lag_dot(signal, start, lag) / divisor).sqrt() * 50.0) as f32)
        .collect();
    (ac, best)
}

fn read_ac(samples: &[f32], position: f64) -> f64 {
    let i = position.floor() as i32;
    if i < -1 || samples.len() < 4 || (i + 2) as usize >= samples.len() {
        return 0.0;
    }
    let t = position - i as f64;
    let previous = samples[(i - 1).unsigned_abs() as usize] as f64;
    let current = samples[i.unsigned_abs() as usize] as f64;
    let next = samples[(i + 1) as usize] as f64;
    let next_next = samples[(i + 2) as usize] as f64;
    let t2 = t * t;
    let t3 = t * t2;
    let three_t3 = t3 * 3.0;
    let a = (t2.mul_add(-2.0, t) + t3) * -0.5;
    let b = (t2.mul_add(5.0, -2.0) - three_t3) * -0.5;
    let c = (t2.mul_add(-4.0, -t) + three_t3) * -0.5;
    let d = (t2 - t3) * -0.5;
    let value = current * b;
    let value = previous.mul_add(a, value);
    let value = next.mul_add(c, value);
    next_next.mul_add(d, value)
}

fn quantile_f32(sorted: &[f32], fraction: f64) -> f64 {
    if sorted.is_empty() {
        return 0.0;
    }
    if fraction <= 0.0 || sorted.len() == 1 {
        return sorted[0] as f64;
    }
    let position = fraction.min(1.0) * (sorted.len() - 1) as f64;
    let index = (position.floor() as usize).min(sorted.len() - 2);
    let difference = (sorted[index + 1] - sorted[index]) as f64;
    difference.mul_add(position - index as f64, sorted[index] as f64)
}

fn coarse_scores(ac: &[f32], rate: f64) -> Vec<f32> {
    let mut period = BASE_PERIOD;
    let mut scores = Vec::new();
    while period < 0.7692307692307693 {
        let bpm = 60.0 / period;
        let step = rate * (60.0 / bpm);
        let mut lag = 0.0;
        let mut n = 0_usize;
        let mut sums = [0.0_f64; 8];
        while lag < ac.len() as f64 {
            let v = read_ac(ac, lag);
            sums[0] += v;
            for k in [2, 3, 4, 8] {
                sums[k - 1] += if n.is_multiple_of(k) { v } else { -0.0 };
            }
            lag += step;
            n += 1;
        }
        for k in 1..=8 {
            sums[k - 1] = ((sums[k - 1] * (60.0 / (k as f64 * bpm))) * rate) / 3000.0;
        }
        let positive = sums[3].mul_add(2.5, sums[1].mul_add(1.4, sums[0]));
        let negative = sums[2].mul_add(1.3, sums[7] * 3.5);
        let log_t = (bpm.ln() + -4.852030263919617) / std::f64::consts::LN_2;
        let prior = ((log_t * log_t) * -0.7).exp2();
        scores.push((prior * (positive - negative)) as f32);
        period += PERIOD_STEP;
    }
    let mut mask = vec![0.0_f32; scores.len()];
    let mut residual = vec![0.0_f32; scores.len()];
    for index in 0..scores.len() {
        let mut neighborhood =
            scores[index.saturating_sub(32)..(index + 33).min(scores.len())].to_vec();
        neighborhood.sort_by(f32::total_cmp);
        let q40 = quantile_f32(&neighborhood, 0.4);
        let q45 = quantile_f32(&neighborhood, 0.45);
        mask[index] = if (q40 * 1.2) * 0.5 <= scores[index] as f64 {
            1.0
        } else {
            0.0
        };
        residual[index] = ((scores[index] as f64 - q45 * 0.5).max(0.0)) as f32;
    }
    for index in 0..scores.len() {
        let dilated = mask[index.saturating_sub(2)..(index + 3).min(mask.len())]
            .iter()
            .copied()
            .fold(0.0_f32, f32::max);
        scores[index] = dilated * residual[index];
    }
    scores
        .into_iter()
        .map(|x| {
            let d = x as f64;
            d.mul_add(d, 0.0).sqrt() as f32
        })
        .collect()
}

fn select_peak(scores: &[f32]) -> Option<usize> {
    if scores.len() < 5 {
        return None;
    }
    let mut best = 0.0_f64;
    let mut selected = None;
    for index in 2..scores.len() - 2 {
        let value = scores[index];
        if value as f64 <= best {
            continue;
        }
        let mut right = index;
        while right + 1 < scores.len() && scores[right + 1] == scores[right] {
            right += 1;
        }
        if right + 1 < scores.len() && scores[right + 1] > scores[right] {
            continue;
        }
        let mut left = index;
        while left > 0 && scores[left - 1] == scores[left] {
            left -= 1;
        }
        if left > 0 && scores[left - 1] > scores[left] {
            continue;
        }
        selected = Some(index);
        best = value as f64;
    }
    selected
}

fn fine_bpm(ac: &[f32], rate: f64, index: usize) -> f64 {
    let mut q = index as f64;
    let mut width = 1.0;
    for _ in 0..24 {
        let mut count = 0_i32;
        for direction in [-1.0_f64, 1.0] {
            let lag = direction
                .mul_add(width, q)
                .mul_add(PERIOD_STEP, BASE_PERIOD)
                * rate;
            count = count.max((((ac.len() - 3) as f64 / lag).floor() as i32) - 1);
        }
        let mut values = [0.0_f64; 2];
        for (side, direction) in [-1.0_f64, 1.0].into_iter().enumerate() {
            let lag = direction
                .mul_add(width, q)
                .mul_add(PERIOD_STEP, BASE_PERIOD)
                * rate;
            let mut sum = 0.0;
            let mut multiplier = 0.0;
            for _ in 0..count.max(0) {
                sum += read_ac(ac, lag * multiplier);
                multiplier += 1.0;
            }
            let normalized = sum / 3000.0;
            values[side] = normalized.mul_add(normalized, values[side]);
        }
        width *= 0.5;
        q += if values[1].sqrt() > values[0].sqrt() {
            width
        } else {
            -width
        };
    }
    60.0 / q.mul_add(PERIOD_STEP, BASE_PERIOD)
}

fn corrected_bpm(mut bpm: f64, median: f64) -> f64 {
    while median * 1.75 < bpm {
        bpm *= 0.5;
    }
    while bpm < median / 1.75 {
        bpm += bpm;
    }
    let ratio = bpm / median;
    if (ratio - 0.75).abs() < 0.01 && (116.0..=140.0).contains(&median) {
        bpm *= 1.3333333333333333;
    }
    if (ratio - 0.875).abs() < 0.01 && (116.0..=140.0).contains(&median) {
        bpm *= 1.1428571428571428;
    }
    if (ratio - 0.6666666666666666).abs() < 0.01 && median >= 110.0 {
        bpm *= 1.5;
    }
    if (ratio - 1.4444444444444444).abs() < 0.01 && (85.0..=100.0).contains(&median) {
        bpm *= 0.6923076923076923;
    }
    if (ratio - 1.3333333333333333).abs() < 0.01 {
        bpm *= 0.75;
    }
    if (ratio - 0.8).abs() < 0.01 {
        bpm *= 1.25;
    }
    bpm
}

pub(crate) fn fix_tempo_range(mut bpm: f64) -> Result<f64, String> {
    // Bounds79..192 admit octave folding; SDK's effective lower bound is79.
    // FRINTP0xb5be4 folds high values, FRINTM0xb5c14 raises low values.
    if bpm > 192.0 {
        let factor = (-((bpm / 192.0).ln() / std::f64::consts::LN_2).ceil()).exp2();
        bpm *= factor;
    }
    if bpm < 79.0 && bpm > 0.0 {
        let factor = (-((bpm / 79.0).ln() / std::f64::consts::LN_2).floor()).exp2();
        bpm *= factor;
    }
    Ok(bpm)
}

fn fit_period_phase(samples: &[f64], bpm: f64, frame_duration: f64) -> Result<(f64, f64), String> {
    if bpm == 0.0 {
        return Ok((0.46875 / frame_duration, 0.0));
    }
    let mut period = (60.0 / bpm) / frame_duration;
    let mut period_step = period * 0.0001;
    let mut phase = 0.0;
    let mut phase_width = 1.0;
    for pass in 0..6 {
        let period_start = if pass == 0 { -8 } else { -4 };
        let period_end = -period_start;
        let phase_bound = if pass < 2 { 32 } else { 6 };
        let mut best_score = 0.0;
        let mut best_period = 0.0;
        let mut best_phase = 0.0;
        for p in period_start..=period_end {
            let candidate_period = (p as f64).mul_add(period_step, period);
            for s in -phase_bound..=phase_bound {
                let candidate_phase =
                    (s as f64 / (phase_bound * 2 + 1) as f64).mul_add(phase_width, phase);
                let candidate_phase = candidate_phase - candidate_phase.floor();
                let score = calculate_beat_score(samples, candidate_period, candidate_phase)?.score;
                if score > best_score {
                    best_score = score;
                    best_period = candidate_period;
                    best_phase = candidate_phase;
                }
            }
        }
        period = best_period;
        phase = best_phase;
        period_step *= 0.5;
        phase_width *= 0.5;
        if pass >= 4 {
            period_step *= 0.25;
            phase_width *= 0.25;
        }
    }
    let mut best_score = 0.0;
    let mut best_phase = 0.0;
    for shift in 0..4 {
        let shifted = phase + shift as f64 * 0.25;
        let shifted = shifted - shifted.floor();
        let score = calculate_beat_score(samples, period, shifted)?.score;
        if score > best_score {
            best_score = score;
            best_phase = shifted;
        }
    }
    Ok((period, best_phase))
}

pub fn refine_tempo(input: &TempoInput) -> Result<TempoResult, String> {
    if !input.sample_rate.is_finite()
        || !input.legacy_tempo.is_finite()
        || input.samples.iter().any(|x| !x.is_finite())
        || input.legacy_beats.iter().any(|x| !x.is_finite())
    {
        return Err("stage inputs must be finite".into());
    }
    crate::dsp::beat_refinement_full_rust_v3::bounds::validate_waveform(
        input.sample_rate,
        input.hop,
        input.duration,
        &input.samples,
        &input.flags,
    )?;
    crate::dsp::beat_refinement_full_rust_v3::bounds::validate_legacy_entry(
        &input.legacy_beats,
        input.legacy_tempo,
    )?;
    let beats = normalized_legacy(input);
    let mut result = TempoResult {
        tempo: input.legacy_tempo,
        branch: "legacy passthrough",
        initial_normalized_count: beats.len(),
        median_tempo: 0.0,
        variation: 0.0,
        selected_ac_block: None,
        coarse_peak_index: None,
        waveform_bpm: None,
        adjusted_bpm: None,
        fitted_period_frames: None,
        fitted_phase: None,
    };
    if input.sample_rate <= 0.0 || input.hop <= 0 {
        result.branch = "invalid waveform geometry";
        return Ok(result);
    }
    let Some((median, variation)) = regularity(&beats) else {
        result.branch = "invalid legacy interval";
        return Ok(result);
    };
    result.median_tempo = median;
    result.variation = variation;
    let legacy_valid =
        (10.0..=1000.0).contains(&input.legacy_tempo) && (10.0..=1000.0).contains(&median);
    if variation > 1.23 && legacy_valid {
        result.branch = "variable legacy grid";
        return Ok(result);
    }
    let samples: Vec<f64> = input.samples.iter().map(|x| *x as f64).collect();
    let mut bpm = if samples.is_empty() {
        // Native empty upsample path enters candidate failure with-1.0.
        -1.0
    } else {
        let signal = peak_signal(&samples);
        let (ac, block) = autocorrelation(&signal);
        result.selected_ac_block = Some(block);
        let rate = (input.sample_rate as i32) as f64 / (input.hop / 2) as f64;
        let scores = coarse_scores(&ac, rate);
        let peak = select_peak(&scores);
        result.coarse_peak_index = peak;
        peak.map(|p| fine_bpm(&ac, rate, p)).unwrap_or(0.0)
    };
    result.waveform_bpm = Some(bpm);
    if !(10.0..=1000.0).contains(&bpm) && legacy_valid {
        result.branch = "invalid waveform tempo keeps legacy";
        return Ok(result);
    }
    if !(10.0..=1000.0).contains(&bpm) {
        bpm = 0.0;
    }
    if legacy_valid && bpm > 0.0 {
        bpm = corrected_bpm(bpm, median);
    }
    bpm = fix_tempo_range(bpm)?;
    result.adjusted_bpm = Some(bpm);
    let frame_duration = (1.0 / input.sample_rate) * input.hop as f64;
    let (period, phase) = fit_period_phase(&samples, bpm, frame_duration)?;
    result.tempo = 60.0 / (frame_duration * period);
    result.branch = "waveform tempo rebuild";
    result.fitted_period_frames = Some(period);
    result.fitted_phase = Some(phase);
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::fix_tempo_range;

    #[test]
    fn tempo_range_uses_octaves_instead_of_clamping_boundary_values() {
        assert_eq!(
            fix_tempo_range(200.0).unwrap().to_bits(),
            100.0_f64.to_bits()
        );
        assert_eq!(fix_tempo_range(40.0).unwrap().to_bits(), 80.0_f64.to_bits());
        assert_eq!(
            fix_tempo_range(192.0).unwrap().to_bits(),
            192.0_f64.to_bits()
        );
        assert_eq!(fix_tempo_range(79.0).unwrap().to_bits(), 79.0_f64.to_bits());
    }
    fn empty_input() -> super::TempoInput {
        super::TempoInput {
            sample_rate: 44100.0,
            hop: 1024,
            samples: vec![],
            flags: vec![],
            legacy_beats: vec![],
            legacy_tempo: 0.0,
            duration: 0.0,
        }
    }

    #[test]
    fn empty_waveform_and_invalid_legacy_use_native128_tempo_fallback() {
        let result = super::refine_tempo(&empty_input()).unwrap();
        assert_eq!(result.tempo.to_bits(), 128.0_f64.to_bits());
    }

    #[test]
    fn variable_valid_legacy_grid_bypasses_waveform_rebuild() {
        let mut input = empty_input();
        input.legacy_tempo = 120.0;
        input.legacy_beats.push(0.0);
        for n in 0..96 {
            input
                .legacy_beats
                .push(input.legacy_beats.last().unwrap() + if n < 64 { 0.5 } else { 1.0 });
        }
        let result = super::refine_tempo(&input).unwrap();
        assert_eq!(result.branch, "variable legacy grid");
        assert_eq!(result.tempo.to_bits(), 120.0_f64.to_bits());
    }

    #[test]
    fn legacy_constructor_removes_alternate_double_rate_beats() {
        let mut input = empty_input();
        input.legacy_tempo = 120.0;
        input.legacy_beats = (0..129).map(|i| i as f64 * 0.25).collect();
        let beats = super::normalized_legacy(&input);
        assert_eq!(beats, (0..65).map(|i| i as f64 * 0.5).collect::<Vec<_>>());
    }
}
