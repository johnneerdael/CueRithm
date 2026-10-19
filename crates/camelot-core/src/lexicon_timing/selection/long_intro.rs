use super::{Context, anchors, onset, phase::CorrectionState};
use crate::lexicon_timing::intro::IntroKind;

pub(super) fn select(c: &Context<'_>, state: &mut CorrectionState) -> Option<f64> {
    if !c.intro.has_intro || c.intro.intro_end_position < 20. {
        return None;
    }
    let first = c.peaks[0];
    if (3. ..15.).contains(&first)
        && c.initial_max_rms(1f64.min(first - 0.1)) < 0.5 * c.rms(first, 30.)
        && c.ar(first, 30.) >= 2.
    {
        let refined = c.refine_first_peak();
        let drop = c.snap(refined);
        let walked = c.walk(drop, Some(c.ar(refined, 30.)));
        let mut anchor = walked.position;
        if c.intro.intro_type == IntroKind::Buildup
            && anchor < c.period
            && drop - anchor > 4. * c.period
            && walked.beats_walked_back > 0.
            && c.ar(anchor, 30.) < 2.
        {
            anchor = drop;
        }
        return Some(c.correct(anchor, state));
    }
    // A quiet isolated leading pulse must not count as an early drum section.
    let early_drums = c
        .peaks
        .iter()
        .enumerate()
        .take_while(|(_, at)| **at < 5.)
        .any(|(index, &at)| {
            let start = c.index(at);
            let before = start.saturating_sub(c.width(30.));
            let rms = if start > before {
                c.rms_index(before, start - before)
            } else {
                0.
            };
            !(rms < 0.01 && (0.01..0.1).contains(&at))
                && c.ar(at, 30.) > 3.
                && (c.peaks.get(index + 1).copied().unwrap_or(f64::INFINITY) - at) / c.period <= 4.
        });
    if early_drums {
        return None;
    }
    let candidates: Vec<_> = c
        .peaks
        .iter()
        .copied()
        .filter(|at| {
            *at >= (c.intro.intro_end_position - 2.).max(0.)
                && *at <= c.intro.intro_end_position + 2.
        })
        .map(|at| (at, c.ar(at, 30.)))
        .collect();
    let mut best = *candidates.first()?;
    for &candidate in &candidates {
        if candidate.1 > best.1 {
            best = candidate;
        }
    }
    // The strength supplied to walkback remains the pre-preference maximum.
    let strength = best.1;
    let earlier = candidates
        .iter()
        .find(|candidate| (candidate.0 - (best.0 - c.period)).abs() < 0.05)
        .filter(|candidate| candidate.1 > 1.8)
        .copied();
    if let Some(candidate) = earlier {
        best = candidate;
    }
    if best.1 <= 2. {
        return None;
    }
    let refined = if best.1 >= 3. || earlier.is_some() {
        best.0
    } else {
        c.dynamic(
            onset::refine_drop(c.pcm, c.rate, best.0, 0.03).position,
            None,
        )
    };
    let mut drop = c.snap(refined);
    let beats = drop / c.period;
    if (beats - 32. * onset::js_round(beats / 32.)).abs() * c.period > 0.1
        && let Some(dnb) = anchors::dnb_anchor(c.pcm, c.rate, c.bpm, &c.peaks, Some(drop))
    {
        drop = c.snap(c.dynamic(
            onset::refine_drop(c.pcm, c.rate, dnb.position, 0.03).position,
            None,
        ));
    }
    let walked = c.walk(drop, Some(strength));
    let mut anchor = walked.position;
    if c.intro.intro_type == IntroKind::Buildup
        && anchor < c.period
        && walked.beats_walked_back > 32.
        && first > anchor + 0.5 * c.period
        && first < 4. * c.period
    {
        if anchor < 0.1 {
            let hits = (1..=4)
                .filter(|beat| c.ar(anchor + *beat as f64 * c.period, 20.) > 1.5)
                .count();
            if hits < 2 {
                let width = c.width(10.);
                let current = onset::power(c.pcm, c.index(anchor), c.index(anchor) + width);
                let first_power = onset::power(c.pcm, c.index(first), c.index(first) + width);
                if first_power > 0. && current < 0.05 * first_power {
                    anchor = first;
                }
            }
        } else if c.ar(first, 30.) > c.ar(anchor, 30.) {
            anchor = first;
        }
    }
    Some(c.correct(anchor, state))
}
