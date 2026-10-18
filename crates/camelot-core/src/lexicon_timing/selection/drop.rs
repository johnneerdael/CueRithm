use super::{
    Context, anchors,
    onset::{self, js_round},
    phase::{self, CorrectionState},
};

pub(super) fn weak_start(c: &Context<'_>, state: &mut CorrectionState) -> Option<f64> {
    if c.ar(c.peaks[0], 30.) >= 2. {
        return None;
    }
    let drop = anchors::find_drop(c.pcm, c.rate, c.period, &c.peaks, 120.)?;
    if drop.attack_ratio < 2.5 {
        return None;
    }
    let refined = onset::refine_drop(c.pcm, c.rate, drop.position, 0.03).position;
    let anchor = c
        .walk(c.snap(c.dynamic(refined, None)), Some(drop.attack_ratio))
        .position;
    Some(c.correct(anchor, state))
}

pub(super) fn select(c: &Context<'_>, state: &mut CorrectionState) -> f64 {
    let first = c.peaks[0];
    if first > 30. {
        let (mut best, mut position) = (0., first);
        let mut index = c.index((first - 0.1).max(0.));
        while index <= c.index(first) {
            let at = index as f64 / c.rate as f64;
            let ratio = c.ar(at, 30.);
            if ratio > best {
                best = ratio;
                position = at;
            }
            index += c.width(2.);
        }
        let anchor = c
            .walk(c.snap(c.dynamic(position, None)), Some(best))
            .position;
        return c.correct(anchor, state);
    }
    let Some(drop) = anchors::find_drop(c.pcm, c.rate, c.period, &c.peaks, 180.) else {
        return c.correct(c.walk(first, None).position, state);
    };
    let distance = (drop.position - first) / c.period;
    if (3. ..=15.).contains(&first) && (0. ..4.).contains(&distance) {
        return c.correct(first, state);
    }
    if drop.position < 2. && c.peaks.len() >= 2 && drop.position != first && first / c.period < 3. {
        return early_drop(c, state);
    }
    let refined = onset::refine_drop(c.pcm, c.rate, drop.position, 0.03);
    let mut position = refined.position;
    if refined.attack_ratio >= 2. && refined.attack_ratio < 8. {
        position = c.dynamic(position, Some((8., 0.3)));
    }
    let snapped = c.snap(position);
    if snapped > 20.
        && let Some(dnb) = anchors::dnb_anchor(c.pcm, c.rate, c.bpm, &c.peaks, Some(snapped))
    {
        let beats = snapped / c.period;
        if (beats - 32. * js_round(beats / 32.)).abs() * c.period > 0.1 {
            let refined = onset::refine_drop(c.pcm, c.rate, dnb.position, 0.03).position;
            let anchor = c
                .walk(c.snap(c.dynamic(refined, None)), Some(dnb.attack_ratio))
                .position;
            return c.correct(anchor, state);
        }
    }
    let mut anchor = c.walk(snapped, Some(drop.attack_ratio)).position;
    // If walkback landed in silence, only skip ahead when the intervening bed
    // is also quiet; otherwise its early musical content must remain on-grid.
    if c.rms(anchor, 20.) < 0.005 {
        let start = c.index(anchor);
        let limit = c.index((anchor + 8. * c.period).min(c.pcm.len() as f64 / c.rate as f64));
        let mut index = start;
        let mut sound = None;
        while index < limit {
            let end = (index + c.width(3.)).min(c.pcm.len());
            let rms = (onset::power(c.pcm, index, end) / (end - index) as f64).sqrt();
            if rms > 0.02 {
                sound = Some(index);
                break;
            }
            index += c.width(1.);
        }
        if let Some(sound) = sound {
            let mut quiet = true;
            let mut index = start;
            while (index as isize) < sound as isize - c.width(20.) as isize {
                if c.rms_index(index, c.width(20.)) >= 0.01 {
                    quiet = false;
                    break;
                }
                index += c.width(10.);
            }
            if quiet {
                anchor = sound as f64 / c.rate as f64;
            }
        }
    }
    if c.bpm >= 200. {
        let correction = phase::verify_drop(c.pcm, c.rate, anchor, c.bpm, &c.peaks);
        if correction.used_correction {
            anchor = correction.position;
        }
    }
    let correction = phase::subsequent(
        c.pcm,
        c.rate,
        anchor,
        c.bpm,
        &phase::SubsequentOptions::default(),
        state,
    );
    if correction.applied_correction {
        anchor = correction.corrected_first_beat;
    } else {
        let grid = phase::align(c.pcm, c.rate, anchor, c.bpm, 4, 80, true);
        if grid.applied_correction {
            anchor = grid.corrected_first_beat;
            state.note_grid(anchor, grid.offset_ms as f64);
        }
    }
    anchor
}

fn early_drop(c: &Context<'_>, state: &mut CorrectionState) -> f64 {
    let first = c.peaks[0];
    let beats = first / c.period;
    let end = c.index(first).saturating_sub(c.width(20.));
    let (mut best, mut position) = (0., None);
    let mut index = 0;
    while index < end && index + c.width(30.) < c.pcm.len() {
        let at = index as f64 / c.rate as f64;
        let ratio = c.ar(at, 15.);
        if ratio > best {
            best = ratio;
            position = Some(at);
        }
        index += c.width(2.);
    }
    if let Some(mut at) = position
        && best > 1.3
    {
        if (1..=4).any(|beat| (at + beat as f64 * c.period - first).abs() < 0.05) {
            at = first;
        }
        return c.correct(at, state);
    }
    let index = c.index(first);
    let previous = c.rms_index(index.saturating_sub(c.width(20.)), c.width(3.));
    let sharpness = if previous > 1e-4 {
        c.rms_index(index, c.width(3.)) / previous
    } else {
        100.
    };
    let at = if sharpness < 2. && (1. ..3.).contains(&beats) {
        c.dynamic(first, Some((60., 0.15)))
    } else if (0.1..0.5).contains(&beats) {
        c.dynamic(first, Some((20., 0.2)))
    } else {
        c.dynamic(first, None)
    };
    c.correct(at, state)
}
