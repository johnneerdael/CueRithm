use super::*;

fn refine(pcm: &[f32], rate: u32, anchor: f64) -> Option<f64> {
    if !(0.1..=2.).contains(&anchor) {
        return None;
    }
    let onset = precise_onset(pcm, rate, anchor, 0.05);
    let shift = 1000. * (anchor - onset);
    if !(15. ..=40.).contains(&shift) || attack_ratio(pcm, rate, onset, 15.) < 80. {
        return None;
    }
    Some(onset)
}
pub(in crate::lexicon_timing) fn subsequent(
    pcm: &[f32],
    rate: u32,
    anchor: f64,
    bpm: f64,
    options: &SubsequentOptions,
    state: &mut CorrectionState,
) -> Subsequent {
    let unchanged = near_zero(anchor);
    if !valid(pcm, rate, anchor, bpm)
        || !options.search_window_ms.is_finite()
        || !(0. ..=1000.).contains(&options.search_window_ms)
        || !options.max_offset_ms.is_finite()
        || !options.max_std_dev_ms.is_finite()
        || options.min_attack_ratio.is_some_and(|v| !v.is_finite())
    {
        return unchanged;
    }
    let period = 60. / bpm;
    let duration = pcm.len() as f64 / rate as f64;
    let search = options.search_window_ms / 1000.;
    let mut offsets = vec![];
    for beat in 4..=80 {
        let target = anchor + beat as f64 * period;
        if target + search > duration {
            break;
        }
        let end = target + search;
        let mut at = target - search;
        let (mut position, mut maximum) = (target, 0.);
        while at <= end {
            if at >= 0. {
                let ratio = attack_ratio(pcm, rate, at, 15.);
                if ratio > maximum {
                    maximum = ratio;
                    position = at;
                }
            }
            at += 0.002;
        }
        if maximum > 1.5 {
            offsets.push(position - target);
        }
    }
    if offsets.len() >= 10 {
        offsets.sort_by(f64::total_cmp);
        let trim = (0.15 * offsets.len() as f64).floor() as usize;
        let values = &offsets[trim..offsets.len() - trim];
        if values.len() >= 8 {
            let median = values[values.len() / 2];
            let mean = values.iter().sum::<f64>() / values.len() as f64;
            let variance =
                values.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / values.len() as f64;
            let delta_ms = 1000. * median;
            let std_ms = 1000. * variance.sqrt();
            let negative =
                values.iter().filter(|v| **v < -0.002).count() as f64 / values.len() as f64 >= 0.9;
            let positive =
                values.iter().filter(|v| **v > 0.002).count() as f64 / values.len() as f64 >= 0.9;
            let mut suppress = (-10. ..=-3.).contains(&delta_ms);
            if !suppress
                && (-25. ..=-3.).contains(&delta_ms)
                && attack_ratio(pcm, rate, anchor, 30.) >= 30.
            {
                suppress = true;
            }
            if std_ms < options.max_std_dev_ms
                && delta_ms.abs() >= 5.
                && delta_ms.abs() <= options.max_offset_ms
                && !suppress
                && (negative || positive)
            {
                let mut position = anchor + median;
                if position < 0.003 {
                    position = 0.;
                }
                let corrected = attack_ratio(pcm, rate, position, 30.);
                let original = attack_ratio(pcm, rate, anchor, 30.);
                let minimum =
                    options
                        .min_attack_ratio
                        .unwrap_or(if std_ms < 1. { 0.8 } else { 1.5 });
                let sentinel = original == 100. && delta_ms > 0. && corrected >= 2.;
                let veto = (!(std_ms < 1. && values.len() >= 20)
                    || (original > 20. && original > 50. * corrected))
                    && (anchor >= 0.005 || delta_ms.abs() <= 10.)
                    && original > 10. * corrected
                    && !sentinel;
                if corrected > minimum && !veto {
                    state.last = Some(AppliedCorrection {
                        position,
                        delta_ms,
                        source: "sbc",
                    });
                    return Subsequent {
                        corrected_first_beat: position,
                        applied_correction: true,
                    };
                }
            }
        }
    }
    // The source's onset-refinement fallback deliberately does not update its
    // last-subsequent-correction bookkeeping.
    match refine(pcm, rate, anchor) {
        Some(position) => Subsequent {
            corrected_first_beat: position,
            applied_correction: true,
        },
        None => unchanged,
    }
}
