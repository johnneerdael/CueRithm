//! Optional OpenKeyScan3: direct 44100 Hz mono PCM -> fixed CQT -> pinned RTen graph.
//! Code/model provenance and native-library notices: docs/research/openkeyscan-rust-backend.md.
mod kernels;
#[cfg(test)]
mod tests;

use crate::{KeyCandidate, KeyEstimate, MusicalKey};
use anyhow::{Context, Result, ensure};
use rten::{Model, ValueView};
use rten_tensor::Layout;
use rustfft::{FftPlanner, num_complex::Complex32};
use sha2::{Digest, Sha256};
use std::{ffi::c_char, path::Path};

pub const MODEL_SHA256: &str = "262ac22bce23d371fe045d478e90d200f71b5be16046792a8b55432ae5c9f952";
pub const SAMPLE_RATE: u32 = 44100;
pub const MAX_SAMPLES: usize = 64_000_000;
pub const NATIVE_SHA256: &str = env!("CAMELOT_OPENKEYSCAN_NATIVE_SHA256");

pub struct OpenKeyScanModel {
    model: Model,
}

impl OpenKeyScanModel {
    pub fn load(path: &Path) -> Result<Self> {
        let bytes = std::fs::read(path).context("Read OpenKeyScan model")?;
        ensure!(
            format!("{:x}", Sha256::digest(&bytes)) == MODEL_SHA256,
            "Unrecognized OpenKeyScan model checksum"
        );
        // Parse the exact owned buffer that was checked, with no second path open.
        let model = Model::load(bytes).context("Load pinned OpenKeyScan graph")?;
        model.node_id("spectrogram")?;
        model.node_id("logits")?;
        Ok(Self { model })
    }

    pub fn source_sha256(&self) -> &str {
        MODEL_SHA256
    }

    /// Raw network logits rank keys; scores/margins are not probabilities.
    /// Explicit excerpts are permitted but must remain marked partial by the host.
    pub fn analyze(&mut self, samples: &[f32], sample_rate: u32) -> Result<Option<KeyEstimate>> {
        let Some(tensor) = preprocess(samples, sample_rate)? else {
            return Ok(None);
        };
        let input = ValueView::from_shape([1, 1, 105, tensor.columns], tensor.data.as_slice())?;
        let [output] = self.model.run_n(
            vec![(self.model.node_id("spectrogram")?, input.into())],
            [self.model.node_id("logits")?],
            None,
        )?;
        let logits = output
            .into_tensor::<f32>()
            .context("OpenKeyScan output must be float32")?;
        ensure!(
            logits.shape() == [1, 24],
            "Invalid OpenKeyScan output shape"
        );
        Ok(Some(key_from_logits(&logits.into_data())?))
    }
}

pub fn frontend_sha256() -> String {
    let mut hash = Sha256::new();
    for bytes in [
        include_bytes!("openkeyscan.rs").as_slice(),
        include_bytes!("openkeyscan/kernels.rs").as_slice(),
        include_bytes!("../../../Cargo.lock").as_slice(),
        NATIVE_SHA256.as_bytes(),
        b"rten=0.24.0;rustfft=6;openkeyscan-cqt-v1",
    ] {
        hash.update((bytes.len() as u64).to_le_bytes());
        hash.update(bytes);
    }
    format!("{:x}", hash.finalize())
}

fn key_from_logits(logits: &[f32]) -> Result<KeyEstimate> {
    ensure!(
        logits.len() == 24 && logits.iter().all(|x| x.is_finite()),
        "Invalid OpenKeyScan logits"
    );
    let mut candidates = Vec::with_capacity(24);
    for (class, &score) in logits.iter().enumerate() {
        let camelot = format!("{}{}", class % 12 + 1, if class < 12 { "A" } else { "B" });
        candidates.push(KeyCandidate {
            key: MusicalKey::from_camelot(&camelot)?,
            camelot,
            score: f64::from(score),
        });
    }
    // Stable sorting preserves the source's first-class tie rule.
    candidates.sort_by(|a, b| b.score.partial_cmp(&a.score).expect("finite logits"));
    Ok(KeyEstimate {
        key: candidates[0].key,
        camelot: candidates[0].camelot.clone(),
        margin: candidates[0].score - candidates[1].score,
        candidates,
        features: None,
    })
}

struct Spectrogram {
    columns: usize,
    data: Vec<f32>,
}

