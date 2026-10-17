//! Reference comparison only. This module is never used to produce predictions.
use crate::{Cue, MusicalKey};
use anyhow::{Result, ensure};
use serde::Serialize;
use std::collections::BTreeMap;

pub struct Comparison {
    pub reference_key: MusicalKey,
    pub reference_bpm: Option<f64>,
    pub reference_cues: Vec<Cue>,
    pub predicted_key: Option<MusicalKey>,
    pub predicted_bpm: Option<f64>,
    pub predicted_cues: Option<Vec<Cue>>,
    pub failed: bool,
}

#[derive(Debug, Serialize)]
pub struct CueScore {
    pub tolerance_seconds: f64,
    pub reference_count: usize,
    pub predicted_count: usize,
    pub matched_count: usize,
    pub tracks_with_prediction: usize,
    pub tracks_with_exact_count: usize,
    pub matched_name_count: usize,
    pub precision: f64,
    pub recall: f64,
    pub f1: f64,
    pub mean_absolute_error_seconds: Option<f64>,
}

#[derive(Debug, Serialize)]
pub struct EvaluationScore {
    pub tracks: usize,
    pub failed_tracks: usize,
    pub key_predictions: usize,
    pub exact_key_matches: usize,
    pub key_agreement: f64,
    pub key_confusion: BTreeMap<String, usize>,
    pub bpm_references: usize,
    pub bpm_predictions: usize,
    pub exact_display_bpm_matches: usize,
    pub bpm_within_half_matches: usize,
    pub bpm_agreement_within_half: f64,
    pub half_double_bpm_matches: usize,
    pub mean_absolute_bpm_error: Option<f64>,
    pub cues: CueScore,
}

fn ratio(n: usize, d: usize) -> f64 {
    if d == 0 { 0.0 } else { n as f64 / d as f64 }
}

#[derive(Clone, Copy, Default)]
struct Cell {
    matches: usize,
    error: f64,
    action: u8,
}

fn better(a: Cell, b: Cell) -> bool {
    a.matches > b.matches || (a.matches == b.matches && a.error < b.error)
}

fn match_cues<'a>(
    reference: &'a [Cue],
    predicted: &'a [Cue],
    tolerance: f64,
) -> Vec<(&'a Cue, &'a Cue)> {
    let mut reference: Vec<_> = reference.iter().collect();
    let mut predicted: Vec<_> = predicted.iter().collect();
    reference.sort_by(|a, b| a.time_seconds.total_cmp(&b.time_seconds));
    predicted.sort_by(|a, b| a.time_seconds.total_cmp(&b.time_seconds));
    let width = predicted.len() + 1;
    let mut table = vec![Cell::default(); (reference.len() + 1) * width];
    for i in (0..reference.len()).rev() {
        for j in (0..predicted.len()).rev() {
            let mut best = Cell {
                action: 1,
                ..table[(i + 1) * width + j]
            };
            let skip = Cell {
                action: 2,
                ..table[i * width + j + 1]
            };
            if better(skip, best) {
                best = skip;
            }
            let error = (reference[i].time_seconds - predicted[j].time_seconds).abs();
            if error <= tolerance {
                let next = table[(i + 1) * width + j + 1];
                let matched = Cell {
                    matches: next.matches + 1,
                    error: next.error + error,
                    action: 3,
                };
                if better(matched, best) {
                    best = matched;
                }
            }
            table[i * width + j] = best;
        }
    }
    let (mut i, mut j) = (0, 0);
    let mut matches = Vec::new();
    while i < reference.len() && j < predicted.len() {
        match table[i * width + j].action {
            1 => i += 1,
            2 => j += 1,
            3 => {
                matches.push((reference[i], predicted[j]));
                i += 1;
                j += 1;
            }
            _ => break,
        }
    }
    matches
}

