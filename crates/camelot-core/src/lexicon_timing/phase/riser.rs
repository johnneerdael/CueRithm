use super::*;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(in crate::lexicon_timing) struct Riser {
    pub corrected_first_beat: f64,
    pub was_in_riser: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub strategy: Option<&'static str>,
}
impl Riser {
    fn unchanged(anchor: f64) -> Self {
        Self {
            corrected_first_beat: anchor,
            was_in_riser: false,
            strategy: None,
        }
    }
    fn corrected(position: f64, strategy: &'static str) -> Self {
        Self {
            corrected_first_beat: position,
            was_in_riser: true,
            strategy: Some(strategy),
        }
    }
}
pub(in crate::lexicon_timing) fn validate_riser(
    pcm: &[f32],
    rate: u32,
    anchor: f64,
    bpm: f64,
    peaks: &[f64],
) -> Riser {
    if !valid(pcm, rate, anchor, bpm)
        || anchor >= 1.
        || peaks.iter().any(|p| (p - anchor).abs() < 0.02)
    {
        return Riser::unchanged(anchor);
    }
    let period = 60. / bpm;
    let permissive = grid_hits(pcm, rate, anchor, period, 20, 2.);
    let strict = grid_hits(pcm, rate, anchor, period, 10, 3.);
    if permissive >= 3 || strict >= 2 {
        return Riser::unchanged(anchor);
    }
    let soft = grid_hits(pcm, rate, anchor, period, 25, 1.3);
    if soft >= 7 {
        return Riser::unchanged(anchor);
    }
    let mut selected = None;
    for peak in peaks
        .iter()
        .copied()
        .filter(|p| *p > anchor && *p <= anchor + 8.)
    {
        let hits = grid_hits(pcm, rate, peak, period, 40, 2.);
        if hits >= 5 {
            selected = Some(peak);
            break;
        }
        if hits >= 1 {
            for &peer in peaks {
                if peer == peak {
                    continue;
                }
                let apart = (peer - peak).abs() / period;
                let nearest = apart.round();
                if (2. ..=8.).contains(&nearest)
                    && (apart - nearest).abs() < 0.08
                    && attack_ratio(pcm, rate, peer, 15.) > 2.
                {
                    let peer_hits = grid_hits(pcm, rate, peer, period, 40, 2.);
                    let prefer = peer < peak && peer_hits > hits;
                    let score = if prefer { peer_hits } else { hits };
                    if score < 3 && soft < 4 {
                        continue;
                    }
                    selected = Some(if prefer { peer } else { peak });
                    break;
                }
            }
            if selected.is_some() {
                break;
            }
        }
    }
    if let Some(peak) = selected {
        let refined = best_attack(pcm, rate, peak, 25, 2, 15.).0;
        let mut back = 0;
        loop {
            back += 1;
            let previous = refined - back as f64 * period;
            if previous < 0. || best_attack(pcm, rate, previous, 40, 2, 15.).1 <= 2. {
                break;
            }
        }
        let earliest = refined - (back - 1) as f64 * period;
        let position = best_attack(pcm, rate, earliest, 25, 2, 15.).0;
        let advanced = (position - anchor) / period;
        if advanced >= 1.5
            || (advanced <= -0.9 && grid_hits(pcm, rate, position, period, 40, 2.) >= 6)
        {
            return Riser::corrected(position, "A");
        }
        // A rejected peak strategy returns directly; it does not fall through
        // to the grid strategy in the original detector.
        return Riser::unchanged(anchor);
    }
    if soft < 2 {
        return Riser::unchanged(anchor);
    }
    let duration = pcm.len() as f64 / rate as f64;
    let end = (anchor + 7.).min(duration - 8. * period);
    let mut trial = anchor + period / 2.;
    while trial < end {
        if grid_hits(pcm, rate, trial, period, 40, 2.) >= 4 {
            let refined = best_attack(pcm, rate, trial, 40, 2, 15.).0;
            let mut back = 0;
            loop {
                let previous = refined - (back + 4) as f64 * period;
                if previous < 0. {
                    break;
                }
                let mut hits = 0;
                for beat in 0..4 {
                    let at = previous + beat as f64 * period;
                    if best_attack(pcm, rate, at, 90, 2, 15.).1 > 1.5 {
                        hits += 1;
                    }
                }
                if hits < 2 {
                    break;
                }
                back += 4;
            }
            while back > 0 {
                let at = refined - back as f64 * period;
                if at < 0. || best_attack(pcm, rate, at, 90, 2, 15.).1 > 2. {
                    break;
                }
                back -= 1;
            }
            let earliest = refined - back as f64 * period;
            let position = best_attack(pcm, rate, earliest, 40, 2, 15.).0;
            if grid_hits(pcm, rate, position, period, 40, 2.) >= strict + 2
                && (position - anchor) / period >= 4.
            {
                return Riser::corrected(position, "B");
            }
        }
        trial += period / 2.;
    }
    Riser::unchanged(anchor)
}
