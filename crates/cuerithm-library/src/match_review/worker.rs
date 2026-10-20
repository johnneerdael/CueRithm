use super::{workflow::Decision, *};
use crate::{
    Library,
    model::now,
    organization::{Authority, Fingerprint, NativePath},
};
use anyhow::{Result, ensure};
use rusqlite::{Transaction, params};
use serde::{Deserialize, Serialize};
#[derive(Clone, Serialize, Deserialize)]
pub(super) struct Binding {
    pub path: NativePath,
    pub parent: Authority,
    pub fingerprint: Fingerprint,
    pub path_revision: u64,
    pub metadata_revision: u64,
    pub owned_tag_writes: Vec<String>,
}
pub(super) fn store_decision_tx(
    tx: &Transaction<'_>,
    job: &str,
    decision: &Decision,
) -> Result<()> {
    let capture = &decision.capture;
    let binding = Binding {
        path: capture.path.clone(),
        parent: capture.parent.clone(),
        fingerprint: capture.fingerprint.clone(),
        path_revision: capture.path_revision,
        metadata_revision: capture.metadata_revision,
        owned_tag_writes: vec![],
    };
    let payload = serde_json::to_string(decision)?;
    ensure!(
        payload.len() <= 8 * 1024 * 1024,
        "Protected review decision exceeds storage limit"
    );
    tx.execute("INSERT INTO preparation_review_decision_history(id,job_id,payload_json,created) VALUES(?1,?2,?3,?4)",params![decision.preview_id,job,payload,now()])?;
    tx.execute("INSERT INTO preparation_review_decisions(job_id,track_id,source_hash,settings_revision,generation,accepted_attempt,claimed_attempt,payload_json,binding_json,state,created,updated) VALUES(?1,?2,?3,?4,?5,?6,NULL,?7,?8,'pending',?9,?9) ON CONFLICT(job_id) DO UPDATE SET source_hash=excluded.source_hash,settings_revision=excluded.settings_revision,generation=excluded.generation,accepted_attempt=excluded.accepted_attempt,claimed_attempt=NULL,payload_json=excluded.payload_json,binding_json=excluded.binding_json,state='pending',updated=excluded.updated",params![job,capture.public.track_id,capture.public.source_sha256,capture.public.settings_revision,capture.public.generation,capture.public.attempt,payload,serde_json::to_string(&binding)?,now()])?;
    Ok(())
}
use crate::JobRun;
use anyhow::Context;
use cuerithm_autotag::{AutotagConfig, MatchStatus, MetadataPatch, TagMatchOutcome};
use cuerithm_media::MediaRuntime;
use rusqlite::OptionalExtension;
pub(crate) struct ProtectedReview {
    pub config: AutotagConfig,
    pub outcome: TagMatchOutcome,
    pub patch: MetadataPatch,
    pub provenance: serde_json::Value,
}
impl Library {
    fn verify_review_binding(&self, run: &JobRun, binding: &Binding) -> Result<()> {
        ensure!(
            binding.path == NativePath::new(&run.path)
                && binding.fingerprint.sha256 == run.source_sha256,
            "Protected review belongs to another source revision"
        );
        let (parent, fingerprint) = crate::organization::capture_regular_source(&run.path)?;
        ensure!(
            parent.path == binding.parent.path
                && parent.ancestors == binding.parent.ancestors
                && fingerprint.identity == binding.fingerprint.identity
                && fingerprint.sha256 == binding.fingerprint.sha256
                && fingerprint.bytes == binding.fingerprint.bytes,
            "Protected review native source changed; explicitly requery or edit the current revision"
        );
        Ok(())
    }
    pub(crate) fn protected_match_review_for_run(
        &self,
        run: &JobRun,
        runtime: &MediaRuntime,
    ) -> Result<Option<ProtectedReview>> {
        let row:Option<(String,String,String,u32,String)>=self.conn()?.query_row("SELECT payload_json,binding_json,source_hash,accepted_attempt,settings_revision FROM preparation_review_decisions WHERE job_id=?1 AND state IN ('pending','claimed','applied')",[&run.id],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?))).optional()?;
        let Some((payload, binding, hash, accepted, revision)) = row else {
            return Ok(None);
        };
        ensure!(
            payload.len() <= 8 * 1024 * 1024 && binding.len() <= 2 * 1024 * 1024,
            "Review decision exceeds storage limits"
        );
        ensure!(
            hash == run.source_sha256
                && revision == run.settings_revision
                && run.attempt > accepted,
            "Protected review no longer owns this source/configuration attempt"
        );
        let decision: Decision = serde_json::from_str(&payload)?;
        let binding: Binding = serde_json::from_str(&binding)?;
        self.verify_review_binding(run, &binding)?;
        let patch = if binding.owned_tag_writes.is_empty() {
            self.materialize_match_patch(
                runtime,
                &decision.patch,
                decision.artwork_asset.as_ref(),
                decision.artwork_target.as_ref(),
            )?
        } else {
            MetadataPatch::default()
        };
        {
            let mut conn = self.conn()?;
            let tx = conn.transaction()?;
            ensure!(
                Self::run_owns(&tx, run)?,
                "Preparation attempt was superseded"
            );
            ensure!(
                tx.query_row(
                    "SELECT path_revision=?2 AND metadata_revision=?3 FROM tracks WHERE id=?1",
                    params![
                        run.track_id,
                        binding.path_revision,
                        binding.metadata_revision
                    ],
                    |r| r.get::<_, bool>(0)
                )?,
                "Protected review metadata/path changed; request a new review"
            );
            ensure!(tx.execute("UPDATE preparation_review_decisions SET claimed_attempt=?1,state='claimed',updated=?2 WHERE job_id=?3 AND source_hash=?4 AND settings_revision=?5 AND payload_json=?6",params![run.attempt,now(),run.id,run.source_sha256,run.settings_revision,payload])?==1,"Review decision changed while claiming it");
            tx.commit()?;
        }
        let mut config = decision.capture.config.clone();
        let outcome = match decision.request.action {
            MatchReviewAction::KeepExistingOnce => {
                config.no_match = cuerithm_autotag::NoMatchPolicy::KeepExisting;
                TagMatchOutcome {
                    status: MatchStatus::NoMatch,
                    matches: vec![],
                    attempted: vec![],
                    warnings: vec![],
                }
            }
            MatchReviewAction::UseSelected => {
                config.options.tags.clear();
                config.required_tags.clear();
                config.write_energy = false;
                TagMatchOutcome {
                    status: MatchStatus::Matched,
                    matches: vec![
                        decision
                            .selected
                            .clone()
                            .context("Selected recording evidence is missing")?,
                    ],
                    attempted: vec![],
                    warnings: vec![],
                }
            }
        };
        Ok(Some(ProtectedReview {
            config,
            outcome,
            patch,
            provenance: serde_json::json!({"schema":"protected-match-review-v1","preview_id":decision.preview_id,"original_generation":decision.capture.public.generation,"original_source_sha256":decision.capture.public.source_sha256,"original_status":decision.capture.public.status,"action":decision.request.action,"fields":decision.request.fields,"acknowledge_conflicts":decision.request.acknowledge_conflicts,"candidate":decision.selected}),
        }))
    }
    pub(crate) fn verify_protected_match_review_source(&self, run: &JobRun) -> Result<()> {
        let row:Option<(String,u32,String)>=self.conn()?.query_row("SELECT binding_json,claimed_attempt,source_hash FROM preparation_review_decisions WHERE job_id=?1 AND state='claimed'",[&run.id],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?))).optional()?;
        if let Some((binding, attempt, hash)) = row {
            ensure!(
                attempt == run.attempt && hash == run.source_sha256,
                "Protected review attempt changed before publication"
            );
            self.verify_review_binding(run, &serde_json::from_str::<Binding>(&binding)?)?;
        }
        Ok(())
    }
    pub(crate) fn ack_protected_match_review_tx(
        tx: &Transaction<'_>,
        run: &JobRun,
        journal_id: &str,
        after: &str,
        proof: Option<(&NativePath, &Authority, &Fingerprint)>,
    ) -> Result<()> {
        let row:Option<String>=tx.query_row("SELECT binding_json FROM preparation_review_decisions WHERE job_id=?1 AND state='claimed' AND claimed_attempt=?2 AND source_hash=?3 AND settings_revision=?4",params![run.id,run.attempt,run.source_sha256,run.settings_revision],|r|r.get(0)).optional()?;
        let Some(binding) = row else { return Ok(()) };
        let mut binding: Binding = serde_json::from_str(&binding)?;
        ensure!(tx.query_row("SELECT job_id=?2 AND attempt=?3 AND settings_revision=?4 AND before_hash=?5 AND after_hash=?6 AND state='committed' FROM tag_writes WHERE id=?1",params![journal_id,run.id,run.attempt,run.settings_revision,run.source_sha256,after],|r|r.get::<_,bool>(0))?,"Only this job's verified native tag publication can rebind a protected decision");
        let (path, parent, fingerprint) = proof
            .context("Protected review requires a durable post-publication native identity")?;
        ensure!(
            *path == binding.path && fingerprint.sha256 == after,
            "Published review source identity differs"
        );
        binding.path = path.clone();
        binding.parent = parent.clone();
        binding.fingerprint = fingerprint.clone();
        let (pr, mr): (u64, u64) = tx.query_row(
            "SELECT path_revision,metadata_revision FROM tracks WHERE id=?1",
            [&run.track_id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )?;
        binding.path_revision = pr;
        binding.metadata_revision = mr;
        binding.owned_tag_writes.push(journal_id.into());
        ensure!(
            binding.owned_tag_writes.len() <= 10_000,
            "Protected decision publication history exceeds safety limit"
        );
        tx.execute("UPDATE preparation_review_decisions SET source_hash=?1,binding_json=?2,state='applied',updated=?3 WHERE job_id=?4 AND claimed_attempt=?5",params![after,serde_json::to_string(&binding)?,now(),run.id,run.attempt])?;
        Ok(())
    }
    pub(crate) fn retry_protected_preparation(
        &self,
        job: &str,
        revision: Option<&str>,
    ) -> Result<bool> {
        let active:bool=self.conn()?.query_row("SELECT EXISTS(SELECT 1 FROM preparation_review_decisions WHERE job_id=?1 AND state IN ('pending','claimed','applied')) OR EXISTS(SELECT 1 FROM metadata_edit_overrides WHERE job_id=?1 AND state IN ('queued','claimed'))",[job],|r|r.get(0))?;
        if !active {
            return Ok(false);
        }
        let (track,hash,attempt,current_revision):(String,String,u32,String)=self.conn()?.query_row("SELECT t.id,t.source_hash,j.attempts,j.settings_revision FROM jobs j JOIN tracks t ON t.current_job=j.id WHERE j.id=?1 AND t.state='incoming' AND t.archived=0",[job],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?)))?;
        let mut requested = revision;
        if let Some(next) = revision
            && next != current_revision
        {
            let old = self.settings_revision(&current_revision)?;
            let new = self.settings_revision(next)?;
            if Self::settings_equal_except_solver(&old, &new)? {
                requested = None;
            } else {
                let mut compatible = old.clone();
                compatible.settings.organization = new.settings.organization.clone();
                ensure!(
                    Self::settings_equal_except_solver(&compatible, &new)?,
                    "An accepted review/editor decision uses captured preparation settings; resume those settings or explicitly requery before replacing catalog policy"
                );
            }
        }
        if self.retry_preparation_checkpoint(job, requested)? {
            return Ok(true);
        }
        let _org = self
            .inner
            .organization_gate
            .lock()
            .map_err(|_| anyhow::anyhow!("Organization gate interrupted"))?;
        let _tag = self
            .inner
            .tag_gate
            .lock()
            .map_err(|_| anyhow::anyhow!("Publication gate interrupted"))?;
        let mut conn = self.conn()?;
        let tx = conn.transaction()?;
        Self::queue_preparation_edit_tx(&tx, &track, job, attempt, &hash)?;
        if let Some(next) = requested {
            tx.execute(
                "UPDATE jobs SET settings_revision=?1 WHERE id=?2",
                params![next, job],
            )?;
            tx.execute(
                "UPDATE preparation_review_decisions SET settings_revision=?1 WHERE job_id=?2",
                params![next, job],
            )?;
            tx.execute("UPDATE metadata_edit_overrides SET settings_revision=?1 WHERE job_id=?2 AND state IN ('queued','claimed')",params![next,job])?;
        }
        Self::bump(&tx)?;
        tx.commit()?;
        Ok(true)
    }
    pub(crate) fn rebind_protected_match_replay_tx(
        tx: &Transaction<'_>,
        run: &JobRun,
        journal: &str,
        proof: &crate::artifacts::ArtifactRestoreProof,
    ) -> Result<()> {
        let saved:Option<String>=tx.query_row("SELECT binding_json FROM preparation_review_decisions WHERE job_id=?1 AND state IN ('pending','claimed','applied')",[&run.id],|r|r.get(0)).optional()?;
        let Some(saved) = saved else { return Ok(()) };
        let mut binding: Binding = serde_json::from_str(&saved)?;
        let(input,restore):(Option<String>,Option<String>)=tx.query_row("SELECT input_source_proof_json,restore_source_proof_json FROM tag_writes WHERE id=?1 AND job_id=?2 AND track_id=?3 AND settings_revision=?4 AND before_hash=?5 AND state IN ('prepared','rolled_back')",params![journal,run.id,run.track_id,run.settings_revision,run.source_sha256],|r|Ok((r.get(0)?,r.get(1)?)))?;
        let known = [input, restore]
            .into_iter()
            .flatten()
            .map(|s| serde_json::from_str::<crate::artifacts::ArtifactRestoreProof>(&s))
            .collect::<serde_json::Result<Vec<_>>>()?
            .into_iter()
            .any(|p| {
                p.path == proof.path
                    && p.parent.path == proof.parent.path
                    && p.parent.ancestors == proof.parent.ancestors
                    && p.fingerprint.identity == proof.fingerprint.identity
                    && p.fingerprint.sha256 == proof.fingerprint.sha256
                    && p.fingerprint.bytes == proof.fingerprint.bytes
            });
        ensure!(
            known && proof.path == binding.path && proof.fingerprint.sha256 == run.source_sha256,
            "Protected review replay is not the exact source retained by its own tag journal"
        );
        binding.parent = proof.parent.clone();
        binding.fingerprint = proof.fingerprint.clone();
        tx.execute("UPDATE preparation_review_decisions SET binding_json=?1,claimed_attempt=?2,state='claimed' WHERE job_id=?3 AND settings_revision=?4",params![serde_json::to_string(&binding)?,run.attempt,run.id,run.settings_revision])?;
        Ok(())
    }
    pub(crate) fn finish_protected_match_review_tx(
        tx: &Transaction<'_>,
        run: &JobRun,
    ) -> Result<()> {
        tx.execute("UPDATE preparation_review_decisions SET state='complete',updated=?1 WHERE job_id=?2 AND state='applied' AND claimed_attempt=?3 AND EXISTS(SELECT 1 FROM jobs j JOIN tracks t ON t.current_job=j.id WHERE j.id=?2 AND j.attempts=?3 AND j.state='completed' AND t.state='ready' AND t.archived=0)",params![now(),run.id,run.attempt])?;
        Ok(())
    }
    pub(crate) fn has_protected_match_review(&self, job: &str) -> Result<bool> {
        Ok(self.conn()?.query_row("SELECT EXISTS(SELECT 1 FROM preparation_review_decisions WHERE job_id=?1 AND state IN ('pending','claimed','applied'))",[job],|r|r.get(0))?)
    }
}
