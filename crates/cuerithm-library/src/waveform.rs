use anyhow::{Result, ensure};
use cuerithm_media::{AudioClock, PreparedAudio};
use serde::{Deserialize, Serialize};

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
}

pub fn waveform(audio: &PreparedAudio, bins: usize) -> Result<Waveform> {
    ensure!((1..=16384).contains(&bins), "Invalid waveform resolution");
    let mut output = vec![WaveBin { min: 0., max: 0. }; bins];
    let mut pos = 0;
    while pos < audio.frames {
        let samples = audio.read_frames(pos, 65536)?;
        for (i, pair) in samples.chunks_exact(2).enumerate() {
            let bin = ((pos + i as u64) * bins as u64 / audio.frames) as usize;
            output[bin].min = output[bin].min.min(pair[0]).min(pair[1]);
            output[bin].max = output[bin].max.max(pair[0]).max(pair[1]);
        }
        pos += samples.len() as u64 / 2;
    }
    Ok(Waveform {
        duration_seconds: audio.clock.duration_seconds(),
        clock: audio.clock.clone(),
        bins: output,
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
