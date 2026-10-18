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
