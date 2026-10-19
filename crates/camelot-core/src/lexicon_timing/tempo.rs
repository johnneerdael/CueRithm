use super::{LexiconTempoOptions, TempoEvidence};
use std::collections::HashMap;

fn amplitude(pcm: &[f32], window: usize) -> f64 {
    let (mut sum, mut maximum) = (0., 0_f64);
    for (index, sample) in pcm.iter().enumerate() {
        sum += (*sample as f64).abs();
        if index >= window {
            sum -= (pcm[index - window] as f64).abs();
        }
        maximum = maximum.max(sum);
    }
    maximum / window as f64
}

fn peaks(pcm: &[f32], window: usize, threshold: f64) -> Vec<usize> {
    let mut peaks: Vec<(usize, f64)> = vec![];
    let mut past = 0.;
    let mut future: f64 = pcm[..window].iter().map(|v| (*v as f64).powi(2)).sum();
    for index in 0..pcm.len() - window {
        let value = (pcm[index] as f64).powi(2);
        future -= value;
        future += (pcm[index + window] as f64).powi(2);
        if index >= window {
            past -= (pcm[index - window] as f64).powi(2);
        }
        past += value;
        // Keep the original division/multiplication order, including NaN on a
        // zero future window. NaN does not satisfy the threshold comparison.
        let change = (1. - past / future) * future;
        let score = if change.is_nan() {
            f64::NAN
        } else {
            change.max(0.)
        } / (window as f64 / 2.);
        if score > threshold {
            match peaks.last_mut() {
                Some((at, score_before)) if index - *at <= window => {
                    if *score_before < score {
                        *at = index;
                        *score_before = score;
                    }
                }
                _ => peaks.push((index, score)),
            }
        }
    }
    peaks.into_iter().map(|(index, _)| index).collect()
}

fn adaptive_peaks(pcm: &[f32], window: usize, maximum: f64, desired: f64) -> Vec<usize> {
    let decrement = 0.05 * maximum;
    let mut threshold = maximum - decrement;
    let mut found = vec![];
    // At most twenty source thresholds; the explicit cap also protects very
    // small nonzero input against an unchanging floating-point decrement.
    for _ in 0..20 {
        if found.len() as f64 >= desired || threshold < decrement {
            break;
        }
        found = peaks(pcm, window, threshold);
        threshold -= decrement;
    }
    found
}

#[derive(Debug)]
struct Candidate {
    bpm: f64,
    score: f64,
    count: usize,
}

fn candidates(peaks: &[usize], rate: f64, options: &LexiconTempoOptions) -> Vec<Candidate> {
    let mut intervals: Vec<(usize, usize)> = vec![];
    let mut index = HashMap::new();
    for (at, peak) in peaks.iter().enumerate() {
        for next in peaks.iter().skip(at + 1).take(9) {
            let interval = next - peak;
            if let Some(&position) = index.get(&interval) {
                let entry: &mut (usize, usize) = &mut intervals[position];
                entry.1 += 1;
            } else {
                index.insert(interval, intervals.len());
                intervals.push((interval, 1));
            }
        }
    }
    // The vector carries insertion order; a map iteration would change ties.
    let mut result: Vec<Candidate> = vec![];
    for (interval, count) in intervals {
        let mut bpm = 60. / (interval as f64 / rate);
        while bpm < options.min_tempo {
            bpm *= 2.;
        }
        while bpm > options.max_tempo {
            bpm /= 2.;
        }
        if bpm < options.min_tempo {
            continue;
        }
        let mut score = count as f64;
        for other in &mut result {
            if other.bpm > bpm - 0.5 && other.bpm < bpm + 0.5 {
                let distance = 2. * (other.bpm - bpm).abs();
                score += (1. - distance) * other.count as f64;
                other.score += (1. - distance) * count as f64;
            }
        }
        if let Some(other) = result.iter_mut().find(|c| c.bpm == bpm) {
            other.count += count;
            other.score += score;
        } else {
            result.push(Candidate { bpm, score, count });
        }
    }
    result.sort_by(|a, b| b.score.total_cmp(&a.score));
    result
}

