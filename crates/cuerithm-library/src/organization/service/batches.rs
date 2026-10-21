use super::{
    capture::{FrozenPlan, FrozenRow},
    *,
};
use crate::organization::{
    OrganizationMode, OrganizationPhase, OrganizationRequest,
    native::{self, Parent},
    publish::Boundary,
};
use anyhow::{Context, Result, ensure};
use cuerithm_media::CancellationToken;
use rusqlite::{OptionalExtension, params};
use std::{
    path::Path,
    time::{Duration, Instant},
};

impl Library {
    pub fn apply_organization_preview(
        &self,
        id: &str,
        revision: &str,
        cancel: &CancellationToken,
    ) -> Result<OrganizationBatch> {
        self.apply_organization_preview_with(id, revision, cancel, |_| Ok(()))
    }

    #[cfg(test)]
    pub(crate) fn apply_preparation_organization_preview(
        &self,
        id: &str,
        revision: &str,
        cancel: &CancellationToken,
    ) -> Result<OrganizationBatch> {
        self.apply_preparation_organization_preview_with(id, revision, cancel, |_| Ok(()))
    }

    pub(crate) fn apply_preparation_organization_preview_with(
        &self,
        id: &str,
        revision: &str,
        cancel: &CancellationToken,
        boundary: impl FnMut(Boundary) -> Result<()>,
    ) -> Result<OrganizationBatch> {
        self.apply_organization_preview_inner(id, revision, cancel, true, boundary)
    }

    pub(crate) fn apply_organization_preview_with(
        &self,
        id: &str,
        revision: &str,
        cancel: &CancellationToken,
        boundary: impl FnMut(Boundary) -> Result<()>,
    ) -> Result<OrganizationBatch> {
        self.apply_organization_preview_inner(id, revision, cancel, false, boundary)
    }

    fn apply_organization_preview_inner(
        &self,
        id: &str,
        revision: &str,
        cancel: &CancellationToken,
        preparation: bool,
        boundary: impl FnMut(Boundary) -> Result<()>,
    ) -> Result<OrganizationBatch> {
        cancel.check()?;
        let plan = self.organization_plan(id)?;
        ensure!(
            plan.public.revision == revision,
            "Organization preview revision changed; preview again"
        );
        ensure!(
            plan.public.can_apply
                || (preparation
                    && plan.public.totals.ready == 0
                    && plan.public.totals.blocked == 0),
            "Organization preview has no applicable files or insufficient storage"
        );
        ensure!(
            preparation == plan.preparation_job_id.is_some(),
            "Preparation previews must be applied by their owning preparation job"
        );
        let rows = self.organization_plan_rows(id)?;
        self.validate_organization_batch_scope(&plan, &rows, cancel)?;
        let batch = new_batch(Some(id.into()), "apply");
        {
            let mut conn = self.conn()?;
            let tx = conn.transaction()?;
            ensure!(tx.execute("UPDATE organization_plans SET state='applied' WHERE id=?1 AND revision=?2 AND state='preview'", params![id, revision])? == 1, "Organization preview was already applied or discarded");
            insert_batch(&tx, &batch, None, &rows)?;
            if let Some(job) = &plan.preparation_job_id {
                ensure!(
                    rows.len() == 1,
                    "Preparation organization requires exactly one job-owned track"
                );
                let row = &rows[0];
                let frozen = row
                    .captured
                    .as_ref()
                    .context("Missing prepared track proof")?;
                ensure!(
                    frozen.current_job.as_ref() == Some(job) && frozen.state == "incoming",
                    "Preparation job no longer owns its Incoming organization preview"
                );
                Self::validate_frozen_track(&tx, &row.public.track_id, frozen, None)?;
                ensure!(tx.execute("UPDATE preparation_checkpoints SET batch_id=?1,state='organizing',updated=?2 WHERE job_id=?3 AND track_id=?4 AND state='prepared' AND batch_id IS NULL AND attempt=?5 AND output_hash=?6 AND EXISTS(SELECT 1 FROM jobs j JOIN tracks t ON t.id=j.track_id WHERE j.id=preparation_checkpoints.job_id AND j.track_id=preparation_checkpoints.track_id AND j.attempts=preparation_checkpoints.attempt AND j.settings_revision=preparation_checkpoints.settings_revision AND t.current_job=j.id AND t.archived=0 AND t.state='incoming')", params![batch.id, crate::model::now(), job, row.public.track_id, frozen.attempt, frozen.fingerprint.sha256])? == 1, "Preparation checkpoint changed or already owns an organization batch");
            }
            tx.commit()?;
        }
        // Reserve and bind the ENTIRE ready scope before any filesystem publication.
        let prepared = self.reserve_organization_batch(&batch.id, &plan, &rows, cancel);
        if let Err(error) = prepared {
            self.stop_unpublished_organization_batch(
                &batch.id,
                &format!("{error:#}"),
                self.organization_batch_cancelled(&batch.id, cancel)?,
            )?;
            return self.finish_organization_batch(&batch.id);
        }
        self.run_organization_batch_with(&batch.id, cancel, boundary)
    }

