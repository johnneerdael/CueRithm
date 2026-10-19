use super::{
    intro::IntroKind,
    onset::{attack_ratio, first_audio, js_round, power},
};
use serde::Serialize;

mod negative;
mod positive;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct Walkback {
    pub position: f64,
    pub beats_walked_back: f64,
}
#[derive(Clone, Copy)]
struct Peak {
    at: f64,
    ratio: f64,
}
impl Peak {
    fn at(at: f64) -> Self {
        Self { at, ratio: 0. }
    }
}
struct Context<'a> {
    pcm: &'a [f32],
    rate: u32,
    drop: f64,
    period: f64,
    peaks: &'a [f64],
    strength: Option<f64>,
    kind: IntroKind,
}
fn result(position: f64, beats: f64) -> Walkback {
    Walkback {
        position,
        beats_walked_back: beats,
    }
}
fn mean(values: &[f64]) -> f64 {
    values.iter().sum::<f64>() / values.len() as f64
}
fn variation(values: &[f64]) -> f64 {
    let avg = mean(values);
    if avg > 0. {
        (values.iter().map(|v| (v - avg).powi(2)).sum::<f64>() / values.len() as f64).sqrt() / avg
    } else {
        f64::INFINITY
    }
}
impl Context<'_> {
    fn strong(&self) -> bool {
        self.strength.is_some_and(|s| s >= 3.)
    }
    fn scan(&self, begin: f64, end: f64, step: f64, inclusive: bool, mut best: Peak) -> Peak {
        let mut at = begin;
        while if inclusive { at <= end } else { at < end } {
            if at >= 0. {
                let ratio = attack_ratio(self.pcm, self.rate, at, 15.);
                if ratio > best.ratio {
                    best = Peak { at, ratio };
                }
            }
            at += step;
        }
        best
    }
    fn around(&self, at: f64, radius: f64, step: f64) -> Peak {
        self.scan(at - radius, at + radius, step, true, Peak::at(at))
    }
    fn offsets(&self, center: f64, begin: f64, end: f64, step: f64, mut best: Peak) -> Peak {
        let mut offset = begin;
        while offset <= end {
            let at = center + offset;
            if at >= 0. {
                let ratio = attack_ratio(self.pcm, self.rate, at, 15.);
                if ratio > best.ratio {
                    best = Peak { at, ratio };
                }
            }
            offset += step;
        }
        best
    }
    fn first_peak(&self, begin: f64, end: f64, inclusive: bool) -> Option<f64> {
        self.peaks
            .iter()
            .copied()
            .find(|p| *p >= begin && if inclusive { *p <= end } else { *p < end })
    }
    fn rms(&self, at: f64, seconds: f64, available: bool) -> f64 {
        if at < 0. || !at.is_finite() {
            return f64::NAN;
        }
        let start = (at * self.rate as f64).round() as usize;
        let width = (seconds * self.rate as f64).round() as usize;
        let end = start.saturating_add(width).min(self.pcm.len());
        let divisor = if available {
            end as f64 - start as f64
        } else {
            width as f64
        };
        (power(self.pcm, start, end) / divisor).sqrt()
    }
    fn following(&self, at: f64) -> Vec<f64> {
        (1..=3)
            .map(|beat| {
                self.around(at + beat as f64 * self.period, 0.03, 0.005)
                    .ratio
            })
            .collect()
    }
    fn near_zero(&self, candidate: f64, phrases: usize) -> Option<Walkback> {
        let beats = phrases as f64 * 16.;
        if let Some(peak) = self.first_peak(0., 0.015, false) {
            return Some(result(peak, beats));
        }
        if self.strong() {
            let gap = self.peaks.len() >= 2 && self.peaks[1] - self.peaks[0] > 20. * self.period;
            if !self.peaks.is_empty() && (self.peaks[0] > 15. || gap) {
                let ratios: Vec<f64> = (1..=4)
                    .map(|beat| {
                        let at = beat as f64 * self.period;
                        self.scan((at - 0.03).max(0.05), at + 0.03, 0.005, true, Peak::at(at))
                            .ratio
                    })
                    .collect();
                let minimum = ratios.iter().copied().fold(f64::INFINITY, f64::min);
                let threshold = if gap && self.peaks[0] <= 15. { 1.5 } else { 1. };
                if !(minimum > 5. || variation(&ratios) < 0.15 && minimum > threshold) {
                    return None;
                }
            }
            let mut position = candidate.max(0.);
            if position > 0. && position < 0.008 {
                position = 0.;
            }
            return Some(result(position, beats));
        }
        if let Some(peak) = self.first_peak(0., 0.05, false) {
            return Some(result(peak, beats));
        }
        let best = self.scan(0., 0.08, 0.002, true, Peak::at(0.));
        if self.rms(0., 0.02, false) > 0.005 || best.ratio < 3. || best.at < 0.008 {
            return Some(result(0., beats));
        }
        if best.ratio > 1.5 {
            return Some(result(best.at, beats));
        }
        None
    }
    fn single_steps(&self, start: f64) -> (f64, f64) {
        let (mut position, mut walked, mut misses) = (start, 0., 0);
        for beat in 1..=(start / self.period).floor() as usize {
            let at = start - beat as f64 * self.period;
            if at < 0.05 {
                if let Some(peak) = self.first_peak(0., 0.05, false) {
                    position = peak;
                    walked = beat as f64;
                } else {
                    let best = self.scan(0., 0.05, 0.002, true, Peak::at(0.));
                    if best.ratio > 2. {
                        position = best.at;
                        walked = beat as f64;
                    }
                }
                break;
            }
            let best = self.around(at, 0.03, 0.003);
            if best.ratio > 2. {
                position = best.at;
                walked = beat as f64;
                misses = 0;
            } else {
                misses += 1;
                if misses >= 3 {
                    break;
                }
            }
        }
        let half = position - 0.5 * self.period;
        let best = if half >= 0.05 {
            Some(self.around(half, 0.03, 0.003))
        } else if half >= 0. {
            Some(self.scan(0., 0.05, 0.002, true, Peak::at(0.)))
        } else {
            None
        };
        if let Some(best) = best
            && best.ratio > 2.
        {
            position = best.at;
            walked += 0.5;
        }
        (position, walked)
    }
    fn silence_fallback(&self, limit: f64) -> Option<Walkback> {
        let audio = first_audio(self.pcm, self.rate, limit);
        if audio.has_silence && audio.audio_start < limit {
            let end = (audio.audio_start + self.period).min(limit);
            let found = self.scan(
                (audio.audio_start - 0.1).max(0.),
                end,
                0.003,
                false,
                Peak::at(audio.audio_start),
            );
            return Some(result(
                found.at,
                js_round((self.drop - found.at) / self.period),
            ));
        }
        None
    }
}
pub(super) fn walk_back(
    pcm: &[f32],
    rate: u32,
    anchor: f64,
    period: f64,
    peaks: &[f64],
    strength: Option<f64>,
    kind: IntroKind,
) -> Walkback {
    if !(8000..=192000).contains(&rate)
        || !anchor.is_finite()
        || anchor < 0.
        || anchor > pcm.len() as f64 / rate as f64
        || !period.is_finite()
        || !(0.2..=1.2).contains(&period)
        || peaks.iter().any(|p| !p.is_finite() || *p < 0.)
        || strength.is_some_and(|s| !s.is_finite())
    {
        return result(anchor, 0.);
    }
    let context = Context {
        pcm,
        rate,
        drop: anchor,
        period,
        peaks,
        strength,
        kind,
    };
    if anchor < 0.012 && context.rms(0., 0.02, false) > 0.15 * context.rms(anchor, 0.02, false) {
        return result(0., 0.);
    }
    if anchor < 0.5 * period {
        return result(anchor, 0.);
    }
    let phrase = 16. * period;
    let count = (anchor / phrase).ceil() as usize;
    for phrases in (1..=count).rev() {
        let candidate = anchor - phrases as f64 * phrase;
        let found = if candidate < 0.1 && candidate > -0.2 {
            context.near_zero(candidate, phrases)
        } else if candidate < 0. {
            context.negative(candidate, phrases)
        } else {
            context.positive(candidate, phrases)
        };
        if let Some(found) = found {
            return found;
        }
    }
    if anchor > 3.
        && kind != IntroKind::Buildup
        && let Some(found) = context.silence_fallback(anchor)
    {
        return found;
    }
    result(anchor, 0.)
}