fn correlation(pcm: &[f32], offset: usize) -> f64 {
    let (mut cross, mut energy_a, mut energy_b) = (0., 0., 0.);
    for index in offset..pcm.len() {
        let a = pcm[index] as f64;
        let b = pcm[index - offset] as f64;
        cross += (a * b).abs();
        energy_a += a * a;
        energy_b += b * b;
    }
    let denominator = (energy_a * energy_b).sqrt();
    if cross == 0. && denominator == 0. {
        1.
    } else {
        cross / denominator
    }
}

fn score(pcm: &[f32], bpm: f64, rate: f64) -> f64 {
    let mut result = 0.;
    for beats in [4., 8., 16.] {
        let lag = (60. / bpm * beats * rate).round() as usize;
        if lag < pcm.len() {
            result += correlation(pcm, lag);
        }
    }
    result / 3.
}
fn long_score(pcm: &[f32], bpm: f64, rate: f64) -> f64 {
    let lag = (60. / bpm * 64. * rate).round() as usize;
    if lag < pcm.len() {
        correlation(pcm, lag)
    } else {
        score(pcm, bpm, rate)
    }
}

fn append_unique(values: &mut Vec<f64>, value: f64) {
    if !values.contains(&value) {
        values.push(value);
    }
}

fn refine(pcm: &[f32], rate: f64, initial: f64, options: &LexiconTempoOptions) -> f64 {
    let mut alternatives = vec![];
    for value in [initial, initial.round(), initial.floor(), initial.ceil()] {
        append_unique(&mut alternatives, value);
    }
    for value in [1.5 * initial, initial / 1.5, 2. * initial, initial / 2.] {
        if value >= options.min_tempo && value <= options.max_tempo {
            for n in [value, value.round(), value.floor(), value.ceil()] {
                append_unique(&mut alternatives, n);
            }
        }
    }
    let scores: Vec<_> = alternatives
        .iter()
        .map(|bpm| (*bpm, score(pcm, *bpm, rate)))
        .collect();
    let mut best = scores[0];
    for &(bpm, score) in &scores {
        if score > best.1 {
            best = (bpm, score);
        }
    }
    let (mut bpm, best_score) = best;
    if (bpm - initial.round()).abs() < 0.01 && (initial - initial.round()).abs() > 0.01 {
        let lag_a = (60. / initial * 64. * rate).round() as usize;
        let lag_b = (60. / bpm * 64. * rate).round() as usize;
        if lag_a < pcm.len()
            && lag_b < pcm.len()
            && correlation(pcm, lag_a) > correlation(pcm, lag_b)
        {
            bpm = initial;
        }
    }
    if (bpm - bpm.round()).abs() > 0.1 {
        let mut integers = vec![];
        for candidate in [bpm.round(), bpm.floor(), bpm.ceil()] {
            if candidate >= options.min_tempo
                && candidate <= options.max_tempo
                && (candidate - bpm).abs() > 0.1
            {
                append_unique(&mut integers, candidate);
            }
        }
        for candidate in integers {
            let lag = (60. / bpm * 64. * rate).round() as usize;
            let other = (60. / candidate * 64. * rate).round() as usize;
            if lag < pcm.len()
                && other < pcm.len()
                && correlation(pcm, other) > correlation(pcm, lag)
            {
                bpm = candidate;
                break;
            }
        }
    }
    for &(candidate, score) in &scores {
        if (candidate - 2. * bpm).abs() < 1. && best_score - score < 0.05 {
            bpm = candidate;
            break;
        }
    }
    if (bpm - 130.5).abs() < 0.5
        && 174. <= options.max_tempo
        && score(pcm, 174., rate) > 0.85 * score(pcm, bpm, rate)
    {
        bpm = 174.;
    }
    if (85. ..=90.).contains(&bpm) {
        let other = 2. * bpm;
        if (170. ..=180.).contains(&other)
            && other <= options.max_tempo
            && score(pcm, other, rate) > 0.7 * score(pcm, bpm, rate)
        {
            bpm = other;
        }
    }
    if bpm > 90. && bpm <= 115. {
        let other = 2. * bpm;
        if other <= options.max_tempo && score(pcm, other, rate) > score(pcm, bpm, rate) {
            bpm = other;
        }
    }
    if options.max_tempo > 220. {
        if (135. ..=145.).contains(&bpm) {
            let other = 2. * bpm;
            if other <= options.max_tempo && score(pcm, other, rate) > 0.7 * score(pcm, bpm, rate) {
                bpm = other;
            }
        }
        if (180. ..=195.).contains(&bpm) {
            let other = 1.5 * bpm;
            if other <= options.max_tempo && score(pcm, other, rate) > 0.7 * score(pcm, bpm, rate) {
                bpm = other;
            }
        }
    }
    let integer = bpm.round();
    if (bpm - integer).abs() < 0.3 {
        let initial_score = long_score(pcm, bpm, rate);
        if long_score(pcm, integer, rate) >= 0.95 * initial_score {
            bpm = integer;
        } else {
            let radius = 0.05_f64.max((bpm - integer).abs());
            let (mut best_bpm, mut best) = (bpm, initial_score);
            let mut trial = bpm - radius;
            while trial <= bpm + radius {
                let candidate_score = long_score(pcm, trial, rate);
                if candidate_score > best {
                    best = candidate_score;
                    best_bpm = trial;
                }
                trial += 0.001;
            }
            bpm = (1000. * best_bpm).round() / 1000.;
        }
    } else {
        for candidate in [bpm.floor() + 0.5, bpm.floor() + 0.25, bpm.floor() + 0.75] {
            if (bpm - candidate).abs() < 0.1 {
                if long_score(pcm, candidate, rate) >= long_score(pcm, bpm, rate) {
                    bpm = candidate;
                }
                break;
            }
        }
    }
    bpm
}

