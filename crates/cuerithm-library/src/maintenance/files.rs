use super::MaintenanceFileState;
use crate::model::bytes_path;
use anyhow::{Context, Result};
use cuerithm_media::{CancellationToken, file_sha256_cancellable};
use rusqlite::{Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(super) struct FileProof {
    pub path: Vec<u8>,
    pub state: MaintenanceFileState,
    pub hash: Option<String>,
}
/// A file as it was when its hash was taken: the hash, and what identifies the physical file and
/// its state, read from the same open file the hash came from. The Trash and permanent deletion
/// act on a path, so before they run the path is checked against this: it must still lead to this
/// very file (same device and inode, or the Windows file id), with the same size and modification
/// time. A file swapped in under the same name is another file and fails the check.
pub(super) struct HashedFile {
    pub hash: String,
    handle: same_file::Handle,
    size: u64,
    modified: Option<std::time::SystemTime>,
}
impl HashedFile {
    pub(super) fn open(path: &Path) -> std::io::Result<Self> {
        use sha2::{Digest, Sha256};
        use std::io::Read;
        let mut file = std::fs::File::open(path)?;
        let before = file.metadata()?;
        let handle = same_file::Handle::from_file(file.try_clone()?)?;
        let mut hasher = Sha256::new();
        let mut buffer = vec![0u8; 1 << 16];
        loop {
            let n = file.read(&mut buffer)?;
            if n == 0 {
                break;
            }
            hasher.update(&buffer[..n]);
        }
        let after = file.metadata()?;
        if before.len() != after.len() || before.modified().ok() != after.modified().ok() {
            return Err(std::io::Error::other("The file changed while it was read"));
        }
        Ok(Self {
            hash: format!("{:x}", hasher.finalize()),
            handle,
            size: before.len(),
            modified: before.modified().ok(),
        })
    }
    pub(super) fn same_file_as(&self, other: &Self) -> bool {
        self.handle == other.handle
    }
    /// The path still leads to the file that was hashed, as a regular file, unchanged.
    pub(super) fn still_at(&self, path: &Path) -> bool {
        current_state(path).is_available()
            && same_file::Handle::from_path(path).is_ok_and(|now| now == self.handle)
            && std::fs::metadata(path)
                .is_ok_and(|meta| meta.len() == self.size && meta.modified().ok() == self.modified)
    }
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

/// The availability of the file behind a recorded path right now: the scans' own file-state
/// classification, plus a readability check, so a file that exists but cannot be opened is
/// not treated as available.
pub(super) fn current_state(path: &Path) -> MaintenanceFileState {
    match file_state(path) {
        MaintenanceFileState::Present if std::fs::File::open(path).is_err() => {
            MaintenanceFileState::Unavailable
        }
        state => state,
    }
}
/// The reason for an unavailable file, in the words Missing & broken and Duplicates share.
pub(super) fn state_phrase(state: MaintenanceFileState) -> &'static str {
    match state {
        MaintenanceFileState::Missing => "file not found",
        MaintenanceFileState::Unavailable => "location not available",
        _ => "file cannot be read",
    }
}
/// Refuse to go on when any of the tracks no longer has an available audio file. Nothing has
/// been changed when this fails, so the message says so.
pub(super) fn ensure_available(conn: &Connection, ids: &BTreeSet<String>) -> Result<()> {
    let mut problems = vec![];
    for id in ids {
        let row: Option<(Option<Vec<u8>>, String, String)> = conn
            .query_row(
                "SELECT path,artist,title FROM tracks WHERE id=?1",
                [id],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .optional()?;
        match row {
            None => problems.push("a track that is no longer in the library".to_owned()),
            // A track from a portable restore may not be linked to a local file yet.
            Some((None, artist, title)) => {
                problems.push(format!("{artist} - {title} (not linked to a local file)"));
            }
            Some((Some(path), artist, title)) => {
                let state = current_state(&bytes_path(&path)?);
                if !state.is_available() {
                    problems.push(format!("{artist} - {title} ({})", state_phrase(state)));
                }
            }
        }
    }
    if problems.is_empty() {
        return Ok(());
    }
    let shown = problems
        .iter()
        .take(3)
        .cloned()
        .collect::<Vec<_>>()
        .join(", ");
    let more = problems.len().saturating_sub(3);
    anyhow::bail!(
        "Cannot merge: {} no longer {} an available audio file: {shown}{}. Nothing was changed. Scan again, and review these tracks in Missing & broken.",
        if problems.len() == 1 {
            "1 track".to_owned()
        } else {
            format!("{} tracks", problems.len())
        },
        if problems.len() == 1 { "has" } else { "have" },
        if more > 0 {
            format!(" and {more} more")
        } else {
            String::new()
        }
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_hashed_file_is_recognised_only_while_the_path_leads_to_the_same_unchanged_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("a.wav");
        std::fs::write(&path, b"the audio bytes").unwrap();
        let hashed = HashedFile::open(&path).unwrap();
        assert_eq!(hashed.hash.len(), 64);
        assert!(hashed.still_at(&path));
        // Another file of the same size and time under the same name is not the hashed file.
        let modified = std::fs::metadata(&path).unwrap().modified().unwrap();
        let other = dir.path().join("other.tmp");
        std::fs::write(&other, b"different bytes").unwrap();
        std::fs::File::options()
            .write(true)
            .open(&other)
            .unwrap()
            .set_modified(modified)
            .unwrap();
        std::fs::rename(&other, &path).unwrap();
        assert!(!hashed.still_at(&path));
        // A file that is edited in place, or gone, is not it either.
        let again = HashedFile::open(&path).unwrap();
        assert!(again.still_at(&path));
        std::fs::write(&path, b"grown bytes in place").unwrap();
        assert!(!again.still_at(&path));
        std::fs::remove_file(&path).unwrap();
        assert!(!again.still_at(&path));
        // The same file under a second name is the same file.
        let linked = dir.path().join("b.wav");
        std::fs::write(&path, b"x").unwrap();
        std::fs::hard_link(&path, &linked).unwrap();
        let (first, second) = (
            HashedFile::open(&path).unwrap(),
            HashedFile::open(&linked).unwrap(),
        );
        assert!(first.same_file_as(&second));
    }
}
