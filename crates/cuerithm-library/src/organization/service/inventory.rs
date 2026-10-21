use crate::organization::native::{Authority, NativePath, Parent};
use anyhow::{Context, Result, ensure};
use cuerithm_media::CancellationToken;
use cuerithm_organize::{
    paths::{NativePath as PurePath, collision_key},
    plan::{CompletedPlan, NonFileEntry, NonFileKind},
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::{Path, PathBuf},
};
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(super) struct DirectorySnapshot {
    pub authority: Authority,
    pub digest: String,
}
#[derive(Debug, Clone)]
struct Entry {
    path: PathBuf,
    kind: &'static str,
}
pub(super) struct Inventory {
    pub directories: Vec<DirectorySnapshot>,
    pub non_files: Vec<NonFileEntry>,
    pub occupied: BTreeSet<String>,
}
fn listing(path: &Path, cancel: &CancellationToken) -> Result<(DirectorySnapshot, Vec<Entry>)> {
    let (_, authority) = Parent::capture(path)?;
    let mut entries = Vec::new();
    let mut digest_entries = Vec::new();
    for entry in std::fs::read_dir(path)? {
        cancel.check()?;
        let entry = entry?;
        let ty = entry.file_type()?;
        let kind = if ty.is_symlink() {
            "symlink"
        } else if ty.is_dir() {
            "directory"
        } else if ty.is_file() {
            "file"
        } else {
            "other"
        };
        let name = NativePath::new(Path::new(&entry.file_name()));
        digest_entries.push((name.0, kind));
        entries.push(Entry {
            path: entry.path(),
            kind,
        });
    }
    digest_entries.sort();
    Parent::reopen(&authority)?;
    let digest = format!("{:x}", Sha256::digest(serde_json::to_vec(&digest_entries)?));
    Ok((DirectorySnapshot { authority, digest }, entries))
}
pub(super) fn inspect(plan: &CompletedPlan, cancel: &CancellationToken) -> Result<Inventory> {
    let mut needed = BTreeSet::new();
    let mut roots = BTreeSet::new();
    let mut wanted = BTreeSet::new();
    for row in &plan.rows {
        for file in &row.files {
            wanted.insert(collision_key(&file.destination.0).map_err(anyhow::Error::msg)?);
            let mut parent = file
                .destination
                .0
                .parent()
                .context("Missing destination parent")?;
            loop {
                needed.insert(collision_key(parent).map_err(anyhow::Error::msg)?);
                match Parent::capture(parent) {
                    Ok(_) => {
                        roots.insert(parent.to_path_buf());
                        break;
                    }
                    Err(error)
                        if error
                            .downcast_ref::<std::io::Error>()
                            .is_some_and(|e| e.kind() == std::io::ErrorKind::NotFound) =>
                    {
                        parent = parent.parent().context("Destination root is missing")?
                    }
                    Err(error) => return Err(error),
                }
            }
        }
    }
    // Visit only the complete parent namespace relevant to prospective files;
    // unrelated audio bytes are never hashed just to discover an occupied name.
    let mut queue = roots.into_iter().collect::<Vec<_>>();
    let mut seen = BTreeSet::new();
    let mut directories = Vec::new();
    let mut occupied = BTreeSet::new();
    let mut non_files = BTreeMap::<String, NonFileEntry>::new();
    let mut count = 0usize;
    while let Some(root) = queue.pop() {
        cancel.check()?;
        if !seen.insert(root.clone()) {
            continue;
        }
        let (snapshot, entries) = listing(&root, cancel)?;
        directories.push(snapshot);
        for entry in entries {
            count += 1;
            ensure!(
                count <= 1_000_000,
                "Organization destination inventory exceeds one million entries; narrow the scope"
            );
            let key = collision_key(&entry.path).map_err(anyhow::Error::msg)?;
            if wanted.contains(&key) && entry.kind == "file" {
                occupied.insert(key.clone());
            }
            if needed.contains(&key) || wanted.contains(&key) {
                if entry.kind != "file" {
                    let kind = match entry.kind {
                        "directory" => NonFileKind::Directory,
                        "symlink" => NonFileKind::Symlink,
                        _ => NonFileKind::Other,
                    };
                    let candidate = NonFileEntry {
                        path: PurePath(entry.path.clone()),
                        kind,
                    };
                    if let Some(old) = non_files.get_mut(&key) {
                        if old.path != candidate.path {
                            old.kind = NonFileKind::Other;
                        }
                    } else {
                        non_files.insert(key.clone(), candidate);
                    }
                } else if needed.contains(&key) {
                    occupied.insert(key.clone());
                }
            }
            if entry.kind == "directory" && needed.contains(&key) {
                queue.push(entry.path);
            }
        }
    }
    Ok(Inventory {
        directories,
        non_files: non_files.into_values().collect(),
        occupied,
    })
}
pub(super) fn validate(snapshots: &[DirectorySnapshot], cancel: &CancellationToken) -> Result<()> {
    for expected in snapshots {
        cancel.check()?;
        Parent::reopen(&expected.authority)?;
        let (actual, _) = listing(&expected.authority.path.path()?, cancel)?;
        ensure!(
            actual.digest == expected.digest,
            "Destination directory changed; create a new organization preview"
        );
    }
    Ok(())
}
