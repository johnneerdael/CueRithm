use super::*;
use crate::lexicon_timing::onset::js_round;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(in crate::lexicon_timing) struct HalfBeat {
    pub position: f64,
    pub used_correction: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub on_beat_kicks: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub half_beat_kicks: Option<usize>,
}
pub(in crate::lexicon_timing) fn half_beat(
    pcm: &[f32],
    rate: u32,
    anchor: f64,
    bpm: f64,
) -> HalfBeat {
    let unchanged = HalfBeat {
        position: anchor,
        used_correction: false,
        on_beat_kicks: None,
        half_beat_kicks: None,
    };
    if !valid(pcm, rate, anchor, bpm) {
        return unchanged;
    }
    let period = 60. / bpm;
    let half = period / 2.;
    let duration = pcm.len() as f64 / rate as f64;
    let candidate = anchor - half;
    if candidate < 0. {
        return unchanged;
    }
    let end = ((duration - 1.) / period).floor().min(80.) as usize;
    if end <= 20 {
        return unchanged;
    }
    let (mut on, mut between, mut count) = (0, 0, 0);
    for beat in 16..end {
        let at = anchor + beat as f64 * period;
        let shifted = at + half;
        if shifted >= duration - 0.1 {
            break;
        }
        if best_attack(pcm, rate, at, 5, 1, 15.).1 >= 2. {
            on += 1;
        }
        if best_attack(pcm, rate, shifted, 5, 1, 15.).1 >= 2. {
            between += 1;
        }
        count += 1;
    }
    if count == 0 || between <= on {
        return unchanged;
    }
    let threshold = if anchor > 3. { 0.4 } else { 0.28 };
    if ((between - on) as f64 / count as f64) < threshold
        || (between as f64 / count as f64) < 0.6
        || (on as f64 / count as f64) < 0.15
    {
        return unchanged;
    }
    HalfBeat {
        position: candidate,
        used_correction: true,
        on_beat_kicks: Some(on),
        half_beat_kicks: Some(between),
    }
}
fn range_rms(pcm: &[f32], rate: u32, begin: f64, end: f64) -> f64 {
    let begin = (begin * rate as f64).round().max(0.) as usize;
    let end = ((end * rate as f64).round() as usize).min(pcm.len());
    if end <= begin {
        0.
    } else {
        (power(pcm, begin, end) / (end - begin) as f64).sqrt()
    }
}
pub(in crate::lexicon_timing) fn rescue_silent(
    pcm: &[f32],
    rate: u32,
    anchor: f64,
    bpm: f64,
) -> Rescue {
    let unchanged = Rescue {
        position: anchor,
        applied: false,
    };
    if !valid(pcm, rate, anchor, bpm) {
        return unchanged;
    }
    let period = 60. / bpm;
    let duration = pcm.len() as f64 / rate as f64;
    if range_rms(pcm, rate, anchor, anchor + period) >= 0.0015 {
        return unchanged;
    }
    let (mut found, mut ratio) = (-1., 0.);
    for beat in 1..=64 {
        let at = anchor + beat as f64 * period;
        if at + period > duration {
            break;
        }
        if range_rms(pcm, rate, at, at + period) < 0.02 {
            continue;
        }
        let (mut position, mut maximum) = (-1., 0.);
        let first = js_round(-0.4 * period * 1000.) as i32;
        let last = js_round(period * 1000.) as i32;
        for offset in (first..=last).step_by(4) {
            let trial = at + offset as f64 / 1000.;
            if trial < 0. || trial >= duration {
                continue;
            }
            let r = attack_ratio(pcm, rate, trial, 15.);
            if r > maximum {
                maximum = r;
                position = trial;
            }
        }
        if maximum >= 2.5 {
            // The original fine search follows its mutable best position.
            for offset in (-6..=6).step_by(2) {
                let trial = position + offset as f64 / 1000.;
                if trial < 0. || trial >= duration {
                    continue;
                }
                let r = attack_ratio(pcm, rate, trial, 15.);
                if r > maximum {
                    maximum = r;
                    position = trial;
                }
            }
            found = position;
            ratio = maximum;
            break;
        }
    }
    if found < 0. {
        return unchanged;
    }
    if ratio == 100. {
        let width = (0.003 * rate as f64).round() as usize;
        let step = (0.001 * rate as f64).round() as usize;
        let span = (0.03 * rate as f64).round() as usize;
        let start = (found * rate as f64).round() as usize;
        let end = (start + span).min(pcm.len().saturating_sub(width));
        let mut maximum = 0_f64;
        for at in (start..=end).step_by(step) {
            maximum = maximum.max((power(pcm, at, at + width) / width as f64).sqrt());
        }
        if maximum > 0.001 {
            for at in (start..=end).step_by(step) {
                let rms = (power(pcm, at, at + width) / width as f64).sqrt();
                if rms >= 0.1 * maximum {
                    if at > start {
                        found = at as f64 / rate as f64;
                    }
                    break;
                }
            }
        }
    }
    if grid_hits(pcm, rate, found, period, 30, 1.5) < 5 {
        return unchanged;
    }
    Rescue {
        position: found,
        applied: true,
    }
}
#[cfg(test)]
pub(in crate::lexicon_timing) fn rescue_weak(
    pcm: &[f32],
    rate: u32,
    anchor: f64,
    bpm: f64,
) -> Rescue {
    let unchanged = Rescue {
        position: anchor,
        applied: false,
    };
    if !valid(pcm, rate, anchor, bpm) {
        return unchanged;
    }
    let period = 60. / bpm;
    let duration = pcm.len() as f64 / rate as f64;
    let old_hits = grid_hits(pcm, rate, anchor, period, 20, 2.);
    if old_hits > 2 {
        return unchanged;
    }
    let average = |shift: f64| {
        let (mut total, mut count) = (0., 0);
        for beat in 4..=40 {
            let at = anchor + beat as f64 * period + shift;
            if at < 0. || at >= duration - 0.015 {
                continue;
            }
            total += attack_ratio(pcm, rate, at, 15.) * rms(pcm, rate, at, 0.015);
            count += 1;
        }
        if count > 0 { total / count as f64 } else { 0. }
    };
    let baseline = average(0.);
    let (mut offset, mut maximum) = (0., baseline);
    let radius = 0.55 * period;
    let mut shift = -radius;
    while shift <= radius {
        if shift.abs() >= 0.015 {
            let score = average(shift);
            if score > maximum {
                maximum = score;
                offset = shift;
            }
        }
        shift += 0.002;
    }
    let improvement = if baseline > 0. {
        maximum / baseline
    } else {
        f64::INFINITY
    };
    if offset == 0. || improvement < 1.8 {
        return unchanged;
    }
    let mut candidate = anchor + offset;
    if candidate < 0. {
        candidate += period;
    }
    if grid_hits(pcm, rate, candidate, period, 20, 2.) < old_hits + 3 {
        return unchanged;
    }
    Rescue {
        position: best_attack(pcm, rate, candidate, 20, 2, 15.).0,
        applied: true,
    }
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(in crate::lexicon_timing) struct GridValidation {
    pub validated_first_beat: f64,
    pub skipped_beats: usize,
}
pub(in crate::lexicon_timing) fn validate_grid(
    pcm: &[f32],
    rate: u32,
    anchor: f64,
    bpm: f64,
) -> GridValidation {
    let unchanged = GridValidation {
        validated_first_beat: anchor,
        skipped_beats: 0,
    };
    if !valid(pcm, rate, anchor, bpm) || anchor >= 5. {
        return unchanged;
    }
    let period = 60. / bpm;
    let duration = pcm.len() as f64 / rate as f64;
    let center_rms = |at: f64| {
        let half = ((10. / 1000. * rate as f64) / 2.).floor() as usize;
        let center = (at * rate as f64).floor() as usize;
        let start = center.saturating_sub(half);
        let end = (center + half).min(pcm.len());
        if end > start {
            (power(pcm, start, end) / (end - start) as f64).sqrt()
        } else {
            f64::NAN
        }
    };
    let average = |position: f64| {
        let (mut total, mut count) = (0., 0);
        for beat in 0..4 {
            let at = position + beat as f64 * period;
            if at >= duration {
                break;
            }
            total += center_rms(at);
            count += 1;
        }
        if count > 0 { total / count as f64 } else { 0. }
    };
    let baseline = average(anchor);
    let old_hits = grid_hits(pcm, rate, anchor, period, 10, 1.5);
    if old_hits >= 7 {
        return unchanged;
    }
    for beat in 1..=16 {
        let at = anchor + beat as f64 * period;
        if at >= 5. {
            break;
        }
        if grid_hits(pcm, rate, at, period, 10, 1.5) >= 8 && average(at) > 2. * baseline {
            return GridValidation {
                validated_first_beat: at,
                skipped_beats: beat,
            };
        }
    }
    unchanged
}
