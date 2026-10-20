//! Local RBF SVC inference and a single duration-modal track energy estimate.
//! Window classifications are internal; structural cues use their own DSP.
use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{io::Read, path::Path};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TrackEnergy {
    pub level: u8,
    pub method: String,
}

#[derive(Deserialize)]
struct Artifact {
    gamma: f64,
    classes: Vec<u8>,
    n_support: Vec<usize>,
    support_vectors: Vec<Vec<f64>>,
    dual_coef: Vec<Vec<f64>>,
    intercept: Vec<f64>,
}

pub struct EnergyModel {
    data: Artifact,
    starts: Vec<usize>,
    sha256: String,
}

impl EnergyModel {
    /// Read/hash/parse the same owned bytes. No model lookup or network access.
    pub fn load(path: &Path) -> Result<Self> {
        let file = std::fs::File::open(path).context("Open energy SVC model")?;
        let mut bytes = Vec::new();
        file.take(256 * 1024 * 1024 + 1).read_to_end(&mut bytes)?;
        ensure!(
            bytes.len() <= 256 * 1024 * 1024,
            "Energy model exceeds byte limit"
        );
        let data: Artifact = serde_json::from_slice(&bytes).context("Read energy SVC model")?;
        let k = data.classes.len();
        let n = data.support_vectors.len();
        ensure!(
            (2..=10).contains(&k) && (1..=100_000).contains(&n),
            "Invalid SVC size"
        );
        ensure!(
            data.gamma.is_finite() && data.gamma > 0.,
            "Invalid SVC gamma"
        );
        ensure!(
            data.classes.iter().all(|x| (1..=10).contains(x))
                && data.classes.windows(2).all(|p| p[0] < p[1]),
            "Invalid energy classes"
        );
        ensure!(
            data.n_support.len() == k && data.n_support.iter().all(|x| *x > 0 && *x <= n),
            "Invalid support counts"
        );
        ensure!(
            data.n_support.iter().sum::<usize>() == n,
            "Wrong support count sum"
        );
        ensure!(
            data.support_vectors
                .iter()
                .all(|v| v.len() == 183 && v.iter().all(|x| x.is_finite())),
            "Expected finite 183-feature support vectors"
        );
        ensure!(
            data.dual_coef.len() == k - 1
                && data
                    .dual_coef
                    .iter()
                    .all(|v| v.len() == n && v.iter().all(|x| x.is_finite())),
            "Invalid SVC coefficient matrix"
        );
        ensure!(
            data.intercept.len() == k * (k - 1) / 2 && data.intercept.iter().all(|x| x.is_finite()),
            "Invalid SVC intercepts"
        );
        let mut starts = vec![0];
        for count in &data.n_support {
            starts.push(starts.last().unwrap() + count);
        }
        Ok(Self {
            data,
            starts,
            sha256: format!("{:x}", Sha256::digest(&bytes)),
        })
    }

    pub fn source_sha256(&self) -> &str {
        &self.sha256
    }

    /// Numeric model primitive, independent of files, tags, IDs or cue positions.
    pub fn predict(&self, features: &[f64]) -> Result<u8> {
        ensure!(
            features.len() == 183 && features.iter().all(|x| x.is_finite()),
            "Expected finite 183-feature energy vector"
        );
        let kernels: Vec<f64> = self
            .data
            .support_vectors
            .iter()
            .map(|sv| {
                let distance: f64 = features
                    .iter()
                    .zip(sv)
                    .map(|(x, y)| (x - y) * (x - y))
                    .sum();
                (-self.data.gamma * distance).exp()
            })
            .collect();
        let k = self.data.classes.len();
        let mut votes = vec![0; k];
        let mut pair = 0;
        for i in 0..k {
            for j in i + 1..k {
                let mut value = 0.;
                for (s, kernel) in kernels
                    .iter()
                    .enumerate()
                    .take(self.starts[i + 1])
                    .skip(self.starts[i])
                {
                    value += kernel * self.data.dual_coef[j - 1][s];
                }
                for (s, kernel) in kernels
                    .iter()
                    .enumerate()
                    .take(self.starts[j + 1])
                    .skip(self.starts[j])
                {
                    value += kernel * self.data.dual_coef[i][s];
                }
                value += self.data.intercept[pair];
                pair += 1;
                ensure!(value.is_finite(), "Energy decision overflow");
                // sklearn exposes the negated libsvm decision in the binary case.
                let winner = if k == 2 {
                    if value < 0. { i } else { j }
                } else if value > 0. {
                    i
                } else {
                    j
                };
                votes[winner] += 1;
            }
        }
        let mut winner = 0;
        for i in 1..k {
            if votes[i] > votes[winner] {
                winner = i;
            }
        }
        Ok(self.data.classes[winner])
    }

    /// Overall level as the 80th percentile of the internal window levels, with linear interpolation and ties to even.
    /// Measured on 282 development songs with out-of-fold window classification, this reproduces the server's
    /// overall energy (which equals the Mixed In Key file tag) on 74.8% exactly and 99.6% within one level, where the
    /// duration-weighted mode below reaches 58.7% on the same data. The patent that describes the method allows a 75th
    /// percentile, median or mean; 80 was the best of eleven tried. Window levels stay internal.
    pub fn estimate_overall(
        &self,
        windows: &[camelot_source_analysis::EnergySegment],
    ) -> Result<Option<TrackEnergy>> {
        if windows.is_empty() {
            return Ok(None);
        }
        let mut levels = Vec::with_capacity(windows.len());
        let mut prior_end = 0.;
        for w in windows {
            ensure!(
                w.start_time.is_finite()
                    && w.end_time.is_finite()
                    && w.start_time >= prior_end
                    && w.end_time > w.start_time,
                "Invalid energy window bounds"
            );
            levels.push(f64::from(self.predict(&w.features)?));
            prior_end = w.end_time;
        }
        levels.sort_by(f64::total_cmp);
        let position = 0.8 * (levels.len() - 1) as f64;
        let (low, high) = (position.floor() as usize, position.ceil() as usize);
        let value = levels[low] + (position - low as f64) * (levels[high] - levels[low]);
        ensure!(value.is_finite(), "Energy percentile overflow");
        Ok(Some(TrackEnergy {
            level: value.round_ties_even().clamp(1., 10.) as u8,
            method: "svc-percentile80-v1".into(),
        }))
    }

    /// Aggregate internal windows by total nominal duration. Highest level wins
    /// an exact duration tie, matching the retained client-side track-tag rule.
    /// This is distinct from the unrecovered server OverallEnergy mapping.
    pub fn estimate(
        &self,
        windows: &[camelot_source_analysis::EnergySegment],
    ) -> Result<Option<TrackEnergy>> {
        if windows.is_empty() {
            return Ok(None);
        }
        let mut duration = [0.; 11];
        let mut prior_end = 0.;
        for w in windows {
            ensure!(
                w.start_time.is_finite()
                    && w.end_time.is_finite()
                    && w.start_time >= prior_end
                    && w.end_time > w.start_time,
                "Invalid energy window bounds"
            );
            let level = self.predict(&w.features)?;
            duration[usize::from(level)] += w.end_time - w.start_time;
            prior_end = w.end_time;
        }
        let mut winner = 10;
        for level in (1..10).rev() {
            if duration[level] > duration[winner] {
                winner = level;
            }
        }
        ensure!(duration[winner].is_finite(), "Energy duration overflow");
        Ok(Some(TrackEnergy {
            level: winner as u8,
            method: "svc-duration-mode-v1".into(),
        }))
    }
}
