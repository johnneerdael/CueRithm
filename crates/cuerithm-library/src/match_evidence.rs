//! Source/attempt-bound catalog evidence is retained before a write or attention outcome.
use crate::{JobRun, Library, model::now};
use anyhow::{Context, Result, ensure};
use cuerithm_autotag::{TagInput, TagMatchOutcome};
use cuerithm_media::file_sha256;
use rusqlite::{OptionalExtension, params};
use serde::{Deserialize, Serialize};

const MAX_EVIDENCE_BYTES: usize = 2 * 1024 * 1024;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PreparationMatchSnapshot {
    pub job_id: String,
    pub track_id: String,
    pub attempt: u32,
    pub source_sha256: String,
    pub settings_revision: String,
    pub outcome: serde_json::Value,
    pub created: i64,
}

impl Library {
    pub fn record_match_outcome(
        &self,
        run: &JobRun,
        input: &TagInput,
        outcome: &TagMatchOutcome,
    ) -> Result<()> {
        let _publication = self
            .inner
            .tag_gate
            .lock()
            .map_err(|_| anyhow::anyhow!("Publication gate interrupted"))?;
        ensure!(
            run.snapshot.settings.autotag_enabled,
            "Catalog matching is disabled"
        );
        let config = run
            .snapshot
            .settings
            .tagging
            .as_ref()
            .context("Missing tagging settings")?;
        ensure!(
            input.path == run.path && input.source_sha256 == run.source_sha256,
            "Catalog input differs from preparation source"
        );
        ensure!(
            file_sha256(&run.path)? == run.source_sha256,
            "Source changed before catalog evidence publication"
        );
        let mut value = serde_json::to_value(outcome.provenance(input, config)?)?;
        // Keep failure classifications for attention/retry without persisting response bodies.
        value["warnings"] = serde_json::to_value(&outcome.warnings)?;
        let payload = serde_json::to_string(&value)?;
        ensure!(
            payload.len() <= MAX_EVIDENCE_BYTES,
            "Catalog evidence exceeds storage limit"
        );
        let mut conn = self.conn()?;
        let tx = conn.transaction()?;
        ensure!(Self::run_owns(&tx, run)?, "Preparation was superseded");
        tx.execute(
            "INSERT INTO preparation_match_snapshots(job_id,track_id,attempt,source_hash,settings_revision,payload_json,created) VALUES(?1,?2,?3,?4,?5,?6,?7) ON CONFLICT(job_id) DO UPDATE SET track_id=excluded.track_id,attempt=excluded.attempt,source_hash=excluded.source_hash,settings_revision=excluded.settings_revision,payload_json=excluded.payload_json,created=excluded.created",
            params![run.id,run.track_id,run.attempt,run.source_sha256,run.settings_revision,payload,now()],
        )?;
        tx.commit()?;
        Ok(())
    }

    /// Return evidence only while it still belongs to the current source and claimed attempt.
    pub fn preparation_match_snapshot(
        &self,
        job_id: &str,
    ) -> Result<Option<PreparationMatchSnapshot>> {
        let conn = self.conn()?;
        let encoded = conn.query_row(
            "SELECT m.job_id,m.track_id,m.attempt,m.source_hash,m.settings_revision,m.payload_json,m.created FROM preparation_match_snapshots m JOIN jobs j ON j.id=m.job_id JOIN tracks t ON t.id=m.track_id WHERE m.job_id=?1 AND t.current_job=m.job_id AND t.archived=0 AND t.source_hash=m.source_hash AND j.source_hash=m.source_hash AND j.settings_revision=m.settings_revision AND j.attempts=m.attempt AND j.state IN ('running','attention','completed')",
            [job_id],
            |row| Ok((row.get::<_,String>(0)?,row.get::<_,String>(1)?,row.get::<_,u32>(2)?,row.get::<_,String>(3)?,row.get::<_,String>(4)?,row.get::<_,String>(5)?,row.get::<_,i64>(6)?)),
        ).optional()?;
        encoded
            .map(
                |(
                    job_id,
                    track_id,
                    attempt,
                    source_sha256,
                    settings_revision,
                    payload,
                    created,
                )| {
                    ensure!(
                        payload.len() <= MAX_EVIDENCE_BYTES,
                        "Catalog evidence exceeds storage limit"
                    );
                    Ok(PreparationMatchSnapshot {
                        job_id,
                        track_id,
                        attempt,
                        source_sha256,
                        settings_revision,
                        outcome: serde_json::from_str(&payload)?,
                        created,
                    })
                },
            )
            .transpose()
    }
}
