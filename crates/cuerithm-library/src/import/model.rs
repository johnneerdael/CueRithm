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
#[derive(Debug)]
pub(crate) struct ImportRuntime {
    pub nonce: String,
    pub inspections: BTreeMap<String, Arc<ImportSnapshot>>,
}
impl Default for ImportRuntime {
    fn default() -> Self {
        Self {
            nonce: uuid::Uuid::new_v4().to_string(),
            inspections: BTreeMap::new(),
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