pub fn evaluate(rows: &[Comparison], cue_tolerance_seconds: f64) -> Result<EvaluationScore> {
    ensure!(
        cue_tolerance_seconds.is_finite() && cue_tolerance_seconds >= 0.0,
        "Invalid cue tolerance"
    );
    let mut score = EvaluationScore {
        tracks: rows.len(),
        failed_tracks: 0,
        key_predictions: 0,
        exact_key_matches: 0,
        key_agreement: 0.0,
        key_confusion: BTreeMap::new(),
        bpm_references: 0,
        bpm_predictions: 0,
        exact_display_bpm_matches: 0,
        bpm_within_half_matches: 0,
        bpm_agreement_within_half: 0.0,
        half_double_bpm_matches: 0,
        mean_absolute_bpm_error: None,
        cues: CueScore {
            tolerance_seconds: cue_tolerance_seconds,
            reference_count: 0,
            predicted_count: 0,
            matched_count: 0,
            tracks_with_prediction: 0,
            tracks_with_exact_count: 0,
            matched_name_count: 0,
            precision: 0.0,
            recall: 0.0,
            f1: 0.0,
            mean_absolute_error_seconds: None,
        },
    };
    let mut bpm_error_sum = 0.0;
    let mut cue_error_sum = 0.0;
    for row in rows {
        ensure!(
            row.reference_bpm
                .into_iter()
                .chain(row.predicted_bpm)
                .all(|v| v.is_finite() && v > 0.0),
            "Invalid BPM value"
        );
        ensure!(
            row.reference_cues.len() <= 1024
                && row.predicted_cues.as_ref().is_none_or(|c| c.len() <= 1024),
            "Excessive cue count"
        );
        ensure!(
            row.reference_cues
                .iter()
                .chain(row.predicted_cues.iter().flatten())
                .all(|c| c.time_seconds.is_finite()),
            "Invalid cue timestamp"
        );
        score.failed_tracks += usize::from(row.failed);
        let predicted_key = if row.failed { None } else { row.predicted_key };
        if let Some(key) = predicted_key {
            score.key_predictions += 1;
            score.exact_key_matches += usize::from(key == row.reference_key);
        }
        *score
            .key_confusion
            .entry(format!(
                "{} -> {}",
                row.reference_key.camelot(),
                predicted_key.map_or_else(|| "unknown".into(), |k| k.camelot())
            ))
            .or_default() += 1;
        if let Some(reference) = row.reference_bpm {
            score.bpm_references += 1;
            if let Some(predicted) = row.predicted_bpm.filter(|_| !row.failed) {
                score.bpm_predictions += 1;
                let error = (predicted - reference).abs();
                bpm_error_sum += error;
                score.exact_display_bpm_matches +=
                    usize::from(format!("{predicted:.2}") == format!("{reference:.2}"));
                score.bpm_within_half_matches += usize::from(error <= 0.5);
                score.half_double_bpm_matches += usize::from(
                    error > 0.5
                        && ((predicted * 2.0 - reference).abs() <= 0.5
                            || (predicted / 2.0 - reference).abs() <= 0.5),
                );
            }
        }
        score.cues.reference_count += row.reference_cues.len();
        let predicted_cues = if row.failed {
            None
        } else {
            row.predicted_cues.as_deref()
        };
        if let Some(predicted) = predicted_cues {
            score.cues.tracks_with_prediction += 1;
            score.cues.predicted_count += predicted.len();
            score.cues.tracks_with_exact_count +=
                usize::from(predicted.len() == row.reference_cues.len());
            for (reference, predicted) in
                match_cues(&row.reference_cues, predicted, cue_tolerance_seconds)
            {
                score.cues.matched_count += 1;
                score.cues.matched_name_count += usize::from(reference.name == predicted.name);
                cue_error_sum += (reference.time_seconds - predicted.time_seconds).abs();
            }
        }
    }
    score.key_agreement = ratio(score.exact_key_matches, score.tracks);
    score.bpm_agreement_within_half = ratio(score.bpm_within_half_matches, score.bpm_references);
    if score.bpm_predictions > 0 {
        score.mean_absolute_bpm_error = Some(bpm_error_sum / score.bpm_predictions as f64);
    }
    score.cues.precision = ratio(score.cues.matched_count, score.cues.predicted_count);
    score.cues.recall = ratio(score.cues.matched_count, score.cues.reference_count);
    if score.cues.precision + score.cues.recall > 0.0 {
        score.cues.f1 = 2.0 * score.cues.precision * score.cues.recall
            / (score.cues.precision + score.cues.recall);
    }
    if score.cues.matched_count > 0 {
        score.cues.mean_absolute_error_seconds =
            Some(cue_error_sum / score.cues.matched_count as f64);
    }
    Ok(score)
}
