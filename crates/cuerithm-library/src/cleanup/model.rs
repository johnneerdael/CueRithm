use cuerithm_cleanup::CleanupReport;
use serde::{Deserialize, Serialize};
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", content = "track_ids", rename_all = "snake_case")]
pub enum CleanupScope {
    AllReady,
    TrackIds(Vec<String>),
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CleanupProposal {
    pub id: String,
    pub track_id: String,
    pub title: String,
    pub report: CleanupReport,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CleanupFailure {
    pub track_id: String,
    pub message: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CleanupPreview {
    pub id: String,
    pub created: i64,
    pub proposals: Vec<CleanupProposal>,
    pub unchanged: Vec<String>,
    pub failed: Vec<CleanupFailure>,
}
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CleanupBatchResult {
    pub id: String,
    pub applied: Vec<String>,
    pub unchanged: Vec<String>,
    pub conflicted: Vec<CleanupFailure>,
    pub failed: Vec<CleanupFailure>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CleanupHistory {
    pub can_undo: bool,
    pub id: String,
    pub kind: String,
    pub state: String,
    pub created: i64,
    pub result: CleanupBatchResult,
}
