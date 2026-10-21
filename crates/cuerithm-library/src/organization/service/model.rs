use serde::{Deserialize, Serialize};
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OrganizationSettingMode {
    #[default]
    LeaveInPlace,
    RenameInPlace,
    Move,
    Copy,
}
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OrganizationMissingPolicy {
    #[default]
    Block,
    OmitOptional,
    SkipOptional,
}
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OrganizationCollisionPolicy {
    #[default]
    Block,
    Skip,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct OrganizationSettings {
    pub mode: OrganizationSettingMode,
    pub pattern: String,
    pub missing: OrganizationMissingPolicy,
    pub collision: OrganizationCollisionPolicy,
    pub organization_optional: bool,
    pub destination_root_id: Option<String>,
}
impl Default for OrganizationSettings {
    fn default() -> Self {
        Self {
            mode: Default::default(),
            pattern: "{artist} - {title}".into(),
            missing: Default::default(),
            collision: Default::default(),
            organization_optional: false,
            destination_root_id: None,
        }
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrganizationRoot {
    pub id: String,
    pub display_path: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum OrganizationScope {
    Tracks { track_ids: Vec<String> },
    Playlist { playlist_id: String },
    Folder { root_id: String, recursive: bool },
    AllReady,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrganizationPreviewRequest {
    pub scope: OrganizationScope,
    pub settings: OrganizationSettings,
}
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct OrganizationPreviewTotals {
    pub total: usize,
    pub ready: usize,
    pub unchanged: usize,
    pub skipped: usize,
    pub blocked: usize,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrganizationPreview {
    pub id: String,
    pub revision: String,
    pub created: i64,
    pub totals: OrganizationPreviewTotals,
    pub can_apply: bool,
    #[serde(default)]
    pub estimated_bytes: Option<u64>,
    #[serde(default)]
    pub available_bytes: Option<u64>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrganizationRow {
    pub id: String,
    pub track_id: String,
    pub title: String,
    pub before_path: String,
    pub after_path: Option<String>,
    pub outcome: String,
    pub reason: Option<String>,
    pub operation_id: Option<String>,
    pub references: Vec<crate::OrganizationReferenceInfo>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrganizationPage {
    pub rows: Vec<OrganizationRow>,
    pub total: usize,
    pub offset: usize,
    pub limit: usize,
}
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct OrganizationBatchTotals {
    pub total: usize,
    pub completed: usize,
    pub unchanged: usize,
    pub skipped: usize,
    pub blocked: usize,
    pub failed: usize,
    pub recovery_required: usize,
    pub cancelled: usize,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrganizationBatch {
    pub id: String,
    pub preview_id: Option<String>,
    pub kind: String,
    pub state: String,
    pub created: i64,
    pub totals: OrganizationBatchTotals,
    pub can_undo: bool,
    pub can_recover: bool,
    #[serde(default)]
    pub resume_job_id: Option<String>,
    pub error: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrganizationExample {
    pub valid: bool,
    pub error: Option<String>,
    pub tokens: Vec<String>,
    pub examples: Vec<OrganizationExampleRow>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrganizationExampleRow {
    pub label: String,
    pub original_name: String,
    pub result: Option<String>,
    pub outcome: String,
    pub reason: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrganizationAlternate {
    pub operation_id: String,
    pub track_id: String,
    pub display_path: String,
    pub captured_sha256: String,
    pub state: String,
    pub observed_sha256: Option<String>,
    pub ignored_sha256: Option<String>,
}
pub type OrganizationAlternateResolution = super::super::AlternateResolution;
