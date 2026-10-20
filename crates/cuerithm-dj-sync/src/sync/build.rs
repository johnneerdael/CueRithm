//! Adapter scratch space is disposable; transaction staged/ and backup/ are not.
use anyhow::{Context, Result};
use std::{fs, path::Path};

pub(super) struct BuildDirectory(tempfile::TempDir);
impl BuildDirectory {
    pub(super) fn new(journal: &Path) -> Result<Self> {
        Ok(Self(
            tempfile::Builder::new()
                .prefix("build-")
                .tempdir_in(journal)?,
        ))
    }
    pub(super) fn path(&self) -> &Path {
        self.0.path()
    }
    pub(super) fn close(&self) -> Result<()> {
        match fs::remove_dir_all(self.path()) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::PermissionDenied => {
                // Windows rejects removal of copied read-only resources. Change
                // attributes only within this owned scratch tree, never sources.
                make_writable(self.path())?;
                fs::remove_dir_all(self.path()).context("Remove temporary DJ export build")
            }
            Err(error) => Err(error).context("Remove temporary DJ export build"),
        }
    }
}
impl Drop for BuildDirectory {
    fn drop(&mut self) {
        // Every preparation error also drops this guard after adapter handles.
        // TempDir provides a final best-effort retry after this cleanup attempt.
        let _ = self.close();
    }
}
fn make_writable(path: &Path) -> std::io::Result<()> {
    let metadata = fs::symlink_metadata(path)?;
    if metadata.file_type().is_symlink() {
        return Ok(());
    }
    let mut permissions = metadata.permissions();
    if permissions.readonly() {
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            permissions.set_mode(permissions.mode() | 0o200);
        }
        #[cfg(not(unix))]
        permissions.set_readonly(false);
        fs::set_permissions(path, permissions)?;
    }
    if metadata.is_dir() {
        for entry in fs::read_dir(path)? {
            make_writable(&entry?.path())?;
        }
    }
    Ok(())
}
