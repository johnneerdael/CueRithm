use super::onset::{DropPosition, attack_ratio, js_round};

fn valid(pcm: &[f32], rate: u32, peaks: &[f64]) -> bool {
    (8000..=192000).contains(&rate)
        && peaks
            .iter()
            .all(|p| p.is_finite() && *p >= 0. && *p <= pcm.len() as f64 / rate as f64)
}
pub(super) fn find_drop(
    pcm: &[f32],
    rate: u32,
    period: f64,
    peaks: &[f64],
    max_seconds: f64,
) -> Option<DropPosition> {
    if !valid(pcm, rate, peaks)
        || !period.is_finite()
        || period <= 0.
        || !max_seconds.is_finite()
        || max_seconds < 0.
    {
        return None;
    }
    let limit = max_seconds.min(pcm.len() as f64 / rate as f64 - 1.);
    let mut candidates = vec![];
    for &at in peaks {
        if at > limit {
            break;
        }
        candidates.push((at, attack_ratio(pcm, rate, at, 30.)));
    }
    if candidates.is_empty() {
        return None;
    }
    let mut ratios: Vec<f64> = candidates.iter().map(|c| c.1).collect();
    ratios.sort_by(f64::total_cmp);
    let percentile = ratios[(0.75 * ratios.len() as f64).floor() as usize];
    let cutoff = (if percentile == 0. {
        ratios[ratios.len() - 1]
    } else {
        percentile
    })
    .max(1.5);
    let mut best = candidates
        .iter()
        .find(|(_, r)| *r >= cutoff)
        .copied()
        .unwrap_or_else(|| {
            let mut best = candidates[0];
            for &(position, ratio) in &candidates {
                if ratio > best.1 {
                    best = (position, ratio);
                }
            }
            best
        });
    if best.1 < 3.5 {
        let first = candidates[0].0;
        let on_bars: Vec<_> = candidates
            .iter()
            .filter(|(at, _)| {
                let beats = (at - first) / period;
                let nearest = js_round(beats);
                (beats - nearest).abs() < 0.1 && nearest % 4. == 0.
            })
            .copied()
            .collect();
        if let Some(&first) = on_bars.first() {
            let mut strongest = first;
            for &candidate in &on_bars {
                if candidate.1 > strongest.1 {
                    strongest = candidate;
                }
            }
            if strongest.1 >= 2. * best.1 {
                best = strongest;
            }
        }
    }
    Some(DropPosition {
        position: best.0,
        attack_ratio: best.1,
    })
}
pub(super) fn dnb_anchor(
    pcm: &[f32],
    rate: u32,
    bpm: f64,
    peaks: &[f64],
    preferred: Option<f64>,
) -> Option<DropPosition> {
    if !valid(pcm, rate, peaks)
        || !bpm.is_finite()
        || !(170. ..=178.).contains(&bpm)
        || preferred.is_some_and(|p| !p.is_finite())
    {
        return None;
    }
    let period = 60. / bpm;
    let mut positions = vec![32. * period, 64. * period, 128. * period, 192. * period];
    if let Some(preferred) = preferred {
        positions.sort_by(|a, b| (a - preferred).abs().total_cmp(&(b - preferred).abs()));
    }
    for position in positions {
        let selected: Vec<_> = peaks
            .iter()
            .copied()
            .filter(|p| (p - position).abs() < 0.05)
            .collect();
        let Some(&first) = selected.first() else {
            continue;
        };
        let mut best = DropPosition {
            position: first,
            attack_ratio: attack_ratio(pcm, rate, first, 30.),
        };
        for at in selected {
            let ratio = attack_ratio(pcm, rate, at, 30.);
            if ratio > best.attack_ratio {
                best = DropPosition {
                    position: at,
                    attack_ratio: ratio,
                };
            }
        }
        if best.attack_ratio >= 2. {
            return Some(best);
        }
    }
    None
}
pub(super) fn starts_immediately(peaks: &[f64], period: f64, limit: f64) -> bool {
    if !period.is_finite()
        || period <= 0.
        || !limit.is_finite()
        || limit <= 0.
        || peaks.iter().any(|p| !p.is_finite() || *p < 0.)
    {
        return false;
    }
    let early: Vec<f64> = peaks.iter().copied().filter(|p| *p < limit).collect();
    if early.len() < 4 || early[0] > 2. {
        return false;
    }
    if early[0] >= 0.01 && early[0] < 0.5 {
        for index in 0..3.min(early.len() - 1) {
            if (early[index + 1] - early[index]) / period >= 4. {
                let start = peaks[index + 1];
                let later: Vec<f64> = peaks.iter().copied().filter(|p| *p >= start).collect();
                let mut regular = 0;
                for pair in later.windows(2).take(10) {
                    let beats = (pair[1] - pair[0]) / period;
                    if (beats - 1.).abs() < 0.25 || (beats - 2.).abs() < 0.25 {
                        regular += 1;
                    }
                }
                if regular >= 5 && index < 3 {
                    return false;
                }
            }
        }
    }
    let mut aligned = 0;
    for &peak in early.iter().skip(1) {
        let beats = js_round((peak - early[0]) / period);
        if (peak - (early[0] + beats * period)).abs() <= 0.3 * period {
            aligned += 1;
        }
    }
    if aligned >= early.len() / 2 {
        return true;
    }
    if early.len() >= 6 && early[0] < 0.1 {
        let mut pairs = 0;
        for index in (0..8.min(early.len() - 2)).step_by(2) {
            let sum =
                (early[index + 1] - early[index] + (early[index + 2] - early[index + 1])) / period;
            if (sum - 1.).abs() < 0.2 {
                pairs += 1;
            }
        }
        if pairs >= 3 {
            return true;
        }
    }
    false
}
