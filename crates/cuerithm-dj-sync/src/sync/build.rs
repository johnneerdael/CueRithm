//! Adapter scratch space is disposable; transaction staged/ and backup/ are not.
use anyhow::{Context, Result};
use std::path::Path;

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
        crate::private_cleanup::remove(self.path()).context("Remove temporary DJ export build")
    }
}
impl Drop for BuildDirectory {
    fn drop(&mut self) {
        // Every preparation error also drops this guard after adapter handles.
        // TempDir provides a final best-effort retry after this cleanup attempt.
        let _ = self.close();
    }
}
