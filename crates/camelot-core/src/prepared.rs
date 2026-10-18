//! Common-format application boundary; float data is not quantized for key/cues.
use anyhow::Result;
use cuerithm_media::{AudioClock, PreparedAudio};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PreparedMediaIdentity {
    pub decoder: String,
    pub codec: String,
    pub source_sha256: String,
    #[serde(default)]
    pub pcm_sha256: String,
    pub clock: AudioClock,
}
pub(crate) fn float_wave(audio: &PreparedAudio) -> Result<tempfile::NamedTempFile> {
    let file = tempfile::Builder::new().suffix(".wav").tempfile()?;
    let mut out = hound::WavWriter::create(
        file.path(),
        hound::WavSpec {
            channels: 2,
            sample_rate: 44100,
            bits_per_sample: 32,
            sample_format: hound::SampleFormat::Float,
        },
    )?;
    let mut pos = 0;
    while pos < audio.frames {
        let samples = audio.read_frames(pos, 65536)?;
        for sample in &samples {
            out.write_sample(*sample)?;
        }
        pos += samples.len() as u64 / 2;
    }
    out.finalize()?;
    Ok(file)
}
