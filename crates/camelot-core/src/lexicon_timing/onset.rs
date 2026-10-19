//! Onset primitives at the original Float32 PCM / Float64 arithmetic boundary.
//! PCM/rate validation belongs to the public timing entry point. Invalid
//! internal search coordinates produce NaN scores, which never win a search.

use serde::Serialize;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct FirstAudio {
    pub audio_start: f64,
    pub has_silence: bool,
}
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct DropPosition {
    pub position: f64,
    pub attack_ratio: f64,
}
fn valid(pcm: &[f32], rate: u32, at: f64) -> bool {
    (8000..=192000).contains(&rate)
        && at.is_finite()
        && at >= 0.
        && at <= pcm.len() as f64 / rate as f64
}
fn bounded_window(seconds: f64) -> bool {
    seconds.is_finite() && (0. ..=1.).contains(&seconds)
}
pub(super) fn power(pcm: &[f32], start: usize, end: usize) -> f64 {
    let end = end.min(pcm.len());
    if start >= end {
        return 0.;
    }
    pcm[start..end].iter().map(|v| (*v as f64).powi(2)).sum()
}
pub(super) fn js_round(value: f64) -> f64 {
    if !value.is_finite() {
        return value;
    }
    let floor = value.floor();
    let result = if value - floor < 0.5 {
        floor
    } else {
        floor + 1.
    };
    if result == 0. && value.is_sign_negative() {
        -0.
    } else {
        result
    }
}
fn rms_window(pcm: &[f32], start: usize, count: usize) -> f64 {
    (power(pcm, start, start.saturating_add(count)) / count as f64).sqrt()
}
pub(super) fn attack_ratio(pcm: &[f32], rate: u32, at: f64, window_ms: f64) -> f64 {
    if !valid(pcm, rate, at) || !window_ms.is_finite() || window_ms <= 0. || window_ms > 1000. {
        return f64::NAN;
    }
    let width = (window_ms / 1000. * rate as f64).round() as usize;
    let index = (at * rate as f64).round() as usize;
    let start = index.saturating_sub(width);
    let before = if index > start {
        (power(pcm, start, index) / (index - start) as f64).sqrt()
    } else {
        0.
    };
    let end = pcm.len().min(index.saturating_add(width));
    let after = if end > index {
        (power(pcm, index, end) / (end - index) as f64).sqrt()
    } else {
        0.
    };
    if before < 1e-4 {
        if after > 0.01 { 100. } else { 1. }
    } else {
        after / before
    }
}
pub(super) fn first_audio(pcm: &[f32], rate: u32, limit: f64) -> FirstAudio {
    if !(8000..=192000).contains(&rate) || !limit.is_finite() || limit <= 0. {
        return FirstAudio {
            audio_start: f64::NAN,
            has_silence: false,
        };
    }
    let width = (0.05 * rate as f64).round() as usize;
    let end = (limit * rate as f64).min(pcm.len().saturating_sub(width) as f64);
    let (mut silent, mut first) = (0, 0);
    let mut index = 0;
    while (index as f64) < end {
        if rms_window(pcm, index, width) >= 5e-4 {
            first = index;
            break;
        }
        silent += 1;
        index += width;
    }
    if silent < 10 {
        return FirstAudio {
            audio_start: 0.,
            has_silence: false,
        };
    }
    let start = first as f64 / rate as f64;
    let (mut best, mut position) = (0., start);
    let mut at = start;
    while at < start + 0.5 && at < limit {
        let ratio = attack_ratio(pcm, rate, at, 15.);
        if ratio > best {
            best = ratio;
            position = at;
        }
        at += 0.005;
    }
    FirstAudio {
        audio_start: position,
        has_silence: true,
    }
}
pub(super) fn dynamic_onset(pcm: &[f32], rate: u32, at: f64, options: Option<(f64, f64)>) -> f64 {
    if !valid(pcm, rate, at) {
        return f64::NAN;
    }
    let width = (0.003 * rate as f64).round() as usize;
    let step = (0.001 * rate as f64).round() as usize;
    let index = (at * rate as f64).round() as usize;
    let level = rms_window(pcm, index, width);
    if level < 0.001 {
        return at;
    }
    let before = index.saturating_sub((0.02 * rate as f64).round() as usize);
    let previous = rms_window(pcm, before, width);
    let ratio = if previous > 1e-4 {
        level / previous
    } else {
        100.
    };
    let (window_ms, fraction) = options.unwrap_or(if ratio > 5. {
        (10., 0.28)
    } else if ratio > 2. {
        (20., 0.22)
    } else {
        (80., 0.1)
    });
    if !bounded_window(window_ms / 1000.)
        || !fraction.is_finite()
        || !(0. ..=1.).contains(&fraction)
    {
        return f64::NAN;
    }
    let minimum =
        width.max(index.saturating_sub((window_ms / 1000. * rate as f64).round() as usize));
    let mut position = index;
    let mut cursor = index.checked_sub(step);
    while let Some(i) = cursor.filter(|i| *i >= minimum) {
        if rms_window(pcm, i, width) < level * fraction {
            position = i + step;
            break;
        }
        position = i;
        cursor = i.checked_sub(step);
    }
    let seconds = position as f64 / rate as f64;
    if seconds > 0. && seconds < 0.005 {
        let initial = rms_window(pcm, 0, width);
        if initial > level * if seconds < 0.003 { 0.1 } else { 0.25 } {
            return 0.;
        }
    }
    seconds
}
pub(super) fn refine_drop(pcm: &[f32], rate: u32, at: f64, window: f64) -> DropPosition {
    if !valid(pcm, rate, at) || !bounded_window(window) {
        return DropPosition {
            position: f64::NAN,
            attack_ratio: f64::NAN,
        };
    }
    let minimum = if at < 0.015 { at } else { 0.01 };
    let end = (at + window).min(pcm.len() as f64 / rate as f64);
    let mut cursor = minimum.max(at - window);
    let mut best = DropPosition {
        position: at,
        attack_ratio: attack_ratio(pcm, rate, at, 10.),
    };
    while cursor <= end {
        let ratio = attack_ratio(pcm, rate, cursor, 10.);
        if ratio > best.attack_ratio {
            best = DropPosition {
                position: cursor,
                attack_ratio: ratio,
            };
        }
        cursor += 0.002;
    }
    best
}
pub(super) fn snap(at: f64, peaks: &[f64], period: f64, tolerance: f64) -> f64 {
    if !at.is_finite()
        || !period.is_finite()
        || period <= 0.
        || !tolerance.is_finite()
        || tolerance < 0.
        || peaks.iter().any(|p| !p.is_finite())
    {
        return f64::NAN;
    }
    let mut anchor = peaks.first().copied().unwrap_or(0.);
    let mut distance = (at - anchor).abs();
    for &peak in peaks {
        let d = (at - peak).abs();
        if d < distance {
            distance = d;
            anchor = peak;
        }
    }
    let relative = (at - anchor) / period;
    let rounded = js_round(relative);
    let aligned = anchor + rounded * period;
    if (aligned - at).abs() <= tolerance {
        return aligned;
    }
    for &peak in peaks {
        if (peak - at).abs() <= tolerance {
            return peak;
        }
    }
    at
}
pub(super) fn precise_onset(pcm: &[f32], rate: u32, at: f64, window: f64) -> f64 {
    if !valid(pcm, rate, at) || !bounded_window(window) {
        return f64::NAN;
    }
    let end = (at + window).min(pcm.len() as f64 / rate as f64);
    let mut cursor = (at - window).max(0.);
    let (mut position, mut maximum) = (at, 0.);
    while cursor <= end {
        let ratio = attack_ratio(pcm, rate, cursor, 10.);
        if ratio > maximum {
            maximum = ratio;
            position = cursor;
        }
        cursor += 0.001;
    }
    let width = (0.003 * rate as f64).round() as usize;
    let step = (0.0005 * rate as f64).round() as usize;
    let index = (position * rate as f64).round() as usize;
    let level = rms_window(pcm, index, width);
    if level < 0.001 {
        return position;
    }
    let minimum = index.saturating_sub((0.03 * rate as f64).round() as usize);
    let mut result = index;
    let mut cursor = index.checked_sub(step);
    while let Some(i) = cursor.filter(|i| *i >= minimum) {
        if rms_window(pcm, i, width) < 0.15 * level {
            result = i + step;
            break;
        }
        result = i;
        cursor = i.checked_sub(step);
    }
    result as f64 / rate as f64
}
