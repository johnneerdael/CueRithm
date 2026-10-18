use crate::publish_files;
use anyhow::{Context, Result, bail, ensure};
use cuerithm_media::file_sha256;
use std::path::{Path, PathBuf};
#[derive(Clone)]
pub(crate) struct Artifact {
    pub(crate) target: PathBuf,
    pub(crate) staged: PathBuf,
    pub(crate) backup: Option<PathBuf>,
    pub(crate) before: Option<String>,
    pub(crate) after: String,
    pub(crate) publish: PathBuf,
    pub(crate) displaced: PathBuf,
}

pub(crate) fn current_hash(path: &Path) -> Result<Option<String>> {
    match std::fs::symlink_metadata(path) {
        Ok(meta) => {
            ensure!(
                meta.is_file() && !meta.file_type().is_symlink(),
                "Publication target is not a regular file"
            );
            Ok(Some(file_sha256(path)?))
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e.into()),
    }
}
pub(crate) fn owned_file(path: &Path, workspace: &Path, hash: &str) -> Result<()> {
    ensure!(
        path.canonicalize()?.starts_with(workspace),
        "Staged asset leaves its workspace"
    );
    ensure!(file_sha256(path)? == hash, "Staged asset hash mismatch");
    Ok(())
}
pub(crate) fn safe_target(target: &Path, source: &Path) -> Result<()> {
    let parent = source.parent().context("Missing source folder")?;
    ensure!(
        target.starts_with(parent),
        "Tag destination leaves source folder"
    );
    let mut existing = target.parent().context("Missing destination folder")?;
    while !existing.exists() {
        existing = existing.parent().context("Missing destination ancestor")?;
    }
    ensure!(
        existing.canonicalize()?.starts_with(parent),
        "Destination folder changed"
    );
    Ok(())
}
pub(crate) fn rollback(files: &[Artifact]) -> Result<()> {
    let mut errors = Vec::new();
    for f in files.iter().rev() {
        let result = (|| -> Result<()> {
            let current = current_hash(&f.target)?;
            if current == f.before {
                return Ok(());
            }
            ensure!(
                current.as_deref() == Some(f.after.as_str()),
                "Later user edit preserved; original backup retained"
            );
            if let Some(before) = &f.before {
                let restore = if f.displaced.exists() && file_sha256(&f.displaced)? != f.after {
                    f.displaced.clone()
                } else {
                    let backup = f.backup.as_ref().context("Missing original backup")?;
                    ensure!(file_sha256(backup)? == *before, "Original backup changed");
                    let restore = f
                        .target
                        .parent()
                        .unwrap()
                        .join(format!(".cuerithm-restore-{}.tmp", uuid::Uuid::new_v4()));
                    publish_files::copy_prepared(backup, &restore, before)?;
                    restore
                };
                let displaced = if cfg!(windows) {
                    restore.with_extension("after.tmp")
                } else {
                    restore.clone()
                };
                publish_files::replace(&restore, &f.target, &displaced)?;
                if file_sha256(&displaced)? == f.after {
                    std::fs::remove_file(&displaced)?;
                } else {
                    // A user edit raced the rollback; restore that newer captured version.
                    let other = if cfg!(windows) {
                        displaced.with_extension("previous.tmp")
                    } else {
                        displaced.clone()
                    };
                    publish_files::replace(&displaced, &f.target, &other)?;
                    bail!("Concurrent user edit preserved; recovery backup retained");
                }
            } else {
                let captured = f
                    .target
                    .parent()
                    .unwrap()
                    .join(format!(".cuerithm-rollback-{}.tmp", uuid::Uuid::new_v4()));
                publish_files::create(&f.target, &captured)?;
                if file_sha256(&captured)? == f.after {
                    std::fs::remove_file(captured)?;
                } else {
                    if !f.target.exists() {
                        publish_files::create(&captured, &f.target)?;
                    }
                    bail!("Concurrent artwork edit preserved");
                }
            }
            Ok(())
        })();
        if let Err(e) = result {
            errors.push(format!("{e:#}"));
        }
    }
    if !errors.is_empty() {
        bail!("{}", errors.join("; "));
    }
    Ok(())
}
pub(crate) fn cleanup_temporaries(files: &[Artifact], root: &Path) -> Result<()> {
    for file in files {
        // Only the original recovery copy survives a changed publication. A
        // no-op did not displace any source bytes and needs no audio backup.
        if let Some(before) = &file.before
            && before != &file.after
        {
            let backup = file
                .backup
                .as_ref()
                .context("Missing durable original backup")?;
            ensure!(
                file_sha256(backup)? == *before,
                "Original backup is unavailable; temporary recovery bytes retained"
            );
            // Also cover journals written by older versions before the explicit
            // pre-journal durability barrier existed.
            publish_files::durable_assets(
                std::slice::from_ref(backup),
                &backup
                    .parent()
                    .context("Missing backup workspace")?
                    .canonicalize()?,
                &root.canonicalize()?,
            )?;
        }
        let mut removed = std::collections::HashSet::new();
        for path in [&file.staged, &file.publish, &file.displaced] {
            if !removed.insert(path) {
                continue;
            }
            if let Some(hash) = current_hash(path)? {
                ensure!(
                    hash == file.after || file.before.as_ref() == Some(&hash),
                    "Changed temporary file retained"
                );
                std::fs::remove_file(path)?;
                publish_files::sync_parent(path)?;
            }
        }
        if file.before.as_ref() == Some(&file.after)
            && let Some(backup) = &file.backup
            && let Some(hash) = current_hash(backup)?
        {
            ensure!(
                Some(&hash) == file.before.as_ref(),
                "Changed no-op backup retained"
            );
            std::fs::remove_file(backup)?;
            publish_files::sync_parent(backup)?;
        }
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rollback_restores_the_user_version_captured_by_atomic_exchange() {
        let d = tempfile::tempdir().unwrap();
        let target = d.path().join("audio");
        let displaced = d.path().join("displaced");
        let backup = d.path().join("backup");
        std::fs::write(&target, b"our metadata output").unwrap();
        std::fs::write(&displaced, b"latest user edit").unwrap();
        std::fs::write(&backup, b"expected original").unwrap();
        let artifact = Artifact {
            target: target.clone(),
            staged: d.path().join("staged"),
            backup: Some(backup.clone()),
            before: Some(file_sha256(&backup).unwrap()),
            after: file_sha256(&target).unwrap(),
            publish: if cfg!(windows) {
                d.path().join("publish")
            } else {
                displaced.clone()
            },
            displaced,
        };
        rollback(&[artifact]).unwrap();
        assert_eq!(std::fs::read(target).unwrap(), b"latest user edit");
        assert_eq!(std::fs::read(backup).unwrap(), b"expected original");
    }
    #[test]
    fn cleanup_keeps_displaced_original_when_its_durable_backup_is_missing() {
        let d = tempfile::tempdir().unwrap();
        let target = d.path().join("target");
        let displaced = d.path().join("old");
        std::fs::write(&target, b"new").unwrap();
        std::fs::write(&displaced, b"original").unwrap();
        let artifact = Artifact {
            target: target.clone(),
            staged: d.path().join("stage"),
            backup: Some(d.path().join("missing")),
            before: Some(file_sha256(&displaced).unwrap()),
            after: file_sha256(&target).unwrap(),
            publish: displaced.clone(),
            displaced: displaced.clone(),
        };
        assert!(cleanup_temporaries(&[artifact], d.path()).is_err());
        assert_eq!(std::fs::read(displaced).unwrap(), b"original");
    }
}
