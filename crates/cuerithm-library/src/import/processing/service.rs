use super::super::guard::MediaGuard;
use super::super::preview::{PreviewMeta, TrackProposal};
use super::stages::{self, Stage};
use super::*;
pub struct ImportProcessingService {
    library: Library,
    runtime: MediaRuntime,
    gate: Arc<Mutex<()>>,
    stop: AtomicBool,
    #[cfg(feature = "autotag")]
    tagger: Option<Arc<cuerithm_autotag::Tagger>>,
    active: Mutex<Option<CancellationToken>>,
    #[cfg(test)]
    claim_hook: Mutex<Option<Box<dyn Fn() + Send>>>,
}
impl ImportProcessingService {
    pub fn start(library: Library, runtime: MediaRuntime) -> Result<Self> {
        let gate = {
            let mut state = library
                .inner
                .import_runtime
                .lock()
                .map_err(|_| anyhow::anyhow!("Import runtime interrupted"))?;
            state.decoder_identity = Some(runtime.identity.clone());
            state.media_runtime = Some(runtime.clone());
            state.processing_gate.clone()
        };
        Ok(Self {
            library,
            runtime,
            gate,
            stop: AtomicBool::new(false),
            #[cfg(feature = "autotag")]
            tagger: None,
            active: Mutex::new(None),
            #[cfg(test)]
            claim_hook: Mutex::new(None),
        })
    }
    /// Share the host's configured catalog backend and request gates.
    #[cfg(feature = "autotag")]
    pub fn start_with_tagger(
        library: Library,
        runtime: MediaRuntime,
        tagger: Arc<cuerithm_autotag::Tagger>,
    ) -> Result<Self> {
        let mut service = Self::start(library, runtime)?;
        service.tagger = Some(tagger);
        Ok(service)
    }
    #[cfg(test)]
    pub(crate) fn set_claim_hook(&self, hook: Box<dyn Fn() + Send>) {
        *self.claim_hook.lock().unwrap() = Some(hook);
    }
    pub fn shutdown(&self) {
        self.stop.store(true, Ordering::Release);
        if let Ok(active) = self.active.lock()
            && let Some(token) = active.as_ref()
        {
            token.cancel();
        }
    }
    fn check(&self, cancel: &CancellationToken) -> Result<()> {
        ensure!(
            !self.stop.load(Ordering::Acquire),
            "Import processor is shut down"
        );
        cancel.check()
    }
    pub fn run_pending(&self, cancel: &CancellationToken) -> Result<ImportProcessingSummary> {
        self.check(cancel)?;
        let _gate = self
            .gate
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        self.check(cancel)?;
        *self
            .active
            .lock()
            .map_err(|_| anyhow::anyhow!("Import cancellation interrupted"))? =
            Some(cancel.clone());
        let result = self.run_locked(cancel);
        *self
            .active
            .lock()
            .map_err(|_| anyhow::anyhow!("Import cancellation interrupted"))? = None;
        result
    }
    fn run_locked(&self, cancel: &CancellationToken) -> Result<ImportProcessingSummary> {
        let mut summary = ImportProcessingSummary::default();
        // The shared gate proves there is no live generator. A crashed/panicked
        // proposal attempt has no published effects and may be safely retried.
        self.library.conn()?.execute("UPDATE import_processing_receipts SET state='abandoned',updated=?1 WHERE state='running'",[now()])?;
        let mut after = 0i64;
        loop {
            self.check(cancel)?;
            let operation: Option<(i64, String, String, String, String)> = {
                let conn = self.library.conn()?;
                conn.query_row("SELECT o.rowid,o.id,o.policy_id,p.json,p.id FROM import_operations o JOIN import_previews p ON p.id=o.preview_id WHERE o.state IN ('processing_pending','processing_running') AND o.rowid>?1 ORDER BY o.rowid LIMIT 1",[after],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?))).optional()?
            };
            let Some((rowid, operation_id, policy_id, raw, preview_id)) = operation else {
                break;
            };
            after = rowid;
            let meta: PreviewMeta = serde_json::from_str(&raw)?;
            self.library.conn()?.execute(
                "UPDATE import_operations SET state='processing_running',updated=?1 WHERE id=?2",
                params![now(), operation_id],
            )?;
            let mut sequence = -1i64;
            let mut op_blocked = false;
            let mut op_failed = false;
            let mut op_processed = false;
            loop {
                if self.check(cancel).is_err() {
                    summary.cancelled += 1;
                    self.set_operation(&operation_id, "processing_cancelled")?;
                    return Ok(summary);
                }
                let next: Option<(i64, String)> = {
                    let conn = self.library.conn()?;
                    conn.query_row("SELECT sequence,proposal_json FROM import_preview_effects WHERE preview_id=?1 AND kind='track' AND sequence>?2 ORDER BY sequence LIMIT 1",params![preview_id,sequence],|r|Ok((r.get(0)?,r.get(1)?))).optional()?
                };
                let Some((next_sequence, raw)) = next else {
                    break;
                };
                sequence = next_sequence;
                let proposal: TrackProposal = serde_json::from_str(&raw)?;
                if proposal.candidate.identity_blocked || !proposal.process {
                    continue;
                }
                let candidate = &proposal.candidate;
                let mut fields = super::super::reconcile::local_fields(
                    &*self.library.conn()?,
                    &candidate.item_id,
                )?;
                let planned = stages::plan(&meta.request.policy, &fields);
                let mut audio = None;
                for stage in planned {
                    self.check(cancel)?;
                    let complete:bool=self.library.conn()?.query_row("SELECT EXISTS(SELECT 1 FROM import_processing_receipts WHERE operation_id=?1 AND item_id=?2 AND stage=?3 AND state='proposed')",params![operation_id,candidate.item_id,stage.name],|r|r.get(0))?;
                    if complete {
                        continue;
                    }
                    let attempt = uuid::Uuid::new_v4().to_string();
                    let runtime_nonce = self
                        .library
                        .inner
                        .import_runtime
                        .lock()
                        .map_err(|_| anyhow::anyhow!("Import runtime interrupted"))?
                        .nonce
                        .clone();
                    let expected = fields.get(stage.field).cloned();
                    let initial = json!({"runtime_nonce":runtime_nonce,"media_guard":candidate.media,"policy_id":policy_id,"expected_value":expected,"field":stage.field,"decoder":self.runtime.identity});
                    self.library.conn()?.execute("INSERT INTO import_processing_receipts VALUES(?1,?2,?3,?4,'running',?5,'{}',?6)",params![operation_id,candidate.item_id,stage.name,attempt,serde_json::to_string(&initial)?,now()])?;
                    #[cfg(test)]
                    if let Some(hook) = self.claim_hook.lock().unwrap().as_ref() {
                        hook();
                    }
                    let outcome = self.execute(
                        &stage,
                        (&meta.processing, &meta.request.policy),
                        candidate.media.as_ref(),
                        &mut audio,
                        &fields,
                        cancel,
                    );
                    let (state, receipt, identity) = match outcome {
                        Ok((value, identity)) => {
                            if let Some(changes) = value.get("fields").and_then(Value::as_object) {
                                for (field, value) in changes {
                                    fields.insert(field.clone(), value.clone());
                                }
                            }
                            if identity.get("cache_reuse").is_some() {
                                summary.reused += 1;
                            } else {
                                summary.processed += 1;
                            }
                            op_processed = true;
                            (
                                "proposed",
                                json!({"proposal":value,"error":null}),
                                merge_identity(initial, identity),
                            )
                        }
                        Err(error) => {
                            let cancelled =
                                cancel.is_cancelled() || self.stop.load(Ordering::Acquire);
                            let blocked = error.downcast_ref::<BlockedStage>().is_some();
                            let state = if cancelled {
                                summary.cancelled += 1;
                                "cancelled"
                            } else if blocked {
                                summary.blocked += 1;
                                op_blocked = true;
                                "blocked"
                            } else {
                                summary.failed += 1;
                                op_failed = true;
                                "failed"
                            };
                            (
                                state,
                                json!({"proposal":null,"error":error.to_string()}),
                                initial,
                            )
                        }
                    };
                    let encoded = serde_json::to_string(&receipt)?;
                    ensure!(
                        encoded.len() as u64 <= ImportLimits::default().blob_bytes,
                        "Import processing receipt exceeds byte limit"
                    );
                    self.library.conn()?.execute("UPDATE import_processing_receipts SET state=?1,identity_json=?2,receipt_json=?3,updated=?4 WHERE operation_id=?5 AND item_id=?6 AND stage=?7 AND attempt=?8 AND state='running'",params![state,serde_json::to_string(&identity)?,encoded,now(),operation_id,candidate.item_id,stage.name,attempt])?;
                    if state == "cancelled" {
                        self.set_operation(&operation_id, "processing_cancelled")?;
                        return Ok(summary);
                    }
                }
            }
            let state = if op_failed {
                "processing_failed"
            } else if op_blocked {
                "processing_blocked"
            } else if op_processed || self.has_proposals(&operation_id)? {
                "effects_review_ready"
            } else {
                summary.unchanged += 1;
                "processing_complete"
            };
            self.set_operation(&operation_id, state)?;
        }
        Ok(summary)
    }
    fn execute(
        &self,
        stage: &Stage,
        configuration: (&FrozenProcessing, &ImportPolicy),
        media: Option<&MediaGuard>,
        audio: &mut Option<cuerithm_media::PreparedAudio>,
        fields: &BTreeMap<String, Value>,
        cancel: &CancellationToken,
    ) -> Result<(Value, Value)> {
        let (frozen, policy) = configuration;
        self.check(cancel)?;
        if let Some(reason) = &stage.blocker {
            return Err(BlockedStage(reason.clone()).into());
        }
        if !stage.selection.any() {
            return super::optional::execute(
                stage,
                super::optional::OptionalContext {
                    library: &self.library,
                    runtime: &self.runtime,
                    #[cfg(feature = "autotag")]
                    tagger: self.tagger.as_deref(),
                },
                frozen,
                policy,
                media,
                fields,
                cancel,
            );
        }
        if frozen
            .decoder_identity
            .as_ref()
            .is_some_and(|expected| expected != &self.runtime.identity)
        {
            return Err(
                BlockedStage("Decoder changed after preview; create a new preview".into()).into(),
            );
        }
        let media = media.ok_or_else(|| BlockedStage("Imported media is unresolved".into()))?;
        if media.sha256.is_none() {
            return Err(BlockedStage("Imported media is missing".into()).into());
        }
        media
            .validate(cancel)
            .map_err(|e| BlockedStage(e.to_string()))?;
        frozen
            .validate_models(stage.selection, cancel)
            .map_err(|e| BlockedStage(e.to_string()))?;
        if audio.is_none() {
            *audio = Some(self.runtime.decode_cancellable(
                &media.path,
                &self.library.inner.root.join("import-media-cache"),
                cancel,
            )?);
        }
        let prepared = audio.as_ref().expect("prepared media initialized");
        ensure!(
            Some(&prepared.source_sha256) == media.sha256.as_ref(),
            "Decoded source differs from the frozen import media"
        );
        let signature = stages::signature(stage, frozen, prepared, fields)?;
        let cached = super::cache::get(
            &self.library,
            super::cache::Query {
                stage,
                frozen,
                audio: prepared,
                fields,
                signature: &signature,
            },
        )?;
        let result = if let Some(cached) = cached {
            Ok(cached)
        } else {
            stages::analyze(stage, frozen, prepared, fields, cancel).and_then(
                |(value, identity)| {
                    let digest = super::cache::proposal_digest(&value)?;
                    Ok((
                        value,
                        merge_identity(
                            identity,
                            json!({"stage_signature":signature,"proposal_sha256":digest}),
                        ),
                    ))
                },
            )
        };
        self.check(cancel)?;
        media
            .validate(cancel)
            .map_err(|e| BlockedStage(e.to_string()))?;
        frozen
            .validate_models(stage.selection, cancel)
            .map_err(|e| BlockedStage(e.to_string()))?;
        result
    }
    fn has_proposals(&self, op: &str) -> Result<bool> {
        Ok(self.library.conn()?.query_row("SELECT EXISTS(SELECT 1 FROM import_processing_receipts WHERE operation_id=?1 AND state='proposed')",[op],|r|r.get(0))?)
    }
    fn set_operation(&self, op: &str, state: &str) -> Result<()> {
        self.library.conn()?.execute(
            "UPDATE import_operations SET state=?1,updated=?2 WHERE id=?3",
            params![state, now(), op],
        )?;
        Ok(())
    }
}
impl Drop for ImportProcessingService {
    fn drop(&mut self) {
        self.shutdown()
    }
}
#[derive(Debug)]
pub(super) struct BlockedStage(pub(super) String);
impl std::fmt::Display for BlockedStage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}
impl std::error::Error for BlockedStage {}
fn merge_identity(mut before: Value, after: Value) -> Value {
    if let (Some(before), Some(after)) = (before.as_object_mut(), after.as_object()) {
        before.extend(after.clone());
    }
    before
}

