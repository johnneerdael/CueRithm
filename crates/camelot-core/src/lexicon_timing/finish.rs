use super::{
    dp,
    onset::{attack_ratio, power},
    phase,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(super) struct Candidate {
    #[serde(rename = "tempo")]
    pub bpm: f64,
    #[serde(rename = "firstBeat")]
    pub first_beat: f64,
    pub confidence: f64,
    #[serde(default)]
    pub peaks: Vec<f64>,
}
struct Audio<'a> {
    pcm: &'a [f32],
    rate: u32,
}
impl Audio<'_> {
    fn duration(&self) -> f64 {
        self.pcm.len() as f64 / self.rate as f64
    }
    fn rms_index(&self, index: usize, width: usize) -> f64 {
        (power(self.pcm, index, index.saturating_add(width)) / width as f64).sqrt()
    }
    fn span_rms(&self, start: f64, end: f64) -> f64 {
        let start = (start * self.rate as f64).round().max(0.) as usize;
        let end = ((end * self.rate as f64).round() as usize).min(self.pcm.len());
        if end <= start {
            0.
        } else {
            (power(self.pcm, start, end) / (end - start) as f64).sqrt()
        }
    }
    fn best(&self, center: f64, radius: i32, step: usize, window_ms: f64) -> (f64, f64) {
        let (mut position, mut maximum) = (center, 0.);
        for offset in (-radius..=radius).step_by(step) {
            let at = center + offset as f64 / 1000.;
            if at < 0. || at >= self.duration() {
                continue;
            }
            let ratio = attack_ratio(self.pcm, self.rate, at, window_ms);
            if ratio > maximum {
                position = at;
                maximum = ratio;
            }
        }
        (position, maximum)
    }
    fn grid_score(&self, anchor: f64, period: f64) -> f64 {
        let mut score = 0.;
        for beat in 0..8 {
            let at = anchor + beat as f64 * period;
            if at >= self.duration() - 0.1 {
                break;
            }
            score += self.best(at, 20, 2, 15.).1;
        }
        score
    }
}
fn subsequent(
    audio: &Audio<'_>,
    candidate: &mut Candidate,
    options: &phase::SubsequentOptions,
    state: &mut phase::CorrectionState,
) {
    let adjusted = phase::subsequent(
        audio.pcm,
        audio.rate,
        candidate.first_beat,
        candidate.bpm,
        options,
        state,
    );
    if adjusted.applied_correction {
        candidate.first_beat = adjusted.corrected_first_beat;
    }
}
fn riser(audio: &Audio<'_>, candidate: &mut Candidate, state: &mut phase::CorrectionState) {
    let original = candidate.first_beat;
    let decision = phase::validate_riser(
        audio.pcm,
        audio.rate,
        original,
        candidate.bpm,
        &candidate.peaks,
    );
    if !decision.was_in_riser {
        return;
    }
    candidate.first_beat = decision.corrected_first_beat;
    let period = 60. / candidate.bpm;
    let anchor = candidate.first_beat;
    let mut walked = 0;
    while decision.strategy == Some("B") && anchor > 3. && walked < 16 {
        let at = anchor - (walked + 1) as f64 * period;
        if at < 0. {
            break;
        }
        if audio.best(at, 80, 2, 15.).1 <= 1.5 {
            break;
        }
        walked += 1;
    }
    if walked > 0 {
        let earlier = anchor - walked as f64 * period;
        let (mut best, mut score) = (earlier, 0.);
        let mut offset = -period / 2.;
        let step = period / 16.;
        while offset <= period / 2. {
            let at = earlier + offset;
            if at >= 0. {
                let s = audio.grid_score(at, period);
                if s > score {
                    score = s;
                    best = at;
                }
            }
            offset += step;
        }
        if score > 1.05 * audio.grid_score(earlier, period) && best > 0. {
            candidate.first_beat = audio.best(best, 25, 2, 15.).0;
        }
    }
    subsequent(
        audio,
        candidate,
        &phase::SubsequentOptions {
            search_window_ms: 40.,
            max_offset_ms: 40.,
            max_std_dev_ms: 8.,
            min_attack_ratio: Some(0.5),
        },
        state,
    );
    let low = original.min(candidate.first_beat);
    let high = original.max(candidate.first_beat);
    let gap = ((high - low) / period).floor() as usize;
    if gap >= 3 {
        let (mut hits, mut count) = (0, 0);
        for beat in 1..gap {
            let at = low + beat as f64 * period;
            count += 1;
            if audio.best(at, 15, 2, 15.).1 > 1.5 {
                hits += 1;
            }
        }
        if count > 0 && hits as f64 / count as f64 >= 0.4 {
            candidate.first_beat = original;
        }
    }
}
fn early_beat(audio: &Audio<'_>, candidate: &mut Candidate, state: &mut phase::CorrectionState) {
    let period = 60. / candidate.bpm;
    let mut moved = false;
    if candidate.first_beat > 0.5 * period && candidate.first_beat < 4. * period {
        let mut previous = candidate.first_beat - period;
        if previous < 0. && previous > -0.05 {
            previous = 0.;
        }
        if previous >= 0. {
            let best = audio.best(previous, 30, 2, 15.);
            if best.1 > 1.5 {
                let before = best.0 - period;
                let strength = if before >= 0. {
                    audio.best(before, 30, 5, 15.).1
                } else {
                    0.
                };
                if strength < 1.5 {
                    moved = true;
                    candidate.first_beat = best.0;
                    subsequent(audio, candidate, &Default::default(), state);
                }
            }
        }
    }
    if !moved
        || candidate
            .peaks
            .iter()
            .any(|p| *p > 0.001 && (*p - candidate.first_beat).abs() <= 0.08)
    {
        return;
    }
    let width = (0.005 * audio.rate as f64).round() as usize;
    let after = (0.01 * audio.rate as f64).round() as usize;
    let index = (candidate.first_beat * audio.rate as f64).round() as usize + after;
    if audio.rms_index(index, width) >= 0.04 {
        return;
    }
    let target = candidate.first_beat + period;
    let mut distance = f64::INFINITY;
    let mut closest = None;
    for &peak in &candidate.peaks {
        let d = (peak - target).abs();
        if d <= 0.08 && d < distance {
            closest = Some(peak);
            distance = d;
        }
    }
    if let Some(peak) = closest
        && audio.best(peak, 20, 2, 15.).1 > 2.
    {
        candidate.first_beat = peak;
        subsequent(audio, candidate, &Default::default(), state);
    }
}
fn silence_nudge(audio: &Audio<'_>, candidate: &mut Candidate, state: &phase::CorrectionState) {
    let width = (0.003 * audio.rate as f64).round() as usize;
    let step = (0.001 * audio.rate as f64).round() as usize;
    let span = (0.025 * audio.rate as f64).round() as usize;
    let index = (candidate.first_beat * audio.rate as f64).round() as usize;
    let initial = audio.rms_index(index, width);
    let mut maximum = initial;
    let end = (index + span).min(audio.pcm.len().saturating_sub(width));
    for at in (index + step..=end).step_by(step) {
        maximum = maximum.max(audio.rms_index(at, width));
    }
    if !(maximum > 0.001 && initial < 0.1 * maximum) {
        return;
    }
    for at in (index..=end).step_by(step) {
        if audio.rms_index(at, width) < 0.1 * maximum {
            continue;
        }
        let position = at as f64 / audio.rate as f64;
        let delta_ms = 1000. * (position - candidate.first_beat);
        let veto = state.last.as_ref().is_some_and(|last| {
            (candidate.first_beat - last.position).abs() < 5e-4
                && ((last.delta_ms > 0. && delta_ms > 0.5 * last.delta_ms)
                    || (last.source == "gridcorr" && last.delta_ms < 0. && delta_ms > 3.))
        });
        if !veto && delta_ms > 1. {
            let period = 60. / candidate.bpm;
            let (mut old, mut new) = (0., 0.);
            for beat in 4..=12 {
                let earlier = candidate.first_beat + beat as f64 * period;
                let later = position + beat as f64 * period;
                if earlier >= audio.duration() || later >= audio.duration() {
                    break;
                }
                old += attack_ratio(audio.pcm, audio.rate, earlier, 10.);
                new += attack_ratio(audio.pcm, audio.rate, later, 10.);
            }
            if new >= 0.95 * old {
                candidate.first_beat = position;
            }
        }
        break;
    }
}
pub(super) fn finish(
    pcm: &[f32],
    rate: u32,
    mut candidate: Candidate,
    state: &mut phase::CorrectionState,
) -> Candidate {
    let audio = Audio { pcm, rate };
    riser(&audio, &mut candidate, state);
    let half = phase::half_beat(pcm, rate, candidate.first_beat, candidate.bpm);
    if half.used_correction {
        candidate.first_beat = half.position;
    }
    early_beat(&audio, &mut candidate, state);
    let aligned = phase::align(pcm, rate, candidate.first_beat, candidate.bpm, 4, 80, false);
    if aligned.applied_correction && (aligned.offset_ms < 0 || candidate.first_beat < 0.01) {
        candidate.first_beat = aligned.corrected_first_beat;
    }
    // Unlike the inner selection stage, this outer alignment does not write
    // correction bookkeeping in the inspected implementation.
    silence_nudge(&audio, &mut candidate, state);
    let mut skip_dp = false;
    let rescued = phase::rescue_silent(pcm, rate, candidate.first_beat, candidate.bpm);
    if rescued.applied {
        candidate.first_beat = rescued.position;
        skip_dp = true;
        subsequent(&audio, &mut candidate, &Default::default(), state);
    }
    let period = 60. / candidate.bpm;
    if candidate.first_beat + 3. * period < audio.duration() {
        let first = audio.span_rms(candidate.first_beat, candidate.first_beat + period);
        let second = audio.span_rms(
            candidate.first_beat + period,
            candidate.first_beat + 2. * period,
        );
        let third = audio.span_rms(
            candidate.first_beat + 2. * period,
            candidate.first_beat + 3. * period,
        );
        let first_ratio = first / if second != 0. { second } else { 1e-9 };
        let second_ratio = second / if third != 0. { third } else { 1e-9 };
        if first_ratio < 0.4 && (0.7..=1.2).contains(&second_ratio) && second >= 0.1 {
            candidate.first_beat += period;
            skip_dp = true;
        }
    }
    // Weak-grid rescue is disabled by the fixed flag in module 358. Preserve
    // that behavior rather than enabling a helper merely because it exists.
    if !skip_dp {
        let corrected = dp::correct_phase(pcm, rate, candidate.first_beat, candidate.bpm);
        if corrected.applied {
            candidate.first_beat = corrected.position;
        }
    }
    let grid = phase::validate_grid(pcm, rate, candidate.first_beat, candidate.bpm);
    if grid.skipped_beats > 0 {
        candidate.first_beat = grid.validated_first_beat;
    }
    candidate
}
