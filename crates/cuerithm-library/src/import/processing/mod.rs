mod cache;
mod frozen;
mod optional;
mod service;
mod stages;
#[cfg(test)]
pub(super) fn planned_stage_blockers(
    policy: &ImportPolicy,
    fields: &BTreeMap<String, Value>,
) -> Vec<(String, Option<String>)> {
    stages::plan(policy, fields)
        .into_iter()
        .map(|s| (s.name.into(), s.blocker))
        .collect()
}
use super::*;
use crate::{Library, model::now};
use anyhow::{Context, Result, ensure};
use cuerithm_dj_sync::import::*;
use cuerithm_media::{CancellationToken, MediaRuntime};
pub(crate) use frozen::{FrozenProcessing, freeze};
use rusqlite::{OptionalExtension, params};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
pub use service::ImportProcessingService;
use std::{
    collections::BTreeMap,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
};

pub(super) fn has_stages(policy: &ImportPolicy, fields: &BTreeMap<String, Value>) -> bool {
    !stages::plan(policy, fields).is_empty()
}

pub(crate) fn implementation_sha256() -> String {
    use sha2::{Digest, Sha256};
    let mut digest = Sha256::new();
    for source in [
        include_str!("mod.rs"),
        include_str!("frozen.rs"),
        include_str!("stages.rs"),
        include_str!("service.rs"),
        include_str!("cache.rs"),
        include_str!("optional.rs"),
        include_str!("../publication.rs"),
        include_str!("../publication/files.rs"),
        include_str!("../recovery.rs"),
        include_str!("../../metadata_edit/service.rs"),
        include_str!("../../metadata_edit/journal.rs"),
        include_str!("../../organization/service/planning.rs"),
        include_str!("../../organization/service/capture.rs"),
        include_str!("../../organization/service/batches.rs"),
        include_str!("../../organization_references.rs"),
        include_str!("../../publication.rs"),
    ] {
        digest.update((source.len() as u64).to_le_bytes());
        digest.update(source);
    }
    format!("{:x}", digest.finalize())
}
pub(super) fn validate_effect(
    frozen: &FrozenProcessing,
    stage: &str,
    cancel: &CancellationToken,
) -> Result<bool> {
    let selected = camelot_core::AnalysisSelection {
        key: stage == "analysis.key",
        bpm: stage == "analysis.bpm",
        beatgrid: stage == "analysis.beatgrid",
        energy: stage == "analysis.energy",
        cues: stage == "generation.cues",
    };
    frozen.validate_models(selected, cancel)?;
    Ok(selected.any())
}
