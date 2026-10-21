//! Internal recoverable file organization. Desktop commands use the guarded
//! service rather than raw coordinator acknowledgements. Publication pauses for local/native-reference
//! acknowledgement; this engine never marks an Incoming track Ready.
mod coordinator;
mod copy_conflicts;
pub(crate) use copy_conflicts::OrganizationReferenceSourceProof;
#[cfg(test)]
mod copy_conflicts_tests;
mod journal;
mod native;
#[cfg(feature = "autotag")]
pub(crate) use native::{Parent as NativeParent, matches as same_native_file};
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

/// Capture a no-follow native source identity for a higher-level frozen preview.
#[cfg(feature = "autotag")]
pub(crate) fn capture_regular_source(
    path: &std::path::Path,
) -> anyhow::Result<(Authority, Fingerprint)> {
    use anyhow::Context;
    let (parent, authority) =
        native::Parent::capture(path.parent().context("Missing source parent")?)?;
    let fingerprint = parent.fingerprint(std::path::Path::new(
        path.file_name().context("Missing source name")?,
    ))?;
    Ok((authority, fingerprint))
}