fn preprocess(samples: &[f32], sample_rate: u32) -> Result<Option<Spectrogram>> {
    ensure!(
        sample_rate == SAMPLE_RATE,
        "OpenKeyScan requires direct 44100 Hz PCM"
    );
    ensure!(
        samples.len() <= MAX_SAMPLES,
        "OpenKeyScan supports at most 64000000 analyzed frames"
    );
    ensure!(
        samples.iter().all(|x| x.is_finite()),
        "Non-finite OpenKeyScan PCM input"
    );
    if samples.len() < SAMPLE_RATE as usize * 3 {
        return Ok(None);
    }
    let rms =
        (samples.iter().map(|x| f64::from(*x).powi(2)).sum::<f64>() / samples.len() as f64).sqrt();
    if rms <= 1e-5 {
        return Ok(None);
    }
    cqt(samples).map(Some)
}

unsafe extern "C" {
    fn cw_soxr_half(
        input: *const f32,
        count: usize,
        output: *mut f32,
        capacity: usize,
        written: *mut usize,
        message: *mut c_char,
        message_capacity: usize,
    ) -> i32;
}

fn half(input: &[f32]) -> Result<Vec<f32>> {
    let mut output = vec![0.0; input.len() / 2 + 1];
    let mut written = 0;
    let mut message = [0 as c_char; 256];
    // Both slices remain allocated through the call; the C adapter receives their
    // exact bounds, owns/deletes its resampler and retains no caller pointers.
    let code = unsafe {
        cw_soxr_half(
            input.as_ptr(),
            input.len(),
            output.as_mut_ptr(),
            output.len(),
            &mut written,
            message.as_mut_ptr(),
            message.len(),
        )
    };
    let message: Vec<u8> = message
        .iter()
        .take_while(|&&c| c != 0)
        .map(|&c| c as u8)
        .collect();
    ensure!(code == 0, "SoXR: {}", String::from_utf8_lossy(&message));
    ensure!(written <= output.len(), "Invalid SoXR output count");
    output.truncate(written);
    output.resize(input.len().div_ceil(2), 0.0);
    let scale = 0.5_f64.sqrt() as f32;
    for sample in &mut output {
        *sample /= scale;
    }
    Ok(output)
}

fn cqt(samples: &[f32]) -> Result<Spectrogram> {
    let level1 = half(samples)?;
    let level2 = half(&level1)?;
    let signals = [
        samples,
        level1.as_slice(),
        level2.as_slice(),
        level2.as_slice(),
        level2.as_slice(),
    ];
    let columns = kernels::BANKS
        .iter()
        .zip(&signals)
        .map(|(bank, s)| 1 + s.len() / bank.hop)
        .min()
        .unwrap();
    ensure!(
        columns >= 10,
        "network requires at least10 CQT frames before the two-frame crop"
    );
    let kept = columns - 2;
    let mut result = vec![0_f32; 105 * kept];
    let mut planner = FftPlanner::<f32>::new();
    for (bank, signal) in kernels::BANKS.iter().zip(signals) {
        ensure!(
            bank.first_bin + bank.rows <= 105
                && bank.indptr.len() == bank.rows + 1
                && bank.indptr[bank.rows] as usize == bank.indices.len()
                && bank.indices.len() == bank.coefficient_bits.len(),
            "invalid fixed kernel"
        );
        let fft = planner.plan_fft_forward(bank.n_fft);
        let mut frame = vec![Complex32::new(0.0, 0.0); bank.n_fft];
        let coefficients: Vec<_> = bank
            .coefficient_bits
            .iter()
            .map(|v| Complex32::new(f32::from_bits(v[0]), f32::from_bits(v[1])))
            .collect();
        for column in 0..kept {
            for (i, sample) in frame.iter_mut().enumerate() {
                let source = column as i64 * bank.hop as i64 + i as i64 - (bank.n_fft / 2) as i64;
                sample.re = if source >= 0 {
                    signal.get(source as usize).copied().unwrap_or(0.0)
                } else {
                    0.0
                };
                sample.im = 0.0;
            }
            fft.process(&mut frame);
            for row in 0..bank.rows {
                let bin = bank.first_bin + row;
                let mut sum = Complex32::new(0.0, 0.0);
                for k in bank.indptr[row] as usize..bank.indptr[row + 1] as usize {
                    sum += coefficients[k] * frame[bank.indices[k] as usize];
                }
                let length = f64::from_bits(kernels::NORM_BITS[bin]);
                let real = (f64::from(sum.re) / length) as f32;
                let imag = (f64::from(sum.im) / length) as f32;
                result[bin * kept + column] = real.hypot(imag).ln_1p();
            }
        }
    }
    ensure!(result.iter().all(|v| v.is_finite()), "nonfinite CQT");
    Ok(Spectrogram {
        columns: kept,
        data: result,
    })
}
