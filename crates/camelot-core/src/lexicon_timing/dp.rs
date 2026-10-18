use super::onset::attack_ratio;
use serde::Serialize;
use std::{collections::VecDeque, f64::consts::PI};

const SIZE: usize = 512;
const HOP: usize = 128;
const BANDS: usize = 24;

pub(super) struct Envelope {
    pub values: Vec<f32>,
    pub hop_seconds: f64,
}
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct BeatChain {
    pub beats: Vec<f64>,
    pub first_beat: f64,
    pub chain_score: f32,
}
impl BeatChain {
    fn empty() -> Self {
        Self {
            beats: vec![],
            first_beat: 0.,
            chain_score: 0.,
        }
    }
}
#[derive(Debug, Serialize)]
pub(super) struct PhaseCorrection {
    pub position: f64,
    pub applied: bool,
}

// This FFT stores each butterfly result in Float32 but evaluates intermediates
// and rotating twiddles in Float64. A generic f32 FFT changes that boundary.
fn fft(real: &mut [f32; SIZE], imaginary: &mut [f32; SIZE]) {
    let mut reversed = 0;
    for index in 1..SIZE {
        let mut bit = SIZE >> 1;
        while reversed & bit != 0 {
            reversed ^= bit;
            bit >>= 1;
        }
        reversed ^= bit;
        if index < reversed {
            real.swap(index, reversed);
            imaginary.swap(index, reversed);
        }
    }
    let mut width = 2;
    while width <= SIZE {
        let angle = -2. * PI / width as f64;
        let (cosine, sine) = (angle.cos(), angle.sin());
        for start in (0..SIZE).step_by(width) {
            let (mut wr, mut wi) = (1., 0.);
            for offset in 0..width / 2 {
                let left = start + offset;
                let right = left + width / 2;
                let (a, b) = (real[left] as f64, imaginary[left] as f64);
                let c = real[right] as f64 * wr - imaginary[right] as f64 * wi;
                let d = real[right] as f64 * wi + imaginary[right] as f64 * wr;
                real[left] = (a + c) as f32;
                imaginary[left] = (b + d) as f32;
                real[right] = (a - c) as f32;
                imaginary[right] = (b - d) as f32;
                let next = wr * cosine - wi * sine;
                wi = wr * sine + wi * cosine;
                wr = next;
            }
        }
        width *= 2;
    }
}
pub(super) fn envelope(pcm: &[f32], rate: u32) -> Envelope {
    if !(8000..=192000).contains(&rate) {
        return Envelope {
            values: vec![],
            hop_seconds: 0.,
        };
    }
    let frames = pcm.len().saturating_sub(SIZE) / HOP;
    let hop_seconds = HOP as f64 / rate as f64;
    let mut hann = [0_f32; SIZE];
    for (index, value) in hann.iter_mut().enumerate() {
        *value = (0.5 * (1. - (2. * PI * index as f64 / (SIZE - 1) as f64).cos())) as f32;
    }
    let bin_width = rate as f64 / SIZE as f64;
    let highest = 5000_f64.min(rate as f64 / 2. - bin_width);
    let mut edges = [0_usize; BANDS + 1];
    for (index, value) in edges.iter_mut().enumerate() {
        let hz = 43. * (highest / 43.).powf(index as f64 / BANDS as f64);
        *value = ((hz / bin_width).round() as usize).clamp(1, SIZE / 2 - 1);
    }
    let mut flux = vec![0_f32; frames];
    let mut previous = [0_f32; BANDS];
    let mut bands = [0_f32; BANDS];
    let mut real = [0_f32; SIZE];
    let mut imaginary = [0_f32; SIZE];
    for (frame, value) in flux.iter_mut().enumerate() {
        for index in 0..SIZE {
            real[index] = (pcm[frame * HOP + index] as f64 * hann[index] as f64) as f32;
            imaginary[index] = 0.;
        }
        fft(&mut real, &mut imaginary);
        for band in 0..BANDS {
            let mut magnitude = 0.;
            for bin in edges[band]..(edges[band] + 1).max(edges[band + 1]) {
                let (a, b) = (real[bin] as f64, imaginary[bin] as f64);
                magnitude += (a * a + b * b).sqrt();
            }
            bands[band] = (10. * magnitude).ln_1p() as f32;
        }
        let mut difference = 0.;
        if frame > 0 {
            for band in 0..BANDS {
                let delta = bands[band] as f64 - previous[band] as f64;
                if delta > 0. {
                    difference += delta;
                }
            }
        }
        *value = difference as f32;
        previous = bands;
    }
    let length = (0.5 / hop_seconds).round() as usize;
    let mut window = VecDeque::new();
    let mut total = 0.;
    let mut values = vec![0_f32; frames];
    for (index, flux) in flux.into_iter().enumerate() {
        let flux = flux as f64;
        total += flux;
        window.push_back(flux);
        if window.len() > length {
            total -= window.pop_front().unwrap();
        }
        values[index] = (flux - total / window.len() as f64).max(0.) as f32;
    }
    Envelope {
        values,
        hop_seconds,
    }
}
pub(super) fn track(envelope: &Envelope, bpm: f64, tightness: f64) -> BeatChain {
    if !bpm.is_finite()
        || bpm <= 0.
        || !tightness.is_finite()
        || tightness < 0.
        || !envelope.hop_seconds.is_finite()
        || envelope.hop_seconds <= 0.
        || envelope.values.iter().any(|v| !v.is_finite() || *v < 0.)
    {
        return BeatChain::empty();
    }
    let period = 60. / bpm / envelope.hop_seconds;
    let length = envelope.values.len();
    if (length as f64) < 4. * period {
        return BeatChain::empty();
    }
    let sum: f64 = envelope.values.iter().map(|v| *v as f64).sum();
    // Silence has no beat evidence. The source helper produces a lone zero
    // beat; it cannot correct phase either, so keep that explicitly unknown.
    if sum <= 0. {
        return BeatChain::empty();
    }
    let mean = sum / length as f64;
    let normalized: Vec<f32> = envelope
        .values
        .iter()
        .map(|v| (*v as f64 / mean) as f32)
        .collect();
    let mut scores = vec![0_f32; length];
    let mut parents = vec![None; length];
    let shortest = (0.5 * period).round() as usize;
    let longest = (2. * period).round() as usize;
    if shortest == 0 || longest < shortest {
        return BeatChain::empty();
    }
    for index in 0..length {
        let (mut best, mut parent) = (0., None);
        if let Some(end) = index.checked_sub(shortest) {
            for (previous, previous_score) in scores
                .iter()
                .enumerate()
                .take(end + 1)
                .skip(index.saturating_sub(longest))
            {
                let lag = (index - previous) as f64;
                let error = (lag / period).ln();
                let score = *previous_score as f64 - tightness * error * error;
                if score > best {
                    best = score;
                    parent = Some(previous);
                }
            }
        }
        scores[index] = (normalized[index] as f64 + best) as f32;
        parents[index] = parent;
    }
    let mut best = 0;
    for index in 1..length {
        if scores[index] > scores[best] {
            best = index;
        }
    }
    let mut indices = vec![];
    let mut cursor = Some(best);
    while let Some(index) = cursor {
        indices.push(index);
        cursor = parents[index];
    }
    indices.reverse();
    let beats: Vec<f64> = indices
        .into_iter()
        .map(|i| i as f64 * envelope.hop_seconds)
        .collect();
    BeatChain {
        first_beat: beats[0],
        beats,
        chain_score: scores[best],
    }
}
fn grid_hits(pcm: &[f32], rate: u32, anchor: f64, period: f64) -> usize {
    let duration = pcm.len() as f64 / rate as f64;
    let mut count = 0;
    for beat in 0..8 {
        let position = anchor + beat as f64 * period;
        if position >= duration - 0.1 {
            break;
        }
        let mut best = 0.;
        for offset in (-20..=20).step_by(2) {
            let at = position + offset as f64 / 1000.;
            if at < 0. || at >= duration {
                continue;
            }
            let ratio = attack_ratio(pcm, rate, at, 15.);
            if ratio > best {
                best = ratio;
            }
        }
        if best > 2. {
            count += 1;
        }
    }
    count
}
pub(super) fn correct_phase(pcm: &[f32], rate: u32, anchor: f64, bpm: f64) -> PhaseCorrection {
    let unchanged = PhaseCorrection {
        position: anchor,
        applied: false,
    };
    if !(8000..=192000).contains(&rate)
        || !anchor.is_finite()
        || anchor < 0.1
        || !bpm.is_finite()
        || bpm <= 0.
    {
        return unchanged;
    }
    let chain = track(&envelope(pcm, rate), bpm, 300.);
    if chain.beats.len() < 32 {
        return unchanged;
    }
    let period = 60. / bpm;
    let duration = pcm.len() as f64 / rate as f64;
    let mut offsets = vec![];
    for beat in chain.beats {
        let mut delta = (beat - anchor) % period;
        if delta > period / 2. {
            delta -= period;
        }
        if delta < -period / 2. {
            delta += period;
        }
        offsets.push(delta);
    }
    let agreement =
        offsets.iter().filter(|v| v.abs() < 0.035).count() as f64 / offsets.len() as f64;
    offsets.sort_by(f64::total_cmp);
    let shift_ms = 1000. * offsets[offsets.len() / 2] + 30.;
    if agreement >= 0.3 || shift_ms.abs() < 40. {
        return unchanged;
    }
    let old_hits = grid_hits(pcm, rate, anchor, period);
    if old_hits > 2 {
        return unchanged;
    }
    let mut candidate = anchor + shift_ms / 1000.;
    if candidate < 0. {
        candidate += period;
    }
    if candidate >= duration {
        return unchanged;
    }
    let (mut position, mut best) = (candidate, 0.);
    for offset in (-25..=25).step_by(2) {
        let at = candidate + offset as f64 / 1000.;
        if at < 0. || at >= duration {
            continue;
        }
        let ratio = attack_ratio(pcm, rate, at, 15.);
        if ratio > best {
            best = ratio;
            position = at;
        }
    }
    if grid_hits(pcm, rate, position, period) < old_hits + 2 {
        return unchanged;
    }
    PhaseCorrection {
        position,
        applied: true,
    }
}
