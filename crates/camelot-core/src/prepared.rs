//! Common-format application boundary; float data is not quantized for key/cues.
use anyhow::Result;
use cuerithm_media::{AudioClock, PreparedAudio, SourceExportTiming};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PreparedMediaIdentity {
    pub decoder: String,
    pub codec: String,
    pub source_sha256: String,
    #[serde(default)]
    pub pcm_sha256: String,
    pub clock: AudioClock,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub export_timing: Option<SourceExportTiming>,
    /// Effective track genre supplied to cue detection, after any override.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cue_genre: Option<String>,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn export_timing_survives_the_saved_media_identity_boundary() {
        let input = serde_json::json!({
            "decoder":"fixture-decoder", "codec":"mp3", "source_sha256":"a".repeat(64),
            "clock":{"frames":44100,"sample_rate":44100,"leading_padding_frames":null,"trailing_padding_frames":null},
            "export_timing":{"profile":"dj-timeline-v1","source_sha256":"a".repeat(64),"source_sample_rate":44100,
                "codec_profile":{"type":"mp3","case":"B"},"measurement_identity":"b".repeat(64),"decoded_sample_count":23040}
        });
        let saved: PreparedMediaIdentity = serde_json::from_value(input.clone()).unwrap();
        assert_eq!(
            serde_json::to_value(saved).unwrap()["export_timing"],
            input["export_timing"]
        );
        let mut without_count = input.clone();
        without_count["export_timing"]
            .as_object_mut()
            .unwrap()
            .remove("decoded_sample_count");
        let saved: PreparedMediaIdentity = serde_json::from_value(without_count).unwrap();
        assert_eq!(saved.export_timing.unwrap().decoded_sample_count, None);
        let mut legacy = input;
        legacy.as_object_mut().unwrap().remove("export_timing");
        let saved: PreparedMediaIdentity = serde_json::from_value(legacy).unwrap();
        assert!(saved.export_timing.is_none());
        assert!(
            serde_json::to_value(saved)
                .unwrap()
                .get("export_timing")
                .is_none()
        );
    }
}
