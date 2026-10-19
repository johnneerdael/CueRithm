use anyhow::{Result, ensure};
use cuerithm_media::{AudioClock, PreparedAudio};
use rustfft::{Fft, FftPlanner, num_complex::Complex};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

const WINDOW: usize = 2048;
const SAMPLE_RATE: f32 = 44100.;
// Band edges and weights follow Lexicon's RGB waveform: bass, mids and highs map to red, green and blue.
const BANDS: [(f32, f32, f32); 3] = [(0., 150., 1.2), (150., 1500., 1.), (1500., 6000., 1.)];

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WaveBin {
    pub min: f32,
    pub max: f32,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Waveform {
    pub duration_seconds: f64,
    pub clock: AudioClock,
    pub bins: Vec<WaveBin>,
    /// Per-bin red/green/blue weights from bass, mid and high energy, strongest band at 255.
    /// Empty in caches written before colour was added.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub bands: Vec<[u8; 3]>,
}

struct BandAnalyzer {
    fft: Arc<dyn Fft<f32>>,
    taper: Vec<f32>,
    buffer: Vec<Complex<f32>>,
    ranges: [(usize, usize); 3],
}
impl BandAnalyzer {
    fn new() -> Self {
        let hz = SAMPLE_RATE / WINDOW as f32;
        let last = WINDOW / 2 - 1;
        let edge = |hz_edge: f32| ((hz_edge / hz).floor() as usize).min(last);
        Self {
            fft: FftPlanner::new().plan_fft_forward(WINDOW),
            taper: (0..WINDOW)
                .map(|i| 0.5 - 0.5 * (std::f32::consts::TAU * i as f32 / WINDOW as f32).cos())
                .collect(),
            buffer: vec![Complex::new(0., 0.); WINDOW],
            ranges: BANDS.map(|(low, high, _)| (edge(low), edge(high))),
        }
    }
    fn colour(&mut self, mono: &[f32]) -> [u8; 3] {
        for (i, slot) in self.buffer.iter_mut().enumerate() {
            *slot = Complex::new(mono.get(i).map_or(0., |v| v * self.taper[i]), 0.);
        }
        self.fft.process(&mut self.buffer);
        let mut level = [0f32; 3];
        for (band, (first, last)) in self.ranges.iter().enumerate() {
            let power: f32 = self.buffer[*first..=*last]
                .iter()
                .map(|c| c.norm_sqr())
                .sum();
            level[band] = (power / (last - first + 1) as f32).sqrt() * BANDS[band].2;
        }
        let strongest = level.iter().cloned().fold(0., f32::max);
        if strongest <= f32::EPSILON {
            return [0; 3];
        }
        level.map(|v| (v / strongest * 255.).round() as u8)
    }
}

pub fn waveform(audio: &PreparedAudio, bins: usize) -> Result<Waveform> {
    ensure!((1..=16384).contains(&bins), "Invalid waveform resolution");
    let mut output = vec![WaveBin { min: 0., max: 0. }; bins];
    let mut bands = vec![[0u8; 3]; bins];
    let mut analyzer = BandAnalyzer::new();
    // Mono samples from the first frame still needed by an unanalysed bin.
    let (mut mono, mut base, mut next) = (Vec::<f32>::new(), 0u64, 0usize);
    let start_of = |bin: usize| (bin as u64 * audio.frames).div_ceil(bins as u64);
    let mut pos = 0;
    while pos < audio.frames {
        let samples = audio.read_frames(pos, 65536)?;
        for (i, pair) in samples.as_chunks::<2>().0.iter().enumerate() {
            let bin = ((pos + i as u64) * bins as u64 / audio.frames) as usize;
            output[bin].min = output[bin].min.min(pair[0]).min(pair[1]);
            output[bin].max = output[bin].max.max(pair[0]).max(pair[1]);
            if pos + i as u64 >= base + mono.len() as u64 {
                mono.push((pair[0] + pair[1]) / 2.);
            }
        }
        pos += samples.len() as u64 / 2;
        let end = base + mono.len() as u64;
        while next < bins {
            let from = start_of(next);
            if from + WINDOW as u64 > end && end < audio.frames {
                break;
            }
            let at = (from - base) as usize;
            bands[next] = analyzer.colour(&mono[at..(at + WINDOW).min(mono.len())]);
            next += 1;
        }
        let keep = if next < bins { start_of(next) } else { end };
        if keep >= end {
            mono.clear();
        } else {
            mono.drain(..(keep - base) as usize);
        }
        base = keep;
    }
    Ok(Waveform {
        duration_seconds: audio.clock.duration_seconds(),
        clock: audio.clock.clone(),
        bins: output,
        bands,
    })
}

pub fn cached_waveform(
    audio: &PreparedAudio,
    bins: usize,
    cache: &std::path::Path,
) -> Result<Waveform> {
    use sha2::{Digest, Sha256};
    use std::io::{Read, Write};
    ensure!((1..=16384).contains(&bins), "Invalid waveform resolution");
    let key = format!(
        "{:x}",
        Sha256::digest(format!(
            "{}:{}:{bins}",
            audio.source_sha256, audio.decoder_identity
        ))
    );
    std::fs::create_dir_all(cache)?;
    let path = cache.join(format!("{key}.json"));
    if let Ok(file) = std::fs::File::open(&path)
        && let Ok(value) = serde_json::from_reader::<_, Waveform>(file.take(4 * 1024 * 1024))
        && value.clock.frames == audio.frames
        && value.clock.sample_rate == 44100
        && value.duration_seconds == audio.clock.duration_seconds()
        && value.bins.len() == bins
        && value.bands.len() == bins
        && value
            .bins
            .iter()
            .all(|b| b.min.is_finite() && b.max.is_finite() && b.min <= b.max)
    {
        return Ok(value);
    }
    let value = waveform(audio, bins)?;
    let mut temp = tempfile::NamedTempFile::new_in(cache)?;
    temp.write_all(&serde_json::to_vec(&value)?)?;
    temp.as_file().sync_all()?;
    temp.persist(path)?;
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn tone(hz: f32) -> Vec<f32> {
        (0..WINDOW)
            .map(|i| (std::f32::consts::TAU * hz * i as f32 / SAMPLE_RATE).sin())
            .collect()
    }
    #[test]
    fn bass_mids_and_highs_map_to_red_green_and_blue() {
        let mut analyzer = BandAnalyzer::new();
        for (hz, channel) in [(60., 0), (700., 1), (3500., 2)] {
            let colour = analyzer.colour(&tone(hz));
            assert_eq!(
                colour[channel], 255,
                "{hz} Hz should peak channel {channel}"
            );
            for other in (0..3).filter(|c| *c != channel) {
                assert!(
                    colour[other] < 60,
                    "{hz} Hz leaked into channel {other}: {colour:?}"
                );
            }
        }
    }
    #[test]
    fn silence_has_no_colour() {
        assert_eq!(BandAnalyzer::new().colour(&vec![0.; WINDOW]), [0; 3]);
    }
}
