//! Explicit canonical PCM input; container-independent, without file tag inputs.
use anyhow::{Result, ensure};

pub struct PcmProfile<'a> {
    pub interleaved: &'a [f32],
    pub full_frames: usize,
    pub analyzed_frames: usize,
}
impl<'a> PcmProfile<'a> {
    pub fn new(interleaved: &'a [f32], full_frames: usize) -> Result<Self> {
        ensure!(interleaved.len().is_multiple_of(2), "Stereo PCM required");
        let analyzed_frames = interleaved.len() / 2;
        ensure!(
            full_frames >= analyzed_frames && full_frames <= 64_000_000,
            "Invalid frame counts"
        );
        ensure!(
            interleaved
                .iter()
                .all(|x| x.is_finite() && (-1.0..1.0).contains(x) && (*x * 32768.).fract() == 0.),
            "Signed16 normalized PCM required"
        );
        Ok(Self {
            interleaved,
            full_frames,
            analyzed_frames,
        })
    }
}
pub fn analyze_timing_pcm(profile: &PcmProfile<'_>) -> Result<crate::TimingAnalysis> {
    let valid = PcmProfile::new(profile.interleaved, profile.full_frames)?;
    ensure!(
        valid.analyzed_frames == profile.analyzed_frames,
        "Wrong PCM boundary"
    );
    let main = crate::timing::compute(profile.interleaved, profile.full_frames)?;
    Ok(crate::TimingAnalysis {
        profile: "cuerithm-canonical-pcm16-44100-stereo-v1".into(),
        full_source_frames: profile.full_frames,
        analyzed_frames: profile.analyzed_frames,
        bpm: main.wire_tempo,
        downbeat_time: main.wire_downbeat,
        beats_seconds: main.beat_times,
    })
}
pub fn analyze_energy_pcm(profile: &PcmProfile<'_>) -> Result<Vec<crate::EnergySegment>> {
    let valid = PcmProfile::new(profile.interleaved, profile.full_frames)?;
    ensure!(
        valid.analyzed_frames == profile.analyzed_frames,
        "Wrong PCM boundary"
    );
    crate::energy::analyze(profile.interleaved, profile.full_frames)
}
