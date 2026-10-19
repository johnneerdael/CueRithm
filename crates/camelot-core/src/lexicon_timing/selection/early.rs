use super::{Context, phase::CorrectionState};

pub(super) fn delayed_transient(c: &Context<'_>, state: &mut CorrectionState) -> Option<f64> {
    let first = c.peaks[0];
    if (3. ..15.).contains(&first)
        && c.ar(first, 30.) >= 2.
        && c.initial_max_rms(1f64.min(first - 0.5)) > 0.1 * c.rms(first, 50.)
    {
        let at = c.refine_first_peak();
        return Some(c.correct(c.walk(c.snap(at), Some(c.ar(at, 30.))).position, state));
    }
    None
}

pub(super) fn non_immediate(
    c: &Context<'_>,
    immediate: bool,
    state: &mut CorrectionState,
) -> Option<f64> {
    if immediate {
        return None;
    }
    let first = c.peaks[0];
    if first < 0.05 && c.rms(0., 30.) > 0.1 {
        let mut at = c.dynamic(first, Some((15., 0.22)));
        if first >= 0.01 && at < 0.003 && at > 0. && c.rms(0., 30.) > 0.5 * c.rms(at, 30.) {
            at = 0.;
        }
        return Some(c.correct(at, state));
    }
    if c.ar(first, 15.) > 1.5 || c.peaks.len() < 6 || !(0.01..0.5).contains(&first) {
        return None;
    }
    let mut after_gap = None;
    for index in 0..c.peaks.len() - 1 {
        if c.peaks[index] >= 10. {
            break;
        }
        let gap = c.peaks[index + 1] - c.peaks[index];
        if gap / c.period >= 5. && index <= 2 {
            let middle = c.peaks[index] + gap / 2.;
            if c.rms(middle, 50.) > 0.5 * c.rms(first, 50.) {
                break;
            }
            after_gap = Some(index + 1);
            break;
        }
    }
    let index = after_gap?;
    let peaks = &c.peaks[index..];
    let consistent = peaks
        .windows(2)
        .take(10)
        .filter(|pair| {
            let beats = (pair[1] - pair[0]) / c.period;
            (beats - 1.).abs() < 0.25 || (beats - 2.).abs() < 0.25
        })
        .count();
    if consistent >= 5 {
        Some(c.correct(c.dynamic(peaks[0], None), state))
    } else {
        None
    }
}

pub(super) fn near_zero(c: &Context<'_>, state: &mut CorrectionState) -> Option<f64> {
    let first = c.peaks[0];
    if first / c.period >= 0.1 {
        return None;
    }
    let at = if first < 0.01 {
        let at = c.dynamic(first, Some((10., 0.25)));
        if at < 0.002 { 0. } else { at }
    } else if first < 0.02 {
        let at = c.dynamic(first, Some((15., 0.2)));
        if at < 0.005 { 0. } else { at }
    } else {
        return None;
    };
    Some(c.correct(at, state))
}

pub(super) fn initial_energy(c: &Context<'_>, state: &mut CorrectionState) -> Option<f64> {
    let first = c.peaks[0];
    let beats = first / c.period;
    if !(0.5..=8.).contains(&beats) {
        return None;
    }
    let first_rms = c.rms(first, 30.);
    if c.rms(0., 50.) <= 0.1 * first_rms {
        return None;
    }
    let width = c.width(30.);
    let mut first_energy = 0;
    let mut index = 0;
    while index < c.index(first) && index + width < c.pcm.len() {
        if c.rms_index(index, width) > 0.05 * first_rms {
            first_energy = index;
            break;
        }
        index += c.width(2.);
    }
    let at = first_energy as f64 / c.rate as f64;
    if at >= 0.1 {
        return None;
    }
    let attack = c.ar(at, 20.);
    let mut noise = false;
    if first > 0.5 {
        if attack < 3. {
            noise = true;
        } else {
            let threshold = if at < 0.005 { 1.2 } else { 1.5 };
            noise = c.following_hits(at, 3, threshold) < 2;
            if !noise {
                noise = (1..=3).any(|beat| (at + beat as f64 * c.period - first).abs() < 0.05);
            }
        }
    }
    let anchor = if noise {
        c.dynamic(first, Some((20., 0.2)))
    } else {
        at
    };
    Some(c.correct(anchor, state))
}

pub(super) fn late_first_peak(c: &Context<'_>, state: &mut CorrectionState) -> Option<f64> {
    let first = c.peaks[0];
    if first / c.period <= 8. {
        return None;
    }
    let width = c.width(30.);
    let cutoff = 0.15 * c.rms(first, 30.);
    let limit = c.index(3f64.min(first - 0.1));
    let mut index = 0;
    let mut detected = None;
    while index < limit && index + width < c.pcm.len() {
        if c.rms_index(index, width) > cutoff {
            let mut earliest = index;
            let mut before = index as isize - c.width(10.) as isize;
            while before >= 0 && before > index as isize - c.width(50.) as isize {
                if c.rms_index(before as usize, width) < 0.5 * cutoff {
                    earliest = before as usize + c.width(2.);
                    break;
                }
                before -= c.width(2.) as isize;
            }
            detected = Some(earliest as f64 / c.rate as f64);
            break;
        }
        index += c.width(5.);
    }
    let mut at = detected?;
    let limit = (at + 0.05).min(first);
    let (mut best, mut position) = (0., at);
    let mut probe = at;
    while probe <= limit {
        let rms = c.rms(probe, 30.);
        if rms > best {
            best = rms;
            position = probe;
        }
        probe += 0.002;
    }
    at = position;
    let attack = c.ar(at, 30.);
    if (1.5..3.).contains(&attack) && at < 0.1 && c.following_hits(at, 4, 1.2) >= 3 {
        return Some(c.correct(at, state));
    }
    if attack <= 3. {
        return None;
    }
    let steps = (at / c.period).floor() as usize;
    let mut anchor = at;
    if steps == 0 && at > 0.1 && first > 30. {
        let mut probe = 0.;
        while probe <= at - 0.05 {
            let ratio = c.ar(probe, 15.);
            if ratio > 2. {
                let before = if probe > 0.002 {
                    c.ar(probe - 0.002, 15.)
                } else {
                    0.
                };
                let after = c.ar(probe + 0.002, 15.);
                if ratio >= before && ratio >= after && c.following_hits(probe, 3, 1.1) >= 2 {
                    anchor = probe;
                    break;
                }
            }
            probe += 0.002;
        }
    }
    for beat in 1..=steps.min(8) {
        let earlier = at - beat as f64 * c.period;
        if earlier < 0.01 {
            let (mut best, mut position) = (0., 0.);
            let mut probe = 0.;
            while probe <= 0.05 {
                let ratio = c.ar(probe, 15.);
                if ratio > best {
                    best = ratio;
                    position = probe;
                }
                probe += 0.002;
            }
            if best > 2. {
                anchor = position;
            }
            break;
        }
        let (mut best, mut position) = (0., earlier);
        let mut probe = earlier - 0.03;
        while probe <= earlier + 0.03 {
            if probe >= 0. {
                let ratio = c.ar(probe, 15.);
                if ratio > best {
                    best = ratio;
                    position = probe;
                }
            }
            probe += 0.003;
        }
        if best <= 2. {
            break;
        }
        anchor = position;
    }
    Some(c.correct(anchor, state))
}
