use crate::{rhythm, tonal};
use anyhow::{Result, ensure};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalysisConfig {
    pub key_frame_size: usize,
    pub key_hop_size: usize,
    pub rhythm_frame_size: usize,
    pub rhythm_hop_size: usize,
    /// Lower bound for the classical tempo search; learned beat inference uses its own metre.
    pub min_bpm: f64,
    /// Upper bound for the classical tempo search; does not fold learned beat tempo into range.
    pub max_bpm: f64,
}

impl Default for AnalysisConfig {
    fn default() -> Self {
        Self {
            key_frame_size: 4096,
            key_hop_size: 1024,
            rhythm_frame_size: 1024,
            rhythm_hop_size: 128,
            min_bpm: 60.0,
            max_bpm: 200.0,
        }
    }
}

impl AnalysisConfig {
    pub fn validate(&self) -> Result<()> {
        for (frame, hop) in [
            (self.key_frame_size, self.key_hop_size),
            (self.rhythm_frame_size, self.rhythm_hop_size),
        ] {
            ensure!(
                (256..=32768).contains(&frame) && frame.is_power_of_two(),
                "FFT size must be a power of two in 256..=32768"
            );
            ensure!(hop > 0 && hop <= frame, "Hop must be in 1..=frame size");
        }
        ensure!(
            self.min_bpm.is_finite()
                && self.max_bpm.is_finite()
                && self.min_bpm >= 30.0
                && self.min_bpm < self.max_bpm
                && self.max_bpm <= 300.0,
            "Invalid BPM search range"
        );
        Ok(())
    }

    pub fn digest(&self) -> Result<String> {
        Ok(format!("{:x}", Sha256::digest(serde_json::to_vec(self)?)))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Cue {
    pub time_seconds: f64,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Analysis {
    pub engine: String,
    pub engine_source_sha256: String,
    #[serde(default)]
    pub models: std::collections::BTreeMap<String, String>,
    pub config_sha256: String,
    pub duration_seconds: f64,
    pub sample_rate: u32,
    pub key: Option<tonal::KeyEstimate>,
    pub tempo: Option<rhythm::TempoEstimate>,
    /// Optional experimental timeline for cue/energy development.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub structure: Option<crate::structure::StructureFeatures>,
    /// None means no cue engine was run; an empty list is a distinct result.
    pub cues: Option<Vec<Cue>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lexicon_cues: Option<crate::lexicon_cues::StructuralCueResult>,
    /// One estimated rating for the whole track; no per-segment energy is exposed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub energy: Option<crate::energy::TrackEnergy>,
}

pub fn analyze_pcm(samples: &[f32], sample_rate: u32, config: &AnalysisConfig) -> Result<Analysis> {
    analyze_components(samples, sample_rate, config, true)
}

pub(crate) fn analyze_components(
    samples: &[f32],
    sample_rate: u32,
    config: &AnalysisConfig,
    compute_tempo: bool,
) -> Result<Analysis> {
    config.validate()?;
    ensure!(
        (8000..=192000).contains(&sample_rate),
        "Invalid PCM sample rate"
    );
    ensure!(
        samples.iter().all(|x| x.is_finite()),
        "Non-finite PCM samples"
    );
    let rms = if samples.is_empty() {
        0.0
    } else {
        (samples.iter().map(|x| (*x as f64).powi(2)).sum::<f64>() / samples.len() as f64).sqrt()
    };
    let audible = rms > 1e-5;
    Ok(Analysis {
        engine: format!("camelot-dsp-{}", env!("CARGO_PKG_VERSION")),
        engine_source_sha256: engine_source_sha256(),
        models: Default::default(),
        config_sha256: config.digest()?,
        duration_seconds: samples.len() as f64 / sample_rate as f64,
        sample_rate,
        key: if audible {
            tonal::estimate(samples, sample_rate, config)
        } else {
            None
        },
        tempo: if audible && compute_tempo {
            rhythm::estimate(samples, sample_rate, config)
        } else {
            None
        },
        cues: None,
        lexicon_cues: None,
        energy: None,
        structure: None,
    })
}

pub fn engine_source_sha256() -> String {
    let mut hash = Sha256::new();
    for source in [
        include_str!("analysis.rs"),
        include_str!("audio.rs"),
        include_str!("key.rs"),
        include_str!("tonal.rs"),
        include_str!("rhythm.rs"),
        include_str!("spectral.rs"),
        include_str!("neural.rs"),
        include_str!("profile.rs"),
        include_str!("engine.rs"),
        include_str!("structure.rs"),
        include_str!("lexicon_cues.rs"),
        include_str!("structural_cues.rs"),
        include_str!("energy.rs"),
        include_str!("neural_key.rs"),
        include_str!("openkeyscan.rs"),
        include_str!("openkeyscan/kernels.rs"),
        include_str!("../build.rs"),
        include_str!("../Cargo.toml"),
        include_str!("../../../Cargo.lock"),
    ] {
        hash.update((source.len() as u64).to_le_bytes());
        hash.update(source);
    }
    hash.update(env!("CAMELOT_OPENKEYSCAN_NATIVE_SHA256").as_bytes());
    hash.update(camelot_source_analysis::source_sha256().as_bytes());
    format!("{:x}", hash.finalize())
}
