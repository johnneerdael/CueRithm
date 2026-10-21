use super::{capture::FrozenRow, *};
use anyhow::{Context, Result, ensure};
use std::{
    collections::BTreeSet,
    path::{Path, PathBuf},
};
fn existing_folder(path: &Path) -> Result<PathBuf> {
    let mut path = path.to_path_buf();
    loop {
        if path.is_dir() {
            return Ok(path);
        }
        ensure!(path.pop(), "Storage volume is unavailable");
    }
}
impl Library {
    pub(super) fn organization_storage_estimate(&self, rows: &[FrozenRow]) -> Result<(u64, u64)> {
        let mut bytes = 0u64;
        let mut paths = BTreeSet::from([existing_folder(self.data_dir())?]);
        for row in rows.iter().filter(|row| row.public.outcome == "ready") {
            let frozen = row
                .captured
                .as_ref()
                .context("Missing frozen organization storage proof")?;
            bytes = bytes
                .checked_add(frozen.reference_bytes)
                .context("Organization storage estimate overflow")?;
            for path in &frozen.reference_storage {
                paths.insert(existing_folder(&path.path()?)?);
            }
            for file in &frozen.files {
                if file.source.path != file.destination {
                    bytes = bytes
                        .checked_add(file.source.byte_len)
                        .context("Organization storage estimate overflow")?;
                    paths.insert(existing_folder(
                        file.destination
                            .0
                            .parent()
                            .context("Missing destination folder")?,
                    )?);
                }
            }
        }
        // Conservative aggregate: require the whole allocation on every affected
        // allocation volume. This deliberately overestimates cross-volume plans,
        // while correctly summing audio/native allocations that share a volume.
        if bytes > 0 {
            bytes = bytes
                .checked_add((bytes / 20).max(16 * 1024 * 1024))
                .context("Organization storage estimate overflow")?;
        }
        let available = paths
            .iter()
            .map(fs2::available_space)
            .collect::<std::io::Result<Vec<_>>>()?
            .into_iter()
            .min()
            .unwrap_or(0);
        Ok((bytes, available))
    }
    pub(super) fn ensure_organization_storage(&self, rows: &[FrozenRow]) -> Result<()> {
        let (required, available) = self.organization_storage_estimate(rows)?;
        ensure!(
            required <= available,
            "Organization requires {required} bytes including staging and retained native backups, but only {available} bytes are available on an affected volume. Narrow the scope, free storage, or choose another destination; then preview again."
        );
        Ok(())
    }
}
