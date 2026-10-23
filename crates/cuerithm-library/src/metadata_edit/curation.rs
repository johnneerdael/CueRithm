use super::*;
use anyhow::{Result, ensure};
use cuerithm_media::CancellationToken;
use rusqlite::params;
impl MetadataEditService {
    pub(crate) fn apply_curation_preview_with(
        &self,
        preview: &str,
        publication: &str,
        cancel: &CancellationToken,
        after_link: impl FnOnce() -> Result<()>,
    ) -> Result<MetadataEditBatch> {
        self.apply_linked_with(preview, cancel, Some(publication), after_link)
    }
    pub(crate) fn hide_curation_draft(&self, draft: &str, publication: &str) -> Result<()> {
        self.library.conn()?.execute(
            "INSERT INTO curation_metadata_drafts VALUES(?1,?2)",
            params![draft, publication],
        )?;
        Ok(())
    }
    pub(crate) fn restage_curation_preview(
        &self,
        preview: &str,
        cancel: &CancellationToken,
    ) -> Result<String> {
        let text: String = self.library.conn()?.query_row(
            "SELECT payload_json FROM metadata_edit_previews WHERE id=?1",
            [preview],
            |row| row.get(0),
        )?;
        let mut plan: FrozenPreview = serde_json::from_str(&text)?;
        let patch = self.native_patch(&plan.patch)?;
        ensure!(
            plan.rows.len() == 1,
            "Curate recovery must own exactly one track"
        );
        for row in &mut plan.rows {
            Self::verify_source(&row.capture)?;
            Self::validate_capture(&*self.library.conn()?, &row.capture, None)?;
            let (stage, after) = self.stage_row(&row.capture, &patch, cancel)?;
            row.stage = Some(stage);
            row.public.after = after;
            row.public.outcome = if row.public.before == row.public.after
                && row.capture.public.state != "incoming"
            {
                "unchanged"
            } else {
                "ready"
            }
            .into();
        }
        plan.public.id = uuid::Uuid::new_v4().to_string();
        plan.public.created = crate::model::now();
        plan.public.rows = plan.rows.iter().map(|row| row.public.clone()).collect();
        plan.public.can_apply = plan.rows.iter().any(|row| row.public.outcome == "ready");
        let payload = bounded_json(&plan)?;
        self.library.conn()?.execute(
            "INSERT INTO metadata_edit_previews VALUES(?1,?2,?3,'preview',?4,?5)",
            params![
                plan.public.id,
                plan.public.draft_id,
                plan.public.draft_revision,
                payload,
                plan.public.created
            ],
        )?;
        Ok(plan.public.id)
    }
    pub(crate) fn undo_curation_batch_with(
        &self,
        batch: &str,
        publication: &str,
        cancel: &CancellationToken,
        after_link: impl FnOnce() -> Result<()>,
    ) -> Result<MetadataEditBatch> {
        self.undo_linked_with(batch, cancel, Some(publication), after_link)
    }
    pub(crate) fn verify_curation_undo_source(
        &self,
        batch: &str,
        cancel: &CancellationToken,
    ) -> Result<()> {
        if self.verify_curation_unchanged_child(batch)? {
            return Ok(());
        }
        let _org = self
            .library
            .inner
            .organization_gate
            .lock()
            .map_err(|_| anyhow::anyhow!("Organization gate interrupted"))?;
        let _tag = self
            .library
            .inner
            .tag_gate
            .lock()
            .map_err(|_| anyhow::anyhow!("Publication gate interrupted"))?;
        ensure!(
            child_undo_available(&*self.library.conn()?, batch)?,
            "Native Curate source or metadata revision changed"
        );
        let proofs = {
            let conn = self.library.conn()?;
            let mut query=conn.prepare("SELECT json_object('path',json_extract(payload_json,'$.row.capture.path'),'parent',json_extract(payload_json,'$.row.capture.parent'),'fingerprint',json_extract(payload_json,'$.publication')) FROM metadata_edit_writes WHERE batch_id=?1 AND state='committed' AND undo_of IS NULL AND override_of IS NULL UNION ALL SELECT final_json FROM metadata_edit_overrides WHERE batch_id=?1 AND state='committed' AND undo_batch_id IS NULL AND final_json IS NOT NULL")?;
            query
                .query_map([batch], |row| row.get::<_, String>(0))?
                .collect::<rusqlite::Result<Vec<_>>>()?
        };
        ensure!(!proofs.is_empty(), "Missing native Curate Undo authority");
        #[derive(serde::Deserialize)]
        struct Known {
            path: crate::organization::NativePath,
            parent: crate::organization::Authority,
            fingerprint: crate::organization::Fingerprint,
        }
        for proof in proofs {
            cancel.check()?;
            let known: Known = serde_json::from_str(&proof)?;
            let (parent, actual) =
                crate::organization::capture_regular_source(&known.path.path()?)?;
            ensure!(
                parent.ancestors == known.parent.ancestors
                    && crate::organization::same_native_file(&actual, &known.fingerprint),
                "Native Curate Undo source identity or bytes changed"
            );
        }
        Ok(())
    }
    pub(crate) fn validate_curation_preview(
        &self,
        preview: &str,
        expected: &crate::curation::CurationInput,
        proof: Option<&str>,
    ) -> Result<()> {
        let text: String = self.library.conn()?.query_row(
            "SELECT payload_json FROM metadata_edit_previews WHERE id=?1",
            [preview],
            |row| row.get(0),
        )?;
        let plan: FrozenPreview = serde_json::from_str(&text)?;
        ensure!(
            plan.rows.len() == 1,
            "Curate publication must own exactly one track"
        );
        let capture = &plan.rows[0].capture;
        #[derive(serde::Deserialize)]
        struct Known {
            path: crate::organization::NativePath,
            parent: crate::organization::Authority,
            fingerprint: crate::organization::Fingerprint,
        }
        if let Some(proof) = proof {
            let known: Known = serde_json::from_str(proof)?;
            ensure!(
                capture.path == known.path
                    && capture.parent.ancestors == known.parent.ancestors
                    && crate::organization::same_native_file(
                        &capture.fingerprint,
                        &known.fingerprint
                    ),
                "Curate predecessor source native identity or bytes changed"
            );
        }
        ensure!(
            Some(&capture.path.0) == expected.path.as_ref()
                && Some(capture.fingerprint.sha256.as_str()) == expected.source_hash.as_deref()
                && capture.public.path_revision == expected.path_revision
                && capture.public.metadata_revision == expected.metadata_revision
                && capture.job == expected.current_job
                && capture.public.state == expected.state,
            "Curate source changed since its durable draft; refresh or recover its owned predecessor"
        );
        Ok(())
    }
    pub(crate) fn verify_curation_unchanged_child(&self, batch: &str) -> Result<bool> {
        let rows = {
            let conn = self.library.conn()?;
            let mut query = conn
                .prepare("SELECT payload_json,state FROM metadata_edit_rows WHERE batch_id=?1")?;
            query
                .query_map([batch], |row| {
                    Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
                })?
                .collect::<rusqlite::Result<Vec<_>>>()?
        };
        if rows.len() != 1 || rows[0].1 != "unchanged" {
            return Ok(false);
        }
        let row: FrozenRow = serde_json::from_str(&rows[0].0)?;
        let _org = self
            .library
            .inner
            .organization_gate
            .lock()
            .map_err(|_| anyhow::anyhow!("Organization gate interrupted"))?;
        let _tag = self
            .library
            .inner
            .tag_gate
            .lock()
            .map_err(|_| anyhow::anyhow!("Tag gate interrupted"))?;
        Self::verify_source(&row.capture)?;
        Self::validate_capture(&*self.library.conn()?, &row.capture, None)?;
        Ok(true)
    }
}
pub(crate) fn child_undo_available(conn: &rusqlite::Connection, batch: &str) -> Result<bool> {
    let state: String = conn.query_row(
        "SELECT state FROM metadata_edit_batches WHERE id=?1",
        [batch],
        |row| row.get(0),
    )?;
    let flags = super::journal::curation_eligibility(conn, batch, &state)?;
    if flags {
        return Ok(true);
    }
    Ok(conn.query_row("SELECT EXISTS(SELECT 1 FROM metadata_edit_rows r JOIN tracks t ON t.id=r.track_id WHERE r.batch_id=?1 AND r.state='unchanged' AND t.path_revision=json_extract(r.payload_json,'$.capture.public.path_revision') AND t.metadata_revision=json_extract(r.payload_json,'$.capture.public.metadata_revision') AND t.source_hash=json_extract(r.payload_json,'$.capture.fingerprint.sha256') AND t.state=json_extract(r.payload_json,'$.capture.public.state') AND t.archived=0) AND NOT EXISTS(SELECT 1 FROM metadata_edit_rows WHERE batch_id=?1 AND state!='unchanged')",params![batch],|row|row.get(0))?)
}

