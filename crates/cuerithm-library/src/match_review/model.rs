use serde::{Deserialize, Serialize};
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MatchReviewSource {
    pub title: Option<String>,
    pub artists: Vec<String>,
    pub album: Option<String>,
    pub version: Option<String>,
    pub duration_seconds: f64,
    pub has_artwork: bool,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MatchReviewCandidate {
    pub id: String,
    pub provider: String,
    pub track_id: Option<String>,
    pub release_id: Option<String>,
    pub title: String,
    pub version: Option<String>,
    pub artists: Vec<String>,
    pub album: Option<String>,
    pub isrc: Option<String>,
    pub duration_seconds: Option<f64>,
    pub similarity: f64,
    pub identity_conflicts: Vec<String>,
    pub source_url: Option<String>,
    pub artwork_url: Option<String>,
    pub selectable: bool,
    pub reason: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MatchReviewProviderAccess {
    pub id: String,
    pub available: bool,
    pub requires_solver: bool,
    pub reason: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MatchReviewWarning {
    pub provider: String,
    pub kind: cuerithm_autotag::ProviderFailureKind,
    pub message: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MatchReviewState {
    pub track_id: String,
    pub job_id: Option<String>,
    pub attempt: u32,
    pub generation: String,
    pub source_sha256: String,
    pub settings_revision: String,
    pub status: String,
    pub track_state: String,
    pub can_keep_existing_once: bool,
    pub source: MatchReviewSource,
    pub candidates: Vec<MatchReviewCandidate>,
    pub truncated: bool,
    pub can_requery: bool,
    pub reason: Option<String>,
    #[serde(default)]
    pub providers: Vec<MatchReviewProviderAccess>,
    #[serde(default)]
    pub warnings: Vec<MatchReviewWarning>,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MatchReviewAction {
    UseSelected,
    KeepExistingOnce,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MatchReviewFieldAction {
    Keep,
    Replace,
    Append,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MatchReviewFieldDecision {
    pub field: String,
    pub action: MatchReviewFieldAction,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MatchReviewRequest {
    pub track_id: String,
    pub generation: String,
    pub action: MatchReviewAction,
    pub candidate_id: Option<String>,
    #[serde(default)]
    pub fields: Vec<MatchReviewFieldDecision>,
    #[serde(default)]
    pub acknowledge_conflicts: bool,
    #[serde(default)]
    pub allow_id3_upgrade: bool,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MatchReviewFieldDiff {
    pub field: String,
    pub label: String,
    pub before: Vec<String>,
    pub proposed: Vec<String>,
    pub after: Vec<String>,
    pub action: MatchReviewFieldAction,
    pub allowed_actions: Vec<MatchReviewFieldAction>,
    pub reason: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MatchReviewArtwork {
    pub existing: bool,
    pub candidate_url: Option<String>,
    pub action: MatchReviewFieldAction,
    pub reason: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MatchReviewPreview {
    pub id: String,
    pub revision: String,
    pub track_id: String,
    pub job_id: Option<String>,
    pub attempt: u32,
    pub generation: String,
    pub action: MatchReviewAction,
    pub candidate: Option<MatchReviewCandidate>,
    pub fields: Vec<MatchReviewFieldDiff>,
    pub artwork: MatchReviewArtwork,
    pub requires_id3_upgrade: bool,
    pub can_apply: bool,
    pub blockers: Vec<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MatchReviewApplyResult {
    pub state: String,
    pub job_id: Option<String>,
    pub metadata_batch: Option<crate::MetadataEditBatch>,
}
