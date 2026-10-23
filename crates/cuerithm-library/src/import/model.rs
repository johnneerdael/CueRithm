use cuerithm_dj_sync::import::*;
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, path::PathBuf, sync::Arc};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportSourceSelection {
    #[serde(with = "cuerithm_dj_sync::path_serde")]
    pub root: PathBuf,
    pub artifacts: Vec<ArtifactRequest>,
}
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(tag = "choice", rename_all = "snake_case", deny_unknown_fields)]
pub enum SourceBindingChoice {
    #[default]
    Automatic,
    Replica {
        source_id: String,
    },
    NewLibrary,
}
/// Backend request; desktop commands resolve native source-authority IDs into selection.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ImportInspectRequest {
    pub provider: SourceProvider,
    pub selection: ImportSourceSelection,
    pub binding: SourceBindingChoice,
    pub origin: Option<ClockIdentity>,
    pub path_mappings: Vec<PathMapping>,
    pub capture_file_tags: bool,
    pub include_history: bool,
    pub include_queue: bool,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportInspection {
    pub inspection_id: String,
    pub source_id: String,
    pub profile: SourceProfile,
    pub header: ObservationHeader,
    pub counts: ValidationReport,
}
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ImportRecordKind {
    #[default]
    Track,
    Collection,
    Membership,
    Issue,
}
impl ImportRecordKind {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Track => "track",
            Self::Collection => "collection",
            Self::Membership => "membership",
            Self::Issue => "issue",
        }
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ImportPageQuery {
    pub kind: ImportRecordKind,
    pub limit: u32,
    pub cursor: Option<String>,
}
impl Default for ImportPageQuery {
    fn default() -> Self {
        Self {
            kind: Default::default(),
            limit: 100,
            cursor: None,
        }
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportPage {
    pub records: Vec<ObservationRecord>,
    pub total: u64,
    pub next_cursor: Option<String>,
}
pub(crate) struct ImportRuntime {
    pub nonce: String,
    pub inspections: BTreeMap<String, Arc<ImportSnapshot>>,
    pub processing_gate: Arc<std::sync::Mutex<()>>,
    pub decoder_identity: Option<String>,
    pub media_runtime: Option<cuerithm_media::MediaRuntime>,
}
impl Default for ImportRuntime {
    fn default() -> Self {
        Self {
            nonce: uuid::Uuid::new_v4().to_string(),
            inspections: BTreeMap::new(),
            processing_gate: Arc::new(std::sync::Mutex::new(())),
            decoder_identity: None,
            media_runtime: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportedItemState {
    pub source_ids: Vec<String>,
    pub availability: String,
    pub analysis_state: String,
    pub playback_allowed: bool,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportedTrackObservation {
    pub source_id: String,
    pub external_id: String,
    pub snapshot_id: String,
    pub track: TrackObservation,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportedItemDetail {
    pub id: String,
    pub active_fields: BTreeMap<String, serde_json::Value>,
    pub provenance: BTreeMap<String, serde_json::Value>,
    pub observations: Vec<ImportedTrackObservation>,
    pub availability: String,
    pub analysis_state: String,
    pub playback_allowed: bool,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportExportReadiness {
    pub ready: bool,
    pub reasons: Vec<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportedOccurrence {
    pub occurrence_id: String,
    pub item_id: Option<String>,
    pub source_track_id: String,
    pub source_position: Option<i64>,
    pub position: u64,
    pub track: Option<crate::TrackSummary>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportedCollectionPage {
    pub id: String,
    pub ordered: bool,
    pub kind: CollectionKind,
    pub rows: Vec<ImportedOccurrence>,
    pub total: u64,
    pub next_cursor: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(tag = "selection", content = "ids", rename_all = "snake_case")]
pub enum ImportRecordSelection {
    #[default]
    All,
    Ids(std::collections::BTreeSet<String>),
}
impl ImportRecordSelection {
    pub(crate) fn contains(&self, id: &str) -> bool {
        match self {
            Self::All => true,
            Self::Ids(ids) => ids.contains(id),
        }
    }
}
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(
    tag = "choice",
    content = "value",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum ImportDecision {
    #[default]
    KeepLocal,
    UseSource,
    Custom(serde_json::Value),
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ImportConflictDecision {
    pub external_id: String,
    pub field: String,
    pub decision: ImportDecision,
}
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ImportPreviewRequest {
    pub inspection_id: String,
    pub tracks: ImportRecordSelection,
    pub collections: ImportRecordSelection,
    pub policy: ImportPolicy,
    /// Explicit reviewed identity assignments; an established source link cannot be reassigned.
    pub target_items: BTreeMap<String, String>,
    pub decisions: Vec<ImportConflictDecision>,
}
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ImportPreview {
    pub preview_id: String,
    pub tracks_created: u64,
    pub fields_changed: u64,
    pub baselines_changed: u64,
    pub collections_changed: u64,
    pub conflicts: u64,
    pub blocked: u64,
    pub processing_requested: bool,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportFieldEffect {
    pub field: String,
    pub source: Option<serde_json::Value>,
    pub local: Option<serde_json::Value>,
    pub proposed: Option<serde_json::Value>,
    pub conflict: bool,
    pub blocked_reason: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportPreviewEffect {
    pub external_id: String,
    pub item_id: String,
    pub kind: String,
    pub fields: Vec<ImportFieldEffect>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportPreviewPage {
    pub effects: Vec<ImportPreviewEffect>,
    pub total: u64,
    pub next_cursor: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportApplyResult {
    pub operation_id: String,
    pub tracks_created: u64,
    pub fields_changed: u64,
    pub collections_changed: u64,
    pub processing_requested: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ImportProcessingSummary {
    pub processed: u64,
    pub reused: u64,
    pub blocked: u64,
    pub failed: u64,
    pub cancelled: u64,
    pub recovery_required: u64,
    pub unchanged: u64,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportProcessingStage {
    pub operation_id: String,
    pub item_id: String,
    pub stage: String,
    pub attempt: String,
    pub state: String,
    pub identity: serde_json::Value,
    pub proposal: Option<serde_json::Value>,
    pub error: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportProcessingStagePage {
    pub records: Vec<ImportProcessingStage>,
    pub total: u64,
    pub next_cursor: Option<String>,
}