impl crate::Library {
    /// Caller holds organization then tag gates through the cancellation transaction.
    pub(crate) fn verify_unpublished_curation_child(&self, batch: &str) -> Result<()> {
        let payloads = {
            let conn = self.conn()?;
            let mut query =
                conn.prepare("SELECT r.payload_json FROM metadata_edit_rows r WHERE r.batch_id=?1 AND EXISTS(SELECT 1 FROM metadata_edit_writes w WHERE w.row_id=r.id AND w.state IN ('prepared','publishing','recovery_required','conflict'))")?;
            query
                .query_map([batch], |row| row.get::<_, String>(0))?
                .collect::<rusqlite::Result<Vec<_>>>()?
        };
        // Native publication always persists its write intent before touching
        // the target. With no live write intents there is no target to adopt or
        // restore; cancellation only retires this child's owned staging.

        for payload in payloads {
            let row: FrozenRow = serde_json::from_str(&payload)?;
            MetadataEditService::verify_source(&row.capture)?;
            let current: (Vec<u8>, String, u64, u64) = self.conn()?.query_row(
                "SELECT path,source_hash,path_revision,metadata_revision FROM tracks WHERE id=?1",
                [&row.public.track_id],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
            )?;
            ensure!(
                current.0 == row.capture.path.0
                    && current.1 == row.capture.fingerprint.sha256
                    && current.2 == row.capture.public.path_revision
                    && current.3 == row.capture.public.metadata_revision,
                "Unpublished Curate source path or metadata changed; retain its evidence"
            );
        }
        Ok(())
    }
}
