use super::{
    Context,
    onset::js_round,
    phase::{self, CorrectionState},
};
use crate::lexicon_timing::intro::IntroKind;

pub(super) fn select(c: &Context<'_>, state: &mut CorrectionState) -> f64 {
    let first = c.peaks[0];
    let beats = first / c.period;
    let mut anchor = first;
    if beats > 0.4 && beats < 2.5 {
        let limit = (first - 0.08).max(0.);
        let threshold = if beats > 0.3 && beats < 0.7 { 1.5 } else { 2. };
        let (mut best, mut earlier) = (0., None);
        let mut at = 0.002;
        while at < limit {
            let ratio = c.ar(at, 20.);
            if ratio > best && ratio > threshold {
                best = ratio;
                earlier = Some(at);
            }
            at += 0.005;
        }
        if let Some(earlier) = earlier {
            let hits = (1..=4)
                .filter(|beat| c.ar(earlier + *beat as f64 * c.period, 20.) > 1.5)
                .count();
            let same_grid = (first - earlier) / c.period > 1.
                && (1..=4).any(|beat| (earlier + beat as f64 * c.period - first).abs() < 0.05);
            if hits >= 2 && !same_grid {
                let (mut maximum, mut position) = (0., earlier);
                let mut probe = earlier;
                while probe < earlier + 0.04 && probe < first - 0.05 {
                    let rms = c.rms(probe, 20.);
                    if rms > maximum {
                        maximum = rms;
                        position = probe;
                    }
                    probe += 0.002;
                }
                anchor = position;
            }
        }
    }
    let mut skipped_first = false;
    if beats < 0.1 && first >= 0.01 && c.peaks.len() >= 4 {
        let second_beats = c.peaks[1] / c.period;
        if second_beats > 0.85 && second_beats < 1.15 {
            let mut next: Vec<_> = c
                .peaks
                .iter()
                .take(4)
                .skip(1)
                .map(|at| c.rms(*at, 30.))
                .collect();
            next.sort_by(f64::total_cmp);
            if next[next.len() / 2] / (c.rms(first, 30.) + 1e-4) > 1.6 && c.ar(first, 20.) < 1.5 {
                anchor = c.peaks[1];
                skipped_first = true;
            }
        }
    }
    let mut used_early_energy = false;
    let moved = (anchor - first).abs() > 0.005;
    let anchor_beats = anchor / c.period;
    if !skipped_first
        && ((beats > 0.4 && beats < 0.6)
            || (moved && anchor_beats > 0.3 && anchor_beats < 0.7 && anchor > 0.01))
    {
        let limit = (first - 0.06).max(0.);
        let (mut maximum, mut position) = (0., None);
        let mut index = 0;
        while (index as f64) < limit * c.rate as f64 {
            let rms = c.rms_index(index, c.width(25.));
            if rms > maximum {
                maximum = rms;
                position = Some(index as f64 / c.rate as f64);
            }
            index += c.width(5.);
        }
        if let Some(position) = position
            && maximum > 0.3 * c.rms(first, 25.)
        {
            anchor = c.dynamic(position, Some((20., 0.2)));
            used_early_energy = true;
        }
    }
    let mut pickup = false;
    if !used_early_energy && beats > 0.85 && beats < 0.99 {
        let following = c
            .peaks
            .get(1)
            .map(|next| (next - c.period) / c.period)
            .unwrap_or(0.);
        if c.ar(c.period, 20.) >= 1.3 && (following - js_round(following)).abs() < 0.15 {
            anchor = c.period;
            pickup = true;
        }
    }
    let mut sharpness = None;
    if !pickup {
        let index = c.index(anchor);
        let before = index.saturating_sub(c.width(20.));
        let width = c.width(3.);
        let previous = c.rms_index(before, width);
        let ratio = if previous > 1e-4 {
            c.rms_index(index, width) / previous
        } else {
            100.
        };
        sharpness = Some(ratio);
        anchor = if (0.85..3.).contains(&beats) {
            c.dynamic(anchor, Some((20., 0.2)))
        } else if beats > 0.15 && beats < 0.5 {
            c.dynamic(anchor, Some((60., 0.15)))
        } else if ratio < 2. && beats >= 3. {
            let result = phase::calibrate_onset(c.pcm, c.rate, anchor, c.period, &c.peaks);
            if result.used_calibration {
                result.position
            } else {
                c.dynamic(anchor, None)
            }
        } else {
            c.dynamic(anchor, None)
        };
    }
    let strong_onset =
        sharpness.is_some_and(|ratio| ratio >= 15.) || (0.005..=0.025).contains(&anchor);
    if (170. ..=178.).contains(&c.bpm) && !strong_onset {
        let result = phase::calibrate_drop(c.pcm, c.rate, anchor, c.bpm, &c.peaks);
        if result.used_calibration {
            anchor = result.position;
        }
    }
    if c.bpm >= 200. {
        let result = phase::verify_drop(c.pcm, c.rate, anchor, c.bpm, &c.peaks);
        if result.used_correction {
            anchor = result.position;
        }
    }
    let attack = c.ar(anchor, 30.);
    // The shipped near-zero helper currently returns unchanged.
    anchor = c.correct(phase::near_zero(anchor).corrected_first_beat, state);
    if beats > 0.15 && beats < 0.5 && anchor > 0.05 && c.rms(0., 5.) < 0.005 {
        let mut index = 0;
        while index < c.index(anchor) {
            if c.rms_index(index, c.width(3.)) > 0.02 {
                let start = index as f64 / c.rate as f64;
                if anchor - start > 0.03 {
                    anchor = start;
                }
                break;
            }
            index += c.width(1.);
        }
    }
    if c.intro.has_intro
        && c.intro.intro_end_position >= 20.
        && c.intro.intro_type == IntroKind::Buildup
        && attack < 2.
    {
        let candidates: Vec<_> = c
            .peaks
            .iter()
            .copied()
            .filter(|at| {
                *at >= (c.intro.intro_end_position - 2.).max(0.)
                    && *at <= c.intro.intro_end_position + 2.
            })
            .collect();
        if let Some(&first) = candidates.first() {
            let (mut best, mut position) = (0., first);
            for at in candidates {
                let ratio = c.ar(at, 30.);
                if ratio > best {
                    best = ratio;
                    position = at;
                }
            }
            if best > 2. {
                let distance = (position - anchor) / c.period;
                let nearest = js_round(distance);
                if (distance - nearest).abs() * c.period > 0.02 {
                    anchor = position - nearest * c.period;
                }
            }
        }
    }
    anchor
}