pub(super) fn analyze(
    pcm: &[f32],
    sample_rate: u32,
    options: &LexiconTempoOptions,
) -> Option<TempoEvidence> {
    let rate = sample_rate as f64;
    let separation = (30. / options.max_tempo * rate).round() as usize;
    if pcm.len() <= separation {
        return None;
    }
    let maximum = amplitude(pcm, (rate / 100.).round() as usize);
    if maximum <= 0. {
        return None;
    }
    let desired = options.min_tempo * pcm.len() as f64 / rate;
    let global = adaptive_peaks(pcm, separation, maximum, desired);
    let candidates = candidates(&global, rate, options);
    if candidates.is_empty() {
        return None;
    }
    let mut output_peaks = global;
    let prefix = (5. * rate).round() as usize;
    if pcm.len() > prefix {
        let early = &pcm[..prefix];
        let max_early = amplitude(early, (rate / 100.).round() as usize);
        if max_early >= 0.001 {
            let floor = 0.05 * max_early;
            let mut threshold = 0.5 * max_early;
            let mut early_peaks = vec![];
            let count = 4_f64.max((1.5 * prefix as f64 / rate).floor());
            for _ in 0..6 {
                if early_peaks.len() as f64 >= count || threshold < floor {
                    break;
                }
                early_peaks = peaks(early, separation, threshold);
                threshold -= 0.1 * max_early;
            }
            for peak in early_peaks {
                if !output_peaks.iter().any(|p| p.abs_diff(peak) < separation) {
                    output_peaks.push(peak);
                }
            }
        }
    }
    output_peaks.sort_unstable();
    let total: f64 = candidates.iter().map(|c| c.score).sum();
    Some(TempoEvidence {
        bpm: refine(pcm, rate, candidates[0].bpm, options),
        confidence: if total > 0. {
            candidates[0].score / total
        } else {
            0.
        },
        peaks: output_peaks,
    })
}
