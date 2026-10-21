//! Internal recoverable file organization, intentionally not exposed through
//! desktop commands. Publication pauses for explicit local/native-reference
//! acknowledgement; this engine never marks an Incoming track Ready.
mod coordinator;
mod copy_conflicts;
pub(crate) use copy_conflicts::OrganizationReferenceSourceProof;
#[cfg(test)]
mod copy_conflicts_tests;
mod journal;
mod native;
mod publish;
mod recovery;
pub(crate) mod service;
use crate::Library;
pub use journal::{OrganizationFile, OrganizationPhase, OrganizationRecord};
pub use native::{Authority, Fingerprint, Identity, NativePath};
pub use recovery::{AlternateResolution, AlternateSourceRecord, AlternateSourceState};
use serde::{Deserialize, Serialize};
pub use service::*;
use std::path::PathBuf;
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OrganizationMode {
    Rename,
    Move,
    Copy,
}
#[derive(Debug, Clone)]
pub struct OrganizationRequest {
    pub track_id: String,
    pub mode: OrganizationMode,
    pub source_root: PathBuf,
    pub destination_root: PathBuf,
    /// Audio first, then exclusively owned sidecars. Every pair is explicit;
    /// unknown neighboring files are never inferred to belong to the track.
    pub files: Vec<(PathBuf, PathBuf)>,
}
#[cfg(test)]
mod tests;
