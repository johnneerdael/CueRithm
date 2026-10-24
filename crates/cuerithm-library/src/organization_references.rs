//! Reference-only relocation. Native audio, metadata, cues and grids are never rewritten.
use crate::{
    Library,
    dj_sync_service::{KnownDjReference, OrganizationOwnershipTransition},
    organization::{
        NativePath, OrganizationPhase, OrganizationRecord, OrganizationReferenceSourceProof,
    },
};
use anyhow::{Context, Result, ensure};
use cuerithm_dj_sync::{
    PathReferenceSnapshot, PathRepairPreview, SourcePathChange, SyncTarget,
    transaction::{FileTransaction, JournalState, file_hash},
};
use rusqlite::{Transaction, params};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{fs, path::PathBuf};
pub(crate) fn import_binding_catalog(
    conn: &rusqlite::Connection,
    track: &str,
) -> Result<Vec<(String, String, Option<String>)>> {
    let mut q=conn.prepare("SELECT source_id,external_id,json_extract(identity_json,'$.location') FROM import_bindings WHERE item_id=?1 ORDER BY source_id,external_id")?;
    Ok(
        q.query_map([track], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?
            .collect::<rusqlite::Result<Vec<_>>>()?,
    )
}
pub(crate) fn import_binding_digest(
    conn: &rusqlite::Connection,
    track: &str,
) -> Result<Option<String>> {
    let rows = import_binding_catalog(conn, track)?;
    if rows.is_empty() {
        Ok(None)
    } else {
        Ok(Some(digest(&rows)?))
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrganizationReferenceInfo {
    pub id: String,
    pub target: String,
    pub label: String,
    pub state: String,
    pub reason: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrganizationReferenceSnapshot {
    pub digest: String,
    pub references: Vec<OrganizationReferenceInfo>,
    pub blockers: Vec<String>,
    pub estimated_bytes: u64,
    pub storage_paths: Vec<NativePath>,
}
/// A Rust-only lease of the native-sync lock, held through local acknowledgement.
pub struct OrganizationReferenceGuard {
    operation_id: String,
    digest: String,
    source: NativePath,
    destination: NativePath,
    source_hash: String,
    pub(crate) source_proof: OrganizationReferenceSourceProof,
    edges: Vec<PreparedEdge>,
    _lease: fs::File,
}
struct PreparedEdge {
    key: String,
    before: String,
    after: String,
    json: String,
}
impl OrganizationReferenceGuard {
    pub(crate) fn operation_id(&self) -> &str {
        &self.operation_id
    }
}
#[derive(Clone, Serialize, Deserialize)]
struct Reference {
    known: KnownDjReference,
    snapshot: Option<PathReferenceSnapshot>,
    registration: String,
    blocker: Option<String>,
}
#[derive(Clone, Serialize, Deserialize)]
struct Capture {
    track_id: String,
    source: NativePath,
    source_hash: String,
    catalog_digest: String,
    references: Vec<Reference>,
    #[serde(default)]
    unknown_references: Vec<OrganizationReferenceInfo>,
    blockers: Vec<String>,
}
#[derive(Clone, Serialize, Deserialize)]
struct Receipt {
    before: PathReferenceSnapshot,
    before_registration: String,
    #[serde(with = "cuerithm_dj_sync::path_serde::map")]
    after_files: std::collections::BTreeMap<PathBuf, Option<String>>,
    after_registration: String,
}
#[derive(Clone, Serialize, Deserialize)]
struct Entry {
    reference: Reference,
    journal: Option<String>,
    receipt: Option<Receipt>,
    abandoned: Vec<String>,
    #[serde(default)]
    ownership: Option<OrganizationOwnershipTransition>,
    #[serde(default)]
    anchor: Option<(PathReferenceSnapshot, String)>,
}
#[derive(Clone, Serialize, Deserialize)]
struct Intent {
    version: u8,
    capture: Capture,
    entries: Vec<Entry>,
    #[serde(default)]
    source_proof: Option<OrganizationReferenceSourceProof>,
}
fn digest(value: &impl Serialize) -> Result<String> {
    Ok(format!("{:x}", Sha256::digest(serde_json::to_vec(value)?)))
}
fn receipt_digest(
    files: &std::collections::BTreeMap<PathBuf, Option<String>>,
    registration: &str,
    directories: &std::collections::BTreeMap<
        PathBuf,
        Option<Vec<cuerithm_dj_sync::transaction::DirectoryEntry>>,
    >,
) -> Result<String> {
    #[derive(Serialize)]
    struct NativeReceipt<'a> {
        #[serde(with = "cuerithm_dj_sync::path_serde::map")]
        files: &'a std::collections::BTreeMap<PathBuf, Option<String>>,
        registration: &'a str,
        #[serde(with = "cuerithm_dj_sync::path_serde::map")]
        directories: &'a std::collections::BTreeMap<
            PathBuf,
            Option<Vec<cuerithm_dj_sync::transaction::DirectoryEntry>>,
        >,
    }
    digest(&NativeReceipt {
        files,
        registration,
        directories,
    })
}
impl Library {
    fn load_reference_intent(&self, id: &str) -> Result<(String, String, Intent)> {
        let (digest, state, json): (String, String, String) = {
            let conn = self.conn()?;
            conn.query_row("SELECT digest,state,payload_json FROM organization_reference_intents WHERE operation_id=?1",[id],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?)))?
        };
        let intent: Intent = serde_json::from_str(&json)?;
        ensure!(
            intent.version == 1,
            "Unsupported organization reference journal"
        );
        Ok((digest, state, intent))
    }

    fn capture_organization_references(&self, track_id: &str) -> Result<Capture> {
        let (path, source_hash): (Vec<u8>, String) = self.conn()?.query_row(
            "SELECT path,source_hash FROM tracks WHERE id=?1",
            [track_id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )?;
        let source = NativePath(path);
        let path = source.path()?;
        let known = self.known_organization_exports(track_id)?;
        let (legacy, native) = {
            let conn = self.conn()?;
            let legacy=conn.prepare("SELECT target,external_id,baseline_json FROM sync_links WHERE track_id=?1 ORDER BY target")?.query_map([track_id],|r|Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?,r.get::<_,String>(2)?)))?.collect::<rusqlite::Result<Vec<_>>>()?;
            let native=conn.prepare("SELECT target_id,content_id,status FROM native_track_links WHERE local_track_id=?1 ORDER BY target_id,content_id")?.query_map([track_id],|r|Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?,r.get::<_,String>(2)?)))?.collect::<rusqlite::Result<Vec<_>>>()?;
            (legacy, native)
        };
        let imported = import_binding_catalog(&*self.conn()?, track_id)?;
        let legacy_catalog = digest(&(
            &legacy,
            &native,
            known
                .iter()
                .map(|k| {
                    (
                        &k.key,
                        &k.catalog_digest,
                        k.independent,
                        NativePath::new(&k.path),
                    )
                })
                .collect::<Vec<_>>(),
        ))?;
        // Preserve saved pre-import identities exactly for unaffected libraries.
        let catalog_digest = if imported.is_empty() {
            legacy_catalog
        } else {
            digest(&("import-references-v1", legacy_catalog, &imported))?
        };
        let mut blockers=legacy.iter().map(|(target,_,_)|format!("Known legacy reference '{target}' has no registered path-only repair adapter")).chain(native.iter().map(|(target,content,_)|format!("Imported Rekordbox reference {target}/{content} has no registered database path/write adapter"))).collect::<Vec<_>>();
        let unknown_references = legacy
            .iter()
            .map(|(target, id, _)| OrganizationReferenceInfo {
                id: format!("legacy:{target}:{id}"),
                target: target.clone(),
                label: format!("Legacy link {id}"),
                state: "blocked".into(),
                reason: Some("This link has no registered path-only repair adapter".into()),
            })
            .chain(
                native
                    .iter()
                    .map(|(target, content, _)| OrganizationReferenceInfo {
                        id: format!("rekordbox:{target}:{content}"),
                        target: "rekordbox".into(),
                        label: format!("Imported Rekordbox track {content}"),
                        state: "blocked".into(),
                        reason: Some(
                            "The imported database has no registered path/write adapter".into(),
                        ),
                    }),
            )
            .collect();
        let mut references = vec![];
        for item in known {
            let mut reference = Reference {
                known: item,
                snapshot: None,
                registration: String::new(),
                blocker: None,
            };
            if !reference.known.independent {
                let captured = (|| -> Result<(PathReferenceSnapshot, String)> {
                    ensure!(
                        reference.known.path == path,
                        "Acknowledged native source path differs from the current library path"
                    );
                    ensure!(
                        !reference.known.registration.is_empty(),
                        "Native target has no registered ownership evidence"
                    );
                    let registration =
                        self.organization_target_registration(&reference.known.target)?;
                    ensure!(
                        registration == reference.known.registration,
                        "Native DJ database was replaced; reconcile it before organizing"
                    );
                    Ok((
                        cuerithm_dj_sync::inspect_path_reference(&reference.known.target, &path)?,
                        registration,
                    ))
                })();
                match captured {
                    Ok((snapshot, registration)) => {
                        reference.snapshot = Some(snapshot);
                        reference.registration = registration;
                    }
                    Err(error) => {
                        let message = format!("{}: {error:#}", reference.known.key);
                        blockers.push(message.clone());
                        reference.blocker = Some(message);
                    }
                }
            }
            references.push(reference);
        }
        Ok(Capture {
            track_id: track_id.into(),
            source,
            source_hash,
            catalog_digest,
            references,
            unknown_references,
            blockers,
        })
    }
    /// Import orchestration has separately proved an owned tag replacement.
    /// Only that source byte hash may differ; every native reference proof stays exact.
    pub(crate) fn import_reference_refresh_same(
        &self,
        before: &str,
        after: &str,
        before_hash: &str,
        after_hash: &str,
    ) -> Result<bool> {
        let conn = self.conn()?;
        let load = |id: &str| -> Result<Capture> {
            let raw: String = conn.query_row(
                "SELECT payload_json FROM organization_reference_snapshots WHERE digest=?1",
                [id],
                |r| r.get(0),
            )?;
            Ok(serde_json::from_str(&raw)?)
        };
        let mut old = load(before)?;
        let fresh = load(after)?;
        ensure!(
            old.source_hash == before_hash && fresh.source_hash == after_hash,
            "Import reference source proof mismatch"
        );
        old.source_hash = fresh.source_hash.clone();
        Ok(serde_json::to_value(old)? == serde_json::to_value(fresh)?)
    }
    pub fn organization_reference_snapshot(
        &self,
        track_id: &str,
    ) -> Result<OrganizationReferenceSnapshot> {
        let capture = self.capture_organization_references(track_id)?;
        let hash = digest(&capture)?;
        let encoded_capture = serde_json::to_string(&capture)?;
        self.conn()?.execute("INSERT OR IGNORE INTO organization_reference_snapshots(digest,track_id,payload_json,created) VALUES(?1,?2,?3,?4)",params![hash,track_id,encoded_capture,crate::model::now()])?;
        let mut references:Vec<OrganizationReferenceInfo>=capture.references.iter().map(|r|OrganizationReferenceInfo{id:r.known.key.clone(),target:format!("{:?}",r.known.target.software),label:r.known.target.path.to_string_lossy().into_owned(),state:if r.known.independent {"independent_copy"}else if r.blocker.is_some(){"blocked"}else{"required"}.into(),reason:r.blocker.clone().or_else(||r.known.independent.then(||"This device export owns independent media; source organization does not change it".into()))}).collect();
        references.extend(capture.unknown_references.clone());
        let estimated_bytes = capture
            .references
            .iter()
            .filter_map(|r| r.snapshot.as_ref())
            .try_fold(0_u64, |sum, s| {
                sum.checked_add(s.estimated_bytes)
                    .context("Native repair storage estimate overflow")
            })?;
        let storage_paths = capture
            .references
            .iter()
            .filter_map(|r| r.snapshot.as_ref())
            .filter_map(|s| s.target.path.parent())
            .map(NativePath::new)
            .collect();
        Ok(OrganizationReferenceSnapshot {
            digest: hash,
            references,
            blockers: capture.blockers,
            estimated_bytes,
            storage_paths,
        })
    }
    pub fn bind_organization_references(
        &self,
        operation_id: &str,
        expected_digest: &str,
    ) -> Result<()> {
        // Serialize the final frozen binding with normal Sync. A Sync that
        // already passed its recovery check must finish before this snapshot
        // can authorize audio publication; later Sync sees the active intent.
        let _lease = self.organization_sync_lock()?;
        let record = self.organization_record(operation_id)?;
        if record.phase != OrganizationPhase::Planned {
            ensure!(
                record.phase == OrganizationPhase::AwaitingReferences,
                "References must be frozen before organization publication"
            );
            let original =
                self.organization_record(record.undo_of.as_deref().context(
                    "Published forward operation cannot acquire a new reference binding",
                )?)?;
            ensure!(
                original.mode == crate::organization::OrganizationMode::Copy
                    && original.phase == OrganizationPhase::Complete
                    && original.error.is_none()
                    && original.track_id == record.track_id
                    && original.files.len() == record.files.len(),
                "Late reference binding is limited to a verified Copy inverse"
            );
            for (inverse, forward) in record.files.iter().zip(&original.files) {
                let proof = inverse
                    .staged
                    .as_ref()
                    .context("Copy inverse lacks original-file proof")?;
                ensure!(
                    inverse.source == forward.destination
                        && inverse.destination == forward.source
                        && inverse.published
                        && inverse.stage_complete
                        && !inverse.quarantined
                        && !inverse.case_only
                        && proof.identity == forward.original.identity
                        && proof.sha256 == forward.original.sha256
                        && proof.bytes == forward.original.bytes,
                    "Copy inverse source ownership changed before reference binding"
                );
            }
            self.verify_organization_publication(operation_id)?;
        }
        let json: String = self
            .conn()?
            .query_row(
                "SELECT payload_json FROM organization_reference_snapshots WHERE digest=?1",
                [expected_digest],
                |r| r.get(0),
            )
            .context("Unknown reference snapshot; preview again")?;
        let capture: Capture = serde_json::from_str(&json)?;
        let main = &record.files[0];
        ensure!(
            capture.track_id == record.track_id
                && capture.source == main.source
                && capture.source_hash == main.original.sha256,
            "Organization reference source preview is stale"
        );
        ensure!(
            capture.blockers.is_empty(),
            "Required native references are blocked: {}",
            capture.blockers.join("; ")
        );
        ensure!(
            !main.case_only || capture.references.iter().all(|r| r.known.independent),
            "Linked case-only rename cannot keep its original reference path continuously available"
        );
        let current = self.capture_organization_references(&record.track_id)?;
        ensure!(
            current.catalog_digest == capture.catalog_digest,
            "Known native references changed after preview"
        );
        let mut entries = vec![];
        for mut reference in capture.references.iter().cloned() {
            if !reference.known.independent {
                self.rebase_organization_reference(&mut reference, &current)?;
            }
            entries.push(Entry {
                reference,
                journal: None,
                receipt: None,
                abandoned: vec![],
                ownership: None,
                anchor: None,
            });
        }
        let intent = Intent {
            version: 1,
            capture,
            entries,
            source_proof: None,
        };
        let encoded_intent = serde_json::to_string(&intent)?;
        self.conn()?.execute("INSERT INTO organization_reference_intents(operation_id,track_id,digest,payload_json,state,created,updated) VALUES(?1,?2,?3,?4,'bound',?5,?5) ON CONFLICT(operation_id) DO NOTHING",params![operation_id,record.track_id,expected_digest,encoded_intent,crate::model::now()])?;
        ensure!(
            self.load_reference_intent(operation_id)?.0 == expected_digest,
            "Organization reference snapshot was already bound differently"
        );
        Ok(())
    }
    pub fn repair_organization_references(
        &self,
        operation_id: &str,
    ) -> Result<OrganizationReferenceGuard> {
        self.repair_organization_references_with(operation_id, || {})
    }
    fn repair_organization_references_with(
        &self,
        operation_id: &str,
        before_verification: impl FnOnce(),
    ) -> Result<OrganizationReferenceGuard> {
        self.repair_organization_references_hooks(operation_id, before_verification, |_| {})
    }
    fn repair_organization_references_hooks(
        &self,
        operation_id: &str,
        before_verification: impl FnOnce(),
        mut before_target: impl FnMut(usize),
    ) -> Result<OrganizationReferenceGuard> {
        let lease = self.organization_sync_lock()?;
        let record = self.verify_organization_publication(operation_id)?;
        let (_, state, mut intent) = self.load_reference_intent(operation_id)?;
        ensure!(
            state != "acknowledged",
            "Organization references were already acknowledged"
        );
        let result = (|| -> Result<()> {
            self.save_reference_intent(operation_id, &intent, "repairing", None)?;
            let current = self.capture_organization_references(&record.track_id)?;
            ensure!(
                current.catalog_digest == intent.capture.catalog_digest,
                "Known native references changed after organization preview"
            );
            let main = &record.files[0];
            let source_proof = self.organization_reference_source_proof(&record)?;
            intent.source_proof = Some(source_proof.clone());
            self.save_reference_intent(operation_id, &intent, "repairing", None)?;
            let change = SourcePathChange {
                track_id: record.track_id.clone(),
                from: main.source.path()?,
                to: main.destination.path()?,
                source_sha256: main.original.sha256.clone(),
                approved_from_sha256: source_proof
                    .decision_digest
                    .as_ref()
                    .map(|_| source_proof.from_sha256.clone()),
            };
            for index in 0..intent.entries.len() {
                before_target(index);
                if intent.entries[index].reference.known.independent {
                    continue;
                }
                if let Some(receipt) = &intent.entries[index].receipt {
                    self.verify_organization_reference_receipt(
                        &intent.entries[index].reference,
                        receipt,
                    )?;
                    continue;
                }
                let mut restored = false;
                if let Some(id) = intent.entries[index].journal.clone() {
                    cuerithm_dj_sync::refresh_path_repair_source_proof(
                        &self.reference_journal_root(operation_id),
                        &id,
                        &change,
                    )?;
                    let state =
                        FileTransaction::open(&self.reference_journal_root(operation_id), &id)?
                            .inspect()?
                            .state;
                    if matches!(
                        state,
                        JournalState::Applying | JournalState::Restoring | JournalState::Restored
                    ) {
                        cuerithm_dj_sync::restore_path_repair(
                            &self.reference_journal_root(operation_id),
                            &id,
                        )?;
                        let previous = intent.entries[index]
                            .reference
                            .snapshot
                            .as_ref()
                            .context("Missing interrupted native reference snapshot")?;
                        let fresh = cuerithm_dj_sync::inspect_path_reference(
                            &intent.entries[index].reference.known.target,
                            &change.from,
                        )?;
                        ensure!(
                            fresh.files == previous.files
                                && fresh.external_id == previous.external_id,
                            "Restored native reference differs from its captured before state"
                        );
                        intent.entries[index].reference.snapshot = Some(fresh);
                        intent.entries[index].reference.registration = self
                            .organization_target_registration(
                                &intent.entries[index].reference.known.target,
                            )?;
                        intent.entries[index].abandoned.push(id);
                        intent.entries[index].journal = None;
                        restored = true;
                        self.save_reference_intent(operation_id, &intent, "repairing", None)?;
                    }
                }
                if intent.entries[index].journal.is_none() {
                    if !restored {
                        self.rebase_organization_reference(
                            &mut intent.entries[index].reference,
                            &current,
                        )?;
                    }
                    if intent.entries[index].anchor.is_none() {
                        intent.entries[index].anchor = Some((
                            intent.entries[index]
                                .reference
                                .snapshot
                                .clone()
                                .context("Native reference snapshot missing")?,
                            intent.entries[index].reference.registration.clone(),
                        ));
                    }
                    let snapshot = intent.entries[index]
                        .reference
                        .snapshot
                        .as_ref()
                        .context("Native reference has no repair proof")?;
                    let plan = cuerithm_dj_sync::prepare_path_repair(
                        &self.reference_journal_root(operation_id),
                        snapshot,
                        &change,
                    )?;
                    intent.entries[index].journal = Some(plan.id);
                    self.save_reference_intent(operation_id, &intent, "repairing", None)?;
                }
                self.finish_organization_reference_entry(operation_id, &mut intent.entries[index])?;
                self.save_reference_intent(operation_id, &intent, "repairing", None)?;
            }
            before_verification();
            self.validate_organization_reference_source_proof(&record, &source_proof)?;
            for index in 0..intent.entries.len() {
                if intent.entries[index].reference.known.independent {
                    continue;
                }
                self.verify_organization_reference_receipt(
                    &intent.entries[index].reference,
                    intent.entries[index]
                        .receipt
                        .as_ref()
                        .context("Required reference repair is incomplete")?,
                )?;
                if intent.entries[index].ownership.is_none() {
                    let receipt = intent.entries[index]
                        .receipt
                        .as_ref()
                        .context("Required reference repair is incomplete")?;
                    intent.entries[index].ownership = Some(self.plan_organization_ownership(
                        &intent.entries[index].reference.known.target,
                        &receipt.before.files,
                        &receipt.after_files,
                    )?);
                    self.save_reference_intent(operation_id, &intent, "repairing", None)?;
                }
                self.publish_organization_ownership(
                    intent.entries[index].ownership.as_ref().unwrap(),
                )?;
            }
            self.save_reference_intent(operation_id, &intent, "repaired", None)?;
            Ok(())
        })();
        if let Err(error) = result {
            self.save_reference_intent(
                operation_id,
                &intent,
                "repairing",
                Some(&format!("{error:#}")),
            )?;
            return Err(error);
        }
        self.organization_reference_guard(&record, &intent, lease)
    }
    pub(crate) fn organization_references_ack_tx(
        &self,
        tx: &Transaction<'_>,
        record: &OrganizationRecord,
        guard: &OrganizationReferenceGuard,
    ) -> Result<()> {
        ensure!(
            guard.operation_id == record.id,
            "Native reference lease belongs to another organization operation"
        );
        let (state,verified):(String,Option<String>)=tx.query_row("SELECT state,verification_digest FROM organization_reference_intents WHERE operation_id=?1",[&record.id],|r|Ok((r.get(0)?,r.get(1)?)))?;
        ensure!(
            state == "repaired" && verified.as_deref() == Some(guard.digest.as_str()),
            "Native reference receipt changed before local acknowledgement"
        );
        let main = &record.files[0];
        ensure!(
            guard.source == main.source
                && guard.destination == main.destination
                && guard.source_hash == main.original.sha256,
            "Native reference receipt has another path/content identity"
        );
        self.organization_copy_conflict_ack_tx(tx, record, &guard.source_proof)?;
        for edge in &guard.edges {
            tx.execute("INSERT INTO organization_reference_paths(track_id,target_key,path,operation_id,updated) VALUES(?1,?2,?3,?4,?5) ON CONFLICT(track_id,target_key) DO UPDATE SET path=excluded.path,operation_id=excluded.operation_id,updated=excluded.updated",params![record.track_id,edge.key,main.destination.0,record.id,crate::model::now()])?;
            tx.execute("INSERT OR REPLACE INTO organization_reference_edges(operation_id,target_key,before_digest,after_digest,payload_json) VALUES(?1,?2,?3,?4,?5)",params![record.id,edge.key,edge.before,edge.after,edge.json])?;
        }
        tx.execute("UPDATE organization_reference_intents SET state='acknowledged',updated=?1,error=NULL WHERE operation_id=?2",params![crate::model::now(),record.id])?;
        Ok(())
    }
    fn organization_reference_guard(
        &self,
        record: &OrganizationRecord,
        intent: &Intent,
        lease: fs::File,
    ) -> Result<OrganizationReferenceGuard> {
        let mut edges = vec![];
        for entry in &intent.entries {
            if !entry.reference.known.independent {
                let receipt = entry
                    .receipt
                    .as_ref()
                    .context("Required native receipt is incomplete")?;
                ensure!(
                    entry.ownership.is_some(),
                    "Required ownership publication is incomplete"
                );
                edges.push(PreparedEdge {
                    key: entry.reference.known.key.clone(),
                    before: receipt_digest(
                        &receipt.before.files,
                        &receipt.before_registration,
                        &receipt.before.directories,
                    )?,
                    after: receipt_digest(
                        &receipt.after_files,
                        &receipt.after_registration,
                        &receipt.before.directories,
                    )?,
                    json: serde_json::to_string(receipt)?,
                });
            }
        }
        let main = &record.files[0];
        Ok(OrganizationReferenceGuard {
            operation_id: record.id.clone(),
            digest: digest(intent)?,
            source: main.source.clone(),
            destination: main.destination.clone(),
            source_hash: main.original.sha256.clone(),
            source_proof: intent
                .source_proof
                .clone()
                .context("Missing reference source proof")?,
            edges,
            _lease: lease,
        })
    }
    pub fn verify_acknowledged_organization_references(
        &self,
        operation_id: &str,
    ) -> Result<OrganizationReferenceGuard> {
        let lease = self.organization_sync_lock()?;
        let (_, state, intent) = self.load_reference_intent(operation_id)?;
        ensure!(
            state == "acknowledged",
            "Organization references have no committed local acknowledgement"
        );
        let record = self.organization_record(operation_id)?;
        let guard = self.organization_reference_guard(&record, &intent, lease)?;
        self.validate_live_organization_reference_guard(&record, &guard)?;
        Ok(guard)
    }
    /// A lease excludes other CueRithm sync writers, not external programs.
    /// Recheck native evidence at each destructive/publication boundary, with
    /// all physical work completed before the caller opens its SQLite transaction.
    pub(crate) fn validate_live_organization_reference_guard(
        &self,
        record: &OrganizationRecord,
        guard: &OrganizationReferenceGuard,
    ) -> Result<()> {
        let main = record.files.first().context("Missing organization audio")?;
        ensure!(
            guard.operation_id == record.id
                && guard.source == main.source
                && guard.destination == main.destination
                && guard.source_hash == main.original.sha256,
            "Native reference guard belongs to another operation or content revision"
        );
        let (_, state, intent) = self.load_reference_intent(&record.id)?;
        let acknowledged = matches!(
            record.phase,
            OrganizationPhase::Cleanup | OrganizationPhase::Complete
        );
        ensure!(
            state
                == if acknowledged {
                    "acknowledged"
                } else {
                    "repaired"
                }
                && digest(&intent)? == guard.digest,
            "Native reference receipt changed before live validation"
        );
        if acknowledged {
            Self::verify_organization_reference_destinations(record)?;
            if guard.source_proof.decision_digest.is_some() {
                self.validate_organization_reference_source_proof(record, &guard.source_proof)?;
            }
        }
        for entry in &intent.entries {
            if entry.reference.known.independent {
                continue;
            }
            let receipt = entry
                .receipt
                .as_ref()
                .context("Required reference receipt is missing")?;
            if !acknowledged {
                self.verify_organization_reference_receipt(&entry.reference, receipt)?;
                continue;
            }
            let id = entry
                .journal
                .as_ref()
                .context("Missing native path-repair journal")?;
            let transaction = FileTransaction::open(&self.reference_journal_root(&record.id), id)?;
            let preview: PathRepairPreview = serde_json::from_reader(fs::File::open(
                transaction.path().join("path-repair.json"),
            )?)?;
            ensure!(
                preview.change.track_id == record.track_id
                    && preview.change.to == main.destination.path()?
                    && preview.change.source_sha256 == main.original.sha256
                    && digest(&preview.snapshot.target.identity_target())?
                        == entry.reference.known.key,
                "Native path-repair journal has another captured destination"
            );
            let current = cuerithm_dj_sync::inspect_path_reference(
                &entry.reference.known.target,
                &main.destination.path()?,
            )?;
            ensure!(
                current.external_id == preview.after_external_id,
                "Repaired native track identity changed before source cleanup"
            );
            let registration =
                self.organization_target_registration(&entry.reference.known.target)?;
            self.prove_organization_reference_chain(
                &entry.reference.known.key,
                receipt_digest(
                    &receipt.after_files,
                    &receipt.after_registration,
                    &receipt.before.directories,
                )?,
                &receipt_digest(&current.files, &registration, &current.directories)?,
            )?;
            // The native reader must not hide a write between its captured
            // hashes and the row/identity lookup.
            for (path, hash) in &current.files {
                ensure!(
                    file_hash(path)? == *hash,
                    "Native database changed during cleanup verification"
                );
            }
            for (path, manifest) in &current.directories {
                ensure!(
                    cuerithm_dj_sync::transaction::directory_manifest(path)? == *manifest,
                    "Native playlist hierarchy changed during cleanup verification"
                );
            }
            ensure!(
                self.organization_target_registration(&entry.reference.known.target)?
                    == registration,
                "Native database was replaced during cleanup verification"
            );
            cuerithm_dj_sync::discovery::ensure_closed(entry.reference.known.target.software)?;
        }
        Ok(())
    }
    pub(crate) fn organization_has_required_references(&self, operation_id: &str) -> Result<bool> {
        let exists: bool = self.conn()?.query_row(
            "SELECT EXISTS(SELECT 1 FROM organization_reference_intents WHERE operation_id=?1)",
            [operation_id],
            |r| r.get(0),
        )?;
        if !exists {
            return Ok(false);
        }
        let (_, _, intent) = self.load_reference_intent(operation_id)?;
        Ok(intent
            .entries
            .iter()
            .any(|e| !e.reference.known.independent))
    }
    pub fn organization_reference_retained_bytes(&self, operation_id: &str) -> Result<u64> {
        self.organization_record(operation_id)?;
        let root = self.reference_journal_root(operation_id);
        if !root.exists() {
            return Ok(0);
        }
        let mut total = 0_u64;
        for entry in fs::read_dir(root)? {
            let entry = entry?;
            if !entry.file_type()?.is_dir()
                || uuid::Uuid::parse_str(&entry.file_name().to_string_lossy()).is_err()
            {
                continue;
            }
            for kind in ["backup", "staged"] {
                let folder = entry.path().join(kind);
                if !folder.exists() {
                    continue;
                }
                for file in fs::read_dir(folder)? {
                    let file = file?;
                    let metadata = fs::symlink_metadata(file.path())?;
                    ensure!(
                        metadata.file_type().is_file(),
                        "Native recovery storage contains an unexpected filesystem entry"
                    );
                    total = total
                        .checked_add(metadata.len())
                        .context("Native retained storage byte count overflow")?;
                }
            }
        }
        Ok(total)
    }
    fn reference_journal_root(&self, operation_id: &str) -> PathBuf {
        self.data_dir()
            .join("organization-references")
            .join(operation_id)
    }
    fn save_reference_intent(
        &self,
        id: &str,
        intent: &Intent,
        state: &str,
        error: Option<&str>,
    ) -> Result<()> {
        let json = serde_json::to_string(intent)?;
        let verified =
            (state == "repaired").then(|| format!("{:x}", Sha256::digest(json.as_bytes())));
        self.conn()?.execute("UPDATE organization_reference_intents SET payload_json=?1,state=?2,error=?3,updated=?4,verification_digest=?6 WHERE operation_id=?5",params![json,state,error,crate::model::now(),id,verified])?;
        Ok(())
    }
    fn verify_organization_reference_receipt(
        &self,
        reference: &Reference,
        receipt: &Receipt,
    ) -> Result<()> {
        cuerithm_dj_sync::discovery::ensure_closed(reference.known.target.software)?;
        ensure!(
            self.organization_target_registration(&reference.known.target)?
                == receipt.after_registration,
            "Repaired native database was replaced before acknowledgement"
        );
        for (path, manifest) in &receipt.before.directories {
            ensure!(
                cuerithm_dj_sync::transaction::directory_manifest(path)? == *manifest,
                "Native playlist hierarchy changed before acknowledgement"
            );
        }
        for (path, hash) in &receipt.after_files {
            ensure!(
                file_hash(path)? == *hash,
                "Repaired native reference was edited before acknowledgement"
            );
        }
        Ok(())
    }
    fn finish_organization_reference_entry(
        &self,
        operation_id: &str,
        entry: &mut Entry,
    ) -> Result<()> {
        let id = entry
            .journal
            .as_ref()
            .context("Native path repair journal is missing")?;
        let root = self.reference_journal_root(operation_id);
        let state = FileTransaction::open(&root, id)?.inspect()?.state;
        match state {
            JournalState::Prepared => {
                ensure!(
                    self.organization_target_registration(&entry.reference.known.target)?
                        == entry.reference.registration,
                    "Native target was replaced after repair preview"
                );
                cuerithm_dj_sync::apply_path_repair(&root, id)?;
            }
            JournalState::Applied => {
                cuerithm_dj_sync::verify_path_repair(&root, id)?;
            }
            _ => anyhow::bail!(
                "Interrupted native path repair needs guarded restoration before retry"
            ),
        }
        let before = entry
            .reference
            .snapshot
            .clone()
            .context("Native reference snapshot missing")?;
        let after_files = before
            .files
            .keys()
            .map(|path| Ok((path.clone(), file_hash(path)?)))
            .collect::<Result<_>>()?;
        let after_registration =
            self.organization_target_registration(&entry.reference.known.target)?;
        let (anchor, anchor_registration) = entry
            .anchor
            .clone()
            .unwrap_or_else(|| (before.clone(), entry.reference.registration.clone()));
        entry.receipt = Some(Receipt {
            before: anchor,
            before_registration: anchor_registration,
            after_files,
            after_registration,
        });
        Ok(())
    }
    fn rebase_organization_reference(
        &self,
        reference: &mut Reference,
        current: &Capture,
    ) -> Result<()> {
        let next = current
            .references
            .iter()
            .find(|r| r.known.key == reference.known.key)
            .context("Required native reference disappeared")?;
        let before = reference
            .snapshot
            .as_ref()
            .context("Native reference preview has no target proof")?;
        let after = next.snapshot.as_ref().with_context(|| {
            next.blocker
                .clone()
                .unwrap_or_else(|| "Native target is unavailable".into())
        })?;
        let cursor = receipt_digest(&before.files, &reference.registration, &before.directories)?;
        let target = receipt_digest(&after.files, &next.registration, &after.directories)?;
        self.prove_organization_reference_chain(&reference.known.key, cursor, &target)?;
        reference.snapshot = Some(after.clone());
        reference.registration = next.registration.clone();
        Ok(())
    }
    fn prove_organization_reference_chain(
        &self,
        key: &str,
        mut cursor: String,
        target: &str,
    ) -> Result<()> {
        let mut visited = std::collections::BTreeSet::new();
        while cursor != target {
            ensure!(
                visited.insert(cursor.clone()) && visited.len() <= 100_000,
                "Native receipt chain is cyclic or exceeds its bound"
            );
            let edges: Vec<String> = {
                let conn = self.conn()?;
                let mut query=conn.prepare("SELECT DISTINCT after_digest FROM organization_reference_edges WHERE target_key=?1 AND before_digest=?2")?;
                query
                    .query_map(params![key, cursor], |r| r.get(0))?
                    .collect::<rusqlite::Result<_>>()?
            };
            ensure!(
                edges.len() == 1,
                "Native target changed outside acknowledged path repairs; preview again"
            );
            cursor = edges[0].clone();
        }
        Ok(())
    }
    pub(crate) fn ensure_organization_references_settled(&self, target: &SyncTarget) -> Result<()> {
        let key = digest(&target.identity_target())?;
        let rows = {
            let conn = self.conn()?;
            let mut query=conn.prepare("SELECT i.payload_json FROM organization_reference_intents i JOIN organization_operations o ON o.id=i.operation_id WHERE o.state NOT IN ('complete','undone','cancelled','unchanged')")?;
            query
                .query_map([], |r| r.get::<_, String>(0))?
                .collect::<rusqlite::Result<Vec<_>>>()?
        };
        for json in rows {
            let intent: Intent = serde_json::from_str(&json)?;
            ensure!(
                !intent
                    .entries
                    .iter()
                    .any(|entry| entry.reference.known.key == key
                        && !entry.reference.known.independent),
                "An organization operation must complete or recover before native Sync can replace its reference evidence"
            );
        }
        Ok(())
    }
}

#[cfg(test)]
#[path = "organization_references/tests.rs"]
mod tests;