impl Library {
    pub fn import_processing_stages(
        &self,
        operation_id: &str,
        cursor: Option<&str>,
        limit: u32,
    ) -> Result<ImportProcessingStagePage> {
        ensure!(
            (1..=1000).contains(&limit),
            "Import processing page size must be 1..=1000"
        );
        let after = if let Some(cursor) = cursor {
            let (op, row): (String, i64) = serde_json::from_str(cursor)?;
            ensure!(
                op == operation_id && row > 0,
                "Processing cursor belongs to another operation"
            );
            row
        } else {
            0
        };
        let conn = self.conn()?;
        ensure!(
            conn.query_row(
                "SELECT EXISTS(SELECT 1 FROM import_operations WHERE id=?1)",
                [operation_id],
                |r| r.get::<_, bool>(0)
            )?,
            "Unknown import operation"
        );
        let total = conn.query_row(
            "SELECT COUNT(*) FROM import_processing_receipts WHERE operation_id=?1",
            [operation_id],
            |r| r.get(0),
        )?;
        let mut query=conn.prepare("SELECT rowid,item_id,stage,attempt,state,identity_json,receipt_json FROM import_processing_receipts WHERE operation_id=?1 AND rowid>?2 ORDER BY rowid LIMIT ?3")?;
        let mut rows = query.query(params![operation_id, after, limit + 1])?;
        let mut records = Vec::new();
        let mut bytes = 0u64;
        let mut last = after;
        let mut more = false;
        while let Some(row) = rows.next()? {
            let identity: String = row.get(5)?;
            let receipt: String = row.get(6)?;
            let size = identity.len() as u64 + receipt.len() as u64;
            ensure!(
                size <= ImportLimits::default().blob_bytes,
                "Processing receipt exceeds page byte limit"
            );
            if records.len() == limit as usize
                || (!records.is_empty() && bytes + size > ImportLimits::default().blob_bytes)
            {
                more = true;
                break;
            }
            bytes += size;
            let receipt: Value = serde_json::from_str(&receipt)?;
            records.push(ImportProcessingStage {
                operation_id: operation_id.into(),
                item_id: row.get(1)?,
                stage: row.get(2)?,
                attempt: row.get(3)?,
                state: row.get(4)?,
                identity: serde_json::from_str(&identity)?,
                proposal: receipt.get("proposal").filter(|v| !v.is_null()).cloned(),
                error: receipt
                    .get("error")
                    .and_then(Value::as_str)
                    .map(str::to_owned),
            });
            last = row.get(0)?;
        }
        Ok(ImportProcessingStagePage {
            records,
            total,
            next_cursor: if more {
                Some(serde_json::to_string(&(operation_id, last))?)
            } else {
                None
            },
        })
    }
}
