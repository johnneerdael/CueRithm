//! Explicit disposal only of verified, application-owned recovery originals.
use super::*;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
#[derive(Clone, Serialize, Deserialize)]
struct Backup {
    path: Vec<u8>,
    workspace: Vec<u8>,
    hash: String,
    identity: Identity,
    public: UnusedBackupFile,
}
#[derive(Serialize, Deserialize)]
struct DisposalPreview {
    public: UnusedBackupPreview,
    backups: Vec<Backup>,
    rows: BTreeMap<String, String>,
    workspaces: Vec<Vec<u8>>,
    consumed_by: Option<String>,
    logic: String,
}
#[derive(Serialize, Deserialize)]
struct Disposal {
    schema: String,
    result: UnusedBackupDisposal,
    backups: Vec<Backup>,
    holding: Vec<Vec<u8>>,
    workspaces: Vec<Vec<u8>>,
}
fn digest(text: &str) -> String {
    format!("{:x}", Sha256::digest(text.as_bytes()))
}
fn snapshots(conn: &Connection, workspaces: &[Vec<u8>]) -> Result<BTreeMap<String, String>> {
    let mut q=conn.prepare("SELECT id,payload_json FROM maintenance_scans WHERE kind='unused_removal_operation' ORDER BY id")?;
    let mut rows = BTreeMap::new();
    for row in q.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))? {
        let (id, text) = row?;
        let op: Operation = serde_json::from_str(&text)?;
        if workspaces.contains(&op.workspace) {
            ensure!(
                id == op.result.id
                    && matches!(op.state.as_str(), "committed" | "undone" | "rolled_back")
                    && op.cleanup_complete
                    && !op.backups_disposed,
                "Resolve recovery or restore before disposing these backups"
            );
            rows.insert(id, digest(&text));
        }
    }
    Ok(rows)
}
fn owned(lib: &Library, b: &Backup) -> Result<()> {
    let root = lib.data_dir().canonicalize()?.join("maintenance/removal");
    let workspace = bytes_path(&b.workspace)?;
    let path = bytes_path(&b.path)?;
    ensure!(
        workspace.canonicalize()?.starts_with(&root) && path.parent() == Some(workspace.as_path()),
        "Disposal backup leaves owned recovery storage"
    );
    let name = path
        .file_name()
        .context("Missing backup name")?
        .to_string_lossy();
    ensure!(
        name.strip_prefix("original-")
            .is_some_and(|n| !n.is_empty() && n.bytes().all(|c| c.is_ascii_digit())),
        "Disposal target is not a recovery original"
    );
    Ok(())
}
fn validate(lib: &Library, backups: &[Backup], cancel: &CancellationToken) -> Result<()> {
    for b in backups {
        cancel.check()?;
        owned(lib, b)?;
        let path = bytes_path(&b.path)?;
        ensure!(
            identity(&path)? == b.identity && file_sha256_cancellable(&path, cancel)? == b.hash,
            "Recovery backup changed after review"
        );
    }
    Ok(())
}
pub(super) fn ensure_available(conn: &Connection, workspace: &[u8]) -> Result<()> {
    let mut q=conn.prepare("SELECT payload_json FROM maintenance_scans WHERE kind='unused_backup_disposal' AND json_extract(payload_json,'$.result.state') IN ('prepared','conflict')")?;
    for row in q.query_map([], |r| r.get::<_, String>(0))? {
        let disposal: Disposal = serde_json::from_str(&row?)?;
        ensure!(
            !disposal.workspaces.iter().any(|w| w == workspace),
            "Recovery originals are reserved by backup disposal"
        );
    }
    Ok(())
}
fn save_disposal(conn: &Connection, d: &Disposal) -> Result<()> {
    ensure!(conn.execute("UPDATE maintenance_scans SET payload_json=?1 WHERE id=?2 AND kind='unused_backup_disposal'",params![serde_json::to_string(d)?,d.result.id])?==1,"Backup disposal disappeared");
    Ok(())
}
fn load_disposal(lib: &Library, id: &str) -> Result<Disposal> {
    let d: Disposal = load(&*lib.conn()?, id, "unused_backup_disposal")?;
    ensure!(
        d.schema == "unused-backup-disposal-v1"
            && d.result.id == id
            && d.backups.len() == d.holding.len(),
        "Invalid backup disposal journal"
    );
    for (index, (b, holding)) in d.backups.iter().zip(&d.holding).enumerate() {
        owned(lib, b)?;
        let path = bytes_path(&b.path)?;
        let holding = bytes_path(holding)?;
        ensure!(
            holding
                == path
                    .parent()
                    .unwrap()
                    .join(format!(".dispose-{}-{index}", d.result.id)),
            "Disposal temporary is not owned by this journal"
        );
    }
    Ok(d)
}
fn finish_disposal(lib: &Library, d: &mut Disposal, error: Option<&anyhow::Error>) -> Result<()> {
    if let Some(error) = error {
        let mut failures = vec![];
        for (b, holding) in d.backups.iter().zip(&d.holding).rev() {
            let restored = (|| -> Result<()> {
                owned(lib, b)?;
                let target = bytes_path(&b.path)?;
                let holding = bytes_path(holding)?;
                if !missing(&holding)? {
                    let _ = identity(&holding)?;
                    ensure!(
                        missing(&target)?,
                        "New backup version preserved; captured original retained"
                    );
                    crate::publish_files::create(&holding, &target)?;
                } else {
                    ensure!(
                        !missing(&target)?
                            && identity(&target)? == b.identity
                            && cuerithm_media::file_sha256(&target)? == b.hash,
                        "Changed or missing backup preserved for review"
                    );
                }
                Ok(())
            })();
            if let Err(e) = restored {
                failures.push(format!("{e:#}"));
            }
        }
        d.result.state = if failures.is_empty() {
            "rolled_back"
        } else {
            "conflict"
        }
        .into();
        d.result.error = Some(if failures.is_empty() {
            format!("{error:#}")
        } else {
            format!(
                "{error:#}; recovery needs attention: {}",
                failures.join("; ")
            )
        });
        d.result.cleanup_pending = !failures.is_empty();
        save_disposal(&*lib.conn()?, d)?;
        return Ok(());
    }
    let cleanup = (|| -> Result<()> {
        for (b, holding) in d.backups.iter().zip(&d.holding) {
            owned(lib, b)?;
            let holding = bytes_path(holding)?;
            if missing(&holding)? {
                continue;
            }
            ensure!(
                identity(&holding)? == b.identity
                    && cuerithm_media::file_sha256(&holding)? == b.hash,
                "Unknown disposal temporary preserved"
            );
            ensure!(
                d.result.state == "committed",
                "Uncommitted backup must not be deleted"
            );
            std::fs::remove_file(&holding)?;
            crate::publish_files::sync_parent(&holding)?;
        }
        Ok(())
    })();
    d.result.cleanup_pending = cleanup.is_err();
    d.result.error = cleanup
        .err()
        .map(|e| format!("Backup cleanup pending: {e:#}"));
    save_disposal(&*lib.conn()?, d)
}
impl Library {
    pub fn preview_unused_backup_disposal(
        &self,
        ids: &[String],
        cancel: &CancellationToken,
    ) -> Result<UnusedBackupPreview> {
        cancel.check()?;
        ensure!(
            !ids.is_empty()
                && ids.len() <= 10000
                && ids.iter().collect::<BTreeSet<_>>().len() == ids.len(),
            "Choose distinct removal operations"
        );
        let mut backups = BTreeMap::new();
        let mut workspaces = BTreeSet::new();
        let mut restores_lost = vec![];
        for id in ids {
            cancel.check()?;
            let op = load_operation(self, id)?;
            ensure!(
                op.kind == "remove"
                    && matches!(op.state.as_str(), "committed" | "undone" | "rolled_back")
                    && op.cleanup_complete
                    && !op.backups_disposed,
                "This operation's backups are not available for disposal"
            );
            if op.state == "committed" && op.undone_by.is_none() {
                restores_lost.push(id.clone());
            }
            workspaces.insert(op.workspace.clone());
            for f in op.files {
                cancel.check()?;
                let path = bytes_path(&f.backup)?;
                let b = Backup {
                    path: f.backup,
                    workspace: op.workspace.clone(),
                    hash: f.entry.public.source_sha256.clone(),
                    identity: identity(&path)?,
                    public: UnusedBackupFile {
                        operation_id: id.clone(),
                        file_id: f.entry.public.id,
                        display_source_path: f.entry.public.display_path,
                        size_bytes: std::fs::metadata(&path)?.len(),
                        source_sha256: f.entry.public.source_sha256,
                    },
                };
                backups.entry(b.path.clone()).or_insert(b);
            }
        }
        let backups = backups.into_values().collect::<Vec<_>>();
        validate(self, &backups, cancel)?;
        let workspaces = workspaces.into_iter().collect::<Vec<_>>();
        let conn = self.conn()?;
        for w in &workspaces {
            ensure_available(&conn, w)?;
        }
        let rows = snapshots(&conn, &workspaces)?;
        let public = UnusedBackupPreview {
            id: uuid::Uuid::new_v4().to_string(),
            operation_ids: ids.to_vec(),
            files: backups.iter().map(|b| b.public.clone()).collect(),
            size_bytes: backups.iter().try_fold(0u64, |n, b| {
                n.checked_add(b.public.size_bytes)
                    .context("Backup size overflow")
            })?,
            restores_lost,
        };
        let plan = DisposalPreview {
            public: public.clone(),
            backups,
            rows,
            workspaces,
            consumed_by: None,
            logic: super::super::scan::logic_identity(),
        };
        cancel.check()?;
        conn.execute(
            "INSERT INTO maintenance_scans VALUES(?1,'unused_backup_disposal_plan',?2,?3)",
            params![public.id, serde_json::to_string(&plan)?, crate::now()],
        )?;
        Ok(public)
    }
    pub fn apply_unused_backup_disposal(
        &self,
        id: &str,
        cancel: &CancellationToken,
    ) -> Result<UnusedBackupDisposal> {
        cancel.check()?;
        let mut plan: DisposalPreview = load(&*self.conn()?, id, "unused_backup_disposal_plan")?;
        ensure!(
            plan.consumed_by.is_none() && plan.logic == super::super::scan::logic_identity(),
            "Backup disposal preview was consumed or changed"
        );
        validate(self, &plan.backups, cancel)?;
        #[cfg(feature = "autotag")]
        let _gate = self
            .inner
            .tag_gate
            .lock()
            .map_err(|_| anyhow::anyhow!("Publication gate interrupted"))?;
        let op_id = uuid::Uuid::new_v4().to_string();
        let holding = plan
            .backups
            .iter()
            .enumerate()
            .map(|(i, b)| {
                Ok(path_bytes(
                    &bytes_path(&b.path)?
                        .parent()
                        .unwrap()
                        .join(format!(".dispose-{op_id}-{i}")),
                ))
            })
            .collect::<Result<Vec<_>>>()?;
        let mut d = Disposal {
            schema: "unused-backup-disposal-v1".into(),
            result: UnusedBackupDisposal {
                id: op_id.clone(),
                preview_id: id.into(),
                operation_ids: plan.public.operation_ids.clone(),
                file_count: plan.backups.len(),
                size_bytes: plan.public.size_bytes,
                state: "prepared".into(),
                cleanup_pending: true,
                error: None,
            },
            backups: plan.backups.clone(),
            holding,
            workspaces: plan.workspaces.clone(),
        };
        {
            let mut conn = self.conn()?;
            let tx = conn.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
            ensure!(
                snapshots(&tx, &plan.workspaces)? == plan.rows,
                "Restore/recovery changed after backup review"
            );
            for w in &plan.workspaces {
                ensure_available(&tx, w)?;
            }
            let current: DisposalPreview = load(&tx, id, "unused_backup_disposal_plan")?;
            ensure!(
                current.consumed_by.is_none(),
                "Backup preview already applied"
            );
            cancel.check()?;
            tx.execute(
                "INSERT INTO maintenance_scans VALUES(?1,'unused_backup_disposal',?2,?3)",
                params![op_id, serde_json::to_string(&d)?, crate::now()],
            )?;
            tx.commit()?;
        }
        #[cfg(test)]
        tests::checkpoint("prepared");
        let outcome = (|| -> Result<()> {
            let mut conn = self.conn()?;
            let tx = conn.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
            ensure!(
                snapshots(&tx, &plan.workspaces)? == plan.rows,
                "Restore/recovery changed before backup disposal"
            );
            validate(self, &d.backups, cancel)?;
            for (b, holding) in d.backups.iter().zip(&d.holding) {
                cancel.check()?;
                let path = bytes_path(&b.path)?;
                let holding = bytes_path(holding)?;
                crate::publish_files::create(&path, &holding)?;
                ensure!(
                    identity(&holding)? == b.identity
                        && file_sha256_cancellable(&holding, cancel)? == b.hash,
                    "Backup changed during disposal exchange"
                );
            }
            #[cfg(test)]
            tests::checkpoint("files");
            for row_id in plan.rows.keys() {
                let mut op: Operation = load(&tx, row_id, "unused_removal_operation")?;
                op.backups_disposed = true;
                save(&tx, &op)?;
            }
            d.result.state = "committed".into();
            save_disposal(&tx, &d)?;
            plan.consumed_by = Some(op_id.clone());
            tx.execute(
                "UPDATE maintenance_scans SET payload_json=?1 WHERE id=?2",
                params![serde_json::to_string(&plan)?, id],
            )?;
            super::super::merge::advance(&tx)?;
            cancel.check()?;
            tx.commit()?;
            Ok(())
        })();
        #[cfg(test)]
        if outcome.is_ok() {
            tests::checkpoint("committed");
        }
        finish_disposal(self, &mut d, outcome.as_ref().err())?;
        outcome?;
        Ok(d.result)
    }
    pub fn unused_backup_disposal_history(&self) -> Result<Vec<UnusedBackupDisposal>> {
        let conn = self.conn()?;
        let mut q=conn.prepare("SELECT payload_json FROM maintenance_scans WHERE kind='unused_backup_disposal' ORDER BY created DESC,rowid DESC")?;
        q.query_map([], |r| r.get::<_, String>(0))?
            .map(|row| Ok(serde_json::from_str::<Disposal>(&row?)?.result))
            .collect()
    }
    pub fn retry_unused_backup_disposal(
        &self,
        id: &str,
        cancel: &CancellationToken,
    ) -> Result<Vec<UnusedBackupDisposal>> {
        cancel.check()?;
        #[cfg(feature = "autotag")]
        let _gate = self
            .inner
            .tag_gate
            .lock()
            .map_err(|_| anyhow::anyhow!("Publication gate interrupted"))?;
        let mut d = load_disposal(self, id)?;
        let pending = matches!(d.result.state.as_str(), "prepared" | "conflict");
        let error = anyhow::anyhow!("Explicit backup disposal recovery retry");
        finish_disposal(self, &mut d, pending.then_some(&error))?;
        self.unused_backup_disposal_history()
    }
    pub(crate) fn recover_unused_disposals(&self) -> Result<()> {
        let ids: Vec<String> = {
            let conn = self.conn()?;
            let mut q=conn.prepare("SELECT id FROM maintenance_scans WHERE kind='unused_backup_disposal' AND (json_extract(payload_json,'$.result.state')='prepared' OR (json_extract(payload_json,'$.result.state')='committed' AND json_extract(payload_json,'$.result.cleanup_pending')=1)) ORDER BY created,rowid")?;
            q.query_map([], |r| r.get(0))?
                .collect::<rusqlite::Result<_>>()?
        };
        for id in ids {
            let mut d = match load_disposal(self, &id) {
                Ok(d) => d,
                Err(error) => {
                    let mut raw: Disposal = load(&*self.conn()?, &id, "unused_backup_disposal")?;
                    if raw.result.state == "prepared" {
                        raw.result.state = "conflict".into();
                    }
                    raw.result.error = Some(format!(
                        "Backup disposal recovery assets need attention: {error:#}"
                    ));
                    raw.result.cleanup_pending = true;
                    save_disposal(&*self.conn()?, &raw)?;
                    continue;
                }
            };
            let error = anyhow::anyhow!("Interrupted backup disposal rolled back");
            let pending = d.result.state == "prepared";
            finish_disposal(self, &mut d, pending.then_some(&error))?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    thread_local! {static CRASH:std::cell::RefCell<Option<&'static str>>=const {std::cell::RefCell::new(None)};}
    pub(super) fn checkpoint(phase: &str) {
        CRASH.with(|slot| {
            if *slot.borrow() == Some(phase) {
                slot.take();
                panic!("simulated backup disposal interruption: {phase}");
            }
        });
    }
    #[test]
    fn backup_disposal_recovers_each_side_of_its_commit_and_keeps_restore_contract() {
        for phase in ["prepared", "files", "committed"] {
            let dir = tempfile::tempdir().unwrap();
            let root = dir.path().join("music");
            std::fs::create_dir(&root).unwrap();
            let file = root.join("generated.flac");
            std::fs::write(&file, b"generated source").unwrap();
            let db = dir.path().join("app/db");
            let lib = Library::open(&db).unwrap();
            let token = CancellationToken::default();
            let scan = lib
                .scan_unused_files(
                    UnusedScanRequest {
                        root,
                        extension_mode: ExtensionMode::All,
                        extensions: vec![],
                    },
                    &token,
                )
                .unwrap();
            let plan = lib
                .preview_unused_removal(&scan.id, &[scan.files[0].id.clone()], &token)
                .unwrap();
            let removed = lib.apply_unused_removal(&plan.id, &token).unwrap();
            let preview = lib
                .preview_unused_backup_disposal(std::slice::from_ref(&removed.id), &token)
                .unwrap();
            CRASH.with(|slot| *slot.borrow_mut() = Some(phase));
            assert!(
                std::panic::catch_unwind(std::panic::AssertUnwindSafe(
                    || lib.apply_unused_backup_disposal(&preview.id, &token)
                ))
                .is_err()
            );
            drop(lib);
            let lib = Library::open(&db).unwrap();
            let history = lib.unused_backup_disposal_history().unwrap();
            assert_eq!(
                history[0].state,
                if phase == "committed" {
                    "committed"
                } else {
                    "rolled_back"
                }
            );
            assert!(!history[0].cleanup_pending);
            let available = lib
                .unused_removal_history()
                .unwrap()
                .into_iter()
                .find(|h| h.result.id == removed.id)
                .unwrap()
                .restore_available;
            assert_eq!(available, phase != "committed");
            if available {
                lib.restore_unused_removal(&removed.id, &token).unwrap();
                assert_eq!(std::fs::read(&file).unwrap(), b"generated source");
            } else {
                assert!(!file.exists());
                assert!(lib.restore_unused_removal(&removed.id, &token).is_err());
            }
        }
    }
    #[test]
    fn disposal_recovery_preserves_a_new_backup_and_retry_restores_the_captured_original() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("music");
        std::fs::create_dir(&root).unwrap();
        let file = root.join("generated.flac");
        std::fs::write(&file, b"generated source").unwrap();
        let db = dir.path().join("app/db");
        let lib = Library::open(&db).unwrap();
        let token = CancellationToken::default();
        let scan = lib
            .scan_unused_files(
                UnusedScanRequest {
                    root,
                    extension_mode: ExtensionMode::All,
                    extensions: vec![],
                },
                &token,
            )
            .unwrap();
        let plan = lib
            .preview_unused_removal(&scan.id, &[scan.files[0].id.clone()], &token)
            .unwrap();
        let removed = lib.apply_unused_removal(&plan.id, &token).unwrap();
        let preview = lib
            .preview_unused_backup_disposal(std::slice::from_ref(&removed.id), &token)
            .unwrap();
        let details: DisposalPreview = load(
            &lib.conn().unwrap(),
            &preview.id,
            "unused_backup_disposal_plan",
        )
        .unwrap();
        let backup = bytes_path(&details.backups[0].path).unwrap();
        CRASH.with(|slot| *slot.borrow_mut() = Some("files"));
        assert!(
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(
                || lib.apply_unused_backup_disposal(&preview.id, &token)
            ))
            .is_err()
        );
        drop(lib);
        std::fs::write(&backup, b"newer backup version").unwrap();
        let lib = Library::open(&db).unwrap();
        assert_eq!(std::fs::read(&backup).unwrap(), b"newer backup version");
        let conflict = lib.unused_backup_disposal_history().unwrap().remove(0);
        assert_eq!(conflict.state, "conflict");
        assert!(conflict.cleanup_pending);
        assert!(lib.restore_unused_removal(&removed.id, &token).is_err());
        std::fs::remove_file(&backup).unwrap();
        lib.retry_unused_backup_disposal(&conflict.id, &token)
            .unwrap();
        lib.restore_unused_removal(&removed.id, &token).unwrap();
        assert_eq!(std::fs::read(file).unwrap(), b"generated source");
    }
}
