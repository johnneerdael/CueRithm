use serde::{Deserialize, Serialize};

/// All analysis, waveform and playback positions refer to decoded playable PCM.
/// Missing source gapless information is not an assertion of zero encoder delay.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioClock {
    pub frames: u64,
    pub sample_rate: u32,
    pub leading_padding_frames: Option<u64>,
    pub trailing_padding_frames: Option<u64>,
}
impl AudioClock {
    pub fn duration_seconds(&self) -> f64 {
        self.frames as f64 / self.sample_rate as f64
    }
}
