use crate::{
    JobRun, Library, SettingsSnapshot,
    model::{bytes_path, now, path_bytes},
    publish_files,
};
use anyhow::{Context, Result, bail, ensure};
use camelot_core::FileAnalysis;
use cuerithm_autotag::StagedTagWrite;
use cuerithm_media::{MediaInfo, file_sha256};
use rusqlite::{Connection, params};
use std::path::{Path, PathBuf};
#[derive(Clone)]
struct Artifact {
    target: PathBuf,
    staged: PathBuf,
    backup: Option<PathBuf>,
    before: Option<String>,
    after: String,
    publish: PathBuf,
    displaced: PathBuf,
}
struct Journal {
    id: String,
    job: String,
    track: String,
    attempt: u32,
    revision: String,
    before: String,
    after: String,
    analysis: FileAnalysis,
    metadata: MediaInfo,
    provenance: serde_json::Value,
    workspace: PathBuf,
    state: String,
    files: Vec<Artifact>,
}
fn current_hash(path: &Path) -> Result<Option<String>> {
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
fn owned_file(path: &Path, workspace: &Path, hash: &str) -> Result<()> {
    ensure!(
        path.canonicalize()?.starts_with(workspace),
        "Staged asset leaves its workspace"
    );
    ensure!(file_sha256(path)? == hash, "Staged asset hash mismatch");
    Ok(())
}
fn safe_target(target: &Path, source: &Path) -> Result<()> {
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
impl Library {
    pub fn begin_tag_write(
        &self,
        run: &JobRun,
        staged: &mut StagedTagWrite,
        result: &FileAnalysis,
    ) -> Result<String> {
        Self::validate_analysis(result)?;
        let encoded = serde_json::to_string(result)?;
        let _: FileAnalysis =
            serde_json::from_str(&encoded).context("Analysis contains invalid numeric values")?;
        let media = result
            .media
            .as_ref()
            .context("Tag publication requires PCM provenance")?;
        ensure!(
            media.source_sha256 == staged.source_before_sha256
                && media.pcm_sha256 == staged.pcm_sha256
                && media.decoder == staged.decoder_identity
                && media.clock.frames == staged.frames
                && media.clock.sample_rate == 44100,
            "Analysis and tag snapshot differ"
        );
        ensure!(
            result.analysis.duration_seconds == media.clock.duration_seconds(),
            "Tag analysis clock differs"
        );
        ensure!(
            staged.source_path == run.path && staged.source_before_sha256 == run.source_sha256,
            "Tag source identity differs from job"
        );
        let workspace = staged.workspace().canonicalize()?;
        ensure!(
            workspace.starts_with(self.data_dir().canonicalize()?),
            "Tag workspace must be inside application data"
        );
        owned_file(&staged.path, &workspace, &staged.source_after_sha256)?;
        owned_file(
            &staged.backup_path,
            &workspace,
            &staged.source_before_sha256,
        )?;
        ensure!(
            current_hash(&run.path)?.as_deref() == Some(run.source_sha256.as_str()),
            "Source changed before tag publication"
        );
        let id = uuid::Uuid::new_v4().to_string();
        let mut files = vec![Artifact {
            target: run.path.clone(),
            staged: staged.path.clone(),
            backup: Some(staged.backup_path.clone()),
            before: Some(staged.source_before_sha256.clone()),
            after: staged.source_after_sha256.clone(),
            publish: PathBuf::new(),
            displaced: PathBuf::new(),
        }];
        for sidecar in &staged.sidecars {
            owned_file(&sidecar.path, &workspace, &sidecar.after_sha256)?;
            if let (Some(path), Some(hash)) = (&sidecar.backup_path, &sidecar.before_sha256) {
                owned_file(path, &workspace, hash)?;
            }
            files.push(Artifact {
                target: sidecar.target_path.clone(),
                staged: sidecar.path.clone(),
                backup: sidecar.backup_path.clone(),
                before: sidecar.before_sha256.clone(),
                after: sidecar.after_sha256.clone(),
                publish: PathBuf::new(),
                displaced: PathBuf::new(),
            });
        }
        let mut seen = std::collections::HashSet::new();
        for (n, file) in files.iter_mut().enumerate() {
            safe_target(&file.target, &run.path)?;
            ensure!(
                seen.insert(file.target.clone()),
                "Duplicate tag destinations"
            );
            ensure!(
                current_hash(&file.target)? == file.before,
                "Tag destination changed during staging"
            );
            let parent = file.target.parent().unwrap();
            file.publish = parent.join(format!(".cuerithm-{id}-{n}.tmp"));
            file.displaced = if cfg!(windows) {
                parent.join(format!(".cuerithm-{id}-{n}-before.tmp"))
            } else {
                file.publish.clone()
            };
        }
        let mut conn = self.conn()?;
        let tx = conn.transaction()?;
        ensure!(Self::run_owns(&tx, run)?, "Preparation was superseded");
        for file in &files {
            let busy:i64=tx.query_row("SELECT COUNT(*) FROM tag_write_files f JOIN tag_writes w ON w.id=f.write_id WHERE f.target=?1 AND w.state='prepared'",[path_bytes(&file.target)],|r|r.get(0))?;
            ensure!(
                busy == 0,
                "Another preparation owns this destination; retry tagging"
            );
        }
        tx.execute("INSERT INTO tag_writes(id,job_id,track_id,attempt,settings_revision,before_hash,after_hash,pcm_hash,frames,decoder,analysis_json,metadata_json,provenance_json,workspace,created) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15)",params![id,run.id,run.track_id,run.attempt,run.settings_revision,staged.source_before_sha256,staged.source_after_sha256,staged.pcm_sha256,staged.frames,staged.decoder_identity,serde_json::to_string(result)?,serde_json::to_string(&staged.metadata)?,serde_json::to_string(&staged.provenance)?,path_bytes(&workspace),now()])?;
        for (n, f) in files.iter().enumerate() {
            tx.execute(
                "INSERT INTO tag_write_files VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9)",
                params![
                    id,
                    n,
                    path_bytes(&f.target),
                    path_bytes(&f.staged),
                    f.backup.as_ref().map(|p| path_bytes(p)),
                    f.before,
                    f.after,
                    path_bytes(&f.publish),
                    path_bytes(&f.displaced)
                ],
            )?;
        }
        tx.commit()?;
        staged.retain_for_journal();
        Ok(id)
    }
    fn run_owns(conn: &Connection, run: &JobRun) -> Result<bool> {
        Ok(conn.query_row("SELECT j.state='running' AND j.attempts=?2 AND j.settings_revision=?3 AND j.source_hash=?4 AND t.current_job=j.id AND t.archived=0 AND t.id=?5 AND t.path=?6 FROM jobs j JOIN tracks t ON t.id=j.track_id WHERE j.id=?1",params![run.id,run.attempt,run.settings_revision,run.source_sha256,run.track_id,path_bytes(&run.path)],|r|r.get(0))?)
    }
    fn load_tag_write(&self, id: &str) -> Result<Journal> {
        let conn = self.conn()?;
        let mut journal=conn.query_row("SELECT id,job_id,track_id,attempt,settings_revision,before_hash,after_hash,analysis_json,metadata_json,provenance_json,workspace,state FROM tag_writes WHERE id=?1",[id],|r|Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?,r.get::<_,String>(2)?,r.get::<_,u32>(3)?,r.get::<_,String>(4)?,r.get::<_,String>(5)?,r.get::<_,String>(6)?,r.get::<_,String>(7)?,r.get::<_,String>(8)?,r.get::<_,String>(9)?,r.get::<_,Vec<u8>>(10)?,r.get::<_,String>(11)?)))?;
        let mut stmt=conn.prepare("SELECT target,staged,backup,before_hash,after_hash,publish_path,displaced_path FROM tag_write_files WHERE write_id=?1 ORDER BY ordinal")?;
        let encoded = stmt
            .query_map([id], |r| {
                Ok((
                    r.get::<_, Vec<u8>>(0)?,
                    r.get::<_, Vec<u8>>(1)?,
                    r.get::<_, Option<Vec<u8>>>(2)?,
                    r.get::<_, Option<String>>(3)?,
                    r.get::<_, String>(4)?,
                    r.get::<_, Vec<u8>>(5)?,
                    r.get::<_, Vec<u8>>(6)?,
                ))
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        let files = encoded
            .into_iter()
            .map(
                |(target, staged, backup, before, after, publish, displaced)| {
                    Ok(Artifact {
                        target: bytes_path(&target)?,
                        staged: bytes_path(&staged)?,
                        backup: backup.map(|p| bytes_path(&p)).transpose()?,
                        before,
                        after,
                        publish: bytes_path(&publish)?,
                        displaced: bytes_path(&displaced)?,
                    })
                },
            )
            .collect::<Result<Vec<_>>>()?;
        Ok(Journal {
            id: std::mem::take(&mut journal.0),
            job: journal.1,
            track: journal.2,
            attempt: journal.3,
            revision: journal.4,
            before: journal.5,
            after: journal.6,
            analysis: serde_json::from_str(&journal.7)?,
            metadata: serde_json::from_str(&journal.8)?,
            provenance: serde_json::from_str(&journal.9)?,
            workspace: bytes_path(&journal.10)?,
            state: journal.11,
            files,
        })
    }
    pub fn publish_preparation(&self, run: &JobRun, id: &str) -> Result<()> {
        let _gate = self
            .inner
            .tag_gate
            .lock()
            .map_err(|_| anyhow::anyhow!("Publication gate interrupted"))?;
        let journal = self.load_tag_write(id)?;
        ensure!(
            journal.id == id
                && journal.before == run.source_sha256
                && journal.state == "prepared"
                && journal.job == run.id
                && journal.attempt == run.attempt
                && journal.revision == run.settings_revision,
            "Publication was superseded"
        );
        ensure!(
            journal
                .workspace
                .canonicalize()?
                .starts_with(self.data_dir().canonicalize()?),
            "Invalid journal workspace"
        );
        let preparation = (|| -> Result<()> {
            // Copy all output bytes before acquiring the SQLite commit boundary.
            for f in &journal.files {
                safe_target(&f.target, &run.path)?;
                owned_file(&f.staged, &journal.workspace, &f.after)?;
                if f.before.as_deref() == Some(f.after.as_str()) {
                    continue;
                }
                if !f.publish.exists() {
                    publish_files::copy_prepared(&f.staged, &f.publish, &f.after)?;
                }
            }
            Ok(())
        })();
        let mut conn = self.conn()?;
        let result = preparation.and_then(|_| {
            let tx = conn.transaction()?;
            ensure!(Self::run_owns(&tx, run)?, "Preparation was superseded");
            Self::validate_analysis(&journal.analysis)?;
            let settings: String = tx.query_row(
                "SELECT json FROM settings WHERE revision=?1",
                [&run.settings_revision],
                |r| r.get(0),
            )?;
            let snapshot: SettingsSnapshot = serde_json::from_str(&settings)?;
            ensure!(
                !snapshot.settings.cleanup_enabled,
                "Required cleanup has not completed"
            );
            for f in &journal.files {
                safe_target(&f.target, &run.path)?;
                ensure!(
                    current_hash(&f.target)? == f.before,
                    "Source or artwork changed before publication"
                );
                if f.before != Some(f.after.clone()) {
                    ensure!(
                        file_sha256(&f.publish)? == f.after,
                        "Publication copy changed"
                    );
                }
            }
            for f in &journal.files {
                if f.before.as_deref() == Some(f.after.as_str()) {
                    continue;
                }
                if let Some(before) = &f.before {
                    ensure!(
                        !std::fs::metadata(&f.target)?.permissions().readonly(),
                        "Publication target is read only"
                    );
                    publish_files::replace(&f.publish, &f.target, &f.displaced)?;
                    ensure!(
                        file_sha256(&f.displaced)? == *before,
                        "Source changed during atomic replacement"
                    );
                } else {
                    publish_files::create(&f.publish, &f.target)?;
                }
                ensure!(
                    file_sha256(&f.target)? == f.after,
                    "Published file changed before commit"
                );
            }
            Self::store_analysis_tx(
                &tx,
                &journal.job,
                &journal.track,
                &journal.analysis,
                &journal.metadata,
            )?;
            tx.execute(
                "UPDATE tracks SET source_hash=?1,tagging_json=?2 WHERE id=?3",
                params![
                    journal.after,
                    serde_json::to_string(&journal.provenance)?,
                    journal.track
                ],
            )?;
            tx.execute(
                "UPDATE tag_writes SET state='committed',error=NULL WHERE id=?1",
                [id],
            )?;
            tx.commit()?;
            Ok(())
        });
        if let Err(error) = result {
            let rollback = rollback(&journal.files);
            let message = match &rollback {
                Ok(_) => format!("{error:#}"),
                Err(e) => format!("{error:#}; recovery needs attention: {e:#}"),
            };
            conn.execute(
                "UPDATE tag_writes SET state=?1,error=?2 WHERE id=?3",
                params![
                    if rollback.is_ok() {
                        "rolled_back"
                    } else {
                        "conflict"
                    },
                    message,
                    id
                ],
            )?;
            return Err(error);
        }
        cleanup_temporaries(&journal.files);
        Ok(())
    }
    pub fn recover_tag_writes(&self) -> Result<()> {
        let _gate = self
            .inner
            .tag_gate
            .lock()
            .map_err(|_| anyhow::anyhow!("Publication gate interrupted"))?;
        let ids: Vec<String> = {
            let conn = self.conn()?;
            let mut stmt = conn
                .prepare("SELECT id FROM tag_writes WHERE state='prepared' ORDER BY created,id")?;
            stmt.query_map([], |r| r.get(0))?
                .collect::<rusqlite::Result<_>>()?
        };
        for id in ids {
            let journal = self.load_tag_write(&id)?;
            let outcome = rollback(&journal.files);
            let mut conn = self.conn()?;
            let tx = conn.transaction()?;
            let owned:bool=tx.query_row("SELECT t.current_job=?2 AND t.archived=0 AND j.attempts=?3 FROM tracks t JOIN jobs j ON j.id=?2 WHERE t.id=?1",params![journal.track,journal.job,journal.attempt],|r|r.get(0))?;
            if owned {
                tx.execute(
                    "UPDATE jobs SET state=?1,stage=?2,error=?3 WHERE id=?4",
                    params![
                        if outcome.is_ok() {
                            "queued"
                        } else {
                            "attention"
                        },
                        if outcome.is_ok() {
                            "queued"
                        } else {
                            "tag_recovery"
                        },
                        outcome.as_ref().err().map(|e| format!("{e:#}")),
                        journal.job
                    ],
                )?;
            }
            tx.execute(
                "UPDATE tag_writes SET state=?1,error=?2 WHERE id=?3",
                params![
                    if outcome.is_ok() {
                        "rolled_back"
                    } else {
                        "conflict"
                    },
                    outcome.as_ref().err().map(|e| format!("{e:#}")),
                    id
                ],
            )?;
            tx.commit()?;
        }
        let committed: Vec<String> = {
            let conn = self.conn()?;
            let mut stmt = conn.prepare("SELECT id FROM tag_writes WHERE state='committed'")?;
            stmt.query_map([], |r| r.get(0))?
                .collect::<rusqlite::Result<_>>()?
        };
        for id in committed {
            if let Ok(journal) = self.load_tag_write(&id) {
                cleanup_temporaries(&journal.files);
            }
        }
        Ok(())
    }
}
fn rollback(files: &[Artifact]) -> Result<()> {
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
fn cleanup_temporaries(files: &[Artifact]) {
    for file in files {
        let backup_safe = match (&file.backup, &file.before) {
            (Some(path), Some(hash)) => file_sha256(path).ok().as_ref() == Some(hash),
            _ => false,
        };
        if !backup_safe {
            continue;
        }
        for path in [&file.publish, &file.displaced] {
            if file
                .before
                .as_ref()
                .is_some_and(|before| file_sha256(path).ok().as_ref() == Some(before))
            {
                let _ = std::fs::remove_file(path);
            }
        }
    }
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
        cleanup_temporaries(&[artifact]);
        assert_eq!(std::fs::read(displaced).unwrap(), b"original");
    }
}
