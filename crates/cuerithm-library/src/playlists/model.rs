pub use cuerithm_smartlists::{PlaylistKind, SmartDefinition};
use serde::{Deserialize, Serialize};
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PlaylistNode {
    pub id: String,
    pub parent_id: Option<String>,
    pub kind: PlaylistKind,
    pub name: String,
    pub position: u32,
    pub favorite_position: Option<u32>,
    pub definition: Option<SmartDefinition>,
    pub deleted: bool,
    pub deleted_by: Option<String>,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PlaylistState {
    pub revision: u64,
    pub nodes: Vec<PlaylistNode>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum PlaylistEdit {
    Create {
        id: Option<String>,
        parent_id: Option<String>,
        kind: PlaylistKind,
        name: String,
        definition: Option<SmartDefinition>,
    },
    Rename {
        id: String,
        name: String,
    },
    Move {
        id: String,
        parent_id: Option<String>,
        position: u32,
    },
    SetDefinition {
        id: String,
        definition: SmartDefinition,
    },
    Duplicate {
        id: String,
        parent_id: Option<String>,
        name: Option<String>,
    },
    Remove {
        id: String,
    },
    Restore {
        id: String,
        parent_id: Option<String>,
        position: u32,
    },
    Favorite {
        id: String,
        position: Option<u32>,
    },
    AddMembers {
        id: String,
        track_ids: Vec<String>,
    },
    RemoveMembers {
        id: String,
        track_ids: Vec<String>,
    },
    ReorderMembers {
        id: String,
        track_ids: Vec<String>,
    },
    ReplaceTrackRefs {
        old_ids: Vec<String>,
        keeper: String,
    },
    Batch {
        edits: Vec<PlaylistEdit>,
    },
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlaylistChange {
    pub edit_id: Option<String>,
    pub revision: u64,
    pub created_ids: Vec<String>,
    pub affected_ids: Vec<String>,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub(crate) struct StoredNode {
    pub last_edit_id: String,
    pub node: PlaylistNode,
    pub members: Vec<String>,
}
pub(crate) type Tree = std::collections::BTreeMap<String, StoredNode>;
pub(crate) type Delta = std::collections::BTreeMap<String, Option<StoredNode>>;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PlaylistPreferences {
    pub time_zone: Option<String>,
    pub key_mixing_mode: cuerithm_smartlists::KeyMixingMode,
}
impl Default for PlaylistPreferences {
    fn default() -> Self {
        Self {
            time_zone: None,
            key_mixing_mode: cuerithm_smartlists::KeyMixingMode::Harmonic,
        }
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlaylistPreferencesSnapshot {
    pub revision: u64,
    pub preferences: PlaylistPreferences,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SmartStamp {
    pub input_revision: u64,
    pub playlist_revision: u64,
    pub preference_revision: u64,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SmartState {
    Pending,
    Ready,
    Stale,
    Error,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SmartListStatus {
    pub playlist_id: String,
    pub state: SmartState,
    pub error: Option<String>,
    pub evaluated_at: Option<i64>,
    pub next_refresh: Option<i64>,
    pub last_good_revision: Option<u64>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SmartListFailure {
    pub playlist_id: String,
    pub message: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SmartRefresh {
    pub stamp: SmartStamp,
    pub evaluated: Vec<String>,
    pub unchanged: Vec<String>,
    pub errors: Vec<SmartListFailure>,
}
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct SmartPreviewScope {
    pub track_ids: Option<Vec<String>>,
    pub playlist_id: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SmartPreview {
    pub id: String,
    pub track_ids: Vec<String>,
    pub stamp: SmartStamp,
    pub created: i64,
    pub expires: Option<i64>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SmartTagChoice {
    pub id: String,
    pub category_id: String,
    pub name: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SmartCategoryChoice {
    pub id: String,
    pub name: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SmartFieldCatalog {
    pub catalog: cuerithm_smartlists::FieldCatalog,
    pub tags: Vec<SmartTagChoice>,
    pub categories: Vec<SmartCategoryChoice>,
    pub playlists: Vec<PlaylistNode>,
    pub stamp: SmartStamp,
}
/// Explicit facts from a future native DJ import, never inferred from cue labels.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NativePlaylistFacts {
    pub version: u32,
    pub source: String,
    pub cues: Vec<cuerithm_smartlists::SmartCue>,
    pub tempos: Vec<f64>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PlaylistCombineMode {
    Union,
    Intersection,
    AbsentFromAll,
}
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PlaylistSortOrder {
    NameAscending,
    NameDescending,
    TrackCountDescending,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum PlaylistPrefix {
    Number {
        start: u32,
        leading_zero: bool,
        replace_existing: bool,
    },
    Text {
        text: String,
    },
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum PlaylistToolRequest {
    Combine {
        source_ids: Vec<String>,
        parent_id: Option<String>,
        name: String,
        mode: PlaylistCombineMode,
    },
    SortSiblings {
        parent_id: Option<String>,
        ids: Option<Vec<String>>,
        order: PlaylistSortOrder,
    },
    Prefix {
        ids: Vec<String>,
        prefix: PlaylistPrefix,
    },
    Uppercase {
        ids: Vec<String>,
    },
    ReorderMembers {
        id: String,
        track_ids: Vec<String>,
    },
    Snapshot {
        source_id: String,
        parent_id: Option<String>,
        name: String,
    },
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlaylistPreviewItem {
    pub id: String,
    pub name: String,
    pub previous_name: Option<String>,
    pub kind: PlaylistKind,
    pub parent_id: Option<String>,
    pub position: u32,
    pub track_count: usize,
    pub definition: Option<SmartDefinition>,
    pub is_new: bool,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlaylistToolPreview {
    pub id: String,
    pub stamp: SmartStamp,
    pub created: i64,
    pub expires: Option<i64>,
    pub changes: Vec<PlaylistPreviewItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum GenerationTarget {
    Folder {
        id: String,
    },
    NewFolder {
        parent_id: Option<String>,
        name: String,
    },
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum GenerationDimension {
    Field {
        field: String,
        #[serde(default)]
        include_unobserved: bool,
    },
    Tags {
        tag_ids: Option<Vec<String>>,
    },
    Category {
        category_id: String,
    },
    Decades,
    BpmRanges,
    BitrateBuckets,
    SizeBuckets,
    DurationBuckets,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SmartGenerationRequest {
    pub target: GenerationTarget,
    pub dimensions: Vec<GenerationDimension>,
    #[serde(default)]
    pub combine: bool,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeneratedSmartList {
    pub id: String,
    pub parent_id: String,
    pub name: String,
    pub definition: SmartDefinition,
    pub track_count: usize,
    pub is_new: bool,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenerationPreview {
    pub id: String,
    pub stamp: SmartStamp,
    pub created: i64,
    pub expires: Option<i64>,
    pub folder_id: Option<String>,
    pub new_nodes: usize,
    pub items: Vec<GeneratedSmartList>,
}
