//! Local proof of playlist identity. These records are never written into a DJ library.
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PlaylistBinding {
    pub source_id: String,
    /// Native identity, scoped by the adapter's database identity.
    pub locator: String,
    pub parent_source_id: Option<String>,
    /// True only when before/after staging proves this export created the object.
    pub created: bool,
    pub fingerprint: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PlaylistOwnership {
    pub identity: String,
    pub bindings: Vec<PlaylistBinding>,
}

#[derive(Debug, Clone, Default)]
pub struct PlaylistOwnershipRequest {
    pub previous: PlaylistOwnership,
    /// Explicit removals derived from the authoritative complete source tree.
    pub removed: BTreeSet<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PlaylistOwnershipReport {
    pub current: PlaylistOwnership,
    pub deleted: BTreeSet<String>,
    /// A matched external object is retained when its source playlist is removed.
    pub retired: BTreeSet<String>,
}

pub fn fingerprint(value: &impl Serialize) -> anyhow::Result<String> {
    use sha2::{Digest, Sha256};
    Ok(format!("{:x}", Sha256::digest(serde_json::to_vec(value)?)))
}