    fn validate_organization_batch_scope(
        &self,
        plan: &FrozenPlan,
        rows: &[FrozenRow],
        cancel: &CancellationToken,
    ) -> Result<()> {
        super::inventory::validate(&plan.directories, cancel)?;
        if let Some(root) = &plan.root {
            Parent::reopen(root)?;
        }
        self.ensure_organization_storage(rows)?;
        for row in rows.iter().filter(|row| {
            matches!(
                row.public.outcome.as_str(),
                "ready" | "unchanged" | "skipped"
            )
        }) {
            cancel.check()?;
            let frozen = row
                .captured
                .as_ref()
                .context("Missing frozen organization source")?;
            Self::validate_frozen_track(&*self.conn()?, &row.public.track_id, frozen, None)?;
            let parent = Parent::reopen(&frozen.source_parent)?;
            let source = frozen.path.path()?;
            let actual = parent.fingerprint(Path::new(
                source.file_name().context("Missing source name")?,
            ))?;
            ensure!(
                native::matches(&actual, &frozen.fingerprint),
                "Source changed; create a new organization preview"
            );
        }
        Ok(())
    }

    fn reserve_organization_batch(
        &self,
        batch: &str,
        plan: &FrozenPlan,
        rows: &[FrozenRow],
        cancel: &CancellationToken,
    ) -> Result<()> {
        for row in rows.iter().filter(|row| row.public.outcome == "ready") {
            self.check_organization_batch_cancel(batch, cancel)?;
            let frozen = row
                .captured
                .as_ref()
                .context("Missing frozen organization source")?;
            let source_root = frozen.source_parent.path.path()?;
            let destination_root = plan
                .root
                .as_ref()
                .map(|root| root.path.path())
                .transpose()?
                .unwrap_or_else(|| source_root.clone());
            let request = OrganizationRequest {
                track_id: row.public.track_id.clone(),
                mode: match plan.settings.mode {
                    OrganizationSettingMode::RenameInPlace => OrganizationMode::Rename,
                    OrganizationSettingMode::Move => OrganizationMode::Move,
                    OrganizationSettingMode::Copy => OrganizationMode::Copy,
                    OrganizationSettingMode::LeaveInPlace => {
                        anyhow::bail!("Leave-in-place row cannot require publication")
                    }
                },
                source_root,
                destination_root,
                files: frozen
                    .files
                    .iter()
                    .map(|file| (file.source.path.0.clone(), file.destination.0.clone()))
                    .collect(),
            };
            let operation = self.plan_organization_for_row(request, &row.public.id)?;
            self.bind_organization_references(&operation, &frozen.reference_digest)?;
        }
        Ok(())
    }

