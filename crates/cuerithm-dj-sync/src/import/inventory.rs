//! Bounded native artifact inventory for explicitly selected source folders.
use super::{ArtifactKind, ArtifactRequest, SourceCaptureRequest, SourceProvider};
use anyhow::{Context, ensure};
use cuerithm_media::CancellationToken;
use std::{
    fs,
    path::{Path, PathBuf},
};

pub fn folder_backed(provider: SourceProvider) -> bool {
    matches!(
        provider,
        SourceProvider::Serato3
            | SourceProvider::Serato4
            | SourceProvider::VirtualDj
            | SourceProvider::EngineDj
            | SourceProvider::DjayPro
            | SourceProvider::PioneerUsb
            | SourceProvider::DjStudio
    )
}

pub fn eligible(provider: SourceProvider, relative: &Path) -> bool {
    let name = relative
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or_default();
    let extension = relative
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    match provider {
        SourceProvider::Serato3 => {
            matches!(name, "database V2" | "neworder.pref")
                || matches!(extension.as_str(), "crate" | "scrate")
        }
        SourceProvider::Serato4 => extension == "sqlite",
        SourceProvider::VirtualDj => {
            name.eq_ignore_ascii_case("database.xml")
                || matches!(extension.as_str(), "m3u" | "m3u8" | "vdjfolder")
        }
        SourceProvider::EngineDj => matches!(name, "m.db" | "stm.db" | "hm.db"),
        SourceProvider::DjayPro => {
            matches!(name, "MediaLibrary.db" | "database2" | "Music.library")
        }
        SourceProvider::PioneerUsb => {
            relative
                .components()
                .next()
                .is_some_and(|v| matches!(v.as_os_str().to_str(), Some("PIONEER" | ".PIONEER")))
                && matches!(extension.as_str(), "pdb" | "db" | "dat" | "ext" | "2ex")
        }
        SourceProvider::DjStudio => {
            relative == Path::new("Database/studio.db")
                || relative.components().nth(1).is_some_and(|v| {
                    matches!(
                        v.as_os_str().to_str(),
                        Some(
                            "audio-library-table"
                                | "track-structures-table"
                                | "projects-table"
                                | "projects-meta-table"
                                | "mix-data-table"
                                | "automation-data-table"
                        )
                    )
                })
        }
        _ => false,
    }
}
fn descend(provider: SourceProvider, relative: &Path) -> bool {
    if provider == SourceProvider::DjStudio {
        return relative == Path::new("Database")
            || relative.starts_with("Database")
                && relative.components().nth(1).is_some_and(|v| {
                    matches!(
                        v.as_os_str().to_str(),
                        Some(
                            "audio-library-table"
                                | "track-structures-table"
                                | "projects-table"
                                | "projects-meta-table"
                                | "mix-data-table"
                                | "automation-data-table"
                        )
                    )
                });
    }
    if provider == SourceProvider::PioneerUsb {
        return relative
            .components()
            .next()
            .is_some_and(|v| matches!(v.as_os_str().to_str(), Some("PIONEER" | ".PIONEER")));
    }
    let name = relative
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or_default();
    !matches!(
        name,
        "Cache"
            | "cache"
            | "Contents"
            | "Music"
            | "Audio"
            | "node_modules"
            | "Local Storage"
            | "Session Storage"
            | "Cookies"
            | "GPUCache"
            | ".git"
    )
}
pub fn artifact(root: &Path, relative: PathBuf, id: String) -> anyhow::Result<ArtifactRequest> {
    let extension = relative
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    let name = relative
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or_default();
    let kind = if matches!(extension.as_str(), "sqlite" | "sqlite3" | "db" | "mikdb")
        || matches!(name, "database2" | "Music.library")
    {
        ArtifactKind::Sqlite
    } else {
        ArtifactKind::StableFile
    };
    let request = ArtifactRequest {
        id,
        relative_path: relative,
        kind,
        selected_root: None,
    };
    // Run the shared traversal/symlink validation before minting native authority.
    SourceCaptureRequest::from_selected_root(SourceProvider::M3u, root, vec![request.clone()])?;
    Ok(request)
}
pub fn inventory_source(
    root: &Path,
    provider: SourceProvider,
    cancel: &CancellationToken,
) -> anyhow::Result<Vec<ArtifactRequest>> {
    let mut pending = vec![(root.to_path_buf(), 0usize)];
    let mut artifacts = Vec::new();
    let mut visited = 0;
    while let Some((directory, depth)) = pending.pop() {
        cancel.check()?;
        ensure!(depth <= 128, "Source hierarchy exceeds128 levels");
        for entry in fs::read_dir(directory)? {
            cancel.check()?;
            visited += 1;
            ensure!(visited <= 2_000_000, "Source inventory exceeds limit");
            let entry = entry?;
            let path = entry.path();
            let relative = path.strip_prefix(root)?.to_path_buf();
            let metadata = fs::symlink_metadata(&path)?;
            if metadata.file_type().is_symlink() {
                continue;
            }
            if metadata.is_dir() && descend(provider, &relative) {
                pending.push((path, depth + 1));
            } else if metadata.is_file() && eligible(provider, &relative) {
                ensure!(
                    artifacts.len() < 100_000,
                    "Selected source exceeds artifact limit"
                );
                let id = relative
                    .to_str()
                    .context("Source artifact name is not Unicode")?
                    .replace('\\', "/");
                artifacts.push(artifact(root, relative, id)?);
            }
        }
    }
    ensure!(
        !artifacts.is_empty(),
        "No supported music-library artifacts in the selected folder"
    );
    artifacts.sort_by(|a, b| a.id.cmp(&b.id));
    Ok(artifacts)
}
