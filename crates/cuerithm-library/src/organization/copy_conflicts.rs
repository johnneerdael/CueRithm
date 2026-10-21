use super::{
    native::{self, Parent},
    publish::verify_destination,
    *,
};
use anyhow::{Context, Result, ensure};
use cuerithm_media::CancellationToken;
use rusqlite::{OptionalExtension, Transaction, params};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct OrganizationReferenceSourceProof {
    pub(crate) from_sha256: String,
    pub(crate) decision_digest: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CopyConflict {
    source_parent: Authority,
    source: Fingerprint,
    destination_parent: Authority,
    destination: Fingerprint,
    choice: Option<AlternateResolution>,
    settings_revision: Option<String>,
}
impl CopyConflict {
    fn same_revision(&self, other: &Self) -> bool {
        self.source_parent.path == other.source_parent.path
            && self.source_parent.ancestors == other.source_parent.ancestors
            && self.destination_parent.path == other.destination_parent.path
            && self.destination_parent.ancestors == other.destination_parent.ancestors
            && native::matches(&self.source, &other.source)
            && native::matches(&self.destination, &other.destination)
    }
    fn digest(&self, record: &OrganizationRecord) -> Result<String> {
        let file = &record.files[0];
        Ok(format!(
            "{:x}",
            Sha256::digest(serde_json::to_vec(&(
                &record.id,
                &file.source,
                &file.destination,
                self
            ))?)
        ))
    }
}
fn capture_copy(record: &OrganizationRecord) -> Result<CopyConflict> {
    ensure!(
        record.mode == OrganizationMode::Copy && record.undo_of.is_none(),
        "Only an original Copy operation can resolve a changed original"
    );
    let file = record.files.first().context("Missing Copy source")?;
    ensure!(
        file.published && file.stage_complete,
        "The managed copy has not completed verified publication"
    );
    verify_destination(file)?;
    let source_path = file.source.path()?;
    Parent::reopen(&file.source_parent)?;
    let (source_parent, authority) =
        Parent::capture(source_path.parent().context("Missing original parent")?)?;
    ensure!(
        authority.path == file.source_parent.path
            && authority.ancestors == file.source_parent.ancestors,
        "Original parent authority changed; preserve both files and restore the authorized folder"
    );
    let source = source_parent.fingerprint(Path::new(
        source_path.file_name().context("Missing original name")?,
    ))?;
    Ok(CopyConflict {
        source_parent: authority,
        source,
        destination_parent: file.destination_parent.clone(),
        destination: file
            .staged
            .clone()
            .context("Missing captured managed-copy identity")?,
        choice: None,
        settings_revision: None,
    })
}
type StoredCopyConflict = (String, String, CopyConflict, Option<String>);

fn load_conflict(conn: &rusqlite::Connection, id: &str) -> Result<Option<StoredCopyConflict>> {
    let row: Option<(String, String, String, Option<String>)> = conn.query_row("SELECT state,digest,payload_json,queued_job_id FROM organization_copy_conflicts WHERE operation_id=?1", [id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?))).optional()?;
    row.map(|(state, digest, json, job)| Ok((state, digest, serde_json::from_str(&json)?, job)))
        .transpose()
}
fn alternate_tx(
    tx: &Transaction<'_>,
    record: &OrganizationRecord,
    state: &str,
    observed: Option<&str>,
    ignored: Option<&str>,
) -> Result<()> {
    let file = &record.files[0];
    tx.execute("INSERT INTO organization_alternates(operation_id,track_id,path,source_hash,state,observed_hash,ignored_hash) VALUES(?1,?2,?3,?4,?5,?6,?7) ON CONFLICT(operation_id) DO UPDATE SET state=excluded.state,observed_hash=excluded.observed_hash,ignored_hash=excluded.ignored_hash", params![record.id, record.track_id, file.source.0, file.original.sha256, state, observed, ignored])?;
    Ok(())
}
impl Library {
    fn persist_pending_copy_conflict(
        &self,
        record: &OrganizationRecord,
        observed: CopyConflict,
    ) -> Result<Option<OrganizationReferenceSourceProof>> {
        let mut conn = self.conn()?;
        let tx = conn.transaction()?;
        if let Some((state, digest, previous, _)) = load_conflict(&tx, &record.id)?
            && matches!(state.as_str(), "chosen" | "committed")
            && previous.choice.is_some()
            && previous.same_revision(&observed)
        {
            ensure!(
                previous.digest(record)? == digest,
                "Copy decision evidence is inconsistent"
            );
            return Ok(Some(OrganizationReferenceSourceProof {
                from_sha256: previous.source.sha256,
                decision_digest: Some(digest),
            }));
        }
        let digest = observed.digest(record)?;
        tx.execute("INSERT INTO organization_copy_conflicts(operation_id,state,digest,payload_json,created,updated) VALUES(?1,'pending',?2,?3,?4,?4) ON CONFLICT(operation_id) DO UPDATE SET state='pending',digest=excluded.digest,payload_json=excluded.payload_json,queued_job_id=NULL,updated=excluded.updated", params![record.id, digest, serde_json::to_string(&observed)?, crate::model::now()])?;
        alternate_tx(&tx, record, "conflict", Some(&observed.source.sha256), None)?;
        Self::bump(&tx)?;
        tx.commit()?;
        Ok(None)
    }
    pub(crate) fn organization_reference_source_proof(
        &self,
        record: &OrganizationRecord,
    ) -> Result<OrganizationReferenceSourceProof> {
        let file = record
            .files
            .first()
            .context("Missing organization source")?;
        let parent = Parent::reopen(&file.source_parent)?;
        let path = file.source.path()?;
        let actual =
            parent.fingerprint(Path::new(path.file_name().context("Missing source name")?))?;
        if native::matches(&actual, &file.original) {
            if record.mode == OrganizationMode::Copy {
                let mut conn = self.conn()?;
                let tx = conn.transaction()?;
                let restored = tx.execute("UPDATE organization_copy_conflicts SET state='original_restored',updated=?1 WHERE operation_id=?2 AND state IN ('pending','chosen')", params![crate::model::now(), record.id])?;
                if restored > 0 {
                    alternate_tx(&tx, record, "active", None, None)?;
                    Self::bump(&tx)?;
                }
                tx.commit()?;
            }
            return Ok(OrganizationReferenceSourceProof {
                from_sha256: file.original.sha256.clone(),
                decision_digest: None,
            });
        }
        ensure!(
            record.mode == OrganizationMode::Copy
                && record.phase == OrganizationPhase::AwaitingReferences,
            "Organization source changed; preserve the operation for recovery"
        );
        let observed = capture_copy(record)?;
        if let Some(proof) = self.persist_pending_copy_conflict(record, observed)? {
            return Ok(proof);
        }
        anyhow::bail!(
            "Copy original changed after the managed copy was published. Choose Keep managed copy or Add changed original in organization history."
        )
    }
    pub(crate) fn validate_organization_reference_source_proof(
        &self,
        record: &OrganizationRecord,
        proof: &OrganizationReferenceSourceProof,
    ) -> Result<()> {
        // Once the path and optional new Incoming job commit, Copy no longer
        // owns the original. Its worker may already be tagging those bytes.
        if record.mode == OrganizationMode::Copy
            && matches!(
                record.phase,
                OrganizationPhase::Cleanup | OrganizationPhase::Complete
            )
            && let Some(expected) = &proof.decision_digest
        {
            let saved = load_conflict(&*self.conn()?, &record.id)?
                .context("Missing committed Copy decision")?;
            ensure!(
                saved.0 == "committed"
                    && &saved.1 == expected
                    && saved.2.digest(record)? == *expected
                    && saved.2.source.sha256 == proof.from_sha256,
                "Committed Copy decision changed"
            );
            verify_destination(&record.files[0])?;
            return Ok(());
        }
        ensure!(
            &self.organization_reference_source_proof(record)? == proof,
            "Copy original changed after its explicit decision; choose again for the current revision"
        );
        Ok(())
    }
    pub(crate) fn organization_copy_conflict_ack_tx(
        &self,
        tx: &Transaction<'_>,
        record: &OrganizationRecord,
        proof: &OrganizationReferenceSourceProof,
    ) -> Result<()> {
        let Some(expected) = &proof.decision_digest else {
            ensure!(
                proof.from_sha256 == record.files[0].original.sha256,
                "Unapproved source hash cannot authorize reference repair"
            );
            return Ok(());
        };
        ensure!(
            record.mode == OrganizationMode::Copy && record.undo_of.is_none(),
            "Copy approval cannot authorize another operation"
        );
        let (state, digest, decision, _) =
            load_conflict(tx, &record.id)?.context("Missing explicit Copy decision")?;
        ensure!(
            state == "chosen"
                && digest == *expected
                && decision.digest(record)? == digest
                && decision.choice.is_some()
                && decision.source.sha256 == proof.from_sha256,
            "Copy decision changed before local path acknowledgement"
        );
        Ok(())
    }
    pub(crate) fn complete_organization_copy_conflict_tx(
        &self,
        tx: &Transaction<'_>,
        record: &OrganizationRecord,
        proof: Option<&OrganizationReferenceSourceProof>,
    ) -> Result<()> {
        if record.mode != OrganizationMode::Copy {
            return Ok(());
        }
        let approved = proof.and_then(|proof| proof.decision_digest.as_ref());
        if let Some(digest) = approved {
            let (state, saved_digest, decision, job) =
                load_conflict(tx, &record.id)?.context("Missing Copy decision at path commit")?;
            ensure!(
                state == "chosen" && saved_digest == *digest && decision.digest(record)? == *digest,
                "Copy decision changed during path commit"
            );
            let queued = match decision.choice.context("Missing Copy choice")? {
                AlternateResolution::IgnoreThisRevision => {
                    alternate_tx(
                        tx,
                        record,
                        "ignored",
                        Some(&decision.source.sha256),
                        Some(&decision.source.sha256),
                    )?;
                    None
                }
                AlternateResolution::AddAsNewIncoming => {
                    alternate_tx(tx, record, "retired", Some(&decision.source.sha256), None)?;
                    let source = record.files[0].source.path()?;
                    // The managed track has already moved to its captured copy
                    // in this same transaction, so enqueue cannot adopt it.
                    Some(if let Some(job) = job {
                        job
                    } else {
                        Self::enqueue_one(
                            tx,
                            &source,
                            &decision.source.sha256,
                            decision
                                .settings_revision
                                .as_deref()
                                .context("Missing new Incoming settings revision")?,
                        )?
                    })
                }
            };
            tx.execute("UPDATE organization_copy_conflicts SET state='committed',queued_job_id=?1,updated=?2 WHERE operation_id=?3 AND digest=?4", params![queued, crate::model::now(), record.id, digest])?;
        } else {
            ensure!(!tx.query_row("SELECT EXISTS(SELECT 1 FROM organization_copy_conflicts WHERE operation_id=?1 AND state='chosen')", [&record.id], |r| r.get::<_, bool>(0))?, "A Copy decision requires its checked reference guard");
            alternate_tx(tx, record, "active", None, None)?;
            tx.execute("UPDATE organization_copy_conflicts SET state='original_restored',updated=?1 WHERE operation_id=?2", params![crate::model::now(), record.id])?;
        }
        Ok(())
    }
    pub(super) fn organization_ignored_copy_revision(
        &self,
        record: &OrganizationRecord,
        path: &Path,
        hash: &str,
    ) -> Result<bool> {
        let saved = {
            let conn = self.conn()?;
            load_conflict(&conn, &record.id)?
        };
        let Some((state, digest, decision, _)) = saved else {
            // Preserve pre-proof completed-copy decisions recorded by older builds.
            return Ok(true);
        };
        if state != "committed"
            || decision.choice != Some(AlternateResolution::IgnoreThisRevision)
            || decision.source.sha256 != hash
            || decision.digest(record)? != digest
            || record.files[0].source != NativePath::new(path)
        {
            return Ok(false);
        }
        let parent = Parent::reopen(&decision.source_parent)?;
        let actual = parent.fingerprint(Path::new(
            path.file_name().context("Missing ignored original name")?,
        ))?;
        Ok(native::matches(&actual, &decision.source))
    }
    pub(super) fn record_completed_copy_choice_tx(
        &self,
        tx: &Transaction<'_>,
        record: &OrganizationRecord,
        choice: AlternateResolution,
        source: &Fingerprint,
        revision: &str,
        queued_job: Option<&str>,
    ) -> Result<()> {
        let file = &record.files[0];
        let decision = CopyConflict {
            source_parent: file.source_parent.clone(),
            source: source.clone(),
            destination_parent: file.destination_parent.clone(),
            destination: file.staged.clone().context("Missing managed-copy proof")?,
            choice: Some(choice),
            settings_revision: Some(revision.into()),
        };
        tx.execute("INSERT INTO organization_copy_conflicts(operation_id,state,digest,payload_json,queued_job_id,created,updated) VALUES(?1,'committed',?2,?3,?4,?5,?5) ON CONFLICT(operation_id) DO UPDATE SET state='committed',digest=excluded.digest,payload_json=excluded.payload_json,queued_job_id=excluded.queued_job_id,updated=excluded.updated", params![record.id, decision.digest(record)?, serde_json::to_string(&decision)?, queued_job, crate::model::now()])?;
        Ok(())
    }
    pub fn resolve_organization_alternate_cancellable(
        &self,
        id: &str,
        resolution: AlternateResolution,
        cancel: &CancellationToken,
    ) -> Result<Option<crate::JobId>> {
        cancel.check()?;
        let record = self.organization_record(id)?;
        if record.phase == OrganizationPhase::Complete {
            let saved = {
                let conn = self.conn()?;
                load_conflict(&conn, id)?
            };
            if let Some((state, _, previous, job)) = saved
                && state == "committed"
                && previous.choice == Some(resolution)
                && capture_copy(&record).is_ok_and(|current| current.same_revision(&previous))
            {
                return Ok(job);
            }
            return self.resolve_completed_organization_alternate(id, resolution);
        }
        self.choose_pending_copy_resolution(id, resolution)?;
        // Do not hold organization/tag/DB locks across retry or recovery.
        self.continue_pending_copy_resolution(id, cancel)
    }
    pub(super) fn choose_pending_copy_resolution(
        &self,
        id: &str,
        resolution: AlternateResolution,
    ) -> Result<()> {
        let _gate = self
            .inner
            .organization_gate
            .lock()
            .map_err(|_| anyhow::anyhow!("Organization gate interrupted"))?;
        let record = self.organization_record(id)?;
        ensure!(
            record.phase == OrganizationPhase::AwaitingReferences,
            "Finish verified Copy publication before resolving its original"
        );
        let mut decision = capture_copy(&record)?;
        ensure!(
            !native::matches(&decision.source, &record.files[0].original),
            "The original matches the captured revision again; retry organization recovery"
        );
        decision.choice = Some(resolution);
        decision.settings_revision = Some(self.settings()?.revision);
        let digest = decision.digest(&record)?;
        let mut conn = self.conn()?;
        let tx = conn.transaction()?;
        tx.execute("INSERT INTO organization_copy_conflicts(operation_id,state,digest,payload_json,created,updated) VALUES(?1,'chosen',?2,?3,?4,?4) ON CONFLICT(operation_id) DO UPDATE SET state='chosen',digest=excluded.digest,payload_json=excluded.payload_json,queued_job_id=NULL,updated=excluded.updated", params![record.id, digest, serde_json::to_string(&decision)?, crate::model::now()])?;
        alternate_tx(
            &tx,
            &record,
            "conflict",
            Some(&decision.source.sha256),
            None,
        )?;
        Self::bump(&tx)?;
        tx.commit()?;
        Ok(())
    }
    fn continue_pending_copy_resolution(
        &self,
        id: &str,
        cancel: &CancellationToken,
    ) -> Result<Option<crate::JobId>> {
        cancel.check()?;
        let binding: Option<(String, Option<String>)> = self.conn()?.query_row("SELECT r.batch_id,(SELECT p.job_id FROM preparation_checkpoints p WHERE p.batch_id=r.batch_id AND p.state!='complete') FROM organization_batch_rows r WHERE r.operation_id=?1", [id], |r| Ok((r.get(0)?, r.get(1)?))).optional()?;
        if let Some((batch, job)) = binding {
            if let Some(job) = job {
                return Ok(Some(self.retry(&job, None)?));
            }
            let recovered = self.recover_organization_batch(&batch, cancel)?;
            ensure!(
                self.organization_record(id)?.phase == OrganizationPhase::Complete,
                "Copy decision saved; continuation still needs recovery: {}",
                recovered.error.unwrap_or(recovered.state)
            );
        } else {
            let guard = self.repair_organization_references(id)?;
            cancel.check()?;
            self.acknowledge_organization_references_checked(id, &guard)?;
            self.execute_organization_with_checked(id, &guard, |_| cancel.check())?;
        }
        let job: Option<String> = self.conn()?.query_row(
            "SELECT queued_job_id FROM organization_copy_conflicts WHERE operation_id=?1",
            [id],
            |r| r.get(0),
        )?;
        Ok(job)
    }
}
