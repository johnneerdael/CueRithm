use super::MaintenanceFileState;
use crate::model::bytes_path;
use anyhow::{Context, Result};
use cuerithm_media::{CancellationToken, file_sha256_cancellable};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(super) struct FileProof {
    pub path: Vec<u8>,
    pub state: MaintenanceFileState,
    pub hash: Option<String>,
}
pub(super) fn resolved_prefix(path: &Path) -> Result<PathBuf> {
    for ancestor in path.ancestors() {
        match ancestor.canonicalize() {
            Ok(mut base) => {
                for part in path.strip_prefix(ancestor)?.components() {
                    match part {
                        std::path::Component::Normal(name) => base.push(name),
                        std::path::Component::ParentDir => {
                            base.pop();
                        }
                        std::path::Component::CurDir => {}
                        _ => anyhow::bail!("Invalid ignored-folder suffix"),
                    }
                }
                return Ok(base);
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => continue,
            Err(e) => return Err(e).context("Cannot resolve ignored folder"),
        }
    }
    anyhow::bail!("Cannot resolve ignored folder root")
}
// A missing parent may be disconnected storage rather than a removed file.
pub(super) fn file_state(path: &Path) -> MaintenanceFileState {
    match std::fs::symlink_metadata(path) {
        Ok(m) if m.file_type().is_symlink() => MaintenanceFileState::Symlink,
        Ok(m) if m.is_file() => MaintenanceFileState::Present,
        Ok(_) => MaintenanceFileState::NotRegular,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            if path
                .parent()
                .is_some_and(|p| std::fs::metadata(p).is_ok_and(|m| m.is_dir()))
            {
                MaintenanceFileState::Missing
            } else {
                MaintenanceFileState::Unavailable
            }
        }
        Err(_) => MaintenanceFileState::Unavailable,
    }
}
pub(super) fn proofs_current(files: &[FileProof], cancel: &CancellationToken) -> Result<bool> {
    for proof in files {
        cancel.check()?;
        let path = bytes_path(&proof.path)?;
        if file_state(&path) != proof.state {
            return Ok(false);
        }
        if proof.state == MaintenanceFileState::Present
            && proof.hash.is_none()
            && std::fs::File::open(&path).is_ok()
        {
            return Ok(false);
        }
        if let Some(hash) = &proof.hash {
            let observed = file_sha256_cancellable(&path, cancel);
            cancel.check()?;
            if observed.as_ref().ok() != Some(hash) {
                return Ok(false);
            }
        }
    }
    Ok(true)
}

pub(super) fn decode_error_state(error: &anyhow::Error, expected: &str) -> MaintenanceFileState {
    if let Some(unsupported) = error.downcast_ref::<cuerithm_media::UnsupportedAudio>() {
        if unsupported.source_sha256 != expected {
            MaintenanceFileState::Changed
        } else {
            MaintenanceFileState::Unsupported
        }
    } else if error.downcast_ref::<cuerithm_media::MediaTaskError>()
        == Some(&cuerithm_media::MediaTaskError::TimedOut)
    {
        MaintenanceFileState::TimedOut
    } else {
        MaintenanceFileState::DecodeError
    }
}
