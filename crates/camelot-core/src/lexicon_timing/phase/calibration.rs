use super::*;

pub(in crate::lexicon_timing) fn calibrate_onset(
    pcm: &[f32],
    rate: u32,
    anchor: f64,
    period: f64,
    peaks: &[f64],
) -> Calibration {
    let unchanged = Calibration::unchanged(anchor);
    if !period.is_finite() || period <= 0. || !valid(pcm, rate, anchor, 60. / period) {
        return unchanged;
    }
    let width = (0.003 * rate as f64).round() as usize;
    let step = (0.001 * rate as f64).round() as usize;
    for beats in [2, 3, 4] {
        let target = anchor + beats as f64 * period;
        let Some(&peak) = peaks
            .iter()
            .find(|p| (**p - target).abs() < 0.05 && **p > anchor)
        else {
            continue;
        };
        if peak == 0. {
            continue;
        }
        let index = (peak * rate as f64).round() as usize;
        let level = (power(pcm, index, index.saturating_add(width)) / width as f64).sqrt();
        if level < 0.001 {
            continue;
        }
        let before = index.saturating_sub((0.02 * rate as f64).round() as usize);
        let previous = (power(pcm, before, before.saturating_add(width)) / width as f64).sqrt();
        let ratio = if previous > 1e-4 {
            level / previous
        } else {
            100.
        };
        if ratio < 3. {
            continue;
        }
        let minimum = width.max(index.saturating_sub((0.01 * rate as f64).round() as usize));
        let mut position = index;
        let mut cursor = index.checked_sub(step);
        while let Some(at) = cursor.filter(|at| *at >= minimum) {
            let rms = (power(pcm, at, at.saturating_add(width)) / width as f64).sqrt();
            if rms < 0.32 * level {
                position = at + step;
                break;
            }
            position = at;
            cursor = at.checked_sub(step);
        }
        let corrected = position as f64 / rate as f64 - beats as f64 * period;
        if (corrected - anchor).abs() <= 0.03 {
            return Calibration {
                position: corrected.max(0.),
                used_calibration: true,
                calibration_beat: Some(beats),
                calibration_beats: None,
            };
        }
    }
    unchanged
}
pub(in crate::lexicon_timing) fn calibrate_drop(
    pcm: &[f32],
    rate: u32,
    anchor: f64,
    bpm: f64,
    peaks: &[f64],
) -> Calibration {
    let unchanged = Calibration::unchanged(anchor);
    if !valid(pcm, rate, anchor, bpm) {
        return unchanged;
    }
    let period = 60. / bpm;
    let mut candidates = vec![];
    for beats in [192, 128, 96, 64] {
        let target = anchor + beats as f64 * period;
        let selected: Vec<_> = peaks
            .iter()
            .filter(|p| (**p - target).abs() <= 0.05)
            .copied()
            .collect();
        let Some(&first) = selected.first() else {
            continue;
        };
        let (mut peak, mut ratio) = (first, attack_ratio(pcm, rate, first, 20.));
        for &at in &selected {
            let r = attack_ratio(pcm, rate, at, 20.);
            if r > ratio {
                peak = at;
                ratio = r;
            }
        }
        if ratio >= 2. {
            candidates.push((beats, peak, ratio));
        }
    }
    let Some(&(mut beats, mut peak, mut ratio)) = candidates.first() else {
        return unchanged;
    };
    for &(b, p, r) in candidates.iter().skip(1) {
        if ratio <= r {
            beats = b;
            peak = p;
            ratio = r;
        }
    }
    let onset = if ratio >= 5. {
        precise_onset(pcm, rate, peak, 0.03)
    } else {
        peak
    };
    let position = onset - beats as f64 * period;
    if (position - anchor).abs() > 0.05 {
        return unchanged;
    }
    if ratio < 5.
        && peaks
            .iter()
            .take(10)
            .map(|p| (p - anchor).abs())
            .fold(f64::INFINITY, f64::min)
            < 0.005
    {
        return unchanged;
    }
    Calibration {
        position: position.max(0.),
        used_calibration: true,
        calibration_beat: None,
        calibration_beats: Some(beats),
    }
}
pub(in crate::lexicon_timing) fn verify_drop(
    pcm: &[f32],
    rate: u32,
    anchor: f64,
    bpm: f64,
    peaks: &[f64],
) -> DropCorrection {
    let unchanged = DropCorrection::unchanged(anchor);
    if !valid(pcm, rate, anchor, bpm) {
        return unchanged;
    }
    let period = 60. / bpm;
    let half = period / 2.;
    let duration = pcm.len() as f64 / rate as f64;
    let mut candidates = vec![];
    for &peak in peaks {
        if peak < 10. {
            continue;
        }
        if peak > duration - 10. {
            break;
        }
        let ratio = attack_ratio(pcm, rate, peak, 30.);
        if ratio >= 2.5 {
            candidates.push((peak, ratio));
        }
    }
    if candidates.len() < 3 && bpm >= 200. {
        let mut at = 10_f64.max(anchor + 20. * period);
        let end = 90_f64.min(duration - 10.);
        while at < end {
            let ratio = attack_ratio(pcm, rate, at, 30.);
            if ratio >= 3.
                && !candidates
                    .iter()
                    .any(|(time, _)| (time - at).abs() < period)
            {
                candidates.push((at, ratio));
            }
            at += half;
        }
    }
    let Some(&(mut peak, mut ratio)) = candidates.first() else {
        return unchanged;
    };
    for &(p, r) in candidates.iter().skip(1) {
        if ratio <= r {
            peak = p;
            ratio = r;
        }
    }
    let beats = (peak - anchor) / period;
    let phase = beats - beats.floor();
    if !(phase > 0.4 && phase < 0.6) {
        return unchanged;
    }
    let candidate = anchor - half;
    if candidate < 0. {
        return unchanged;
    }
    let before = rms(pcm, rate, candidate, 0.025);
    let original = rms(pcm, rate, anchor, 0.025);
    if !(before > 0.2 * original || ratio > 10.) {
        return unchanged;
    }
    let width = (0.005 * rate as f64).round() as usize;
    let mut at = (candidate - 0.012).max(0.);
    let end = candidate + 0.012;
    let (mut maximum, mut position) = (0., candidate);
    while at <= end {
        let start = (at * rate as f64).round() as usize;
        let energy = power(pcm, start, start.saturating_add(width));
        if energy > maximum {
            maximum = energy;
            position = at;
        }
        at += 0.001;
    }
    DropCorrection {
        position,
        used_correction: true,
        drop_position: Some(peak),
        drop_attack_ratio: Some(ratio),
    }
}
