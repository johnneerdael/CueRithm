mod cache;
mod frozen;
mod service;
mod stages;
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
        include_str!("../../publication.rs"),
    ] {
        digest.update((source.len() as u64).to_le_bytes());
        digest.update(source);
    }
    format!("{:x}", digest.finalize())
}
