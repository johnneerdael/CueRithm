//! Structural cue detection based on energy envelope analysis.
use serde::{Deserialize, Serialize};
use std::f64::consts::PI;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RampType {
    Up,
    Down,
    Flat,
}

#[derive(Debug, Clone)]
pub struct Section {
    pub energy: f64,
    pub beat_strength: f64,
    pub ramp_type: RampType,
    pub beat_energies: Vec<f64>,
    pub start_time: f64,
    pub duration: f64,
    pub energy_delta: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum DropAtStart {
    #[default]
    Auto,
    Never,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LexiconConfig {
    pub genre: Option<String>,
    pub drop_at_start: DropAtStart,
    pub min_cue_length_beats: u32,
    pub add_emergency_loop: bool,
}

impl Default for LexiconConfig {
    fn default() -> Self {
        Self {
            genre: None,
            drop_at_start: DropAtStart::Auto,
            min_cue_length_beats: 64,
            add_emergency_loop: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StructuralCue {
    pub time_seconds: f64,
    pub name: String,
    pub color: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EmergencyLoop {
    pub time_seconds: f64,
    pub loop_beats: u32,
}

/// Single-pole IIR lowpass, applied `passes` times forward.
/// Matches JS `v(t, e, n, r)`.
pub fn low_pass_iir(samples: &[f32], sample_rate: u32, cutoff_hz: f64, passes: u32) -> Vec<f32> {
    let sr = sample_rate as f64;
    let alpha = 1.0 / (1.0 + sr / (2.0 * PI * cutoff_hz));
    let mut out = samples.to_vec();
    for _ in 0..passes {
        let mut prev = 0.0_f64;
        for s in out.iter_mut() {
            prev = alpha * *s as f64 + (1.0 - alpha) * prev;
            *s = prev as f32;
        }
    }
    out
}

/// Butterworth-style biquad lowpass (Q = 1/sqrt(2)).
/// Matches JS `F(t, e, n)`.
pub fn biquad_lowpass(samples: &[f32], sample_rate: u32, cutoff_hz: f64) -> Vec<f32> {
    let sr = sample_rate as f64;
    let q = 1.0 / std::f64::consts::SQRT_2;
    let w0 = 2.0 * PI * cutoff_hz / sr;
    let sin_w0 = w0.sin();
    let cos_w0 = w0.cos();
    let alpha = sin_w0 / (2.0 * q);
    let a0 = 1.0 + alpha;
    let b0 = (1.0 - cos_w0) / 2.0 / a0;
    let b1 = (1.0 - cos_w0) / a0;
    let b2 = (1.0 - cos_w0) / 2.0 / a0;
    let a1 = (-2.0 * cos_w0) / a0;
    let a2 = (1.0 - alpha) / a0;

    let mut out = vec![0.0f32; samples.len()];
    let mut x1 = 0.0_f64;
    let mut x2 = 0.0_f64;
    let mut y1 = 0.0_f64;
    let mut y2 = 0.0_f64;
    for (i, &x) in samples.iter().enumerate() {
        let x0 = x as f64;
        let y0 = b0 * x0 + b1 * x1 + b2 * x2 - a1 * y1 - a2 * y2;
        x2 = x1;
        x1 = x0;
        y2 = y1;
        y1 = y0;
        out[i] = y0 as f32;
    }
    out
}

/// Cascaded biquad lowpass at [800, 400, 400, 200, 200, 200, 200] Hz.
/// Used for the filtered PCM in drop detection.
pub fn cascaded_lowpass(samples: &[f32], sample_rate: u32) -> Vec<f32> {
    let mut buf = biquad_lowpass(samples, sample_rate, 800.0);
    for &freq in &[400.0, 400.0, 200.0, 200.0, 200.0, 200.0] {
        buf = biquad_lowpass(&buf, sample_rate, freq);
    }
    buf
}

/// Build energy sections from PCM, grouping by `beats_per_section` beats.
/// Matches JS `_(t, e, n, r, a, o)`.
///
/// Parameters:
///   pcm          - mono samples
///   sample_rate  - Hz
///   bpm          - tempo
///   first_beat   - time of first beat in seconds
///   beats_per_section - typically 4
///   ramp_threshold    - typically 0.05 (or 0.02 for internal calls)
pub fn build_sections(
    pcm: &[f32],
    sample_rate: u32,
    bpm: f64,
    first_beat: f64,
    beats_per_section: u32,
    ramp_threshold: f64,
) -> Vec<Section> {
    let sr = sample_rate as f64;
    let window_dur = 60.0 * beats_per_section as f64 / bpm;
    let window_samples = (window_dur * sr).floor() as usize;
    let beat_period = 60.0 / bpm;

    let mut sections = Vec::new();
    let mut cursor = if window_dur > 0.0 {
        first_beat - (first_beat / window_dur).floor() * window_dur
    } else {
        first_beat
    };

    loop {
        let start_sample = (cursor * sr).floor() as usize;
        if start_sample >= pcm.len() {
            break;
        }
        let end_sample = (start_sample + window_samples).min(pcm.len());
        let count = end_sample - start_sample;

        // RMS energy
        let mut sum_sq = 0.0_f64;
        for i in start_sample..end_sample {
            let v = pcm[i] as f64;
            sum_sq += v * v;
        }
        let energy = if count > 0 {
            (sum_sq / count as f64).sqrt()
        } else {
            0.0
        };
        let duration = count as f64 / sr;

        // Beat strength: RMS in ±50ms window around each beat within this section
        let section_end_time = cursor + window_dur;
        let mut first_beat_in_section =
            first_beat + ((cursor - first_beat) / beat_period - 1e-9).ceil() * beat_period;
        let mut beat_sum = 0.0_f64;
        let mut beat_count = 0u32;
        let mut beat_energies = Vec::new();

        while (first_beat_in_section * 1e6).floor() <= (section_end_time * 1e6).floor() {
            let lo = (first_beat_in_section - 0.05).max(0.0);
            let hi = first_beat_in_section + 0.05;
            let a = (lo * sr).floor().max(0.0) as usize;
            let b = (hi * sr).floor().min(pcm.len() as f64) as usize;
            let n = b.saturating_sub(a);
            if n > 0 {
                let mut s = 0.0_f64;
                for i in a..b {
                    let v = pcm[i] as f64;
                    s += v * v;
                }
                let rms = (s / n as f64).sqrt();
                beat_sum += rms;
                beat_count += 1;
                beat_energies.push(rms);
            }
            first_beat_in_section += beat_period;
        }

        let beat_strength = if beat_count > 0 {
            beat_sum / beat_count as f64
        } else {
            0.0
        };

        sections.push(Section {
            energy,
            beat_strength,
            ramp_type: RampType::Flat,
            beat_energies,
            start_time: cursor,
            duration,
            energy_delta: 0.0,
        });

        cursor += window_dur;
    }

    // Ramp classification: linear regression over groups of 4 sections on beat_strength
    let mut i = 0;
    while i < sections.len() {
        let end = (i + 4).min(sections.len());
        let n = end - i;
        if n < 2 {
            i += 4;
            continue;
        }
        let mut sum_x = 0.0_f64;
        let mut sum_y = 0.0_f64;
        let mut sum_xy = 0.0_f64;
        let mut sum_xx = 0.0_f64;
        for j in 0..n {
            let x = j as f64;
            let y = sections[i + j].beat_strength;
            sum_x += x;
            sum_y += y;
            sum_xy += x * y;
            sum_xx += x * x;
        }
        let denom = n as f64 * sum_xx - sum_x * sum_x;
        let slope = if denom != 0.0 {
            (n as f64 * sum_xy - sum_x * sum_y) / denom
        } else {
            0.0
        };
        let mean_y = sum_y / n as f64;
        let threshold = if mean_y > 0.0 {
            mean_y * ramp_threshold
        } else {
            1e-6
        };
        let ramp = if slope > threshold {
            RampType::Up
        } else if slope < -threshold {
            RampType::Down
        } else {
            RampType::Flat
        };
        for j in i..end {
            sections[j].ramp_type = ramp;
        }
        i += 4;
    }

    // Energy deltas
    for i in 0..sections.len() {
        sections[i].energy_delta = if i == 0 {
            0.0
        } else {
            sections[i].energy - sections[i - 1].energy
        };
    }

    sections
}

/// Find the section whose start_time is closest to `time`.
/// Matches JS `x(t, e)`.
pub fn nearest_section(sections: &[Section], time: f64) -> usize {
    let mut best = 0;
    let mut best_dist = f64::INFINITY;
    for (i, s) in sections.iter().enumerate() {
        let d = (s.start_time - time).abs();
        if d < best_dist {
            best_dist = d;
            best = i;
        }
    }
    best
}

/// Average energy of up to `count` sections starting at `start_idx`.
/// Returns 1e-9 on empty input. Matches JS `B(t, e, n)`.
pub fn avg_energy(sections: &[Section], start_idx: usize, count: usize) -> f64 {
    if sections.is_empty() {
        return 1e-9;
    }
    let end = sections.len().min(start_idx + count);
    let mut sum = 0.0;
    let mut n = 0u32;
    for i in start_idx..end {
        sum += sections[i].energy;
        n += 1;
    }
    let avg = if n > 0 { sum / n as f64 } else { 1e-9 };
    if avg > 0.0 { avg } else { 1e-9 }
}

/// Refine a detected drop position by searching forward for a sharper energy rise.
/// Matches JS `w(t, e, n)` with default n=16.
pub fn refine_drop_start(sections: &[Section], idx: usize, max_search: usize) -> usize {
    if idx < 1 || idx >= sections.len() {
        return idx;
    }
    let energy = sections[idx].energy;
    if energy <= 0.0 {
        return idx;
    }
    if sections[idx - 1].energy > 1.3 * energy {
        return idx;
    }

    let ahead_start = idx + 8;
    let ahead_end = sections.len().min(idx + 24);
    if ahead_end.saturating_sub(ahead_start) < 8 {
        return idx;
    }
    let mut ahead_sum = 0.0;
    for i in ahead_start..ahead_end {
        ahead_sum += sections[i].energy;
    }
    let ahead_avg = ahead_sum / (ahead_end - ahead_start) as f64;
    if ahead_avg <= 0.0 || energy >= 0.75 * ahead_avg {
        return idx;
    }

    let search_end = sections.len().saturating_sub(8).min(idx + 1 + max_search);
    let threshold = (1.3 * energy).max(0.9 * ahead_avg);
    for n in (idx + 1)..search_end {
        if sections[n].energy < threshold {
            continue;
        }
        let before_start = if n >= 8 { n - 8 } else { 0 };
        let mut before_sum = 0.0;
        for i in before_start..n {
            before_sum += sections[i].energy;
        }
        let before_avg = before_sum / (n - before_start).max(1) as f64;
        if before_avg <= 0.0 {
            continue;
        }
        let mut after_sum = 0.0;
        for i in n..(n + 8) {
            after_sum += sections[i].energy;
        }
        if after_sum / 8.0 >= 1.5 * before_avg {
            return n;
        }
    }
    idx
}

/// Find the first contiguous run of sections with energy below `threshold_ratio` * peak
/// that spans at least `min_length` sections. Returns (start, end) inclusive.
/// Matches JS `J(t, e, n)`.
pub fn find_energy_gap(
    sections: &[Section],
    threshold_ratio: f64,
    min_length: usize,
) -> Option<(usize, usize)> {
    let peak = sections.iter().map(|s| s.energy).fold(f64::NEG_INFINITY, f64::max);
    let threshold = peak * threshold_ratio;
    let mut gap_start: Option<usize> = None;
    for i in 0..sections.len() {
        if sections[i].energy < threshold {
            if gap_start.is_none() {
                gap_start = Some(i);
            }
        } else {
            if let Some(start) = gap_start {
                if i - start >= min_length {
                    return Some((start, i - 1));
                }
            }
            gap_start = None;
        }
    }
    None
}

/// RMS of samples in window [time - half_window, time + half_window].
/// Matches JS `ae(t, e, n, r)`.
pub fn rms_at_time(samples: &[f32], sample_rate: u32, time: f64, half_window: f64) -> f64 {
    let sr = sample_rate as f64;
    let a = ((time - half_window) * sr).floor().max(0.0) as usize;
    let b = ((time + half_window) * sr).floor().min(samples.len() as f64) as usize;
    if b <= a {
        return 0.0;
    }
    let mut sum = 0.0_f64;
    for i in a..b {
        sum += (samples[i] as f64) * (samples[i] as f64);
    }
    (sum / (b - a) as f64).sqrt()
}

/// Quantile of a slice. `p` in [0, 1]. Matches JS `oe(t, e)`.
pub fn percentile(values: &[f64], p: f64) -> f64 {
    let mut sorted = values.to_vec();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    sorted[(sorted.len() as f64 * p).floor() as usize]
}

/// Ratio of a section's energy to the peak energy in the section list.
/// Matches JS `U(t, e)`.
pub fn energy_ratio_to_peak(sections: &[Section], idx: Option<usize>) -> f64 {
    match idx {
        None => 0.0,
        Some(_) if sections.is_empty() => 0.0,
        Some(i) => {
            let peak = sections
                .iter()
                .map(|s| s.energy)
                .fold(f64::NEG_INFINITY, f64::max)
                .max(1e-9);
            sections[i].energy / peak
        }
    }
}

/// Ratio of average energy after a drop position to the drop energy.
/// Matches the inline function computing `y` in K().
pub fn energy_sustain_after(sections: &[Section], idx: Option<usize>) -> f64 {
    match idx {
        None => 0.0,
        Some(i) => {
            let energy = sections[i].energy;
            if energy == 0.0 {
                return 0.0;
            }
            let mut sum = 0.0;
            let mut count = 0u32;
            let end = sections.len().min(i + 9);
            for j in (i + 1)..end {
                sum += sections[j].energy;
                count += 1;
            }
            if count > 0 {
                sum / energy / count as f64
            } else {
                0.0
            }
        }
    }
}

/// Ratio of a section's energy to the average of the 8 preceding sections.
/// Matches the inline function computing `k` in K().
pub fn energy_contrast_before(sections: &[Section], idx: Option<usize>) -> f64 {
    match idx {
        None => 0.0,
        Some(i) => {
            let energy = sections[i].energy;
            if energy == 0.0 {
                return 0.0;
            }
            let start = i.saturating_sub(8);
            let mut sum = 0.0;
            let mut count = 0u32;
            for j in start..i {
                sum += sections[j].energy;
                count += 1;
            }
            let avg = if count > 0 { sum / count as f64 } else { 0.0 };
            energy / avg.max(1e-9)
        }
    }
}

/// Classify genre from an optional genre string.
/// Returns the normalized genre name used for algorithm branching.
pub fn classify_genre(genre: &Option<String>) -> Option<String> {
    genre.as_deref().map(|g| {
        let lower = g.to_lowercase();
        if lower.contains("techno") {
            "techno".to_string()
        } else if lower.contains("hardcore") {
            "hardcore".to_string()
        } else {
            lower
        }
    })
}

pub struct DropResult {
    pub start: StructuralCue,
    pub drop: Option<StructuralCue>,
}

// --- Drop detection constants (JS: G, N, S, A, R, T, P, D, E, C, L, I, W, q, O, H) ---
const DROP_ROLLING_WINDOW: usize = 4; // G
const DROP_ROLLING_THRESHOLD: f64 = 0.85; // N
const DROP_ENERGY_THRESHOLD: f64 = 0.65; // S
const DROP_TRANSIENT_THRESHOLD: f64 = 0.2; // A
const TRANSIENT_CHUNK_SEC: f64 = 0.005; // R
const TRANSIENT_MULT: f64 = 1.5; // T
const EARLIER_STABLE_LEN: usize = 8; // P
const EARLIER_STABLE_TOL: f64 = 0.25; // D
const EARLIER_BEFORE_RATIO: f64 = 0.6; // E
const EARLIER_MIN_GAP: usize = 16; // C
const RECONCILE_CLOSE_DIST: usize = 16; // L (used as section count)
const ENERGY_RATIO_THRESHOLD: f64 = 0.7; // I
const ENERGY_RATIO_DIFF_MIN: f64 = 0.01; // W
const SUSTAIN_THRESHOLD: f64 = 0.85; // q
const CONTRAST_THRESHOLD: f64 = 1.4; // O
const WEAK_FILTERED_RATIO: f64 = 0.6; // H

/// Find the first section where rolling 4-section avg energy >= 0.85*max and
/// section energy >= 0.65*peak. If found late (>= 40%), try to find an earlier
/// drop via `find_earlier_drop`. Matches JS `j()`.
fn energy_threshold_drop(sections: &[Section]) -> Option<usize> {
    if sections.is_empty() {
        return None;
    }
    // Inner function: basic threshold search
    let basic_find = |secs: &[Section]| -> Option<usize> {
        let rolling: Vec<f64> = (0..secs.len())
            .map(|i| {
                let end = secs.len().min(i + DROP_ROLLING_WINDOW);
                let mut sum = 0.0;
                let mut count = 0;
                for j in i..end {
                    sum += secs[j].energy;
                    count += 1;
                }
                if count == 0 { 0.0 } else { sum / count as f64 }
            })
            .collect();
        let max_rolling = rolling.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
        let peak_energy = secs.iter().map(|s| s.energy).fold(f64::NEG_INFINITY, f64::max);
        let rolling_thresh = max_rolling * DROP_ROLLING_THRESHOLD;
        let energy_thresh = peak_energy * DROP_ENERGY_THRESHOLD;
        for i in 0..secs.len() {
            if rolling[i] >= rolling_thresh && secs[i].energy >= energy_thresh {
                return Some(i);
            }
        }
        None
    };

    let mut result = basic_find(sections)?;

    if result as f64 / sections.len() as f64 >= 0.4 {
        if let Some(earlier) = find_earlier_drop(sections, result) {
            let mut min_between = f64::INFINITY;
            for i in (earlier + 8)..result {
                if sections[i].energy < min_between {
                    min_between = sections[i].energy;
                }
            }
            if min_between < 0.5 * sections[earlier].energy {
                result = earlier;
            }
        }
    }

    Some(result)
}

/// Search backward from `before_idx` for a stable plateau that could be the real drop.
/// Matches JS `z()`.
fn find_earlier_drop(sections: &[Section], before_idx: usize) -> Option<usize> {
    if sections.is_empty() {
        return None;
    }
    let mut energies: Vec<f64> = sections.iter().map(|s| s.energy).collect();
    energies.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let median = energies[energies.len() / 2];

    let scan_start = 0; // $ = Infinity, so max(0, before_idx - Inf) = 0
    let scan_end = if before_idx >= EARLIER_MIN_GAP {
        before_idx - EARLIER_MIN_GAP
    } else {
        return None;
    };

    for i in scan_start..scan_end {
        if sections[i].energy < median {
            continue;
        }
        // Check next P sections stay within ±D of this section's energy
        let mut stable = true;
        for r in (i + 1)..(i + EARLIER_STABLE_LEN) {
            if r >= sections.len() {
                stable = false;
                break;
            }
            let ratio = sections[r].energy / sections[i].energy;
            if ratio < 1.0 - EARLIER_STABLE_TOL || ratio > 1.0 + EARLIER_STABLE_TOL {
                stable = false;
                break;
            }
        }
        if !stable {
            continue;
        }
        // An earlier drop requires a quieter preceding window.
        let before_start = i.saturating_sub(8);
        if i - before_start < 4 && i > 4 {
            continue;
        }
        let mut before_sum = 0.0;
        let mut before_count = 0u32;
        for j in before_start..i {
            before_sum += sections[j].energy;
            before_count += 1;
        }
        let before_avg = if before_count == 0 {
            0.0
        } else {
            before_sum / before_count as f64
        };
        if !(before_avg >= sections[i].energy * EARLIER_BEFORE_RATIO) {
            return Some(i);
        }
    }
    None
}

/// Check for an extended intro pattern starting at `idx`.
/// Matches JS `V()`.
fn check_intro_pattern(sections: &[Section], idx: usize) -> Option<usize> {
    if idx < 1 || idx > 25 {
        return None;
    }
    let n = sections[idx].energy;
    if n <= 0.0 {
        return None;
    }
    // Count sections in [idx, idx+8) with >= 60% of energy
    let r = sections.len().min(idx + 8);
    let mut a = 0usize;
    for i in idx..r {
        if sections[i].energy >= 0.6 * n {
            a += 1;
        }
    }
    if a < 6 {
        return None;
    }
    // Count sections in [idx+8, idx+16) with >= 80% of energy
    let mut o = 0usize;
    let i_end = sections.len().min(idx + 16);
    for i in (idx + 8)..i_end {
        if sections[i].energy >= 0.8 * n {
            o += 1;
        }
    }
    if o >= 7 {
        // Extended plateau case
        let mut plateau_end = idx + 16;
        for j in (idx + 16)..sections.len().min(idx + 40) {
            if sections[j].energy >= 0.6 * n {
                plateau_end = j;
            } else {
                break;
            }
        }
        // Find gap of 4+ below 0.4*n then recovery to 0.7*n
        let mut gap_count = 0usize;
        let mut recovery_idx: Option<usize> = None;
        for j in (plateau_end + 1)..sections.len().min(plateau_end + 20) {
            if sections[j].energy < 0.4 * n {
                gap_count += 1;
            } else if gap_count >= 4 && sections[j].energy >= 0.7 * n {
                recovery_idx = Some(j);
                break;
            }
        }
        let recovery = match recovery_idx {
            Some(ri) => ri,
            None => return None,
        };
        if sections[recovery].energy >= 0.85 * n {
            return None;
        }
    }

    // Search for gap below 0.4*n starting at idx + a
    let mut s: isize = -1;
    let mut c: isize = -1;
    let search_end = sections.len().min(idx + 24);
    for r_idx in (idx + a)..search_end {
        if sections[r_idx].energy < 0.4 * n {
            if s == -1 {
                s = r_idx as isize;
            }
            c = r_idx as isize;
        } else {
            if s != -1 && c - s + 1 >= 4 {
                break;
            }
            s = -1;
            c = -1;
        }
    }
    if s == -1 || c - s + 1 < 4 {
        return None;
    }
    // Find peak energy in [idx, idx+a)
    let mut peak_in_intro = 0.0_f64;
    for j in idx..(idx + a) {
        if sections[j].energy > peak_in_intro {
            peak_in_intro = sections[j].energy;
        }
    }
    // Find recovery after gap
    let gap_end = c as usize;
    let recovery_end = sections.len().saturating_sub(4).min(gap_end + 1 + 24);
    for j in (gap_end + 1)..recovery_end {
        if sections[j].energy < 0.8 * peak_in_intro {
            continue;
        }
        let mut sum4 = 0.0;
        for k in j..(j + 4) {
            sum4 += sections[k].energy;
        }
        if sum4 / 4.0 >= 0.75 * peak_in_intro {
            return Some(j);
        }
    }
    None
}

/// Compute transient counts per section from filtered PCM.
/// Matches the anonymous function inside K() at line ~324.
fn compute_transient_counts(
    sections: &[Section],
    pcm: &[f32],
    sample_rate: u32,
) -> Vec<usize> {
    let chunk_samples = (sample_rate as f64 * TRANSIENT_CHUNK_SEC).floor() as usize;
    if chunk_samples == 0 {
        return vec![0; sections.len()];
    }

    // Build max-amplitude chunks
    let mut chunk_maxes = Vec::new();
    let mut pos = 0;
    while pos < pcm.len() {
        let end = (pos + chunk_samples).min(pcm.len());
        let mut mx = 0.0_f64;
        for i in pos..end {
            let v = (pcm[i] as f64).abs();
            if v > mx {
                mx = v;
            }
        }
        chunk_maxes.push(mx);
        pos += chunk_samples;
    }

    // Compute positive differences
    let mut diffs = vec![0.0_f64];
    let mut pos_sum = 0.0_f64;
    let mut pos_count = 0u32;
    for i in 1..chunk_maxes.len() {
        let d = chunk_maxes[i] - chunk_maxes[i - 1];
        let clamped = if d > 0.0 { d } else { 0.0 };
        diffs.push(clamped);
        if d > 0.0 {
            pos_sum += d;
            pos_count += 1;
        }
    }
    let threshold = if pos_count == 0 {
        0.0
    } else {
        (pos_sum / pos_count as f64) * TRANSIENT_MULT
    };

    // Count peaks per section
    let mut counts = Vec::with_capacity(sections.len());
    for sec in sections {
        let start_chunk = (sec.start_time / TRANSIENT_CHUNK_SEC).floor() as usize;
        let end_chunk = ((sec.start_time + sec.duration) / TRANSIENT_CHUNK_SEC)
            .floor()
            .min(diffs.len() as f64) as usize;
        let mut count = 0usize;
        for i in (start_chunk + 1)..end_chunk.saturating_sub(1) {
            if i >= diffs.len() {
                break;
            }
            if i > 0 && i + 1 < diffs.len() && diffs[i] > diffs[i - 1] && diffs[i] > diffs[i + 1] && diffs[i] > threshold
            {
                count += 1;
            }
        }
        counts.push(count);
    }
    counts
}

/// Energy-threshold drop with transient criteria.
/// Like `energy_threshold_drop` but also requires transient count >= 0.2 * max_transients.
/// Matches the inline function at K() line ~359.
fn energy_threshold_drop_with_transients(
    sections: &[Section],
    transient_counts: &[usize],
) -> Option<usize> {
    if sections.is_empty() {
        return None;
    }
    let rolling: Vec<f64> = (0..sections.len())
        .map(|i| {
            let end = sections.len().min(i + DROP_ROLLING_WINDOW);
            let mut sum = 0.0;
            let mut count = 0;
            for j in i..end {
                sum += sections[j].energy;
                count += 1;
            }
            if count == 0 { 0.0 } else { sum / count as f64 }
        })
        .collect();
    let max_rolling = rolling.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
    let peak_energy = sections.iter().map(|s| s.energy).fold(f64::NEG_INFINITY, f64::max);
    let max_transients = transient_counts.iter().cloned().max().unwrap_or(0);
    let rolling_thresh = max_rolling * DROP_ROLLING_THRESHOLD;
    let energy_thresh = peak_energy * DROP_ENERGY_THRESHOLD;
    let transient_thresh = (max_transients as f64 * DROP_TRANSIENT_THRESHOLD) as usize;

    for i in 0..sections.len() {
        if rolling[i] >= rolling_thresh
            && sections[i].energy >= energy_thresh
            && transient_counts.get(i).copied().unwrap_or(0) >= transient_thresh
        {
            return Some(i);
        }
    }
    None
}

/// Scan from index 4 to `drop_idx - 16` for an earlier "plateau then dip" pattern.
/// Matches the inline function at K() line ~417.
fn find_earlier_plateau(sections: &[Section], drop_idx: usize) -> Option<usize> {
    let peak = sections.iter().map(|s| s.energy).fold(f64::NEG_INFINITY, f64::max);
    if drop_idx < 16 {
        return None;
    }
    let scan_end = drop_idx - 16;
    for a in 4..scan_end {
        let r = sections[a].energy;
        if r < 0.7 * peak {
            continue;
        }
        let mut sustain_count = 0;
        for j in (a + 1)..(a + 9).min(sections.len()) {
            if sections[j].energy >= 0.7 * r {
                sustain_count += 1;
            }
        }
        if sustain_count < 4 {
            continue;
        }
        // Check that preceding sections have a dip
        let mut min_before = f64::INFINITY;
        for j in a.saturating_sub(4)..a {
            if sections[j].energy < min_before {
                min_before = sections[j].energy;
            }
        }
        if min_before >= 0.3 * r {
            continue;
        }
        // Check that sections between a+9 and drop_idx have a dip
        let mut min_between = f64::INFINITY;
        for j in (a + 9)..drop_idx {
            if sections[j].energy < min_between {
                min_between = sections[j].energy;
            }
        }
        if min_between > 0.2 * r {
            continue;
        }
        return Some(a);
    }
    None
}

/// Check for "intro dip" pattern: early drop with dip then real drop.
/// Matches the anonymous function at K() line ~521.
fn check_intro_dip(sections: &[Section], drop_section: usize) -> Option<usize> {
    if drop_section > 4 {
        return None;
    }
    let peak = sections.iter().map(|s| s.energy).fold(f64::NEG_INFINITY, f64::max);
    if peak <= 0.0 {
        return None;
    }
    let threshold = 0.3 * peak;
    // Find first section after drop that dips below threshold
    let mut dip_start: isize = -1;
    for i in (drop_section + 1)..sections.len().min(drop_section + 24) {
        if sections[i].energy < threshold {
            dip_start = i as isize;
            break;
        }
    }
    if dip_start == -1 {
        return None;
    }
    let min_gap = if drop_section >= 1 { 7 } else { 12 };
    let drop_energy = sections[drop_section].energy;
    let mut gap_count = 0usize;
    let mut min_in_gap = f64::INFINITY;
    let mut recovery: isize = -1;
    let search_end = sections.len().min(dip_start as usize + 40);
    for i in (dip_start as usize)..search_end {
        if sections[i].energy < threshold {
            gap_count += 1;
            if sections[i].energy < min_in_gap {
                min_in_gap = sections[i].energy;
            }
        } else {
            let required =
                if drop_section == 0 && drop_energy > 0.0 && min_in_gap < 0.1 * drop_energy {
                    7
                } else {
                    min_gap
                };
            if gap_count >= required {
                recovery = i as isize;
            }
            break;
        }
    }
    if recovery == -1 {
        return None;
    }
    let high_threshold = 0.7 * peak;
    for i in (recovery as usize)..sections.len().min(recovery as usize + 8) {
        if sections[i].energy >= high_threshold {
            return Some(i);
        }
    }
    None
}

/// Check for "constant high energy from start" pattern.
/// Matches the inline function at K() line ~559.
fn check_constant_high_start(sections: &[Section], drop_section: usize) -> Option<usize> {
    if drop_section <= 4 {
        return None;
    }
    if sections.len() < 32 {
        return None;
    }
    let peak = sections.iter().map(|s| s.energy).fold(f64::NEG_INFINITY, f64::max);
    if peak <= 0.0 {
        return None;
    }
    let start_max = sections[0].energy.max(sections.get(1).map_or(0.0, |s| s.energy));
    if start_max < 0.85 * peak {
        return None;
    }
    for i in 0..30.min(sections.len()) {
        if sections[i].energy < 0.5 * peak {
            return None;
        }
    }
    let drop_energy = sections[drop_section].energy;
    if drop_energy < 0.85 * start_max || drop_energy > 1.3 * start_max {
        return None;
    }
    Some(0)
}

/// Genre-specific drop override using 80Hz lowpass.
/// Matches the genre block at K() line ~680.
fn genre_drop_override(
    sections_80hz: &[Section],
    genre: &Option<String>,
    drop_time: f64,
    section_duration: f64,
) -> Option<f64> {
    let genre_str = classify_genre(genre)?;
    if sections_80hz.is_empty() {
        return None;
    }
    let peak = sections_80hz
        .iter()
        .map(|s| s.energy)
        .fold(f64::NEG_INFINITY, f64::max);
    if peak <= 0.0 {
        return None;
    }
    let drop_section_rel = ((drop_time - sections_80hz[0].start_time) / section_duration).round()
        as isize;
    let drop_section = if drop_section_rel >= 0 {
        drop_section_rel as usize
    } else {
        0
    };

    let (thresh_ratio, min_len) = if genre_str == "techno" {
        (0.35, 3)
    } else {
        (0.3, 4)
    };
    let gap = find_energy_gap(sections_80hz, thresh_ratio, min_len)?;

    if genre_str == "hardcore" {
        if drop_section > (0.4 * sections_80hz.len() as f64) as usize || gap.0 <= drop_section {
            return None;
        }
    } else if drop_section > 4 {
        return None;
    }

    for i in (gap.1 + 1)..sections_80hz.len() {
        if sections_80hz[i].energy >= 0.7 * peak {
            return Some(sections_80hz[i].start_time);
        }
    }
    None
}

/// Main drop detection. Matches JS `K()`.
pub fn detect_drop(
    pcm: &[f32],
    sample_rate: u32,
    bpm: f64,
    first_beat: f64,
    precomputed_lowpass: Option<&[f32]>,
    genre: &Option<String>,
) -> DropResult {
    let section_dur = if bpm > 0.0 { 240.0 / bpm } else { 1.0 };

    // Path 1: Cascaded biquad lowpass
    let cascaded;
    let filtered = match precomputed_lowpass {
        Some(p) => p,
        None => {
            cascaded = cascaded_lowpass(pcm, sample_rate);
            &cascaded
        }
    };
    let filtered_sections = build_sections(filtered, sample_rate, bpm, first_beat, 4, 0.02);
    let filtered_drop_idx = energy_threshold_drop(&filtered_sections);
    let filtered_drop_time = filtered_drop_idx
        .and_then(|i| filtered_sections.get(i))
        .map(|s| s.start_time);
    let h = energy_ratio_to_peak(&filtered_sections, filtered_drop_idx);

    // Path 2: IIR lowpass at 300Hz, 6 passes
    let lp = low_pass_iir(pcm, sample_rate, 300.0, 6);
    let lp_sections = build_sections(&lp, sample_rate, bpm, first_beat, 4, 0.02);
    let lp_peak = lp_sections
        .iter()
        .map(|s| s.energy)
        .fold(f64::NEG_INFINITY, f64::max);

    let mut d: Option<usize> = None;
    if !lp_sections.is_empty() {
        // Compute transient counts
        let transient_counts = compute_transient_counts(&lp_sections, pcm, sample_rate);
        d = energy_threshold_drop_with_transients(&lp_sections, &transient_counts);

        // If d is >= 30% through, try find_earlier_drop
        if let Some(d_idx) = d {
            if d_idx as f64 / lp_sections.len() as f64 >= 0.3 {
                if let Some(earlier) = find_earlier_drop(&lp_sections, d_idx) {
                    let mut min_between = f64::INFINITY;
                    for i in (earlier + 8)..d_idx {
                        if lp_sections[i].energy < min_between {
                            min_between = lp_sections[i].energy;
                        }
                    }
                    let current_stronger =
                        lp_sections[d_idx].energy >= 1.2 * lp_sections[earlier].energy;
                    if min_between < 0.5 * lp_sections[earlier].energy && !current_stronger {
                        d = Some(earlier);
                    }
                }
            }
        }
    }

    let b = d.and_then(|i| lp_sections.get(i)).map(|s| s.start_time);
    let g = energy_ratio_to_peak(&lp_sections, d);
    let y = energy_sustain_after(&lp_sections, d);
    let k = energy_contrast_before(&lp_sections, d);

    // Reconcile the two drop candidates
    let mut drop_time: Option<f64> = None;
    match (filtered_drop_time, b) {
        (Some(f_time), Some(b_time)) => {
            if (f_time - b_time).abs() <= RECONCILE_CLOSE_DIST as f64 {
                // Close together: prefer lowpass
                drop_time = Some(b_time);

                // Check for earlier plateau override
                if let Some(d_idx) = d {
                    if d_idx as f64 / lp_sections.len() as f64 >= 0.25 {
                        if let Some(earlier) = find_earlier_plateau(&lp_sections, d_idx) {
                            drop_time = Some(lp_sections[earlier].start_time);
                        }
                    }
                }

                // Check if filtered drop index is valid and close but earlier
                if let (Some(u_idx), Some(d_idx)) = (filtered_drop_idx, d) {
                    if u_idx < d_idx && d_idx - u_idx >= 2 && d_idx - u_idx <= 12 && u_idx >= 2 {
                        let e_at_u = lp_sections[u_idx].energy;
                        let peak_e = lp_sections
                            .iter()
                            .map(|s| s.energy)
                            .fold(f64::NEG_INFINITY, f64::max);
                        let prev1 = lp_sections[u_idx - 1].energy;
                        let prev2 = lp_sections[u_idx - 2].energy;
                        if e_at_u >= 0.5 * peak_e && prev1.min(prev2) < 0.15 * e_at_u {
                            drop_time = Some(lp_sections[u_idx].start_time);
                        }
                    }
                }
            } else if b_time > f_time {
                // Lowpass drop is later
                let u_frac = filtered_drop_idx
                    .map(|i| i as f64 / lp_sections.len() as f64)
                    .unwrap_or(0.0);
                let d_frac = d
                    .map(|i| i as f64 / lp_sections.len() as f64)
                    .unwrap_or(0.0);

                if filtered_drop_idx.is_some()
                    && d.is_some()
                    && u_frac < 0.15
                    && d_frac >= 0.15
                    && d_frac <= 0.4
                    && g >= ENERGY_RATIO_THRESHOLD
                    && y >= 0.8
                    && k >= 1.3
                {
                    drop_time = Some(b_time);
                } else {
                    let filt_strong = h >= ENERGY_RATIO_THRESHOLD;
                    let lp_strong = g >= ENERGY_RATIO_THRESHOLD;
                    let lp_stronger = g - h >= ENERGY_RATIO_DIFF_MIN;
                    let good_sustain = y >= SUSTAIN_THRESHOLD;
                    let good_contrast = k >= CONTRAST_THRESHOLD;

                    let filt_energy_ratio = match filtered_drop_idx {
                        Some(u_idx) if u_idx < lp_sections.len() && !lp_sections.is_empty() => {
                            lp_sections[u_idx].energy / lp_peak.max(1e-9)
                        }
                        _ => 1.0,
                    };

                    let prefer_lp = good_sustain
                        && good_contrast
                        && (if filt_strong {
                            lp_stronger || (lp_strong && filt_energy_ratio < WEAK_FILTERED_RATIO)
                        } else {
                            lp_strong
                        });

                    let filt_frac = filtered_drop_idx
                        .map(|i| i as f64 / lp_sections.len() as f64)
                        .unwrap_or(0.0);
                    let mut mid_range_filtered = filt_strong
                        && filt_frac >= 0.15
                        && filt_frac <= 0.7
                        && filt_energy_ratio >= 0.55
                        && filt_energy_ratio < 0.75;

                    // Check for deep valley between filtered and lowpass drops
                    if mid_range_filtered {
                        if let (Some(u_idx), Some(d_idx)) = (filtered_drop_idx, d) {
                            if u_idx < d_idx {
                                let half_energy = 0.5 * lp_sections[u_idx].energy;
                                let mut run = 0usize;
                                let mut max_run = 0usize;
                                for j in (u_idx + 4)..d_idx {
                                    if lp_sections[j].energy < half_energy {
                                        run += 1;
                                        if run > max_run {
                                            max_run = run;
                                        }
                                    } else {
                                        run = 0;
                                    }
                                }
                                if max_run >= 4 {
                                    mid_range_filtered = false;
                                }
                            }
                        }
                    }

                    drop_time = if prefer_lp && !mid_range_filtered {
                        Some(b_time)
                    } else {
                        Some(f_time)
                    };
                }
            } else {
                // Filtered drop is later (f > b)
                let filt_strong = h >= ENERGY_RATIO_THRESHOLD;
                let lp_strong = g >= ENERGY_RATIO_THRESHOLD;
                let good_sustain = y >= SUSTAIN_THRESHOLD;
                let good_contrast = k >= CONTRAST_THRESHOLD;

                let filt_energy_ratio = match filtered_drop_idx {
                    Some(u_idx) if u_idx < lp_sections.len() && !lp_sections.is_empty() => {
                        lp_sections[u_idx].energy / lp_peak.max(1e-9)
                    }
                    _ => 1.0,
                };

                let all_four = filt_strong
                    && lp_strong
                    && good_sustain
                    && good_contrast
                    && filt_energy_ratio < WEAK_FILTERED_RATIO;

                let mut late_swap = false;
                if let (Some(u_idx), Some(d_idx)) = (filtered_drop_idx, d) {
                    if lp_strong && good_sustain && good_contrast {
                        let u_frac = u_idx as f64 / lp_sections.len() as f64;
                        let d_frac = d_idx as f64 / lp_sections.len() as f64;
                        if u_frac >= 0.55 && d_frac >= 0.08 && d_frac <= 0.45 {
                            late_swap = true;
                        }
                    }
                }

                drop_time = if all_four || late_swap {
                    Some(b_time)
                } else {
                    Some(f_time)
                };
            }
        }
        (Some(f_time), None) => drop_time = Some(f_time),
        (None, Some(b_time)) => drop_time = Some(b_time),
        (None, None) => {}
    }

    // Post-processing on the chosen drop time
    if let Some(ref mut dt) = drop_time {
        if !lp_sections.is_empty() {
            // Find nearest section to drop time
            let mut t_idx = nearest_section(&lp_sections, *dt);

            // Check intro dip pattern
            if let Some(new_idx) = check_intro_dip(&lp_sections, t_idx) {
                *dt = lp_sections[new_idx].start_time;
                t_idx = new_idx;

                // Also check intro pattern after intro dip
                if let Some(intro_idx) = check_intro_pattern(&lp_sections, t_idx) {
                    *dt = lp_sections[intro_idx].start_time;
                    t_idx = intro_idx;
                }
            } else {
                // Check intro pattern
                if let Some(intro_idx) = check_intro_pattern(&lp_sections, t_idx) {
                    *dt = lp_sections[intro_idx].start_time;
                    t_idx = intro_idx;
                } else {
                    // Check constant high start
                    if let Some(new_idx) = check_constant_high_start(&lp_sections, t_idx) {
                        *dt = lp_sections[new_idx].start_time;
                        t_idx = new_idx;
                    }
                }
            }

            // Back-shift: if prev section >= 70% and two-back < 60%
            if t_idx >= 2 {
                let e = lp_sections[t_idx].energy;
                let prev = lp_sections[t_idx - 1].energy;
                let prev2 = lp_sections[t_idx - 2].energy;
                if prev >= 0.7 * e && prev2 < 0.6 * e {
                    *dt = lp_sections[t_idx - 1].start_time;
                    t_idx -= 1;
                }
            }

            // refine_drop_start
            if t_idx < lp_sections.len() {
                let refined = refine_drop_start(&lp_sections, t_idx, 16);
                if refined != t_idx {
                    *dt = lp_sections[refined].start_time;
                    t_idx = refined;
                }
            }

            // Beat-alignment: snap to 4-beat grid
            if t_idx >= 4 {
                let sec_dur = lp_sections[0].duration;
                let dur = if sec_dur > 0.0 { sec_dur } else { 240.0 / bpm };
                let beat_offset =
                    ((lp_sections[t_idx].start_time - first_beat) / dur).round() as isize;
                let phase = ((beat_offset % 4) + 4) % 4;
                if phase != 0 && lp_sections[t_idx].energy > 0.0 {
                    for shift in (1..=3).rev() {
                        let new_idx = t_idx as isize - shift;
                        if new_idx < 1 {
                            continue;
                        }
                        let new_idx = new_idx as usize;
                        if ((beat_offset - shift) % 4 + 4) % 4 != 0 {
                            continue;
                        }
                        let new_energy = lp_sections[new_idx].energy;
                        if lp_sections[new_idx - 1].energy >= 0.7 * new_energy {
                            continue;
                        }
                        let mut all_high = true;
                        let target_energy = lp_sections[t_idx].energy;
                        for j in new_idx..=t_idx {
                            if lp_sections[j].energy < 0.9 * target_energy {
                                all_high = false;
                                break;
                            }
                        }
                        if all_high {
                            *dt = lp_sections[new_idx].start_time;
                            t_idx = new_idx;
                            break;
                        }
                    }
                }
            }

            // Descending energy + plateau shift
            if t_idx >= 4 && t_idx + 7 < lp_sections.len() {
                let e = lp_sections[t_idx].energy;
                let peak_all = lp_sections
                    .iter()
                    .map(|s| s.energy)
                    .fold(f64::NEG_INFINITY, f64::max);
                let prev = if t_idx >= 1 {
                    lp_sections[t_idx - 1].energy
                } else {
                    0.0
                };
                if e > 0.0 && prev <= 0.2 * e && e >= 0.85 * peak_all {
                    let e1 = lp_sections[t_idx + 1].energy;
                    let e2 = lp_sections[t_idx + 2].energy;
                    let e3 = lp_sections[t_idx + 3].energy;
                    if e1 <= e && e2 <= e1 && e3 <= e2 && e3 <= 0.8 * e {
                        let mut mx = 0.0_f64;
                        let mut mn = f64::INFINITY;
                        let mut sum = 0.0;
                        for j in (t_idx + 4)..=(t_idx + 7) {
                            let v = lp_sections[j].energy;
                            if v > mx { mx = v; }
                            if v < mn { mn = v; }
                            sum += v;
                        }
                        let avg = sum / 4.0;
                        if mn > 0.0 && mx <= 1.2 * mn && avg <= 0.9 * e && avg >= 0.55 * e {
                            *dt = lp_sections[t_idx + 4].start_time;
                            t_idx += 4;
                        }
                    }
                }
            }

            // Final beat-phase adjustment
            if t_idx < lp_sections.len() {
                let sec_dur = lp_sections[0].duration;
                let dur = if sec_dur > 0.0 { sec_dur } else { 240.0 / bpm };
                let phase = (((lp_sections[t_idx].start_time - first_beat) / dur).round() as isize
                    % 4
                    + 4)
                    % 4;
                let mut adjustment: isize = 0;

                if phase == 0 && t_idx + 8 < lp_sections.len() {
                    let e = lp_sections[t_idx].energy;
                    let e4 = lp_sections[t_idx + 4].energy;
                    let mut min47 = f64::INFINITY;
                    for j in (t_idx + 4)..=(t_idx + 7) {
                        if lp_sections[j].energy < min47 {
                            min47 = lp_sections[j].energy;
                        }
                    }
                    let prev = if t_idx >= 1 {
                        lp_sections[t_idx - 1].energy
                    } else {
                        0.0
                    };
                    if e > 0.0
                        && e4 >= 1.2 * e
                        && min47 >= 1.1 * e
                        && prev >= 0.7 * e
                        && prev <= e
                    {
                        adjustment = 4;
                    }
                }
                if phase == 1 {
                    adjustment = -1;
                } else if phase == 3 {
                    adjustment = 1;
                } else if phase == 2 && t_idx + 5 < lp_sections.len() && t_idx >= 1 {
                    let e = lp_sections[t_idx].energy;
                    let prev = lp_sections[t_idx - 1].energy;
                    let next = lp_sections[t_idx + 1].energy;
                    let mut max25 = 0.0_f64;
                    for j in (t_idx + 2)..=(t_idx + 5) {
                        if lp_sections[j].energy > max25 {
                            max25 = lp_sections[j].energy;
                        }
                    }
                    if max25 > 0.0
                        && ((e < 0.92 * max25 && prev >= 0.5 * e && prev < 0.9 * e)
                            || (prev >= 0.9 * e && next < 0.8 * e && max25 >= 1.1 * e))
                    {
                        adjustment = 2;
                    }
                }

                if adjustment != 0 {
                    let new_idx = t_idx as isize + adjustment;
                    if new_idx >= 0 && (new_idx as usize) < lp_sections.len() {
                        *dt = lp_sections[new_idx as usize].start_time;
                        t_idx = new_idx as usize;
                    }
                }
            }
        }
    }

    // Genre-specific override
    if let Some(ref mut dt) = drop_time {
        if classify_genre(genre).is_some() {
            let lp80 = low_pass_iir(pcm, sample_rate, 80.0, 6);
            let sections_80hz = build_sections(&lp80, sample_rate, bpm, first_beat, 4, 0.02);
            if !sections_80hz.is_empty() {
                let sec_dur = sections_80hz[0].duration;
                if let Some(new_time) =
                    genre_drop_override(&sections_80hz, genre, *dt, sec_dur)
                {
                    *dt = new_time;
                }
            }
        }
    }

    // Build result
    let sec_dur = if !lp_sections.is_empty() {
        lp_sections[0].duration
    } else {
        section_dur
    };

    let start_time = match drop_time {
        Some(dt) if dt < first_beat => dt,
        _ => first_beat,
    };

    let start = StructuralCue {
        time_seconds: start_time,
        name: "Start".into(),
        color: "Red".into(),
    };

    let drop_cue = drop_time.map(|dt| StructuralCue {
        time_seconds: dt,
        name: "Drop".into(),
        color: "Red".into(),
    });

    DropResult {
        start,
        drop: drop_cue,
    }
}

// ── Breakdown detection constants (JS vt / Ft) ──
const BD_STRIDE: usize = 64;          // Y
const BD_SECTION_STEP: usize = 16;    // Q
const BD_MIN_MULT: usize = 1;         // Z
const BD_MAX_MULT: usize = 30;        // X
const BD_WINDOW: usize = 16;          // tt
const BD_BEFORE_WINDOW: usize = 16;   // et
const BD_POS_CENTER: f64 = 0.48;      // nt
const BD_POS_SIGMA: f64 = 0.18;       // rt
const BD_DIST_CENTER: f64 = 192.0;    // at
const BD_DIST_SIGMA: f64 = 96.0;      // ot
const BD_MIN_GAP_SEC: f64 = 30.0;     // it
const BD_MIN_POS: f64 = 0.25;         // st
const BD_TIGHT_POS: f64 = 0.1;        // ct
const BD_VALIDATE_LEN: usize = 8;     // lt
const BD_VALIDATE_RATIO: f64 = 0.85;  // ut
const BD_BEFORE_DROP: f64 = 0.2;      // ft
const BD_RECOVERY_RATIO: f64 = 0.4;   // ht
const BD_CONSEC_LOW_NEED: usize = 8;  // pt
const BD_LOW_THRESH: f64 = 0.3;       // dt
const BD_LATE_PENALTY: f64 = 0.5;     // mt
const BD_MAX_POS: f64 = 0.55;         // bt
const BD_EDGE_RANGE: usize = 16;      // gt
const BD_GRID_STEP: usize = 4;        // yt
const BD_EDGE_CONTRAST: f64 = 0.4;    // kt
const BD_EDGE_FLOOR: f64 = 0.3;       // Mt
const BD_EDGE_WINDOW: usize = 6;      // _t

// ── Second-drop detection constants (JS Ht / zt) ──
const SD_GRID: usize = 4;             // xt
const SD_MIN_AFTER_DROP: usize = 64;  // Bt
const SD_MAX_SCAN: usize = 320;       // wt
const SD_MIN_AFTER_BD: usize = 96;    // Gt
const SD_FALLBACK_DIST: usize = 192;  // Nt
const SD_ENERGY_WINDOW: usize = 16;   // St
const SD_BEFORE_WINDOW: usize = 16;   // At
const SD_ENERGY_RATIO: f64 = 0.6;     // Rt
const SD_BEFORE_RATIO: f64 = 0.7;     // Tt
const SD_POS_CENTER: f64 = 0.65;      // $t
const SD_POS_SIGMA: f64 = 0.1;        // Pt
const SD_DIST_CENTER: f64 = 128.0;    // Dt
const SD_DIST_SIGMA: f64 = 64.0;      // Et
const SD_MAX_POS: f64 = 0.82;         // Ct
const SD_BD_OFFSET: usize = 128;      // Lt
const SD_DROP_OFFSET: usize = 384;    // It
const SD_FALLBACK_POS: f64 = 0.65;    // Wt

/// Find the precise energy-drop edge aligned to a grid.
/// Matches JS `Ft(t, e, n, r, a, o, i)`.
fn find_breakdown_edge(
    sections: &[Section],
    candidate_idx: usize,
    ref_energy: f64,
    drop_idx: usize,
    total_duration: f64,
    grid_step: usize,
    offset: usize,
) -> isize {
    if grid_step == 0 {
        return -1;
    }
    let e = candidate_idx as isize;
    let r = drop_idx as isize;
    let o = grid_step as isize;
    // Snap to grid: s = e - (((e - r) % o + o) % o) + offset
    let s = e - (((e - r) % o + o) % o) + offset as isize;
    let mut c = s - BD_EDGE_RANGE as isize;
    while c <= r {
        c += o;
    }
    let l = s + BD_EDGE_RANGE as isize;
    let mut pos = c;
    while pos <= l {
        if pos <= 0 {
            pos += o;
            continue;
        }
        let idx = pos as usize;
        if idx + BD_EDGE_WINDOW > sections.len() {
            break;
        }
        if total_duration - sections[idx].start_time < BD_MIN_GAP_SEC {
            pos += o;
            continue;
        }
        let energy = sections[idx].energy;
        let before_start = idx.saturating_sub(4);
        let mut before_sum = 0.0;
        for i in before_start..idx {
            before_sum += sections[i].energy;
        }
        let before_avg = before_sum / (idx - before_start).max(1) as f64;
        if energy >= before_avg * BD_EDGE_CONTRAST {
            pos += o;
            continue;
        }
        if before_avg < 0.5 * ref_energy {
            pos += o;
            continue;
        }
        let next_energy = sections[idx + 1].energy;
        if energy >= ref_energy * BD_EDGE_FLOOR && next_energy >= ref_energy * BD_EDGE_FLOOR {
            pos += o;
            continue;
        }
        let mut window_sum = 0.0;
        for i in idx..(idx + BD_EDGE_WINDOW) {
            window_sum += sections[i].energy;
        }
        if window_sum / BD_EDGE_WINDOW as f64 >= ref_energy * BD_EDGE_FLOOR {
            pos += o;
            continue;
        }
        return pos;
    }
    -1
}

/// Detect the breakdown point before a drop.
/// Matches JS `vt(t, e, n, o, i, s, c, l, u)`.
pub fn detect_breakdown(
    pcm: &[f32],
    sample_rate: u32,
    bpm: f64,
    first_beat: f64,
    drop_time: f64,
    total_duration: f64,
    min_cue_beats: u32,
    genre: &Option<String>,
    use_tight_start: bool,
) -> Option<StructuralCue> {
    let is_not_hardcore = classify_genre(genre).as_deref() != Some("hardcore");
    let lp = low_pass_iir(pcm, sample_rate, 300.0, 6);
    let h = build_sections(&lp, sample_rate, bpm, first_beat, 4, 0.02);
    if h.is_empty() {
        return None;
    }

    let p = nearest_section(&h, drop_time);
    let d = avg_energy(&h, p, 8);
    let drop_frac = h[p].start_time / total_duration;

    // Check for early-drop breakdown pattern
    let mut has_early_breakdown = false;
    if drop_frac < 0.1 {
        for t in BD_MIN_MULT..=BD_MAX_MULT {
            let e = p + t * BD_SECTION_STEP;
            if e + BD_WINDOW > h.len() {
                break;
            }
            let frac = h[e].start_time / total_duration;
            if frac < 0.35 || frac > 0.57 {
                continue;
            }
            let mut energy_sum = 0.0;
            for i in e..(e + BD_WINDOW) {
                energy_sum += h[i].energy;
            }
            energy_sum /= BD_WINDOW as f64;
            let before_start = p.max(e.saturating_sub(BD_BEFORE_WINDOW));
            let mut before_sum = 0.0;
            let mut before_count = 0u32;
            for i in before_start..e {
                before_sum += h[i].energy;
                before_count += 1;
            }
            let before_avg = if before_count > 0 {
                before_sum / before_count as f64
            } else {
                0.0
            };
            let dip = (1.0 - energy_sum / d).max(0.0);
            let contrast = ((before_avg.min(d) - energy_sum) / d).max(0.0);
            if dip * contrast > 0.3 {
                has_early_breakdown = true;
                break;
            }
        }
    }

    // Score all candidate breakdown positions
    struct Candidate {
        section_index: usize,
        score: f64,
    }
    let mut candidates: Vec<Candidate> = Vec::new();

    for t in BD_MIN_MULT..=BD_MAX_MULT {
        let e = p + t * BD_SECTION_STEP;
        if e + BD_WINDOW > h.len() {
            break;
        }
        let mut energy_sum = 0.0;
        for i in e..(e + BD_WINDOW) {
            energy_sum += h[i].energy;
        }
        energy_sum /= BD_WINDOW as f64;

        let before_start = p.max(e.saturating_sub(BD_BEFORE_WINDOW));
        let mut before_sum = 0.0;
        let mut before_count = 0u32;
        for i in before_start..e {
            before_sum += h[i].energy;
            before_count += 1;
        }
        let before_avg = if before_count > 0 {
            before_sum / before_count as f64
        } else {
            0.0
        };

        let dip = (1.0 - energy_sum / d).max(0.0);
        let clamped_before = before_avg.min(d);
        let contrast = ((clamped_before - energy_sum) / d).max(0.0);
        let frac = h[e].start_time / total_duration;

        // Position Gaussian
        let pos_z = (frac - BD_POS_CENTER) / BD_POS_SIGMA;
        let pos_gauss = (-0.5 * pos_z * pos_z).exp();

        // Distance Gaussian
        let dist_z = (t as f64 * BD_STRIDE as f64 - BD_DIST_CENTER) / BD_DIST_SIGMA;
        let dist_gauss = (-0.5 * dist_z * dist_z).exp();

        let energy_frac = (before_avg / d).min(1.0);
        let mut score = dip
            * contrast
            * (if is_not_hardcore {
                pos_gauss.max(dist_gauss)
            } else {
                dist_gauss
            })
            * energy_frac;

        // Too close to end
        if total_duration - h[e].start_time < BD_MIN_GAP_SEC {
            score = 0.0;
        }

        // Too early
        let min_pos = if use_tight_start {
            BD_TIGHT_POS
        } else if drop_frac < 0.1 && dip > 0.75 && !has_early_breakdown {
            0.15
        } else {
            BD_MIN_POS
        };
        if frac < min_pos {
            score = 0.0;
        }

        // Late penalty: check for consecutive low-energy sections between drop and candidate
        if frac > BD_MAX_POS {
            let mut run = 0usize;
            let mut found_long_dip = false;
            for r in (p + 1)..e {
                if h[r].energy < d * BD_LOW_THRESH {
                    run += 1;
                    if run >= BD_CONSEC_LOW_NEED {
                        found_long_dip = true;
                        break;
                    }
                } else {
                    run = 0;
                }
            }
            if found_long_dip {
                score *= BD_LATE_PENALTY;
            }
        }

        candidates.push(Candidate {
            section_index: e,
            score,
        });
    }

    if candidates.is_empty() {
        return None;
    }
    let best = candidates.iter().max_by(|a, b| a.score.total_cmp(&b.score))?;
    if best.score == 0.0 {
        return None;
    }

    // Find precise breakdown edge using Ft logic
    let edge_idx = {
        let o = find_breakdown_edge(&h, best.section_index, d, p, total_duration, BD_GRID_STEP, 0);
        if o != -1 {
            o as usize
        } else {
            // Fallback: check energy at candidate
            let end = h.len().min(best.section_index + BD_EDGE_WINDOW);
            let mut sum = 0.0;
            for i in best.section_index..end {
                sum += h[i].energy;
            }
            let avg = sum / (end - best.section_index).max(1) as f64;
            if avg < 0.5 * d {
                best.section_index
            } else {
                let o2 = find_breakdown_edge(
                    &h,
                    best.section_index,
                    d,
                    p,
                    total_duration,
                    BD_GRID_STEP,
                    BD_GRID_STEP / 2,
                );
                if o2 != -1 {
                    o2 as usize
                } else {
                    best.section_index
                }
            }
        }
    };

    // Validate: energy after breakdown must not be too high
    let w = edge_idx + 1;
    let validate_end = h.len().min(w + BD_VALIDATE_LEN);
    let mut after_sum = 0.0;
    for i in w..validate_end {
        after_sum += h[i].energy;
    }
    let after_avg = after_sum / (validate_end - w).max(1) as f64;
    if after_avg >= d * BD_VALIDATE_RATIO {
        return None;
    }

    // Half-window validation
    let half_end = h.len().min(w + BD_VALIDATE_LEN / 2);
    let mut half_sum = 0.0;
    let mut half_count = 0u32;
    for i in w..half_end {
        half_sum += h[i].energy;
        half_count += 1;
    }
    let half_avg = if half_count > 0 {
        half_sum / half_count as f64
    } else {
        0.0
    };
    if half_avg >= 0.8 * d {
        return None;
    }

    // Before energy must be higher than after
    let before_start = p.max(edge_idx.saturating_sub(BD_BEFORE_WINDOW));
    let mut before_sum = 0.0;
    let mut before_count = 0u32;
    for i in before_start..edge_idx {
        before_sum += h[i].energy;
        before_count += 1;
    }
    let before_avg = if before_count > 0 {
        before_sum / before_count as f64
    } else {
        0.0
    };
    if before_avg - after_avg < d * BD_BEFORE_DROP {
        return None;
    }

    // Energy must recover after breakdown
    let mut recovers = false;
    for i in (edge_idx + BD_WINDOW)..h.len() {
        if h[i].energy >= d * BD_RECOVERY_RATIO {
            recovers = true;
            break;
        }
    }
    if !recovers {
        return None;
    }

    // Edge-walk refinements
    let mut result_idx = edge_idx;
    let prev_high = if edge_idx > 0 {
        h[edge_idx - 1].energy >= 0.7 * d
    } else {
        false
    };
    let mut found_sharp_drop = false;

    // Refinement 1: look for sharp energy drop edge
    if !prev_high {
        let search_start = (p + 1).max(edge_idx.saturating_sub(12));
        for e in (search_start + 1..=edge_idx).rev() {
            let prev_e = h[e - 1].energy;
            let curr_e = h[e].energy;
            if prev_e >= 0.7 * d && curr_e <= 0.6 * prev_e {
                let mut high_count = 0usize;
                for n in (e + 1)..edge_idx {
                    if h[n].energy >= 0.85 * d {
                        high_count += 1;
                    }
                }
                if high_count < 3 {
                    result_idx = e;
                    found_sharp_drop = true;
                    break;
                }
            }
        }
    }

    // Refinement 2: look for gradual energy ratio drop
    let mut found_gradual = false;
    if !prev_high && !found_sharp_drop {
        let search_start = (p + 1).max(edge_idx.saturating_sub(24));
        let mut e = edge_idx;
        while e > search_start {
            if h[e].energy >= 0.5 * d {
                break;
            }
            let prev_e = h[e - 1].energy;
            let curr_e = h[e].energy;
            if prev_e >= 0.5 * d && curr_e > 0.0 && prev_e / curr_e >= 1.5 {
                result_idx = e;
                found_gradual = true;
                break;
            }
            e -= 1;
        }
    }

    // Refinement 3: mid-energy plateau walk-back
    if result_idx > p + 1 && result_idx < edge_idx + 1 {
        let low = 0.3 * d;
        let high = 0.7 * d;
        let recovery = 0.7 * d;
        let mut run = 0usize;
        let mut n = result_idx;
        while n > p {
            n -= 1;
            let e = h[n].energy;
            if e >= low && e < high {
                run += 1;
            } else {
                break;
            }
        }
        if run >= 8 {
            let walk_start = (p + 1).max(result_idx - run);
            for e in (walk_start..result_idx).rev() {
                if h[e - 1].energy >= recovery {
                    result_idx = e;
                    break;
                }
            }
        }
    }

    // Snap to grid
    let grid_offset = ((result_idx as f64 - p as f64) / BD_GRID_STEP as f64).round() as usize
        * BD_GRID_STEP;
    result_idx = (p + BD_GRID_STEP).max(p + grid_offset);

    // Refinement 4: if no sharp/gradual edge found, try recovery-point alignment
    if !prev_high && !found_sharp_drop && !found_gradual {
        // Find first run of 8+ sections at BD_VALIDATE_RATIO * d after edge
        let mut recovery_start: isize = -1;
        let mut run = 0usize;
        for n in (edge_idx + 1)..h.len() {
            if h[n].energy >= d * BD_VALIDATE_RATIO {
                run += 1;
                if run >= 8 {
                    recovery_start = (n as isize) - 7;
                    break;
                }
            } else {
                run = 0;
            }
        }
        if recovery_start > edge_idx as isize {
            let mut rs = recovery_start as usize;
            // Walk back through nearly-qualifying sections
            let mut allowance: isize = 1;
            for r in (edge_idx + 1..rs).rev() {
                if h[r].energy >= d * BD_VALIDATE_RATIO {
                    rs = r;
                    allowance = 1;
                } else {
                    if allowance <= 0 {
                        break;
                    }
                    allowance -= 1;
                }
            }
            // Find last high-energy section before current result
            let mut last_high: isize = -1;
            let search_from = p.max(result_idx.saturating_sub(64));
            for t in (search_from..=result_idx).rev() {
                if h[t].energy >= 0.5 * d {
                    last_high = t as isize;
                    break;
                }
            }
            if last_high > p as isize {
                let span = rs as isize - last_high;
                let grid = 16isize;
                let aligned = (span as f64 / grid as f64).round() as isize * grid;
                if aligned > 0 {
                    result_idx = (p + BD_GRID_STEP).max((rs as isize - aligned) as usize);
                }
            }
        }
    }

    // Final validation: check that breakdown-to-recovery span is long enough
    let mut recovery_start_2: isize = -1;
    let mut run = 0usize;
    for t in (edge_idx + 1)..h.len() {
        if h[t].energy >= d * BD_VALIDATE_RATIO {
            run += 1;
            if run >= 8 {
                recovery_start_2 = (t as isize) - 8 + 1;
                break;
            }
        } else {
            run = 0;
        }
    }
    if recovery_start_2 != -1 {
        let span_beats = 4 * (recovery_start_2 as usize - result_idx);
        let rounded = 16 * ((span_beats + 15) / 16);
        if rounded < min_cue_beats as usize {
            return None;
        }
    }

    // Clamp result_idx
    let result_idx = result_idx.min(h.len() - 1);
    let sec_dur = h.first().map(|s| s.duration).unwrap_or(240.0 / bpm);

    Some(StructuralCue {
        time_seconds: h[result_idx].start_time,
        name: "Breakdown".into(),
        color: "Blue".into(),
    })
}

/// Snap a time to the nearest section on a 4-section grid from `base_idx`.
/// Matches JS `qt(t, e, n)`.
fn snap_to_grid(time: f64, sections: &[Section], base_idx: usize) -> f64 {
    if sections.is_empty() {
        return time;
    }
    let start = base_idx.min(sections.len() - 1);
    let mut best = &sections[start];
    let mut best_dist = (best.start_time - time).abs();
    let mut i = start;
    while i < sections.len() {
        if (i - start) % SD_GRID != 0 {
            i += 1;
            continue;
        }
        let dist = (sections[i].start_time - time).abs();
        if dist < best_dist {
            best_dist = dist;
            best = &sections[i];
        }
        i += 1;
    }
    best.start_time
}

/// Heuristic fallback time for second drop when detection fails.
/// Matches JS `Ot(t, e, n, r, a, o)`.
fn heuristic_fallback_time(
    sections: &[Section],
    base_idx: usize,
    drop_time: f64,
    breakdown_time: Option<f64>,
    total_duration: f64,
    beat_period: f64,
) -> f64 {
    if let Some(bd_time) = breakdown_time {
        if bd_time > drop_time {
            let candidate = bd_time + SD_BD_OFFSET as f64 * beat_period;
            if candidate < total_duration {
                return snap_to_grid(candidate, sections, base_idx);
            }
        }
    }
    let candidate = drop_time + SD_DROP_OFFSET as f64 * beat_period;
    let time = if candidate < total_duration {
        candidate
    } else {
        total_duration * SD_FALLBACK_POS
    };
    snap_to_grid(time, sections, base_idx)
}

/// Refine a second-drop position through a chain of heuristic passes.
/// Matches JS `zt(t, e, n, r)`.
fn refine_second_drop(sections: &[Section], candidate_idx: usize, drop_idx: usize, peak_energy: f64) -> usize {
    let len = sections.len();

    // Pass 1: backward edge scan
    let mut a = {
        let e = candidate_idx;
        let n = drop_idx;
        if e <= n + 1 {
            e
        } else {
            let r = sections[e].energy;
            if r <= 0.0 {
                e
            } else {
                let max_back = 16;
                let mut o = e;
                let mut result = e;
                loop {
                    if o <= n + 1 || e - o >= max_back {
                        break;
                    }
                    let prev = sections[o - 1].energy;
                    if prev < 0.85 * r {
                        if prev < 0.6 * r {
                            result = o;
                            break;
                        }
                        if o >= 2
                            && sections[o - 2].energy >= 0.85 * r
                            && sections[o - 2].energy >= 1.2 * prev
                        {
                            o -= 2;
                            continue;
                        }
                        result = e;
                        break;
                    }
                    o -= 1;
                }
                result
            }
        }
    };

    // Pass 2: refine_drop_start
    a = refine_drop_start(sections, a, 16);

    // Pass 3: forward skip to sustained energy
    a = {
        let e = a;
        if e + 8 >= len {
            e
        } else {
            let n = sections[e].energy;
            if n <= 0.0 {
                e
            } else {
                let mut avg4 = 0.0;
                for i in e..(e + 4) {
                    avg4 += sections[i].energy;
                }
                avg4 /= 4.0;
                if avg4 >= 0.75 * n {
                    e
                } else {
                    let ahead_start = e + 8;
                    let ahead_end = len.min(e + 24);
                    if ahead_end - ahead_start < 8 {
                        e
                    } else {
                        let mut ahead_sum = 0.0;
                        for i in ahead_start..ahead_end {
                            ahead_sum += sections[i].energy;
                        }
                        let ahead_avg = ahead_sum / (ahead_end - ahead_start) as f64;
                        if ahead_avg <= 0.0 || ahead_avg < 1.3 * n {
                            e
                        } else {
                            let search_end = len.saturating_sub(4).min(e + 16);
                            let mut found = e;
                            for j in (e + 1)..search_end {
                                let mut block = 0.0;
                                for k in j..(j + 4) {
                                    block += sections[k].energy;
                                }
                                block /= 4.0;
                                if block < 0.85 * ahead_avg {
                                    continue;
                                }
                                let all_ok = (j..(j + 4)).all(|k| sections[k].energy >= 0.7 * ahead_avg);
                                if all_ok {
                                    found = j;
                                    break;
                                }
                            }
                            found
                        }
                    }
                }
            }
        }
    };

    // Pass 4: skip to peak region
    a = {
        let e = a;
        if e + 12 >= len {
            e
        } else {
            let r = sections[e].energy;
            if r <= 0.0 || r >= 0.83 * peak_energy {
                e
            } else {
                // Early return if transition pattern
                if e >= 1 && e + 1 < len {
                    let prev = sections[e - 1].energy;
                    let next = sections[e + 1].energy;
                    if prev < 0.25 * peak_energy && r >= 0.65 * peak_energy && next >= 1.25 * r {
                        return a; // keep current and skip rest would be wrong; just don't modify
                    }
                }
                let scan_end = len.min(e + 17);
                let mut best_block = 0.0_f64;
                for j in (e + 1)..scan_end {
                    if j + 4 > scan_end {
                        break;
                    }
                    let mut block = 0.0;
                    for k in j..(j + 4) {
                        block += sections[k].energy;
                    }
                    block /= 4.0;
                    if block > best_block {
                        best_block = block;
                    }
                }
                if best_block <= 0.0 || r >= 0.8 * best_block {
                    e
                } else {
                    // Check monotonic increase
                    let check_end = len.min(e + 9);
                    let mut prev_val = r;
                    let mut monotonic = true;
                    for j in (e + 1)..check_end {
                        let v = sections[j].energy;
                        if v < 0.65 * prev_val {
                            monotonic = false;
                            break;
                        }
                        prev_val = v;
                    }
                    if !monotonic {
                        e
                    } else {
                        let mut found = e;
                        for j in (e + 1)..scan_end {
                            if j + 4 > scan_end {
                                break;
                            }
                            if sections[j].energy >= 0.98 * best_block {
                                found = j;
                                break;
                            }
                        }
                        found
                    }
                }
            }
        }
    };

    // Pass 5: skip past brief dip to sustained high
    a = {
        let e = a;
        if e + 24 >= len {
            e
        } else {
            let r = sections[e].energy;
            if r < 0.85 * peak_energy {
                e
            } else {
                let mut consec_high = 0usize;
                for i in e..len.min(e + 8) {
                    if sections[i].energy >= 0.85 * peak_energy {
                        consec_high += 1;
                    } else {
                        break;
                    }
                }
                if consec_high >= 8 {
                    e
                } else {
                    let mut total_high = 0usize;
                    for i in e..len.min(e + 16) {
                        if sections[i].energy >= 0.85 * peak_energy {
                            total_high += 1;
                        }
                    }
                    if total_high >= 13 {
                        e
                    } else {
                        let scan_end = len.saturating_sub(16).min(e + 20);
                        let mut found = e;
                        for r_idx in (e + 4)..=scan_end {
                            if r_idx + 16 > len {
                                break;
                            }
                            let mut all_high = true;
                            let mut block_sum = 0.0;
                            for k in r_idx..(r_idx + 16) {
                                if sections[k].energy < 0.85 * peak_energy {
                                    all_high = false;
                                    break;
                                }
                                block_sum += sections[k].energy;
                            }
                            if !all_high || block_sum / 16.0 < 0.95 * peak_energy {
                                continue;
                            }
                            let has_dip = (e + 1..r_idx).any(|k| sections[k].energy < 0.3 * peak_energy);
                            found = if has_dip { r_idx } else { e };
                            break;
                        }
                        found
                    }
                }
            }
        }
    };

    // Pass 6: small jump 3-5 for step-up
    a = {
        let e = a;
        if e + 12 >= len {
            e
        } else {
            let n = sections[e].energy;
            if n <= 0.0 {
                e
            } else {
                let mut found = e;
                for r in 3..=5 {
                    let target = e + r;
                    if target + 8 >= len {
                        break;
                    }
                    let target_e = sections[target].energy;
                    if target_e < 1.2 * n {
                        continue;
                    }
                    let has_dip = (e + 1..target).any(|k| sections[k].energy < 0.75 * n);
                    if !has_dip {
                        continue;
                    }
                    let sustained = (target..(target + 8)).all(|k| sections[k].energy >= n);
                    if !sustained {
                        continue;
                    }
                    let all_near_target = (e..(e + r)).all(|k| sections[k].energy >= 0.9 * target_e);
                    if !all_near_target {
                        found = target;
                        break;
                    }
                }
                found
            }
        }
    };

    // Pass 7: skip past ramp-down from silent intro
    a = {
        let e = a;
        if e + 7 >= len || e < 3 {
            e
        } else {
            let r = sections[e].energy;
            if r < 0.75 * peak_energy {
                e
            } else {
                let mut avg47 = 0.0;
                for i in (e + 4)..=(e + 7) {
                    avg47 += sections[i].energy;
                }
                avg47 /= 4.0;

                let prev_silent = sections[e - 1].energy < 0.3 * peak_energy
                    && sections[e - 2].energy < 0.3 * peak_energy
                    && sections[e - 3].energy < 0.3 * peak_energy;

                if prev_silent && avg47 >= 0.85 * peak_energy {
                    let has_dip = ((e + 1)..=(e + 3)).any(|k| sections[k].energy <= 0.3 * r);
                    if has_dip {
                        return refine_remaining_passes(sections, e + 4, drop_idx, peak_energy);
                    }
                }

                if avg47 >= 1.1 * r && avg47 >= 0.95 * peak_energy {
                    let has_dip = ((e + 1)..=(e + 3)).any(|k| sections[k].energy <= 0.5 * r);
                    if has_dip {
                        return refine_remaining_passes(sections, e + 4, drop_idx, peak_energy);
                    }
                }

                // Check descending pattern
                if e + 7 < len && e + 3 < len {
                    let descending = (0..3).all(|k| sections[e + k + 1].energy <= 0.9 * sections[e + k].energy);
                    if descending {
                        let mut first4_sum = 0.0;
                        for i in e..=(e + 3) {
                            first4_sum += sections[i].energy;
                        }
                        let first4_avg = first4_sum / 4.0;
                        if first4_avg >= 0.6 * peak_energy && first4_avg <= 0.95 * peak_energy {
                            let mut second4_sum = 0.0;
                            for i in (e + 4)..=(e + 7) {
                                second4_sum += sections[i].energy;
                            }
                            let second4_avg = second4_sum / 4.0;
                            if second4_avg >= 0.95 * peak_energy && second4_avg >= 1.2 * first4_avg {
                                return refine_remaining_passes(sections, e + 4, drop_idx, peak_energy);
                            }
                        }
                    }
                }

                // Check 7-section plateau with dip
                if e + 11 < len {
                    let mut sum7 = 0.0;
                    for i in e..=(e + 6) {
                        sum7 += sections[i].energy;
                    }
                    let avg7 = sum7 / 7.0;
                    if avg7 >= 0.7 * peak_energy
                        && avg7 <= 0.95 * peak_energy
                        && sections[e + 7].energy <= 0.55 * r
                    {
                        let mut sum4 = 0.0;
                        for i in (e + 8)..=(e + 11) {
                            sum4 += sections[i].energy;
                        }
                        let avg4 = sum4 / 4.0;
                        if avg4 >= 0.95 * peak_energy && avg4 >= 1.2 * avg7 {
                            return refine_remaining_passes(sections, e + 8, drop_idx, peak_energy);
                        }
                    }
                }
                e
            }
        }
    };

    // Pass 8: transition pattern
    a = {
        let e = a;
        if e + 5 >= len || e < 2 {
            e
        } else {
            let r = sections[e].energy;
            if r < 0.7 * peak_energy {
                e
            } else {
                let prev1 = sections[e - 1].energy;
                let prev2 = sections[e - 2].energy;
                if prev1 < 0.4 * peak_energy || prev1 > 0.85 * peak_energy {
                    e
                } else if prev2 >= 0.85 * peak_energy {
                    e
                } else {
                    let next1 = sections[e + 1].energy;
                    let next2 = sections[e + 2].energy;
                    if next1 >= 0.85 * r {
                        e
                    } else if next2 < 0.95 * r || next2 < 0.85 * peak_energy {
                        e
                    } else {
                        let mut sum = 0.0;
                        for i in (e + 2)..=(e + 5) {
                            sum += sections[i].energy;
                        }
                        if sum / 4.0 < 0.85 * peak_energy {
                            e
                        } else {
                            e + 2
                        }
                    }
                }
            }
        }
    };

    // Pass 9: skip past deep gap
    a = {
        let e = a;
        if e + 16 >= len {
            e
        } else {
            let r = sections[e].energy;
            if r <= 0.0 || r >= 0.85 * peak_energy {
                e
            } else {
                let mut gap_pos: isize = -1;
                for i in (e + 4)..=(e + 16).min(len - 1) {
                    if sections[i].energy < 0.55 * r {
                        gap_pos = i as isize;
                        break;
                    }
                }
                if gap_pos == -1 {
                    e
                } else {
                    let mut found = e;
                    let scan_end = (e + 24).min(len);
                    for o in ((gap_pos as usize + 1)..scan_end) {
                        if o + 4 >= len {
                            break;
                        }
                        if (o - e) % SD_GRID != 0 {
                            continue;
                        }
                        let mut block = 0.0;
                        for k in o..(o + 4) {
                            block += sections[k].energy;
                        }
                        let avg = block / 4.0;
                        if avg >= 1.3 * r && avg >= 0.95 * peak_energy {
                            found = o;
                            break;
                        }
                    }
                    found
                }
            }
        }
    };

    // Pass 10: skip past tiny spike
    a = {
        let e = a;
        if e + 2 >= len {
            e
        } else {
            let r = sections[e].energy;
            if r <= 0.0 || r >= 0.3 * peak_energy {
                e
            } else {
                let next = sections[e + 1].energy;
                if next >= 3.0 * r && next >= 0.95 * peak_energy {
                    e + 1
                } else {
                    let next2 = sections[e + 2].energy;
                    if next2 >= 3.0 * r && next2 >= 0.95 * peak_energy {
                        e + 2
                    } else {
                        e
                    }
                }
            }
        }
    };

    // Pass 11: grid alignment fix for offset 2
    a = {
        let e = a;
        if e + 5 >= len || e < 1 {
            e
        } else {
            let offset = (e as isize - drop_idx as isize) as isize;
            let phase = ((offset % SD_GRID as isize) + SD_GRID as isize) % SD_GRID as isize;
            if phase != 2 {
                e
            } else {
                let o = sections[e].energy;
                if o < 0.85 * peak_energy {
                    e
                } else {
                    let next1 = sections[e + 1].energy;
                    let next2 = sections[e + 2].energy;
                    if next1 >= 0.92 * o {
                        e
                    } else {
                        let sharp_dip = next1 < 0.4 * o && next2 >= 0.85 * peak_energy;
                        if next2 >= 1.05 * o || sharp_dip {
                            let mut sum = 0.0;
                            for i in (e + 2)..=(e + 5) {
                                sum += sections[i].energy;
                            }
                            if sum / 4.0 < 0.9 * peak_energy {
                                e
                            } else {
                                e + 2
                            }
                        } else {
                            e
                        }
                    }
                }
            }
        }
    };

    // Pass 12: plateau with dip
    a = {
        let e = a;
        if e + 11 >= len {
            e
        } else {
            let r = sections[e].energy;
            if r < 0.7 * peak_energy {
                e
            } else {
                let mut min15 = f64::INFINITY;
                for i in (e + 1)..=(e + 5) {
                    if sections[i].energy < min15 {
                        min15 = sections[i].energy;
                    }
                }
                let stable = min15 >= 0.8 * r;
                if min15 < 0.7 * r {
                    e
                } else {
                    let dip = sections[e + 6].energy.min(sections[e + 7].energy);
                    let threshold = if stable { 0.45 * r } else { 0.25 * r };
                    if dip > threshold {
                        e
                    } else {
                        let recovery = sections[e + 8].energy;
                        if recovery < 1.08 * r {
                            e
                        } else {
                            let mut sum = 0.0;
                            for i in (e + 8)..=(e + 11) {
                                sum += sections[i].energy;
                            }
                            if sum / 4.0 < 0.92 * r {
                                e
                            } else {
                                e + 8
                            }
                        }
                    }
                }
            }
        }
    };

    // Pass 13: pre-dip pattern
    a = {
        let e = a;
        if e + 7 >= len || e < 4 {
            e
        } else {
            let r = sections[e].energy;
            if r < 0.55 * peak_energy {
                e
            } else {
                let mut min_before = f64::INFINITY;
                for i in (e - 4)..e {
                    if sections[i].energy < min_before {
                        min_before = sections[i].energy;
                    }
                }
                if min_before > 0.55 * r {
                    e
                } else {
                    let max_next = sections[e + 1].energy.max(sections[e + 2].energy);
                    if max_next < 1.3 * r {
                        e
                    } else {
                        let e3 = sections[e + 3].energy;
                        if e3 > 0.85 * max_next || e3 < 0.4 * r {
                            e
                        } else {
                            let mut mx = 0.0_f64;
                            let mut mn = f64::INFINITY;
                            let mut sum = 0.0;
                            for i in (e + 4)..=(e + 7) {
                                let v = sections[i].energy;
                                if v > mx { mx = v; }
                                if v < mn { mn = v; }
                                sum += v;
                            }
                            if mn <= 0.0 || mx > 1.2 * mn {
                                e
                            } else {
                                let avg = sum / 4.0;
                                if avg < 0.95 * r || avg < 0.8 * peak_energy {
                                    e
                                } else {
                                    e + 4
                                }
                            }
                        }
                    }
                }
            }
        }
    };

    // Pass 14: descending-then-ascending
    a = {
        let e = a;
        if e + 7 >= len || e < 1 {
            e
        } else {
            let r = sections[e].energy;
            if r <= 0.0 || r < 0.85 * peak_energy {
                e
            } else {
                let prev = sections[e - 1].energy;
                if prev > 0.2 * r {
                    e
                } else {
                    let e1 = sections[e + 1].energy;
                    let e2 = sections[e + 2].energy;
                    let e3 = sections[e + 3].energy;
                    if !(e1 <= r && e2 <= e1 && e3 <= e2 && e3 <= 0.8 * r) {
                        e
                    } else {
                        let mut mx = 0.0_f64;
                        let mut mn = f64::INFINITY;
                        let mut sum = 0.0;
                        for i in (e + 4)..=(e + 7) {
                            let v = sections[i].energy;
                            if v > mx { mx = v; }
                            if v < mn { mn = v; }
                            sum += v;
                        }
                        if mn <= 0.0 || mx > 1.2 * mn {
                            e
                        } else {
                            let avg = sum / 4.0;
                            if avg > 0.9 * r || avg < 0.55 * r {
                                e
                            } else {
                                e + 4
                            }
                        }
                    }
                }
            }
        }
    };

    a
}

/// Helper: apply remaining refinement passes starting from pass 8.
/// Used when pass 7 decides to jump early (e+4 or e+8).
fn refine_remaining_passes(sections: &[Section], idx: usize, drop_idx: usize, peak_energy: f64) -> usize {
    let len = sections.len();
    // We need to run passes 8-14 on the new index.
    // Since these passes are the same as in refine_second_drop passes 8-14,
    // we duplicate the chain here for correctness.
    let mut a = idx;

    // Pass 8 (transition)
    a = {
        let e = a;
        if e + 5 >= len || e < 2 { e } else {
            let r = sections[e].energy;
            if r < 0.7 * peak_energy { e } else {
                let prev1 = sections[e - 1].energy;
                let prev2 = sections[e - 2].energy;
                if prev1 < 0.4 * peak_energy || prev1 > 0.85 * peak_energy { e }
                else if prev2 >= 0.85 * peak_energy { e }
                else {
                    let next1 = sections[e + 1].energy;
                    let next2 = sections[e + 2].energy;
                    if next1 >= 0.85 * r { e }
                    else if next2 < 0.95 * r || next2 < 0.85 * peak_energy { e }
                    else {
                        let mut sum = 0.0;
                        for i in (e + 2)..=(e + 5) { sum += sections[i].energy; }
                        if sum / 4.0 < 0.85 * peak_energy { e } else { e + 2 }
                    }
                }
            }
        }
    };

    // Pass 9 (deep gap)
    a = {
        let e = a;
        if e + 16 >= len { e } else {
            let r = sections[e].energy;
            if r <= 0.0 || r >= 0.85 * peak_energy { e } else {
                let mut gap: isize = -1;
                for i in (e + 4)..=(e + 16).min(len - 1) {
                    if sections[i].energy < 0.55 * r { gap = i as isize; break; }
                }
                if gap == -1 { e } else {
                    let mut found = e;
                    for o in ((gap as usize + 1)..=(e + 24).min(len - 1)) {
                        if o + 4 >= len { break; }
                        if (o - e) % SD_GRID != 0 { continue; }
                        let mut b = 0.0;
                        for k in o..(o + 4) { b += sections[k].energy; }
                        if b / 4.0 >= 1.3 * r && b / 4.0 >= 0.95 * peak_energy { found = o; break; }
                    }
                    found
                }
            }
        }
    };

    // Pass 10 (tiny spike)
    a = {
        let e = a;
        if e + 2 >= len { e } else {
            let r = sections[e].energy;
            if r <= 0.0 || r >= 0.3 * peak_energy { e } else {
                let n = sections[e + 1].energy;
                if n >= 3.0 * r && n >= 0.95 * peak_energy { e + 1 }
                else {
                    let o = sections[e + 2].energy;
                    if o >= 3.0 * r && o >= 0.95 * peak_energy { e + 2 } else { e }
                }
            }
        }
    };

    // Pass 11 (grid alignment)
    a = {
        let e = a;
        if e + 5 >= len || e < 1 { e } else {
            let offset = (e as isize - drop_idx as isize) as isize;
            let phase = ((offset % SD_GRID as isize) + SD_GRID as isize) % SD_GRID as isize;
            if phase != 2 { e } else {
                let o = sections[e].energy;
                if o < 0.85 * peak_energy { e } else {
                    let n1 = sections[e + 1].energy;
                    let n2 = sections[e + 2].energy;
                    if n1 >= 0.92 * o { e } else {
                        let sharp = n1 < 0.4 * o && n2 >= 0.85 * peak_energy;
                        if n2 >= 1.05 * o || sharp {
                            let mut s = 0.0;
                            for i in (e + 2)..=(e + 5) { s += sections[i].energy; }
                            if s / 4.0 < 0.9 * peak_energy { e } else { e + 2 }
                        } else { e }
                    }
                }
            }
        }
    };

    // Pass 12 (plateau with dip)
    a = {
        let e = a;
        if e + 11 >= len { e } else {
            let r = sections[e].energy;
            if r < 0.7 * peak_energy { e } else {
                let mut mn = f64::INFINITY;
                for i in (e + 1)..=(e + 5) { if sections[i].energy < mn { mn = sections[i].energy; } }
                let stable = mn >= 0.8 * r;
                if mn < 0.7 * r { e } else {
                    let dip = sections[e + 6].energy.min(sections[e + 7].energy);
                    if dip > (if stable { 0.45 * r } else { 0.25 * r }) { e } else {
                        let rec = sections[e + 8].energy;
                        if rec < 1.08 * r { e } else {
                            let mut s = 0.0;
                            for i in (e + 8)..=(e + 11) { s += sections[i].energy; }
                            if s / 4.0 < 0.92 * r { e } else { e + 8 }
                        }
                    }
                }
            }
        }
    };

    // Pass 13 (pre-dip)
    a = {
        let e = a;
        if e + 7 >= len || e < 4 { e } else {
            let r = sections[e].energy;
            if r < 0.55 * peak_energy { e } else {
                let mut mn = f64::INFINITY;
                for i in (e - 4)..e { if sections[i].energy < mn { mn = sections[i].energy; } }
                if mn > 0.55 * r { e } else {
                    let mx = sections[e + 1].energy.max(sections[e + 2].energy);
                    if mx < 1.3 * r { e } else {
                        let e3 = sections[e + 3].energy;
                        if e3 > 0.85 * mx || e3 < 0.4 * r { e } else {
                            let mut max_v = 0.0_f64; let mut min_v = f64::INFINITY; let mut sum = 0.0;
                            for i in (e + 4)..=(e + 7) {
                                let v = sections[i].energy;
                                if v > max_v { max_v = v; } if v < min_v { min_v = v; } sum += v;
                            }
                            if min_v <= 0.0 || max_v > 1.2 * min_v { e }
                            else { let avg = sum / 4.0; if avg < 0.95 * r || avg < 0.8 * peak_energy { e } else { e + 4 } }
                        }
                    }
                }
            }
        }
    };

    // Pass 14 (descending-ascending)
    a = {
        let e = a;
        if e + 7 >= len || e < 1 { e } else {
            let r = sections[e].energy;
            if r <= 0.0 || r < 0.85 * peak_energy { e } else {
                let prev = sections[e - 1].energy;
                if prev > 0.2 * r { e } else {
                    let e1 = sections[e + 1].energy;
                    let e2 = sections[e + 2].energy;
                    let e3 = sections[e + 3].energy;
                    if !(e1 <= r && e2 <= e1 && e3 <= e2 && e3 <= 0.8 * r) { e } else {
                        let mut mx = 0.0_f64; let mut mn = f64::INFINITY; let mut sum = 0.0;
                        for i in (e + 4)..=(e + 7) {
                            let v = sections[i].energy;
                            if v > mx { mx = v; } if v < mn { mn = v; } sum += v;
                        }
                        if mn <= 0.0 || mx > 1.2 * mn { e }
                        else { let avg = sum / 4.0; if avg > 0.9 * r || avg < 0.55 * r { e } else { e + 4 } }
                    }
                }
            }
        }
    };

    a
}

/// Detect the second drop after a breakdown.
/// Matches JS `Ht(t, e, n, r, a, o, i)`.
/// Returns (cue, tier) where tier is "principled", "energy-fallback", or "heuristic-fallback".
pub fn detect_second_drop(
    pcm: &[f32],
    sample_rate: u32,
    bpm: f64,
    first_beat: f64,
    drop_time: f64,
    breakdown_time: Option<f64>,
    total_duration: f64,
) -> (Option<StructuralCue>, &'static str) {
    let lp = low_pass_iir(pcm, sample_rate, 300.0, 6);
    let s = build_sections(&lp, sample_rate, bpm, first_beat, 4, 0.02);
    let beat_period = 60.0 / bpm;

    if s.is_empty() {
        let time = heuristic_fallback_time(&[], 0, drop_time, breakdown_time, total_duration, beat_period);
        return (Some(StructuralCue {
            time_seconds: time,
            name: "Second Drop".into(),
            color: "Red".into(),
        }), "heuristic-fallback");
    }

    let l = nearest_section(&s, drop_time);
    let u = avg_energy(&s, l, 8);
    let f = SD_MIN_AFTER_BD / 4;  // Gt/4 = 24
    let h = SD_MIN_AFTER_DROP / 4; // Bt/4 = 16
    let p_val = SD_FALLBACK_DIST / 4; // Nt/4 = 48
    let d_idx = match breakdown_time {
        Some(bt) if bt > drop_time => nearest_section(&s, bt),
        _ => l + p_val,
    };
    let m = l + f;
    let b = d_idx + h;
    let mut g = m.max(b);
    let y = g - l;
    g = l + ((y + SD_GRID - 1) / SD_GRID) * SD_GRID; // ceil division

    let ref_time = match breakdown_time {
        Some(bt) if bt > drop_time => bt,
        _ => drop_time + SD_FALLBACK_DIST as f64 * beat_period,
    };

    struct SDCandidate {
        section_index: usize,
        score: f64,
        cand_energy: f64,
        passes: bool,
    }
    let mut candidates: Vec<SDCandidate> = Vec::new();
    let max_scan = SD_MAX_SCAN / 4; // 80

    let mut t = g;
    while t + SD_ENERGY_WINDOW < s.len() {
        if t - d_idx > max_scan {
            break;
        }
        if s[t].start_time / total_duration > SD_MAX_POS {
            break;
        }

        let beats_from_ref = (s[t].start_time - ref_time) / beat_period;
        let mut energy_avg = 0.0;
        for i in t..(t + SD_ENERGY_WINDOW) {
            energy_avg += s[i].energy;
        }
        energy_avg /= SD_ENERGY_WINDOW as f64;

        let before_start = t.saturating_sub(SD_BEFORE_WINDOW);
        let mut before_sum = 0.0;
        let mut before_count = 0u32;
        for i in before_start..t {
            before_sum += s[i].energy;
            before_count += 1;
        }
        let before_avg = if before_count > 0 {
            before_sum / before_count as f64
        } else {
            0.0
        };

        let energy_pass = energy_avg >= u * SD_ENERGY_RATIO;
        let full_pass = energy_pass && before_avg <= u * SD_BEFORE_RATIO;
        let prev_e = if t >= 1 { s[t - 1].energy } else { 0.0 };
        let curr_e = s[t].energy;
        let passes = full_pass || (energy_pass && curr_e >= 0.85 * u && prev_e < 0.4 * u);

        let energy_frac = (energy_avg / u).min(1.0);
        let energy_contrast = ((energy_avg - before_avg) / u).max(0.0);

        // Position Gaussian
        let pos_z = (s[t].start_time / total_duration - SD_POS_CENTER) / SD_POS_SIGMA;
        let mut pos_gauss = (-0.5 * pos_z * pos_z).exp();

        // Distance Gaussian
        let dist_z = (beats_from_ref - SD_DIST_CENTER as f64) / SD_DIST_SIGMA as f64;
        let dist_gauss = (-0.5 * dist_z * dist_z).exp();

        // Suppress position Gaussian for early drops
        let drop_frac = s[l].start_time / total_duration;
        let ref_frac = ref_time / total_duration;
        if (drop_frac < 0.15 && ref_frac < 0.35)
            || (ref_time + SD_DIST_CENTER as f64 * beat_period) / total_duration < 0.55
        {
            pos_gauss = 0.0;
        }

        let score = energy_frac * energy_contrast * pos_gauss.max(dist_gauss);

        candidates.push(SDCandidate {
            section_index: t,
            score,
            cand_energy: energy_avg,
            passes,
        });

        t += SD_GRID;
    }

    // Tier 1: principled (passes energy checks)
    let passing: Vec<&SDCandidate> = candidates.iter().filter(|c| c.passes).collect();
    if !passing.is_empty() {
        let best = passing.iter().max_by(|a, b| a.score.total_cmp(&b.score)).unwrap();
        let refined = refine_second_drop(&s, best.section_index, l, u);
        let refined = refined.min(s.len() - 1);
        return (Some(StructuralCue {
            time_seconds: s[refined].start_time,
            name: "Second Drop".into(),
            color: "Red".into(),
        }), "principled");
    }

    // No breakdown → no second drop
    if breakdown_time.is_none() {
        return (None, "no-result");
    }

    // Tier 2: energy fallback
    if !candidates.is_empty() {
        let max_energy = candidates
            .iter()
            .map(|c| c.cand_energy)
            .fold(f64::NEG_INFINITY, f64::max);
        let threshold = 0.95 * max_energy;
        let first_near_max = candidates
            .iter()
            .find(|c| c.cand_energy >= threshold)
            .unwrap_or(&candidates[0]);
        let refined = refine_second_drop(&s, first_near_max.section_index, l, u);
        let refined = refined.min(s.len() - 1);
        return (Some(StructuralCue {
            time_seconds: s[refined].start_time,
            name: "Second Drop".into(),
            color: "Red".into(),
        }), "energy-fallback");
    }

    // Tier 3: heuristic fallback
    let time = heuristic_fallback_time(&s, l, drop_time, breakdown_time, total_duration, beat_period);
    (Some(StructuralCue {
        time_seconds: time,
        name: "Second Drop".into(),
        color: "Red".into(),
    }), "heuristic-fallback")
}

// --- Lastbeat detection ---

const LB_HALF_WINDOW: f64 = 0.05;
const LB_RAW_PERCENTILE: f64 = 0.75;
const LB_RAW_THRESHOLD_RATIO: f64 = 0.55;
const LB_LP_THRESHOLD_RATIO: f64 = 0.3;
const LB_GRID_SIZE: usize = 16;
const LB_QUIET_THRESHOLD: f64 = 0.15;
const LB_LOUD_THRESHOLD: f64 = 0.35;
const LB_SUSTAIN_THRESHOLD: f64 = 0.45;
const LB_SEARCH_AHEAD: usize = 64;
const LB_QUIET_WINDOW: usize = 6;
const LB_MIN_RUN: usize = 16;
const LB_SMOOTH_WINDOW: usize = 4;

pub fn detect_lastbeat(
    pcm: &[f32],
    sample_rate: u32,
    bpm: f64,
    first_beat: f64,
    total_duration: f64,
) -> Option<StructuralCue> {
    let beat_period = 60.0 / bpm;
    let num_beats = ((total_duration - first_beat) / beat_period).floor().max(0.0) as usize;
    if num_beats < 4 {
        return None;
    }

    let lp = low_pass_iir(pcm, sample_rate, 300.0, 6);

    let mut raw_rms = Vec::with_capacity(num_beats);
    let mut lp_rms = Vec::with_capacity(num_beats);
    for i in 0..num_beats {
        let t = first_beat + i as f64 * beat_period;
        raw_rms.push(rms_at_time(pcm, sample_rate, t, LB_HALF_WINDOW));
        lp_rms.push(rms_at_time(&lp, sample_rate, t, LB_HALF_WINDOW));
    }

    let p_raw = percentile(&raw_rms, LB_RAW_PERCENTILE);
    let p_lp = percentile(&lp_rms, LB_RAW_PERCENTILE);
    if p_raw <= 0.0 {
        return None;
    }

    let mut smooth_raw = Vec::with_capacity(num_beats);
    let mut smooth_lp = Vec::with_capacity(num_beats);
    for i in 0..num_beats {
        let mut mr = 0.0_f64;
        let mut ml = 0.0_f64;
        let start = i.saturating_sub(LB_SMOOTH_WINDOW - 1);
        for j in start..=i {
            if raw_rms[j] > mr {
                mr = raw_rms[j];
            }
            if lp_rms[j] > ml {
                ml = lp_rms[j];
            }
        }
        smooth_raw.push(mr);
        smooth_lp.push(ml);
    }

    let raw_thresh = p_raw * LB_RAW_THRESHOLD_RATIO;
    let lp_thresh = p_lp * LB_LP_THRESHOLD_RATIO;
    let quiet_level = p_raw * LB_QUIET_THRESHOLD;
    let loud_level = p_raw * LB_LOUD_THRESHOLD;

    let is_active = |i: usize| smooth_raw[i] >= raw_thresh && smooth_lp[i] >= lp_thresh;

    let mut last_active: isize = -1;
    let mut run_len = 0usize;
    let mut run_end: isize = -1;
    for i in 0..num_beats {
        if is_active(i) {
            run_len += 1;
            run_end = i as isize;
        } else {
            if run_len >= LB_MIN_RUN {
                last_active = run_end;
            }
            run_len = 0;
        }
    }
    if run_len >= LB_MIN_RUN {
        last_active = run_end;
    }

    if last_active < 0 {
        for i in (0..num_beats).rev() {
            if smooth_raw[i] >= raw_thresh {
                last_active = i as isize;
                break;
            }
        }
    }

    if last_active < 0 {
        return None;
    }

    let base = last_active as usize;
    let mut candidate = base;

    let search_end = (num_beats - 1).min(base + LB_SEARCH_AHEAD);
    for t in ((base + 1)..=search_end).rev() {
        if raw_rms[t] < loud_level {
            continue;
        }
        let mut has_drop = false;
        let qend = (num_beats - 1).min(t + LB_QUIET_WINDOW);
        for j in (t + 1)..=qend {
            if raw_rms[j] < quiet_level {
                has_drop = true;
                break;
            }
        }
        if !has_drop {
            continue;
        }
        let mut resurgence = false;
        for j in (t + LB_QUIET_WINDOW + 1)..num_beats {
            if raw_rms[j] >= loud_level {
                resurgence = true;
                break;
            }
        }
        if resurgence {
            continue;
        }
        let mut sum = 0.0;
        let mut cnt = 0u32;
        for j in (base + 1)..=t {
            sum += raw_rms[j];
            cnt += 1;
        }
        if cnt > 0 && sum / cnt as f64 >= p_raw * LB_SUSTAIN_THRESHOLD {
            candidate = t;
            break;
        }
    }

    let mut grid = ((candidate as f64 / LB_GRID_SIZE as f64).round() as usize) * LB_GRID_SIZE;
    if grid >= num_beats {
        grid = (candidate / LB_GRID_SIZE) * LB_GRID_SIZE;
    }

    let time = first_beat + grid as f64 * beat_period;
    Some(StructuralCue {
        time_seconds: time,
        name: "Last beat".into(),
        color: "Orange".into(),
    })
}

// --- Second breakdown fallback ---

fn detect_second_breakdown_fallback(
    pcm: &[f32],
    sample_rate: u32,
    bpm: f64,
    first_beat: f64,
    second_drop_time: f64,
) -> Option<StructuralCue> {
    let lp = low_pass_iir(pcm, sample_rate, 300.0, 6);
    let sections = build_sections(&lp, sample_rate, bpm, first_beat, 4, 0.02);
    if sections.is_empty() {
        return None;
    }
    let drop_idx = nearest_section(&sections, second_drop_time);
    let ref_energy = avg_energy(&sections, drop_idx, 8);
    if ref_energy <= 0.0 {
        return None;
    }

    let avg_range = |start: usize, len: usize| -> f64 {
        let end = sections.len().min(start + len);
        let mut s = 0.0;
        let mut c = 0u32;
        for i in start..end {
            s += sections[i].energy;
            c += 1;
        }
        if c > 0 { s / c as f64 } else { 0.0 }
    };

    let step = 16usize;
    for t in (drop_idx + step..sections.len()).step_by(step) {
        if t + 8 >= sections.len() {
            break;
        }
        let e = avg_range(t, 8);
        if e > 0.7 * ref_energy {
            continue;
        }
        if avg_range(t.saturating_sub(8), 8) - e < 0.25 * ref_energy {
            continue;
        }
        let mut recovers = false;
        for j in t..sections.len() {
            if sections[j].energy > 0.95 * ref_energy {
                recovers = true;
                break;
            }
        }
        if recovers {
            continue;
        }
        let mut edge = t;
        let min_edge = (drop_idx + 1).max(t.saturating_sub(step));
        while edge > min_edge && sections[edge - 1].energy <= 0.7 * ref_energy {
            edge -= 1;
        }
        return Some(StructuralCue {
            time_seconds: sections[edge].start_time,
            name: "Second Breakdown".into(),
            color: "Blue".into(),
        });
    }
    None
}

// --- Energy profile check ---

fn has_sustained_energy_profile(sections: &[Section]) -> bool {
    if sections.len() < 8 {
        return false;
    }
    let energies: Vec<f64> = sections.iter().map(|s| s.energy).collect();
    let mut sorted = energies.clone();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let p95 = sorted[(0.95 * (sorted.len() - 1) as f64).floor() as usize].max(1e-9);

    let mut start = 0;
    let mut end = energies.len() - 1;
    while start < end && energies[start] < 0.05 * p95 {
        start += 1;
    }
    while end > start && energies[end] < 0.05 * p95 {
        end -= 1;
    }
    let trimmed = &energies[start..=end];
    if trimmed.len() < 8 {
        return false;
    }

    let loud_frac =
        trimmed.iter().filter(|&&e| e >= 0.5 * p95).count() as f64 / trimmed.len() as f64;
    let mut max_quiet_run = 0usize;
    let mut quiet_run = 0usize;
    let mut total_quiet = 0usize;
    for &e in trimmed {
        if e < 0.2 * p95 {
            quiet_run += 1;
            total_quiet += 1;
            if quiet_run > max_quiet_run {
                max_quiet_run = quiet_run;
            }
        } else {
            quiet_run = 0;
        }
    }
    let quiet_frac = total_quiet as f64 / trimmed.len() as f64;

    loud_frac >= 0.55 && max_quiet_run <= 4 && quiet_frac >= 0.075
}

// --- Emergency loop detection ---

fn detect_emergency_loop(
    sections: &[Section],
    loop_start_time: f64,
    loop_end_time: f64,
) -> Option<EmergencyLoop> {
    if sections.is_empty() {
        return None;
    }

    let start_idx = sections.iter().position(|s| s.start_time >= loop_start_time)?;
    let end_idx = sections
        .iter()
        .position(|s| s.start_time >= loop_end_time)
        .unwrap_or(sections.len());
    if start_idx >= end_idx {
        return None;
    }

    let scan_end = end_idx.saturating_sub(8);
    let scan_start = start_idx + 4;
    if scan_end < scan_start {
        return None;
    }

    let mut peak = 0.0_f64;
    for i in scan_start..end_idx {
        if sections[i].energy > peak {
            peak = sections[i].energy;
        }
    }
    if peak <= 0.0 {
        return None;
    }

    let block_score = |idx: usize| -> Option<(bool, f64)> {
        if idx + 4 > sections.len() {
            return None;
        }
        let mut sum = 0.0_f64;
        let mut mn = f64::INFINITY;
        let mut mx = f64::NEG_INFINITY;
        for j in 0..4 {
            let e = sections[idx + j].energy;
            sum += e;
            if e < mn { mn = e; }
            if e > mx { mx = e; }
        }
        let avg = sum / 4.0;
        if avg <= 0.0 {
            return Some((false, f64::NEG_INFINITY));
        }
        let ratio = avg / peak;
        let variation = (mx - mn) / avg;
        if ratio < 0.45 || variation > 0.7 {
            return Some((false, f64::NEG_INFINITY));
        }

        let mut beat_var_sum = 0.0;
        let mut beat_var_count = 0u32;
        for beat in 0..4usize {
            let mut bmin = f64::INFINITY;
            let mut bmax = f64::NEG_INFINITY;
            let mut bsum = 0.0;
            let mut cnt = 0u32;
            for sec in 0..4usize {
                let s = &sections[idx + sec];
                if beat < s.beat_energies.len() {
                    let e = s.beat_energies[beat];
                    bsum += e;
                    if e < bmin { bmin = e; }
                    if e > bmax { bmax = e; }
                    cnt += 1;
                }
            }
            if cnt > 0 {
                let bavg = bsum / cnt as f64;
                if bavg > 0.0 {
                    beat_var_sum += (bmax - bmin) / bavg;
                    beat_var_count += 1;
                }
            }
        }
        let beat_var = if beat_var_count > 0 {
            beat_var_sum / beat_var_count as f64
        } else {
            0.0
        };

        let position_frac = (idx - scan_start) as f64 / (scan_end - scan_start).max(1) as f64;
        let score = position_frac * 1.0 + 0.5 * ratio - 0.8 * variation - 0.6 * beat_var;
        Some((true, score))
    };

    let mut best_idx: Option<usize> = None;
    let mut best_score = f64::NEG_INFINITY;
    let mut fallback_idx: Option<usize> = None;

    let mut idx = scan_end;
    while idx >= scan_start {
        if fallback_idx.is_none() {
            fallback_idx = Some(idx);
        }
        if let Some((ok, score)) = block_score(idx) {
            if ok && score > best_score {
                best_score = score;
                best_idx = Some(idx);
            }
        }
        if idx < 4 {
            break;
        }
        idx -= 4;
    }

    let chosen = best_idx.or(fallback_idx)?;
    Some(EmergencyLoop {
        time_seconds: sections[chosen].start_time,
        loop_beats: 16,
    })
}

// --- Main orchestrator ---

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StructuralCueResult {
    pub cues: Vec<StructuralCue>,
    pub emergency_loop: Option<EmergencyLoop>,
}

const DROP_AT_START_EARLY_FRAC: f64 = 0.1;
const DROP_AT_START_EARLY_ABS: f64 = 12.0;
const DROP_AT_START_MID_FRAC: f64 = 0.5;

pub fn generate_structural_cues(
    pcm: &[f32],
    sample_rate: u32,
    bpm: f64,
    first_beat: f64,
    config: &LexiconConfig,
) -> StructuralCueResult {
    generate_structural_cues_with_filtered(pcm, sample_rate, bpm, first_beat, config, None)
}

// Renderer input is supplied separately; keep all cue decisions below intact.
pub(crate) fn generate_structural_cues_with_filtered(
    pcm: &[f32],
    sample_rate: u32,
    bpm: f64,
    first_beat: f64,
    config: &LexiconConfig,
    precomputed_lowpass: Option<&[f32]>,
) -> StructuralCueResult {
    let total_duration = pcm.len() as f64 / sample_rate as f64;
    let min_cue_beats = config.min_cue_length_beats;

    // Step 1: Detect drop
    let drop_result = detect_drop(pcm, sample_rate, bpm, first_beat, precomputed_lowpass, &config.genre);
    let mut start_cue = drop_result.start.clone();
    let mut drop_cue = drop_result.drop.clone();
    let mut drop_time = drop_cue.as_ref().map(|c| c.time_seconds);

    let mut breakdown_cue: Option<StructuralCue> = None;
    let mut breakdown_time: Option<f64> = None;
    let mut second_drop_cue: Option<StructuralCue> = None;
    let mut second_drop_time: Option<f64> = None;

    if let Some(dt) = drop_time {
        // Step 2: Detect breakdown
        breakdown_cue = detect_breakdown(
            pcm, sample_rate, bpm, first_beat, dt, total_duration,
            min_cue_beats, &config.genre, false,
        );
        breakdown_time = breakdown_cue.as_ref().map(|c| c.time_seconds);

        // Step 3: Detect second drop
        let sd = detect_second_drop(
            pcm, sample_rate, bpm, first_beat, dt, breakdown_time, total_duration,
        );
        second_drop_cue = sd.0;
        second_drop_time = second_drop_cue.as_ref().map(|c| c.time_seconds);
        let mut _sd_tier = sd.1;

        // Step 4: Handle dropAtStart=Never
        if config.drop_at_start == DropAtStart::Never {
            if let Some(sdt) = second_drop_time {
                if total_duration > 0.0 {
                    let mut should_shift = false;

                    if dt / total_duration < DROP_AT_START_EARLY_FRAC
                        && dt < DROP_AT_START_EARLY_ABS
                    {
                        should_shift = true;
                    } else if dt / total_duration < DROP_AT_START_MID_FRAC
                        && sdt / total_duration < DROP_AT_START_MID_FRAC
                    {
                        let test_bd = detect_breakdown(
                            pcm, sample_rate, bpm, first_beat, sdt, total_duration,
                            min_cue_beats, &config.genre, false,
                        );
                        let test_bd_time = test_bd.as_ref().map(|c| c.time_seconds);
                        let test_sd = detect_second_drop(
                            pcm, sample_rate, bpm, first_beat, sdt, test_bd_time, total_duration,
                        );
                        should_shift = test_sd.0.is_some() && test_sd.1 == "principled";
                    }

                    if should_shift {
                        let lp = low_pass_iir(pcm, sample_rate, 300.0, 6);
                        let check_sections =
                            build_sections(&lp, sample_rate, bpm, first_beat, 4, 0.02);
                        if has_sustained_energy_profile(&check_sections) {
                            let new_bd = detect_breakdown(
                                pcm, sample_rate, bpm, first_beat, dt, total_duration,
                                min_cue_beats, &config.genre, true,
                            );
                            breakdown_cue = new_bd;
                            breakdown_time = breakdown_cue.as_ref().map(|c| c.time_seconds);
                            let new_sd = detect_second_drop(
                                pcm, sample_rate, bpm, first_beat, dt, breakdown_time,
                                total_duration,
                            );
                            second_drop_cue = new_sd.0;
                            second_drop_time = second_drop_cue.as_ref().map(|c| c.time_seconds);
                            _sd_tier = new_sd.1;
                            should_shift = false;
                        }
                    }

                    if should_shift {
                        if let Some(sdt) = second_drop_time {
                            drop_cue = Some(StructuralCue {
                                time_seconds: sdt,
                                name: "Drop".into(),
                                color: "Red".into(),
                            });
                            drop_time = Some(sdt);
                            if start_cue.time_seconds > sdt {
                                start_cue.time_seconds = sdt;
                            }

                            breakdown_cue = detect_breakdown(
                                pcm, sample_rate, bpm, first_beat, sdt, total_duration,
                                min_cue_beats, &config.genre, false,
                            );
                            breakdown_time = breakdown_cue.as_ref().map(|c| c.time_seconds);
                            let new_sd = detect_second_drop(
                                pcm, sample_rate, bpm, first_beat, sdt, breakdown_time,
                                total_duration,
                            );
                            second_drop_cue = new_sd.0;
                            second_drop_time = second_drop_cue.as_ref().map(|c| c.time_seconds);
                        }
                    }
                }
            }
        }
    }

    // Step 5: Collect cues
    let mut cues = vec![start_cue];
    if let Some(ref dc) = drop_cue {
        cues.push(dc.clone());
    }
    if let Some(ref bc) = breakdown_cue {
        cues.push(bc.clone());
    }
    if let Some(ref sc) = second_drop_cue {
        cues.push(sc.clone());
    }

    // Step 6: Second breakdown
    let mut second_breakdown_cue: Option<StructuralCue> = None;
    if let Some(sdt) = second_drop_time {
        let sb = detect_breakdown(
            pcm, sample_rate, bpm, first_beat, sdt, total_duration,
            min_cue_beats, &config.genre, false,
        );
        second_breakdown_cue = sb.or_else(|| {
            detect_second_breakdown_fallback(pcm, sample_rate, bpm, first_beat, sdt)
        });
        if let Some(ref mut sbc) = second_breakdown_cue {
            sbc.name = "Second Breakdown".into();
        }
        if let Some(ref sbc) = second_breakdown_cue {
            cues.push(sbc.clone());
        }
    }

    // Step 7: Lastbeat
    if let Some(lb) = detect_lastbeat(pcm, sample_rate, bpm, first_beat, total_duration) {
        cues.push(lb);
    }

    // Step 8: Emergency loop
    let mut emergency_loop = None;
    if config.add_emergency_loop {
        if let Some(dt) = drop_time {
            let loop_start = second_drop_time.unwrap_or(dt);
            let loop_end = second_breakdown_cue
                .as_ref()
                .map(|c| c.time_seconds)
                .or_else(|| {
                    cues.iter()
                        .find(|c| c.name == "Last beat")
                        .map(|c| c.time_seconds)
                })
                .unwrap_or(total_duration);

            let lp = low_pass_iir(pcm, sample_rate, 300.0, 6);
            let loop_sections = build_sections(&lp, sample_rate, bpm, first_beat, 4, 0.02);
            emergency_loop = detect_emergency_loop(&loop_sections, loop_start, loop_end);
        }
    }

    // Sort by time
    cues.sort_by(|a, b| {
        a.time_seconds
            .partial_cmp(&b.time_seconds)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    StructuralCueResult {
        cues,
        emergency_loop,
    }
}

// Checked application entry points; source arithmetic regressions follow.
pub use crate::structural_cues::{try_generate_renderer_cues, try_generate_structural_cues};

#[cfg(test)]
#[path = "lexicon_cue_regressions.rs"]
mod source_regressions;
