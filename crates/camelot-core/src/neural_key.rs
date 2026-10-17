//! Optional, pinned S-KEY ONNX inference on mono 22050 Hz PCM.
//! Model preprocessing (peak normalization/VQT/dB/cropping) is in the graph.
use crate::{KeyCandidate, KeyEstimate, Mode, MusicalKey};
use anyhow::{Context, Result, ensure};
use beat_this::{Model, RtenRuntime, Runtime, Tensor};
use sha2::{Digest, Sha256};
use std::path::Path;

pub const MODEL_SHA256: &str = "50bac66d5b02868afad07a0b159f08495cc3f5d8492654587e828ed4686412b1";
pub const SAMPLE_RATE: u32 = 22050;

pub struct NeuralKeyModel {
    model: <RtenRuntime as Runtime>::Model,
    source_sha256: String,
}

impl NeuralKeyModel {
    pub fn load(path: &Path) -> Result<Self> {
        let source_sha256 = format!("{:x}", Sha256::digest(std::fs::read(path)?));
        ensure!(
            source_sha256 == MODEL_SHA256,
            "Unrecognized S-KEY model checksum"
        );
        Ok(Self {
            model: RtenRuntime.load_model(path)?,
            source_sha256,
        })
    }

    pub fn source_sha256(&self) -> &str {
        &self.source_sha256
    }

    pub fn analyze(&mut self, samples: &[f32], sample_rate: u32) -> Result<Option<KeyEstimate>> {
        ensure!(
            sample_rate == SAMPLE_RATE,
            "S-KEY requires direct 22050 Hz PCM"
        );
        ensure!(
            samples.iter().all(|x| x.is_finite()),
            "Non-finite S-KEY PCM input"
        );
        if samples.len() < SAMPLE_RATE as usize * 3 {
            return Ok(None);
        }
        let rms = (samples.iter().map(|x| (*x as f64).powi(2)).sum::<f64>() / samples.len() as f64)
            .sqrt();
        if rms <= 1e-5 {
            return Ok(None);
        }
        let input = Tensor {
            shape: vec![1, samples.len()],
            data: samples.to_vec(),
        };
        let mut output = self.model.run(&[("audio", &input)])?;
        let scores = output
            .remove("scores")
            .context("S-KEY has no scores output")?;
        ensure!(scores.shape == [1, 24], "Invalid S-KEY output shape");
        Ok(Some(key_from_scores(&scores.data)?))
    }
}

/// Interpret the pinned graph's source class order. Scores already contain
/// softmax; preserve their values and do not treat them as calibrated certainty.
pub fn key_from_scores(scores: &[f32]) -> Result<KeyEstimate> {
    ensure!(
        scores.len() == 24
            && scores
                .iter()
                .all(|x| x.is_finite() && (0.0..=1.0).contains(x)),
        "Invalid S-KEY scores"
    );
    ensure!(
        (scores.iter().map(|x| *x as f64).sum::<f64>() - 1.0).abs() < 0.001,
        "S-KEY scores must sum to one"
    );
    let mut candidates = Vec::with_capacity(24);
    for (index, score) in scores.iter().enumerate() {
        // Pinned upstream key_map starts A major, then B minor.
        let (tonic, mode) = if index < 12 {
            ((index + 9) % 12, Mode::Major)
        } else {
            ((index - 12 + 11) % 12, Mode::Minor)
        };
        let key = MusicalKey::new(tonic as u8, mode)?;
        candidates.push(KeyCandidate {
            key,
            camelot: key.camelot(),
            score: *score as f64,
        });
    }
    candidates.sort_by(|a, b| b.score.total_cmp(&a.score));
    Ok(KeyEstimate {
        key: candidates[0].key,
        camelot: candidates[0].camelot.clone(),
        margin: candidates[0].score - candidates[1].score,
        features: None,
        candidates,
    })
}
