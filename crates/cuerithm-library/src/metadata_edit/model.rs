use cuerithm_autotag::{ArtworkIdentity, MetadataPatch, MetadataSnapshot};
use serde::{Deserialize, Serialize};
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetadataEditTrack {
    pub track_id: String,
    pub title: String,
    pub display_path: String,
    pub state: String,
    pub source_sha256: String,
    pub path_revision: u64,
    pub metadata_revision: u64,
    pub native: MetadataSnapshot,
    pub measured: serde_json::Value,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetadataEditorSnapshot {
    pub tracks: Vec<MetadataEditTrack>,
    pub mixed: Vec<String>,
}
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct MetadataEditorPatch {
    pub fields: MetadataPatch,
    pub artwork: Vec<MetadataArtworkChoice>,
    pub form_values: std::collections::BTreeMap<String, String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetadataArtworkChoice {
    pub target: Option<ArtworkIdentity>,
    pub asset_id: Option<String>,
    pub kind: Option<u32>,
    pub description: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetadataArtworkAsset {
    pub id: String,
    pub mime: String,
    pub sha256: String,
    pub bytes: u64,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetadataDraft {
    pub id: String,
    pub revision: u64,
    pub track_ids: Vec<String>,
    pub patch: MetadataEditorPatch,
    pub state: String,
    pub created: i64,
    pub updated: i64,
    pub error: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetadataFieldOutcome {
    pub field: String,
    pub status: String,
    pub reason: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetadataEditRow {
    pub track_id: String,
    pub title: String,
    pub before: MetadataSnapshot,
    pub after: MetadataSnapshot,
    pub outcome: String,
    pub fields: Vec<MetadataFieldOutcome>,
    pub error: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetadataEditPreview {
    pub id: String,
    pub draft_id: String,
    pub draft_revision: u64,
    pub created: i64,
    pub rows: Vec<MetadataEditRow>,
    pub can_apply: bool,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetadataEditBatch {
    pub id: String,
    pub draft_id: String,
    pub draft_revision: u64,
    pub state: String,
    pub rows: Vec<MetadataEditRow>,
    pub created: i64,
    pub can_undo: bool,
    pub can_recover: bool,
    pub error: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct MetadataEditSummary {
    pub id: String,
    pub draft_id: String,
    pub draft_revision: u64,
    pub state: String,
    pub created: i64,
    pub can_undo: bool,
    pub can_recover: bool,
    pub error: Option<String>,
    pub counts: std::collections::BTreeMap<String, u64>,
}
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct MetadataEditHistoryPage {
    pub total: usize,
    pub offset: usize,
    pub limit: usize,
    pub batches: Vec<MetadataEditSummary>,
}