    pub fn organization_batch_page(
        &self,
        id: &str,
        offset: usize,
        limit: usize,
    ) -> Result<OrganizationPage> {
        let limit = limit.clamp(1, 1000);
        let conn = self.conn()?;
        ensure!(
            conn.query_row(
                "SELECT EXISTS(SELECT 1 FROM organization_batches WHERE id=?1)",
                [id],
                |r| r.get::<_, bool>(0)
            )?,
            "Unknown organization batch"
        );
        let total = conn.query_row(
            "SELECT COUNT(*) FROM organization_batch_rows WHERE batch_id=?1",
            [id],
            |r| r.get(0),
        )?;
        let mut query = conn.prepare("SELECT payload_json,operation_id,outcome,error FROM organization_batch_rows WHERE batch_id=?1 ORDER BY ordinal LIMIT ?2 OFFSET ?3")?;
        let values = query
            .query_map(params![id, limit, offset], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, Option<String>>(1)?,
                    r.get::<_, String>(2)?,
                    r.get::<_, Option<String>>(3)?,
                ))
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        let rows = values
            .into_iter()
            .map(|(json, operation, outcome, error)| {
                let mut row: FrozenRow = serde_json::from_str(&json)?;
                row.public.operation_id = operation;
                row.public.outcome = outcome;
                if error.is_some() {
                    row.public.reason = error;
                }
                Ok(row.public)
            })
            .collect::<Result<Vec<_>>>()?;
        Ok(OrganizationPage {
            rows,
            total,
            offset,
            limit,
        })
    }

    pub fn organization_batches(&self) -> Result<Vec<OrganizationBatch>> {
        let ids = {
            let conn = self.conn()?;
            let mut query = conn
                .prepare("SELECT id FROM organization_batches ORDER BY created DESC,rowid DESC")?;
            query
                .query_map([], |row| row.get::<_, String>(0))?
                .collect::<rusqlite::Result<Vec<_>>>()?
        };
        ids.iter().map(|id| self.organization_batch(id)).collect()
    }

    pub fn cancel_organization_batch(&self, id: &str) -> Result<()> {
        let active = self.organization_batch(id)?.state == "applying";
        ensure!(self.conn()?.execute("UPDATE organization_batches SET cancel_requested=1,updated=?2 WHERE id=?1 AND state IN ('applying','recovery_required','cancelled','partial')", params![id, crate::model::now()])? == 1, "Organization batch is not active or recoverable");
        if !active {
            for mut row in self.organization_batch_rows(id)? {
                if matches!(row.public.outcome.as_str(), "ready" | "recovery_required") {
                    self.fail_organization_batch_row(
                        &mut row,
                        "Pending organization cancelled",
                        true,
                        true,
                    )?;
                }
            }
            self.finish_organization_batch(id)?;
        }
        Ok(())
    }

    pub fn recover_organization_batch(
        &self,
        id: &str,
        cancel: &CancellationToken,
    ) -> Result<OrganizationBatch> {
        let preparation: bool = self.conn()?.query_row("SELECT EXISTS(SELECT 1 FROM preparation_checkpoints WHERE batch_id=?1 AND state!='complete')", [id], |row| row.get(0))?;
        ensure!(
            !preparation,
            "This organization batch belongs to active preparation; use Retry in Incoming to resume its current job safely"
        );
        self.recover_preparation_organization_batch_with(id, cancel, |_| Ok(()))
    }

    pub(crate) fn recover_preparation_organization_batch_with(
        &self,
        id: &str,
        cancel: &CancellationToken,
        boundary: impl FnMut(Boundary) -> Result<()>,
    ) -> Result<OrganizationBatch> {
        cancel.check()?;
        let batch = self.organization_batch(id)?;
        ensure!(
            native_batch_recoverable(&*self.conn()?, id, &batch.state)?,
            "Organization batch has no recoverable operations"
        );
        self.claim_organization_batch(id)?;
        self.run_organization_batch_with(id, cancel, boundary)
    }

    pub fn undo_organization_batch(
        &self,
        id: &str,
        cancel: &CancellationToken,
    ) -> Result<OrganizationBatch> {
        cancel.check()?;
        let preparation: bool = self.conn()?.query_row("SELECT EXISTS(SELECT 1 FROM preparation_checkpoints WHERE batch_id=?1 AND state!='complete')", [id], |row| row.get(0))?;
        ensure!(
            !preparation,
            "An active Incoming preparation owns this batch; use Retry in Incoming before manual Undo"
        );
        let original = self.organization_batch(id)?;
        ensure!(
            original.can_undo,
            "Organization batch has no completed original operations available for Undo"
        );
        let old_rows = self.organization_batch_rows(id)?;
        let mut rows = vec![];
        let mut operations = vec![];
        for old in old_rows {
            if old.public.outcome != "completed" {
                continue;
            }
            let operation = old
                .public
                .operation_id
                .as_ref()
                .context("Missing completed operation")?;
            let record = self.organization_record(operation)?;
            if record.phase != OrganizationPhase::Complete || record.undo_of.is_some() {
                continue;
            }
            let (mut row, _) =
                match self.capture_organization_row(&old.public.track_id, false, false, cancel) {
                    Ok(captured) => captured,
                    Err(error) => {
                        // A changed or missing managed file blocks only its own row.
                        cancel.check()?;
                        let before_path = self
                            .track_path(&old.public.track_id)
                            .map(|path| path.to_string_lossy().into_owned())
                            .unwrap_or_default();
                        rows.push(FrozenRow {
                            public: OrganizationRow {
                                id: uuid::Uuid::new_v4().to_string(),
                                track_id: old.public.track_id.clone(),
                                title: old.public.title.clone(),
                                before_path,
                                after_path: None,
                                outcome: "blocked".into(),
                                reason: Some(format!(
                                    "Undo blocked because the track changed: {error:#}"
                                )),
                                operation_id: None,
                                references: Vec::new(),
                            },
                            captured: None,
                        });
                        operations.push(operation.clone());
                        continue;
                    }
                };
            let stale = Self::validate_frozen_track(
                &*self.conn()?,
                &old.public.track_id,
                old.captured
                    .as_ref()
                    .context("Missing original frozen Undo proof")?,
                record
                    .files
                    .first()
                    .map(|file| file.destination.path())
                    .transpose()?
                    .as_deref(),
            )
            .err();
            let references = self.organization_reference_snapshot(&row.public.track_id)?;
            row.public.references = references.references;
            row.public.after_path = record
                .files
                .first()
                .map(|file| file.source.path().map(|p| p.to_string_lossy().into_owned()))
                .transpose()?;
            let frozen = row.captured.as_mut().unwrap();
            frozen.reference_digest = references.digest;
            frozen.reference_bytes = references.estimated_bytes;
            frozen.reference_storage = references.storage_paths;
            frozen.files = record
                .files
                .iter()
                .enumerate()
                .map(|(index, file)| {
                    let managed = file.staged.as_ref().unwrap_or(&file.original);
                    Ok(cuerithm_organize::plan::PlannedFile {
                        source: super::capture::snapshot(
                            &file.destination.path()?,
                            managed,
                            if index == 0 { frozen.path_revision } else { 0 },
                        )?,
                        destination: cuerithm_organize::paths::NativePath(file.source.path()?),
                        existing_destination: if record.mode == OrganizationMode::Copy {
                            Some(super::capture::snapshot(
                                &file.source.path()?,
                                &file.original,
                                0,
                            )?)
                        } else {
                            None
                        },
                    })
                })
                .collect::<Result<Vec<_>>>()?;
            if !references.blockers.is_empty() {
                row.public.outcome = "blocked".into();
                row.public.reason = Some(references.blockers.join("; "));
            }
            if let Some(error) = stale {
                row.public.outcome = "blocked".into();
                row.public.reason =
                    Some(format!("Undo blocked because the track changed: {error:#}"));
            }
            operations.push(operation.clone());
            rows.push(row);
        }
        ensure!(
            !rows.is_empty(),
            "No completed operations remain available for Undo"
        );
        self.ensure_organization_storage(&rows)?;
        let batch = new_batch(None, "undo");
        {
            let mut conn = self.conn()?;
            let tx = conn.transaction()?;
            let exists: bool = tx.query_row("SELECT EXISTS(SELECT 1 FROM organization_batches WHERE undo_of=?1 AND state='applying')", [id], |r| r.get(0))?;
            ensure!(!exists, "Organization Undo is already applying");
            insert_batch(&tx, &batch, Some(id), &rows)?;
            tx.commit()?;
        }
        let reserved = (|| -> Result<()> {
            for (row, operation) in rows.iter().zip(operations) {
                if row.public.outcome != "ready" {
                    continue;
                }
                self.check_organization_batch_cancel(&batch.id, cancel)?;
                let inverse = self.plan_organization_undo_for_row(&operation, &row.public.id)?;
                self.bind_organization_references(
                    &inverse,
                    &row.captured.as_ref().unwrap().reference_digest,
                )?;
            }
            Ok(())
        })();
        if let Err(error) = reserved {
            self.stop_unpublished_organization_batch(
                &batch.id,
                &format!("{error:#}"),
                self.organization_batch_cancelled(&batch.id, cancel)?,
            )?;
            return self.finish_organization_batch(&batch.id);
        }
        self.run_organization_batch(&batch.id, cancel)
    }

    fn claim_organization_batch(&self, id: &str) -> Result<()> {
        ensure!(self.conn()?.execute("UPDATE organization_batches SET state='applying',cancel_requested=0,updated=?2 WHERE id=?1 AND state!='applying'", params![id, crate::model::now()])? == 1, "Organization batch is already applying");
        Ok(())
    }

    fn run_organization_batch(
        &self,
        id: &str,
        cancel: &CancellationToken,
    ) -> Result<OrganizationBatch> {
        self.run_organization_batch_with(id, cancel, |_| Ok(()))
    }

    fn run_organization_batch_with(
        &self,
        id: &str,
        cancel: &CancellationToken,
        mut boundary: impl FnMut(Boundary) -> Result<()>,
    ) -> Result<OrganizationBatch> {
        let pending = self.organization_batch_rows(id)?;
        if pending.iter().any(|row| {
            matches!(
                row.public.outcome.as_str(),
                "ready" | "recovery_required" | "failed"
            ) && row.public.operation_id.is_none()
        }) {
            self.stop_unpublished_organization_batch(
                id,
                "Interrupted scope reservation; create a new preview",
                false,
            )?;
            return self.finish_organization_batch(id);
        }
        for mut row in self.organization_batch_rows(id)? {
            if !matches!(row.public.outcome.as_str(), "ready" | "recovery_required") {
                continue;
            }
            if self.organization_batch_cancelled(id, cancel)? {
                self.cancel_organization_batch_row(&mut row, "Organization batch cancelled")?;
                continue;
            }
            let result = (|| -> Result<()> {
                let operation = row
                    .public
                    .operation_id
                    .as_deref()
                    .context("Operation was not fully reserved; create a fresh preview")?;
                let record = self.organization_record(operation)?;
                let missing_copy_undo_intent = if record.phase
                    == OrganizationPhase::AwaitingReferences
                {
                    if let Some(original) = &record.undo_of {
                        self.organization_record(original)?.mode == OrganizationMode::Copy
                            && !self.conn()?.query_row("SELECT EXISTS(SELECT 1 FROM organization_reference_intents WHERE operation_id=?1)", [operation], |r| r.get::<_, bool>(0))?
                    } else {
                        false
                    }
                } else {
                    false
                };
                // Copy Undo starts with its already-owned original in place.
                // Rebind only that verified zero-publication inverse, using its
                // pre-inverse snapshot; never invent intent for a published move.
                if record.phase == OrganizationPhase::Planned || missing_copy_undo_intent {
                    self.bind_organization_references(
                        operation,
                        &row.captured
                            .as_ref()
                            .context("Missing frozen reference proof")?
                            .reference_digest,
                    )?;
                }
                self.advance_organization_batch_operation(id, operation, cancel, &mut boundary)?;
                Ok(())
            })();
            match result {
                Ok(()) => {
                    row.public.outcome = "completed".into();
                    row.public.reason = None;
                }
                Err(error) => {
                    row.public.reason = Some(format!("{error:#}"));
                    if self.organization_batch_cancelled(id, cancel)? {
                        self.cancel_organization_batch_row(&mut row, &format!("{error:#}"))?;
                        continue;
                    }
                    self.fail_organization_batch_row(
                        &mut row,
                        &format!("{error:#}"),
                        false,
                        false,
                    )?;
                }
            }
            self.save_organization_batch_row(&row)?;
        }
        self.finish_organization_batch(id)
    }

    fn advance_organization_batch_operation(
        &self,
        batch: &str,
        operation: &str,
        cancel: &CancellationToken,
        hook: &mut impl FnMut(Boundary) -> Result<()>,
    ) -> Result<()> {
        let mut check_cancel = self.organization_batch_boundary(batch, cancel);
        let mut boundary = |at| {
            hook(at)?;
            check_cancel(at)
        };
        let record = self.organization_record(operation)?;
        if matches!(
            record.phase,
            OrganizationPhase::Complete | OrganizationPhase::Unchanged
        ) {
            return Ok(());
        }
        if record.phase == OrganizationPhase::Cleanup {
            boundary(Boundary::Published)?;
            // Acquire reference guard BEFORE organization/tag/DB locks and before cleanup.
            let guard = self.verify_acknowledged_organization_references(operation)?;
            self.execute_organization_with_checked(operation, &guard, &mut boundary)?;
            return Ok(());
        }
        ensure!(
            !matches!(
                record.phase,
                OrganizationPhase::Cancelled | OrganizationPhase::Undone
            ),
            "Organization operation is no longer recoverable"
        );
        let published = self.execute_organization_with(operation, &mut boundary)?;
        ensure!(
            published.phase == OrganizationPhase::AwaitingReferences,
            "Organization publication did not reach reference repair"
        );
        boundary(Boundary::Published)?;
        let guard = self.repair_organization_references(operation)?;
        // Check captured worker ownership while the repair guard is held,
        // before the short local acknowledgement and subsequent cleanup.
        boundary(Boundary::Published)?;
        self.acknowledge_organization_references_checked(operation, &guard)?;
        boundary(Boundary::Published)?;
        self.execute_organization_with_checked(operation, &guard, &mut boundary)?;
        Ok(())
    }

    fn stop_unpublished_organization_batch(
        &self,
        id: &str,
        error: &str,
        cancelled: bool,
    ) -> Result<()> {
        for mut row in self.organization_batch_rows(id)? {
            if !matches!(row.public.outcome.as_str(), "ready" | "recovery_required") {
                continue;
            }
            if let Some(operation) = &row.public.operation_id {
                if self.cancel_unstarted_organization(operation).is_err() {
                    row.public.outcome = "recovery_required".into();
                } else {
                    row.public.outcome = if cancelled { "cancelled" } else { "failed" }.into();
                }
            } else {
                row.public.outcome = if cancelled { "cancelled" } else { "failed" }.into();
            }
            row.public.reason = Some(error.into());
            self.save_organization_batch_row(&row)?;
        }
        Ok(())
    }

    fn cancel_organization_batch_row(&self, row: &mut FrozenRow, reason: &str) -> Result<()> {
        self.fail_organization_batch_row(row, reason, true, false)
    }

    fn fail_organization_batch_row(
        &self,
        row: &mut FrozenRow,
        reason: &str,
        cancelled: bool,
        retire_staged: bool,
    ) -> Result<()> {
        let terminal = if cancelled { "cancelled" } else { "failed" };
        row.public.outcome = match row.public.operation_id.as_deref() {
            Some(operation) => {
                let record = self.organization_record(operation)?;
                match record.phase {
                    OrganizationPhase::Complete => "completed",
                    OrganizationPhase::Unchanged => "unchanged",
                    OrganizationPhase::Cancelled => terminal,
                    OrganizationPhase::Planned | OrganizationPhase::Publishing
                        if (retire_staged
                            || record.files.iter().all(|file| {
                                file.staged.is_none()
                                    && file.abandoned_stages.is_empty()
                                    && !file.published
                                    && !file.quarantined
                            }))
                            && self.cancel_unstarted_organization(operation).is_ok() =>
                    {
                        terminal
                    }
                    _ => "recovery_required",
                }
            }
            None => terminal,
        }
        .into();
        row.public.reason = if matches!(row.public.outcome.as_str(), "completed" | "unchanged") {
            None
        } else {
            Some(reason.into())
        };
        self.save_organization_batch_row(row)
    }

    fn organization_batch_boundary<'a>(
        &'a self,
        id: &'a str,
        cancel: &'a CancellationToken,
    ) -> impl FnMut(Boundary) -> Result<()> + 'a {
        let mut checked = Instant::now();
        move |boundary| {
            cancel.check()?;
            if boundary != Boundary::StageProgress
                || checked.elapsed() >= Duration::from_millis(100)
            {
                self.check_organization_batch_cancel(id, cancel)?;
                checked = Instant::now();
            }
            Ok(())
        }
    }

    fn organization_batch_cancelled(&self, id: &str, cancel: &CancellationToken) -> Result<bool> {
        let requested: bool = self.conn()?.query_row(
            "SELECT cancel_requested FROM organization_batches WHERE id=?1",
            [id],
            |r| r.get(0),
        )?;
        Ok(cancel.is_cancelled() || requested)
    }
    fn check_organization_batch_cancel(&self, id: &str, cancel: &CancellationToken) -> Result<()> {
        ensure!(
            !self.organization_batch_cancelled(id, cancel)?,
            "Organization batch cancelled"
        );
        Ok(())
    }

    fn organization_batch_rows(&self, id: &str) -> Result<Vec<FrozenRow>> {
        let values = {
            let conn = self.conn()?;
            let mut query = conn.prepare("SELECT payload_json,operation_id,outcome,error FROM organization_batch_rows WHERE batch_id=?1 ORDER BY ordinal")?;
            query
                .query_map([id], |r| {
                    Ok((
                        r.get::<_, String>(0)?,
                        r.get::<_, Option<String>>(1)?,
                        r.get::<_, String>(2)?,
                        r.get::<_, Option<String>>(3)?,
                    ))
                })?
                .collect::<rusqlite::Result<Vec<_>>>()?
        };
        values
            .into_iter()
            .map(|(json, operation, outcome, error)| {
                let mut row: FrozenRow = serde_json::from_str(&json)?;
                row.public.operation_id = operation;
                row.public.outcome = outcome;
                if error.is_some() {
                    row.public.reason = error;
                }
                Ok(row)
            })
            .collect()
    }
    fn save_organization_batch_row(&self, row: &FrozenRow) -> Result<()> {
        self.conn()?.execute(
            "UPDATE organization_batch_rows SET outcome=?1,payload_json=?2,error=?3 WHERE id=?4",
            params![
                row.public.outcome,
                serde_json::to_string(row)?,
                row.public.reason,
                row.public.id
            ],
        )?;
        Ok(())
    }
    fn organization_batch(&self, id: &str) -> Result<OrganizationBatch> {
        let conn = self.conn()?;
        let (json, state): (String, String) = conn
            .query_row(
                "SELECT payload_json,state FROM organization_batches WHERE id=?1",
                [id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .context("Unknown organization batch")?;
        let mut batch: OrganizationBatch = serde_json::from_str(&json)?;
        batch.state = state;
        batch.totals = OrganizationBatchTotals::default();
        let mut query = conn.prepare("SELECT outcome,COUNT(*) FROM organization_batch_rows WHERE batch_id=?1 GROUP BY outcome")?;
        let counts = query
            .query_map([id], |r| {
                Ok((r.get::<_, String>(0)?, r.get::<_, usize>(1)?))
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        for (outcome, count) in counts {
            batch.totals.total += count;
            match outcome.as_str() {
                "completed" => batch.totals.completed = count,
                "unchanged" => batch.totals.unchanged = count,
                "skipped" => batch.totals.skipped = count,
                "blocked" => batch.totals.blocked = count,
                "failed" => batch.totals.failed = count,
                "recovery_required" => batch.totals.recovery_required = count,
                "cancelled" => batch.totals.cancelled = count,
                _ => {}
            }
        }
        batch.can_undo = batch.kind == "apply" && conn.query_row("SELECT EXISTS(SELECT 1 FROM organization_batch_rows r JOIN organization_operations o ON o.id=r.operation_id WHERE r.batch_id=?1 AND r.outcome='completed' AND o.state='complete')", [id], |r| r.get::<_, bool>(0))?;
        // A crash can occur after the last child journal completes but before
        // its row or batch summary is saved. Recovery must reconcile that state.
        batch.can_recover = native_batch_recoverable(&conn, id, &batch.state)?;
        batch.resume_job_id = None;
        let preparation: Option<(String, String, bool)> = conn.query_row("SELECT p.job_id,j.state,t.current_job=j.id AND t.archived=0 AND t.state='incoming' AND j.settings_revision=p.settings_revision AND j.source_hash=p.input_hash FROM preparation_checkpoints p JOIN jobs j ON j.id=p.job_id JOIN tracks t ON t.id=p.track_id WHERE p.batch_id=?1 AND p.state!='complete'", [id], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?))).optional()?;
        if let Some((job, state, owns)) = preparation {
            let resumable = batch.can_recover
                || (matches!(batch.state.as_str(), "complete" | "partial")
                    && batch.totals.failed + batch.totals.blocked + batch.totals.cancelled == 0);
            batch.can_undo = false;
            batch.can_recover = owns && state == "attention" && resumable;
            if batch.can_recover {
                batch.resume_job_id = Some(job);
            }
        }
        if batch.state == "applying" {
            batch.can_undo = false;
            batch.can_recover = false;
        }
        batch.error = conn.query_row("SELECT error FROM organization_batch_rows WHERE batch_id=?1 AND error IS NOT NULL ORDER BY ordinal LIMIT 1", [id], |r| r.get(0)).optional()?.flatten();
        Ok(batch)
    }
    fn finish_organization_batch(&self, id: &str) -> Result<OrganizationBatch> {
        let mut batch = self.organization_batch(id)?;
        batch.state = if batch.totals.recovery_required > 0 {
            "recovery_required"
        } else if batch.totals.cancelled > 0 {
            "cancelled"
        } else if batch.totals.failed + batch.totals.blocked + batch.totals.skipped > 0 {
            "partial"
        } else if batch.kind == "undo" {
            "undone"
        } else {
            "complete"
        }
        .into();
        self.conn()?.execute(
            "UPDATE organization_batches SET state=?1,payload_json=?2,updated=?3 WHERE id=?4",
            params![
                batch.state,
                serde_json::to_string(&batch)?,
                crate::model::now(),
                id
            ],
        )?;
        self.organization_batch(id)
    }
}
fn native_batch_recoverable(conn: &rusqlite::Connection, id: &str, state: &str) -> Result<bool> {
    Ok(state == "recovery_required" || conn.query_row("SELECT EXISTS(SELECT 1 FROM organization_batch_rows r JOIN organization_operations o ON o.id=r.operation_id WHERE r.batch_id=?1 AND (o.state NOT IN ('complete','unchanged','cancelled','undone') OR r.outcome IN ('ready','recovery_required')))", [id], |row| row.get::<_, bool>(0))?)
}

fn new_batch(preview_id: Option<String>, kind: &str) -> OrganizationBatch {
    OrganizationBatch {
        id: uuid::Uuid::new_v4().to_string(),
        preview_id,
        kind: kind.into(),
        state: "applying".into(),
        created: crate::model::now(),
        totals: Default::default(),
        can_undo: false,
        can_recover: false,
        resume_job_id: None,
        error: None,
    }
}
fn insert_batch(
    tx: &rusqlite::Transaction<'_>,
    batch: &OrganizationBatch,
    undo_of: Option<&str>,
    rows: &[FrozenRow],
) -> Result<()> {
    tx.execute("INSERT INTO organization_batches(id,preview_id,undo_of,kind,state,payload_json,created,updated) VALUES(?1,?2,?3,?4,'applying',?5,?6,?6)", params![batch.id, batch.preview_id, undo_of, batch.kind, serde_json::to_string(batch)?, batch.created])?;
    for (ordinal, row) in rows.iter().enumerate() {
        tx.execute("INSERT INTO organization_batch_rows(id,batch_id,track_id,ordinal,outcome,payload_json) VALUES(?1,?2,?3,?4,?5,?6)", params![row.public.id, batch.id, row.public.track_id, ordinal, row.public.outcome, serde_json::to_string(row)?])?;
    }
    Ok(())
}
