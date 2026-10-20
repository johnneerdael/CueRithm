use super::MaintenanceFileState;
use crate::model::bytes_path;
use anyhow::{Context, Result};
use cuerithm_media::{CancellationToken, file_sha256_cancellable};
use serde::{Deserialize, Serialize};
use std::{
    path::{Path, PathBuf},
    sync::{
        Mutex,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
};
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
fn proof_current(proof: &FileProof, cancel: &CancellationToken) -> Result<bool> {
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
    Ok(true)
}
/// Check every proof, hashing whole files, on the preparation service's worker pool (75% of the logical cores).
/// Stops at the first proof that no longer holds. Only cancellation or an unreadable stored path is an error.
pub(super) fn proofs_current(files: &[FileProof], cancel: &CancellationToken) -> Result<bool> {
    let workers =
        crate::prepare::worker_count(std::thread::available_parallelism().map_or(1, |n| n.get()))
            .min(files.len().max(1));
    let next = AtomicUsize::new(0);
    let stop = AtomicBool::new(false);
    let current = AtomicBool::new(true);
    let error: Mutex<Option<anyhow::Error>> = Mutex::new(None);
    std::thread::scope(|scope| {
        for _ in 0..workers {
            scope.spawn(|| {
                while !stop.load(Ordering::SeqCst) {
                    let Some(proof) = files.get(next.fetch_add(1, Ordering::SeqCst)) else {
                        break;
                    };
                    match proof_current(proof, cancel) {
                        Ok(true) => {}
                        Ok(false) => {
                            current.store(false, Ordering::SeqCst);
                            stop.store(true, Ordering::SeqCst);
                        }
                        Err(e) => {
                            error.lock().unwrap().get_or_insert(e);
                            stop.store(true, Ordering::SeqCst);
                        }
                    }
                }
            });
        }
    });
    if let Some(e) = error.into_inner().unwrap() {
        return Err(e);
    }
    cancel.check()?;
    Ok(current.load(Ordering::SeqCst))
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
