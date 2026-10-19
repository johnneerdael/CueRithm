use super::*;

pub(in crate::lexicon_timing) fn align(
    pcm: &[f32],
    rate: u32,
    anchor: f64,
    bpm: f64,
    first: usize,
    last: usize,
    energy: bool,
) -> Alignment {
    let mut result = Alignment {
        corrected_first_beat: anchor,
        applied_correction: false,
        offset_ms: 0,
        improvement: 0.,
    };
    if !valid(pcm, rate, anchor, bpm) || first > last || last > 1000 {
        return result;
    }
    let period = 60. / bpm;
    let duration = pcm.len() as f64 / rate as f64;
    let metric = |at: f64| {
        if energy {
            attack_ratio(pcm, rate, at, 15.) * rms(pcm, rate, at, 0.015)
        } else {
            let ratio = attack_ratio(pcm, rate, at, 10.);
            ratio * ratio
        }
    };
    let average = |shift: f64| {
        let (mut total, mut count) = (0., 0);
        for beat in first..=last {
            let at = anchor + beat as f64 * period + shift;
            if at < 0. || at >= duration - 0.015 {
                continue;
            }
            total += metric(at);
            count += 1;
        }
        if count > 0 { total / count as f64 } else { 0. }
    };
    let baseline = average(0.);
    let mut best = baseline;
    let mut offset = 0;
    for shift in -100..=100 {
        if shift == 0 {
            continue;
        }
        let score = average(shift as f64 / 1000.);
        if score > best {
            best = score;
            offset = shift;
        }
    }
    let improvement = if baseline > 0. { best / baseline } else { 0. };
    result.offset_ms = offset;
    result.improvement = improvement;
    if offset.abs() < 15 || improvement < 1.3 {
        return result;
    }
    let left = average((offset - 10) as f64 / 1000.);
    let right = average((offset + 10) as f64 / 1000.);
    if !(best > 1.1 * left && best > 1.1 * right) {
        return result;
    }
    let mut position = anchor + offset as f64 / 1000.;
    if position < 0.003 {
        position = 0.;
    }
    if !energy {
        if position < 0.1 && anchor >= 0.1 {
            return result;
        }
        if attack_ratio(pcm, rate, position, 10.) < 0.8 * attack_ratio(pcm, rate, anchor, 10.) {
            return result;
        }
    }
    result.corrected_first_beat = position;
    result.applied_correction = true;
    result
}
