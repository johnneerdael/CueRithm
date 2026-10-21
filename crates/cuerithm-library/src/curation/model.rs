use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CurationGroupMode {
    Single,
    #[default]
    Multiple,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CurationValue {
    pub id: String,
    pub name: String,
    pub position: u32,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CurationGroup {
    pub id: String,
    pub name: String,
    pub mode: CurationGroupMode,
    pub position: u32,
    pub values: Vec<CurationValue>,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CurationContainer {
    Id3,
    Vorbis,
    Mp4,
    Asf,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CurationTextField {
    Genre,
    Style,
    Grouping,
    Label,
    Version,
    Composer,
    Lyricist,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CurationEncoding {
    Values,
    Json,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum CurationMappingSource {
    Group { group_id: String },
    Rating,
    Notes,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum CurationMappingTarget {
    Text {
        field: CurationTextField,
        encoding: CurationEncoding,
    },
    Comment {
        language: String,
        description: String,
        encoding: CurationEncoding,
    },
    Rating {
        owner: String,
    },
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CurationMapping {
    pub id: String,
    pub source: CurationMappingSource,
    pub target: CurationMappingTarget,
    pub containers: Vec<CurationContainer>,
    pub enabled: bool,
    pub allow_id3_upgrade: bool,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CurationCapabilities {
    pub native_available: bool,
    pub containers: Vec<CurationContainer>,
    pub text_fields: Vec<CurationTextField>,
    pub native_dj_reason: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CurationCatalog {
    pub revision: u64,
    pub groups: Vec<CurationGroup>,
    pub mappings: Vec<CurationMapping>,
    pub capabilities: CurationCapabilities,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum CurationTaxonomyChange {
    CreateGroup {
        name: String,
        mode: CurationGroupMode,
    },
    RenameGroup {
        id: String,
        name: String,
    },
    SetGroupMode {
        id: String,
        mode: CurationGroupMode,
    },
    DeleteGroup {
        id: String,
        replacement_id: Option<String>,
    },
    CreateValue {
        group_id: String,
        name: String,
    },
    RenameValue {
        id: String,
        name: String,
    },
    DeleteValue {
        id: String,
        replacement_id: Option<String>,
    },
    PutMapping {
        mapping: CurationMapping,
    },
    DeleteMapping {
        id: String,
    },
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CurationTaxonomyRequest {
    pub request_id: String,
    pub expected_revision: u64,
    pub change: CurationTaxonomyChange,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CurationTaxonomyReceipt {
    pub revision: u64,
    pub created_id: Option<String>,
    pub affected_tracks: u64,
    pub reexport_required: u64,
}
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct CurationValues {
    pub categories: BTreeMap<String, Vec<String>>,
    pub user_rating: Option<u8>,
    pub notes: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum CurationGroupEdit {
    Set {
        group_id: String,
        value_ids: Vec<String>,
    },
    Add {
        group_id: String,
        value_ids: Vec<String>,
    },
    Remove {
        group_id: String,
        value_ids: Vec<String>,
    },
    Clear {
        group_id: String,
    },
}
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum CurationValueEdit<T> {
    #[default]
    Keep,
    Set {
        value: T,
    },
    Clear,
}
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct CurationPatch {
    pub groups: Vec<CurationGroupEdit>,
    pub user_rating: CurationValueEdit<u8>,
    pub notes: CurationValueEdit<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CurationExpectedRevision {
    pub track_id: String,
    pub revision: u64,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CurationUpdateRequest {
    pub request_id: String,
    pub taxonomy_revision: u64,
    pub tracks: Vec<CurationExpectedRevision>,
    pub patch: CurationPatch,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CurationAccepted {
    pub track_id: String,
    pub revision: u64,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CurationReceipt {
    pub request_id: String,
    pub accepted: Vec<CurationAccepted>,
    pub library_revision: u64,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CurationTrackState {
    pub track_id: String,
    pub title: String,
    pub desired_revision: u64,
    pub acknowledged_revision: u64,
    pub desired: CurationValues,
    pub committed: CurationValues,
    pub state: String,
    pub error: Option<String>,
    pub active_publication_id: Option<String>,
    pub native_stale: bool,
    pub can_retry: bool,
    pub can_discard: bool,
    pub measured: serde_json::Value,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CurationSnapshot {
    pub taxonomy_revision: u64,
    pub library_revision: u64,
    pub tracks: Vec<CurationTrackState>,
    pub mixed: Vec<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CurationPending {
    pub track_id: String,
    pub title: String,
    pub desired_revision: u64,
    pub acknowledged_revision: u64,
    pub state: String,
    pub error: Option<String>,
    pub native_stale: bool,
    pub can_retry: bool,
    pub can_discard: bool,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CurationPendingPage {
    pub total: u64,
    pub offset: usize,
    pub limit: usize,
    pub rows: Vec<CurationPending>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CurationPublicationSummary {
    pub id: String,
    pub track_id: String,
    pub title: String,
    pub revision: u64,
    pub state: String,
    pub native_required: bool,
    pub created: i64,
    pub error: Option<String>,
    pub can_undo: bool,
    pub can_recover: bool,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CurationHistoryPage {
    pub total: u64,
    pub offset: usize,
    pub limit: usize,
    pub rows: Vec<CurationPublicationSummary>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CurationPublication {
    pub summary: CurationPublicationSummary,
    pub before: CurationValues,
    pub after: CurationValues,
    pub taxonomy_revision: u64,
    pub child_batch_id: Option<String>,
    pub undo_of: Option<String>,
}
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CurationFlushResult {
    pub processed: u64,
    pub pending: u64,
    pub failed: u64,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CurationReexportRequest {
    pub request_id: String,
    pub taxonomy_revision: u64,
    pub tracks: Vec<CurationExpectedRevision>,
}
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct CurationFilter {
    pub all_tags: Vec<String>,
    pub any_tags: Vec<String>,
    pub exclude_tags: Vec<String>,
    pub empty_groups: Vec<String>,
    pub user_rating_min: Option<u8>,
    pub user_rating_max: Option<u8>,
    pub notes: String,
}
