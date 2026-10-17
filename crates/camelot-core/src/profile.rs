//! Shared major/minor profiles applied to relative chroma; no file identities.
use crate::{AnalysisConfig, KeyCandidate, KeyEstimate, Mode, MusicalKey};
use anyhow::{Context, Result, ensure};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::path::Path;

#[derive(Deserialize)]
struct ProfileData {
    schema_version: u32,
    feature_schema: String,
    feature_weights: [f64; 2],
    coefficients: [[f64; 24]; 2],
    intercepts: [f64; 2],
    sample_rate: u32,
    key_frame_size: usize,
    key_hop_size: usize,
}

pub struct KeyProfileModel {
    data: ProfileData,
    source_sha256: String,
}

impl KeyProfileModel {
    pub fn load(path: &Path) -> Result<Self> {
        Self::from_json(&std::fs::read_to_string(path)?)
    }

    pub fn from_json(source: &str) -> Result<Self> {
        let data: ProfileData = serde_json::from_str(source)?;
        ensure!(
            data.schema_version == 1
                && data.feature_schema == "relative-centered-unit-chroma-bass-v1",
            "Unsupported key-profile schema"
        );
        ensure!(
            data.feature_weights
                .iter()
                .all(|x| x.is_finite() && *x >= 0.0)
                && data.feature_weights.iter().sum::<f64>() > 0.0,
            "Invalid profile feature weights"
        );
        ensure!(
            data.coefficients
                .iter()
                .flatten()
                .chain(data.intercepts.iter())
                .all(|x| x.is_finite()),
            "Non-finite profile coefficient"
        );
        ensure!(
            (8000..=192000).contains(&data.sample_rate),
            "Invalid profile sample rate"
        );
        ensure!(
            (256..=32768).contains(&data.key_frame_size)
                && data.key_frame_size.is_power_of_two()
                && data.key_hop_size > 0
                && data.key_hop_size <= data.key_frame_size,
            "Invalid profile frame configuration"
        );
        Ok(Self {
            data,
            source_sha256: format!("{:x}", Sha256::digest(source.as_bytes())),
        })
    }

    pub fn source_sha256(&self) -> &str {
        &self.source_sha256
    }

    pub fn validate_for(&self, sample_rate: u32, config: &AnalysisConfig) -> Result<()> {
        ensure!(
            sample_rate == self.data.sample_rate
                && config.key_frame_size == self.data.key_frame_size
                && config.key_hop_size == self.data.key_hop_size,
            "Key model and feature extraction configuration do not match"
        );
        Ok(())
    }

    pub fn apply(&self, estimate: &mut KeyEstimate) -> Result<()> {
        let mut normalized = [0.0; 24];
        let features = estimate
            .features
            .as_ref()
            .context("Key profile requires DSP chroma features")?;
        for (block, values) in [&features.chroma, &features.bass_chroma]
            .into_iter()
            .enumerate()
        {
            ensure!(
                values.iter().all(|x| x.is_finite()),
                "Non-finite key features"
            );
            let mean = values.iter().sum::<f64>() / 12.0;
            let norm = values
                .iter()
                .map(|x| (x - mean).powi(2))
                .sum::<f64>()
                .sqrt()
                .max(1e-12);
            for i in 0..12 {
                normalized[block * 12 + i] =
                    (values[i] - mean) / norm * self.data.feature_weights[block];
            }
        }
        let mut candidates = Vec::with_capacity(24);
        for (mode_index, mode) in [Mode::Major, Mode::Minor].into_iter().enumerate() {
            for tonic in 0..12 {
                let mut score = self.data.intercepts[mode_index];
                for i in 0..12 {
                    score += self.data.coefficients[mode_index][i] * normalized[(i + tonic) % 12];
                    score += self.data.coefficients[mode_index][12 + i]
                        * normalized[12 + (i + tonic) % 12];
                }
                ensure!(score.is_finite(), "Non-finite key profile score");
                let key = MusicalKey::new(tonic as u8, mode)?;
                candidates.push(KeyCandidate {
                    key,
                    camelot: key.camelot(),
                    score,
                });
            }
        }
        candidates.sort_by(|a, b| b.score.total_cmp(&a.score));
        estimate.key = candidates[0].key;
        estimate.camelot = candidates[0].camelot.clone();
        estimate.margin = candidates[0].score - candidates[1].score;
        estimate.candidates = candidates;
        Ok(())
    }
}
