use anyhow::{Result, ensure};
use cuerithm_dj_sync::import::RecordKey;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IdentityCandidate {
    pub item_id: String,
    pub source_link: Option<RecordKey>,
    #[serde(with = "cuerithm_dj_sync::path_serde::option")]
    pub canonical_path: Option<PathBuf>,
    pub volume_id: Option<String>,
    pub content_sha256: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IdentityEvidence {
    pub established_item_id: Option<String>,
    #[serde(with = "cuerithm_dj_sync::path_serde::option")]
    pub canonical_path: Option<PathBuf>,
    pub volume_id: Option<String>,
    pub content_sha256: Option<String>,
    pub allow_content_matching: bool,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "match", content = "items", rename_all = "snake_case")]
pub enum IdentityResolution {
    Established(String),
    ExactPath(String),
    ContentCandidates(Vec<String>),
    Ambiguous(Vec<String>),
    Unmatched,
}
pub fn resolve_import_identity(
    incoming: &IdentityEvidence,
    candidates: &[IdentityCandidate],
) -> Result<IdentityResolution> {
    if let Some(id) = &incoming.established_item_id {
        ensure!(!id.is_empty(), "Empty established source link");
        return Ok(IdentityResolution::Established(id.clone()));
    }
    let paths: Vec<_> = candidates
        .iter()
        .filter(|c| {
            incoming.canonical_path.is_some()
                && c.canonical_path == incoming.canonical_path
                && incoming.volume_id.is_some()
                && c.volume_id == incoming.volume_id
        })
        .map(|c| c.item_id.clone())
        .collect();
    if paths.len() == 1 {
        return Ok(IdentityResolution::ExactPath(paths[0].clone()));
    }
    if paths.len() > 1 {
        return Ok(IdentityResolution::Ambiguous(paths));
    }
    if incoming.allow_content_matching
        && let Some(hash) = &incoming.content_sha256
    {
        ensure!(
            hash.len() == 64 && hash.bytes().all(|b| b.is_ascii_hexdigit()),
            "Invalid verified content identity"
        );
        let ids = candidates
            .iter()
            .filter(|c| c.content_sha256.as_ref() == Some(hash))
            .map(|c| c.item_id.clone())
            .collect::<Vec<_>>();
        if !ids.is_empty() {
            return Ok(IdentityResolution::ContentCandidates(ids));
        }
    }
    Ok(IdentityResolution::Unmatched)
}
