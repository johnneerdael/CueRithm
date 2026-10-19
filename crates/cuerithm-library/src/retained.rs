//! Explicit disposal of the original files kept for preparation tagging and metadata cleanup.
//!
//! Disposal is never automatic. A write's retained original is only offered once the write is final and
//! its own cleanup has finished. Originals that still back a cleanup Undo are offered separately and say so.
//! The database stops referencing the files in one transaction before any file is removed, so an interrupted
//! disposal can only leave unreferenced files to finish deleting, never a reference to a missing file.
use crate::{
    Library,
    model::{bytes_path, now},
};
use anyhow::{Context, Result, ensure};
use cuerithm_media::{CancellationToken, file_sha256_cancellable};
use rusqlite::{TransactionBehavior, params};
use serde::{Deserialize, Serialize};
use std::path::Path;

const PLAN: &str = "retained_originals_plan";
const DISPOSAL: &str = "retained_originals_disposal";

/// One retained original, as shown for review.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RetainedOriginal {
    /// `tagging` for a preparation tag write, `cleanup` for a manual cleanup write.
    pub source: String,
    pub write_id: String,
    pub display_path: String,
    pub size_bytes: u64,
    /// Disposing of this file removes Undo for its cleanup batch.
    pub undo_lost: bool,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetainedPreview {
    pub id: String,
    pub include_undoable: bool,
    pub files: Vec<RetainedOriginal>,
    pub size_bytes: u64,
    /// Cleanup writes that could no longer be undone after disposal.
    pub undo_lost_writes: usize,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetainedDisposal {
    pub id: String,
    pub preview_id: String,
    pub file_count: usize,
    pub size_bytes: u64,
    pub undo_lost_writes: usize,
    pub created: i64,
    /// `committed` once the database no longer references the files.
    pub state: String,
    /// Some files are still waiting to be deleted; a retry finishes them.
    pub cleanup_pending: bool,
    pub error: Option<String>,
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
struct Candidate {
    table: String,
    write_id: String,
    ordinal: i64,
    path: Vec<u8>,
    workspace: Vec<u8>,
    hash: String,
    public: RetainedOriginal,
}
#[derive(Serialize, Deserialize)]
struct Plan {
    public: RetainedPreview,
    candidates: Vec<Candidate>,
    consumed_by: Option<String>,
}
#[derive(Serialize, Deserialize)]
struct Journal {
    result: RetainedDisposal,
    candidates: Vec<Candidate>,
}

fn owned(lib: &Library, c: &Candidate) -> Result<()> {
    let root = lib.data_dir().canonicalize()?;
    let path = bytes_path(&c.path)?;
    let workspace = bytes_path(&c.workspace)?;
    let meta = std::fs::symlink_metadata(&path)?;
    ensure!(
        meta.is_file() && !meta.file_type().is_symlink(),
        "A retained original is not a regular file"
    );
    let path = path.canonicalize()?;
    ensure!(
        path.starts_with(&root) && path.starts_with(workspace.canonicalize()?),
        "A retained original is outside application storage"
    );
    Ok(())
}
fn is_missing(path: &Path) -> Result<bool> {
    match std::fs::symlink_metadata(path) {
        Ok(_) => Ok(false),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(true),
        Err(e) => Err(e.into()),
    }
}

impl Library {
    /// Every retained original that may be offered for disposal, in a stable order.
    fn retained_candidates(&self, include_undoable: bool) -> Result<Vec<Candidate>> {
        let mut found = vec![];
        let (tagging, cleanup) = {
            let conn = self.conn()?;
            let mut q = conn.prepare("SELECT f.write_id,f.ordinal,f.target,f.backup,f.before_hash,w.workspace FROM tag_write_files f JOIN tag_writes w ON w.id=f.write_id WHERE f.backup IS NOT NULL AND f.before_hash IS NOT NULL AND w.state IN ('committed','rolled_back') AND w.cleanup_complete=1 ORDER BY w.created,f.write_id,f.ordinal")?;
            let tagging = q
                .query_map([], |r| {
                    Ok((
                        r.get::<_, String>(0)?,
                        r.get::<_, i64>(1)?,
                        r.get::<_, Vec<u8>>(2)?,
                        r.get::<_, Vec<u8>>(3)?,
                        r.get::<_, String>(4)?,
                        r.get::<_, Vec<u8>>(5)?,
                    ))
                })?
                .collect::<rusqlite::Result<Vec<_>>>()?;
            let mut q = conn.prepare("SELECT f.write_id,f.ordinal,f.target,f.backup,f.before_hash,w.workspace,w.state,w.payload_json FROM cleanup_write_files f JOIN cleanup_writes w ON w.id=f.write_id WHERE f.backup IS NOT NULL AND f.before_hash IS NOT NULL AND w.state IN ('committed','undone','rolled_back') AND w.cleanup_complete=1 ORDER BY w.created,f.write_id,f.ordinal")?;
            let cleanup = q
                .query_map([], |r| {
                    Ok((
                        r.get::<_, String>(0)?,
                        r.get::<_, i64>(1)?,
                        r.get::<_, Vec<u8>>(2)?,
                        r.get::<_, Vec<u8>>(3)?,
                        r.get::<_, String>(4)?,
                        r.get::<_, Vec<u8>>(5)?,
                        r.get::<_, String>(6)?,
                        r.get::<_, String>(7)?,
                    ))
                })?
                .collect::<rusqlite::Result<Vec<_>>>()?;
            (tagging, cleanup)
        };
        let mut push = |table: &str,
                        source: &str,
                        write_id: String,
                        ordinal: i64,
                        target: Vec<u8>,
                        backup: Vec<u8>,
                        hash: String,
                        workspace: Vec<u8>,
                        undo_lost: bool| {
            let candidate = Candidate {
                table: table.into(),
                write_id: write_id.clone(),
                ordinal,
                path: backup,
                workspace,
                hash,
                public: RetainedOriginal {
                    source: source.into(),
                    write_id,
                    display_path: bytes_path(&target)
                        .map(|p| p.display().to_string())
                        .unwrap_or_default(),
                    size_bytes: 0,
                    undo_lost,
                },
            };
            found.push(candidate);
        };
        for (id, ordinal, target, backup, hash, workspace) in tagging {
            push(
                "tag_write_files",
                "tagging",
                id,
                ordinal,
                target,
                backup,
                hash,
                workspace,
                false,
            );
        }
        for (id, ordinal, target, backup, hash, workspace, state, payload) in cleanup {
            // A committed original apply is the one Undo restores from; undo records themselves are not.
            let payload: serde_json::Value = serde_json::from_str(&payload)?;
            let undoable = state == "committed" && payload["undo_of"].is_null();
            if undoable && !include_undoable {
                continue;
            }
            push(
                "cleanup_write_files",
                "cleanup",
                id,
                ordinal,
                target,
                backup,
                hash,
                workspace,
                undoable,
            );
        }
        // Files that are already gone have nothing to dispose of; ownership problems are reported, not skipped.
        let mut present = vec![];
        for mut c in found {
            let path = bytes_path(&c.path)?;
            if is_missing(&path)? {
                continue;
            }
            owned(self, &c)?;
            c.public.size_bytes = std::fs::metadata(&path)?.len();
            present.push(c);
        }
        Ok(present)
    }

    pub fn preview_retained_originals(
        &self,
        include_undoable: bool,
        cancel: &CancellationToken,
    ) -> Result<RetainedPreview> {
        cancel.check()?;
        let candidates = self.retained_candidates(include_undoable)?;
        ensure!(
            !candidates.is_empty(),
            "No retained originals can be disposed of"
        );
        let mut writes: Vec<&str> = candidates
            .iter()
            .filter(|c| c.public.undo_lost)
            .map(|c| c.write_id.as_str())
            .collect();
        writes.sort_unstable();
        writes.dedup();
        let public = RetainedPreview {
            id: uuid::Uuid::new_v4().to_string(),
            include_undoable,
            files: candidates.iter().map(|c| c.public.clone()).collect(),
            size_bytes: candidates.iter().map(|c| c.public.size_bytes).sum(),
            undo_lost_writes: writes.len(),
        };
        let plan = Plan {
            public: public.clone(),
            candidates,
            consumed_by: None,
        };
        self.conn()?.execute(
            "INSERT INTO maintenance_scans VALUES(?1,?2,?3,?4)",
            params![public.id, PLAN, serde_json::to_string(&plan)?, now()],
        )?;
        Ok(public)
    }

    pub fn dispose_retained_originals(
        &self,
        preview_id: &str,
        cancel: &CancellationToken,
    ) -> Result<RetainedDisposal> {
        cancel.check()?;
        #[cfg(feature = "autotag")]
        let _gate = self
            .inner
            .tag_gate
            .lock()
            .map_err(|_| anyhow::anyhow!("Publication gate interrupted"))?;
        let mut plan: Plan = load(self, preview_id, PLAN)?;
        ensure!(
            plan.consumed_by.is_none(),
            "This disposal preview was already used"
        );
        // The same set, still final and unchanged, as the reviewer saw.
        let current = self.retained_candidates(plan.public.include_undoable)?;
        ensure!(
            current == plan.candidates,
            "Retained originals changed after review; review again"
        );
        for c in &plan.candidates {
            cancel.check()?;
            owned(self, c)?;
            ensure!(
                file_sha256_cancellable(&bytes_path(&c.path)?, cancel)? == c.hash,
                "A retained original changed after review"
            );
        }
        let id = uuid::Uuid::new_v4().to_string();
        let mut journal = Journal {
            result: RetainedDisposal {
                id: id.clone(),
                preview_id: preview_id.into(),
                file_count: plan.candidates.len(),
                size_bytes: plan.public.size_bytes,
                undo_lost_writes: plan.public.undo_lost_writes,
                created: now(),
                state: "committed".into(),
                cleanup_pending: true,
                error: None,
            },
            candidates: plan.candidates.clone(),
        };
        {
            let mut conn = self.conn()?;
            let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
            for c in &journal.candidates {
                // Guarded by the exact backup path so a changed row aborts the whole disposal.
                let sql = format!(
                    "UPDATE {} SET backup=NULL WHERE write_id=?1 AND ordinal=?2 AND backup=?3",
                    c.table
                );
                ensure!(
                    tx.execute(&sql, params![c.write_id, c.ordinal, c.path])? == 1,
                    "A retained original changed during disposal"
                );
            }
            plan.consumed_by = Some(id.clone());
            tx.execute(
                "UPDATE maintenance_scans SET payload_json=?1 WHERE id=?2 AND kind=?3",
                params![serde_json::to_string(&plan)?, preview_id, PLAN],
            )?;
            tx.execute(
                "INSERT INTO maintenance_scans VALUES(?1,?2,?3,?4)",
                params![id, DISPOSAL, serde_json::to_string(&journal)?, now()],
            )?;
            tx.commit()?;
        }
        self.finish_retained_disposal(&mut journal, cancel)?;
        Ok(journal.result)
    }

    /// Delete the files a committed disposal no longer references. Safe to repeat.
    fn finish_retained_disposal(
        &self,
        journal: &mut Journal,
        cancel: &CancellationToken,
    ) -> Result<()> {
        let outcome = (|| -> Result<Vec<String>> {
            let mut preserved = vec![];
            for c in &journal.candidates {
                cancel.check()?;
                let path = bytes_path(&c.path)?;
                if is_missing(&path)? {
                    continue;
                }
                owned(self, c)?;
                // Anything that is no longer the reviewed original, such as a file placed there later, stays.
                if file_sha256_cancellable(&path, cancel)? != c.hash {
                    preserved.push(path.display().to_string());
                    continue;
                }
                std::fs::remove_file(&path)?;
                crate::publish_files::sync_parent(&path)?;
                if let Ok(workspace) = bytes_path(&c.workspace) {
                    // Only an emptied workspace goes; remove_dir refuses a directory that still has files.
                    let _ = std::fs::remove_dir(workspace);
                }
            }
            Ok(preserved)
        })();
        match outcome {
            Ok(preserved) => {
                journal.result.cleanup_pending = false;
                journal.result.error = (!preserved.is_empty()).then(|| {
                    format!(
                        "{} changed file(s) were kept: {}",
                        preserved.len(),
                        preserved.join(", ")
                    )
                });
            }
            Err(error) => {
                journal.result.cleanup_pending = true;
                journal.result.error = Some(format!("Deleting originals is unfinished: {error:#}"));
            }
        }
        self.conn()?.execute(
            "UPDATE maintenance_scans SET payload_json=?1 WHERE id=?2 AND kind=?3",
            params![serde_json::to_string(journal)?, journal.result.id, DISPOSAL],
        )?;
        Ok(())
    }

    pub fn retained_disposal_history(&self) -> Result<Vec<RetainedDisposal>> {
        let rows = {
            let conn = self.conn()?;
            let mut q = conn.prepare(
                "SELECT payload_json FROM maintenance_scans WHERE kind=?1 ORDER BY created DESC,rowid DESC",
            )?;
            q.query_map([DISPOSAL], |r| r.get::<_, String>(0))?
                .collect::<rusqlite::Result<Vec<_>>>()?
        };
        rows.into_iter()
            .map(|text| Ok(serde_json::from_str::<Journal>(&text)?.result))
            .collect()
    }

    pub fn retry_retained_disposal(
        &self,
        id: &str,
        cancel: &CancellationToken,
    ) -> Result<Vec<RetainedDisposal>> {
        cancel.check()?;
        #[cfg(feature = "autotag")]
        let _gate = self
            .inner
            .tag_gate
            .lock()
            .map_err(|_| anyhow::anyhow!("Publication gate interrupted"))?;
        let mut journal: Journal = load(self, id, DISPOSAL)?;
        ensure!(journal.result.id == id, "Invalid retained-original journal");
        if journal.result.cleanup_pending {
            self.finish_retained_disposal(&mut journal, cancel)?;
        }
        self.retained_disposal_history()
    }

    /// Finish interrupted deletions at startup. Failures stay visible in the history instead of blocking the library.
    pub(crate) fn recover_retained_disposals(&self) -> Result<()> {
        for disposal in self.retained_disposal_history()? {
            if disposal.cleanup_pending {
                let _ = self.retry_retained_disposal(&disposal.id, &CancellationToken::default());
            }
        }
        Ok(())
    }
}

fn load<T: serde::de::DeserializeOwned>(lib: &Library, id: &str, kind: &str) -> Result<T> {
    let text: String = lib
        .conn()?
        .query_row(
            "SELECT payload_json FROM maintenance_scans WHERE id=?1 AND kind=?2",
            params![id, kind],
            |r| r.get(0),
        )
        .context("Retained-original record not found")?;
    Ok(serde_json::from_str(&text)?)
}
