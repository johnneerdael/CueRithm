use super::{journal::*, native::*, *};
use anyhow::{Context, Result, ensure};
use rusqlite::{OptionalExtension, params};
use serde::{Deserialize, Serialize};
use std::path::Path;
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AlternateSourceState {
    Unowned,
    Owned {
        track_id: String,
    },
    Conflict {
        operation_id: String,
        track_id: String,
    },
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlternateSourceRecord {
    pub operation_id: String,
    pub track_id: String,
    pub path: NativePath,
    pub captured_sha256: String,
    pub state: String,
    pub observed_sha256: Option<String>,
    pub ignored_sha256: Option<String>,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AlternateResolution {
    IgnoreThisRevision,
    AddAsNewIncoming,
}
impl Library {
    /// Reopen can roll publication/cleanup forward using only hash-bound native
    /// evidence. AwaitingReferences never authorizes source cleanup by itself.
    /// Conflicts remain discoverable history errors, rather than blocking opening.
    pub fn recover_organization(&self) -> Result<Vec<OrganizationRecord>> {
        let records = self.organization_history()?;
        for record in &records {
            if matches!(
                record.phase,
                OrganizationPhase::Publishing | OrganizationPhase::Cleanup
            ) {
                // Service-owned recovery must obtain current reference receipts
                // before any remaining source quarantine after a restart.
                if self.organization_service_owned(&record.id)?
                    || self.organization_has_required_references(&record.id)?
                {
                    continue;
                }
                let _preserved_error = self.execute_organization(&record.id);
            }
        }
        self.organization_history()
    }
    pub(crate) fn organization_alternate_records(&self) -> Result<Vec<AlternateSourceRecord>> {
        let conn = self.conn()?;
        let mut statement=conn.prepare("SELECT operation_id,track_id,path,source_hash,state,observed_hash,ignored_hash FROM organization_alternates ORDER BY rowid")?;
        Ok(statement
            .query_map([], |r| {
                Ok(AlternateSourceRecord {
                    operation_id: r.get(0)?,
                    track_id: r.get(1)?,
                    path: NativePath(r.get(2)?),
                    captured_sha256: r.get(3)?,
                    state: r.get(4)?,
                    observed_sha256: r.get(5)?,
                    ignored_sha256: r.get(6)?,
                })
            })?
            .collect::<rusqlite::Result<_>>()?)
    }
    /// Called by both watch and explicit enqueue. Same bytes are suppressed;
    /// changed bytes become a durable, discoverable source conflict.
    pub fn organization_source_state(
        &self,
        path: &Path,
        hash: &str,
    ) -> Result<AlternateSourceState> {
        self.organization_source_state_with(path, hash, owned_revision)
    }
    pub(crate) fn organization_source_state_with(
        &self,
        path: &Path,
        hash: &str,
        mut validate: impl FnMut(&OrganizationRecord, &Path, &str) -> Result<bool>,
    ) -> Result<AlternateSourceState> {
        let encoded = crate::model::path_bytes(path);
        for _ in 0..4 {
            let snapshot = {
                let conn = self.conn()?;
                ownership_snapshot(&conn, &encoded)?
            };
            let (operation, result) = match &snapshot {
                OwnershipSnapshot::Unowned => return Ok(AlternateSourceState::Unowned),
                OwnershipSnapshot::Alternate {
                    operation_id,
                    track_id,
                    expected,
                    state,
                    ignored,
                    payload,
                    ..
                } => {
                    let operation: OrganizationRecord = serde_json::from_str(payload)?;
                    let owned = validate(&operation, path, hash).unwrap_or(false);
                    let ignored_revision = state == "ignored"
                        && ignored.as_deref() == Some(hash)
                        && self
                            .organization_ignored_copy_revision(&operation, path, hash)
                            .unwrap_or(false);
                    let result = if (hash == expected && owned) || ignored_revision {
                        AlternateSourceState::Owned {
                            track_id: track_id.clone(),
                        }
                    } else {
                        AlternateSourceState::Conflict {
                            operation_id: operation_id.clone(),
                            track_id: track_id.clone(),
                        }
                    };
                    (operation, result)
                }
                OwnershipSnapshot::Reserved {
                    track_id, payload, ..
                } => {
                    let operation: OrganizationRecord = serde_json::from_str(payload)?;
                    let result = if validate(&operation, path, hash).unwrap_or(false) {
                        AlternateSourceState::Owned {
                            track_id: track_id.clone(),
                        }
                    } else {
                        AlternateSourceState::Conflict {
                            operation_id: operation.id.clone(),
                            track_id: track_id.clone(),
                        }
                    };
                    (operation, result)
                }
            };
            let mut conn = self.conn()?;
            let tx = conn.transaction()?;
            // Filesystem work deliberately happens without the connection lock.
            // Recheck the complete ownership revision before recording/returning it.
            if ownership_snapshot(&tx, &encoded)? != snapshot {
                tx.rollback()?;
                continue;
            }
            if matches!(result, AlternateSourceState::Conflict { .. })
                && matches!(snapshot, OwnershipSnapshot::Alternate { .. })
            {
                tx.execute("UPDATE organization_alternates SET state='conflict',observed_hash=?1 WHERE operation_id=?2",params![hash,operation.id])?;
                Self::bump(&tx)?;
            }
            tx.commit()?;
            return Ok(result);
        }
        anyhow::bail!("Organization source ownership changed during validation; retry adding it")
    }
    pub fn resolve_organization_alternate(
        &self,
        id: &str,
        resolution: AlternateResolution,
    ) -> Result<Option<crate::JobId>> {
        self.resolve_organization_alternate_cancellable(
            id,
            resolution,
            &cuerithm_media::CancellationToken::default(),
        )
    }
    pub(super) fn resolve_completed_organization_alternate(
        &self,
        id: &str,
        resolution: AlternateResolution,
    ) -> Result<Option<crate::JobId>> {
        let _gate = self
            .inner
            .organization_gate
            .lock()
            .map_err(|_| anyhow::anyhow!("Organization gate interrupted"))?;
        let record = self.organization_record(id)?;
        ensure!(
            record.mode == OrganizationMode::Copy && record.phase == OrganizationPhase::Complete,
            "Only completed copy alternates can be resolved"
        );
        let source = &record.files[0];
        super::publish::verify_destination(source)?;
        let parent = Parent::reopen(&source.source_parent)?;
        let path = source.source.path()?;
        let fingerprint = parent.fingerprint(Path::new(
            path.file_name().context("Missing alternate name")?,
        ))?;
        let hash = &fingerprint.sha256;
        let revision = self.settings()?.revision;
        let mut conn = self.conn()?;
        let tx = conn.transaction()?;
        let state: String = tx.query_row(
            "SELECT state FROM organization_alternates WHERE operation_id=?1",
            [id],
            |r| r.get(0),
        )?;
        ensure!(
            state == "conflict",
            "Alternate no longer has an unresolved conflict"
        );
        let job = match resolution {
            AlternateResolution::IgnoreThisRevision => {
                tx.execute("UPDATE organization_alternates SET state='ignored',ignored_hash=?1 WHERE operation_id=?2",params![hash,id])?;
                None
            }
            AlternateResolution::AddAsNewIncoming => {
                tx.execute(
                    "UPDATE organization_alternates SET state='retired' WHERE operation_id=?1",
                    [id],
                )?;
                Some(Self::enqueue_one(&tx, &path, hash, &revision)?)
            }
        };
        self.record_completed_copy_choice_tx(
            &tx,
            &record,
            resolution,
            &fingerprint,
            &revision,
            job.as_deref(),
        )?;
        Self::bump(&tx)?;
        tx.commit()?;
        Ok(job)
    }
}

pub(super) fn owned_revision(
    operation: &OrganizationRecord,
    path: &Path,
    hash: &str,
) -> Result<bool> {
    let encoded = crate::model::path_bytes(path);
    for file in &operation.files {
        let (parent, expected) = if file.destination.0 == encoded && !file.unchanged {
            let Some(staged) = &file.staged else {
                return Ok(false);
            };
            let parent = Parent::reopen(&file.destination_parent)?;
            if !file.published
                && (!file.stage_complete || parent.optional(Path::new(&file.stage_name))?.is_some())
            {
                return Ok(false);
            }
            (parent, staged)
        } else if file.source.0 == encoded {
            let expected = if file.case_only && file.published {
                file.staged
                    .as_ref()
                    .context("Missing case-rename identity")?
            } else {
                &file.original
            };
            (Parent::reopen(&file.source_parent)?, expected)
        } else {
            continue;
        };
        let actual = parent.fingerprint(Path::new(
            path.file_name().context("Missing organization file name")?,
        ))?;
        return Ok(hash == actual.sha256 && matches(&actual, expected));
    }
    Ok(false)
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum OwnershipSnapshot {
    Unowned,
    Alternate {
        operation_id: String,
        track_id: String,
        expected: String,
        state: String,
        ignored: Option<String>,
        observed: Option<String>,
        decision_digest: Option<String>,
        payload: String,
    },
    Reserved {
        operation_id: String,
        track_id: String,
        payload: String,
    },
}
fn ownership_snapshot(conn: &rusqlite::Connection, path: &[u8]) -> Result<OwnershipSnapshot> {
    if let Some(alternate)=conn.query_row("SELECT a.operation_id,a.track_id,a.source_hash,a.state,a.ignored_hash,a.observed_hash,o.payload_json,(SELECT c.digest FROM organization_copy_conflicts c WHERE c.operation_id=a.operation_id) FROM organization_alternates a JOIN organization_operations o ON o.id=a.operation_id WHERE a.path=?1 AND a.state!='retired' ORDER BY a.rowid DESC LIMIT 1",[path],|r|Ok(OwnershipSnapshot::Alternate{operation_id:r.get(0)?,track_id:r.get(1)?,expected:r.get(2)?,state:r.get(3)?,ignored:r.get(4)?,observed:r.get(5)?,payload:r.get(6)?,decision_digest:r.get(7)?})).optional()? {return Ok(alternate)}
    Ok(conn.query_row("SELECT o.id,o.track_id,o.payload_json FROM organization_reservations r JOIN organization_operations o ON o.id=r.operation_id WHERE r.path=?1",[path],|r|Ok(OwnershipSnapshot::Reserved{operation_id:r.get(0)?,track_id:r.get(1)?,payload:r.get(2)?})).optional()?.unwrap_or(OwnershipSnapshot::Unowned))
}
