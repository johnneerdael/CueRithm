use super::onset::{attack_ratio, power, precise_onset};
use serde::{Deserialize, Serialize};

mod calibration;
mod grid;
mod rescue;
mod riser;
mod subsequent;
pub(super) use calibration::{calibrate_drop, calibrate_onset, verify_drop};
pub(super) use grid::align;
pub(super) use rescue::{half_beat, rescue_silent, rescue_weak, validate_grid};
pub(super) use riser::validate_riser;
pub(super) use subsequent::subsequent;
pub(super) type Rescue = super::dp::PhaseCorrection;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct AppliedCorrection {
    pub position: f64,
    pub delta_ms: f64,
    pub source: &'static str,
}
#[derive(Default)]
pub(super) struct CorrectionState {
    pub last: Option<AppliedCorrection>,
}
impl CorrectionState {
    pub fn reset(&mut self) {
        self.last = None;
    }
    pub fn note_grid(&mut self, position: f64, delta_ms: f64) {
        self.last = Some(AppliedCorrection {
            position,
            delta_ms,
            source: "gridcorr",
        });
    }
}
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct Subsequent {
    pub corrected_first_beat: f64,
    pub applied_correction: bool,
}
pub(super) fn near_zero(anchor: f64) -> Subsequent {
    Subsequent {
        corrected_first_beat: anchor,
        applied_correction: false,
    }
}
#[derive(Debug, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub(super) struct SubsequentOptions {
    pub search_window_ms: f64,
    pub max_offset_ms: f64,
    pub max_std_dev_ms: f64,
    pub min_attack_ratio: Option<f64>,
}
impl Default for SubsequentOptions {
    fn default() -> Self {
        Self {
            search_window_ms: 25.,
            max_offset_ms: 30.,
            max_std_dev_ms: 4.,
            min_attack_ratio: None,
        }
    }
}
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct Calibration {
    pub position: f64,
    pub used_calibration: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub calibration_beat: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub calibration_beats: Option<usize>,
}
impl Calibration {
    fn unchanged(position: f64) -> Self {
        Self {
            position,
            used_calibration: false,
            calibration_beat: None,
            calibration_beats: None,
        }
    }
}
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct DropCorrection {
    pub position: f64,
    pub used_correction: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub drop_position: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub drop_attack_ratio: Option<f64>,
}
impl DropCorrection {
    fn unchanged(position: f64) -> Self {
        Self {
            position,
            used_correction: false,
            drop_position: None,
            drop_attack_ratio: None,
        }
    }
}
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct Alignment {
    pub corrected_first_beat: f64,
    pub applied_correction: bool,
    pub offset_ms: i32,
    pub improvement: f64,
}
fn valid(pcm: &[f32], rate: u32, anchor: f64, bpm: f64) -> bool {
    (8000..=192000).contains(&rate)
        && anchor.is_finite()
        && anchor >= 0.
        && anchor <= pcm.len() as f64 / rate as f64
        && bpm.is_finite()
        && (50. ..=300.).contains(&bpm)
}
fn rms(pcm: &[f32], rate: u32, at: f64, seconds: f64) -> f64 {
    let index = (at * rate as f64).round() as usize;
    let width = (seconds * rate as f64).round() as usize;
    (power(pcm, index, index.saturating_add(width)) / width as f64).sqrt()
}
fn best_attack(
    pcm: &[f32],
    rate: u32,
    anchor: f64,
    radius: i32,
    step: usize,
    window_ms: f64,
) -> (f64, f64) {
    let duration = pcm.len() as f64 / rate as f64;
    let (mut position, mut best) = (anchor, 0.);
    for offset in (-radius..=radius).step_by(step.max(1)) {
        let at = anchor + offset as f64 / 1000.;
        if at < 0. || at >= duration {
            continue;
        }
        let ratio = attack_ratio(pcm, rate, at, window_ms);
        if ratio > best {
            best = ratio;
            position = at;
        }
    }
    (position, best)
}
fn grid_hits(
    pcm: &[f32],
    rate: u32,
    anchor: f64,
    period: f64,
    radius: i32,
    threshold: f64,
) -> usize {
    let duration = pcm.len() as f64 / rate as f64;
    let mut hits = 0;
    for beat in 0..8 {
        let at = anchor + beat as f64 * period;
        if at >= duration - 0.1 {
            break;
        }
        if best_attack(pcm, rate, at, radius, 2, 15.).1 > threshold {
            hits += 1;
        }
    }
    hits
}
