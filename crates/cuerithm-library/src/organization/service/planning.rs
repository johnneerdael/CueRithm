use super::{
    capture::{self, FrozenPlan, FrozenRow},
    *,
};
use anyhow::{Context, Result, ensure};
use cuerithm_media::CancellationToken;
use cuerithm_organize::{
    paths::collision_key,
    plan::{RowOutcome, TrackInput},
};
use rusqlite::{OptionalExtension, params};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
impl Library {
    pub fn preview_organization(
        &self,
        request: OrganizationPreviewRequest,
        cancel: &CancellationToken,
    ) -> Result<OrganizationPreview> {
        self.preview_organization_ids(request, None, cancel)
    }
    pub(super) fn preview_organization_ids(
        &self,
        request: OrganizationPreviewRequest,
        incoming: Option<Vec<String>>,
        cancel: &CancellationToken,
    ) -> Result<OrganizationPreview> {
        self.preview_organization_inputs(request, incoming, None, cancel)
    }
    pub(crate) fn preview_import_organization(
        &self,
        request: OrganizationPreviewRequest,
        naming: BTreeMap<String, cuerithm_organize::template::NamingContext>,
        cancel: &CancellationToken,
    ) -> Result<OrganizationPreview> {
        self.preview_organization_inputs(request, None, Some(naming), cancel)
    }
    fn preview_organization_inputs(
        &self,
        request: OrganizationPreviewRequest,
        incoming: Option<Vec<String>>,
        naming: Option<BTreeMap<String, cuerithm_organize::template::NamingContext>>,
        cancel: &CancellationToken,
    ) -> Result<OrganizationPreview> {
        request.settings.validate()?;
        cancel.check()?;
        let allow_incoming = incoming.is_some();
        let ids = if let Some(ids) = incoming {
            ids
        } else {
            self.organization_scope_ids(&request.scope)?
        };
        ensure!(
            !ids.is_empty(),
            "Choose at least one eligible library track"
        );
        let root = if matches!(
            request.settings.mode,
            OrganizationSettingMode::Move | OrganizationSettingMode::Copy
        ) {
            Some(
                self.organization_root_authority(
                    request
                        .settings
                        .destination_root_id
                        .as_deref()
                        .context("Choose a destination folder")?,
                )?,
            )
        } else {
            None
        };
        let config = capture::config(&request.settings, root.as_ref())?;
        let mut rows = BTreeMap::<String, FrozenRow>::new();
        let mut inputs = Vec::<TrackInput>::new();
        for id in &ids {
            cancel.check()?;
            match self.capture_organization_row(
                id,
                allow_incoming,
                request.settings.enabled(),
                cancel,
            ) {
                Ok((mut row, mut input)) => {
                    if let Some(context) = naming.as_ref().and_then(|n| n.get(id)) {
                        let mut context = context.clone();
                        context.original_name = input.metadata.original_name.clone();
                        context.parent = input.metadata.parent.clone();
                        input.metadata = context.clone();
                        row.public.title = context.title.clone().unwrap_or_default();
                        if let Some(frozen) = row.captured.as_mut() {
                            frozen.metadata = context;
                        }
                    }
                    rows.insert(id.clone(), row);
                    inputs.push(input)
                }
                Err(error) => {
                    let display = self
                        .track_path(id)
                        .map(|p| p.to_string_lossy().into_owned())
                        .unwrap_or_default();
                    rows.insert(
                        id.clone(),
                        FrozenRow {
                            public: OrganizationRow {
                                id: uuid::Uuid::new_v4().to_string(),
                                track_id: id.clone(),
                                title: String::new(),
                                before_path: display,
                                after_path: None,
                                outcome: "blocked".into(),
                                reason: Some(format!("{error:#}")),
                                operation_id: None,
                                references: Vec::new(),
                            },
                            captured: None,
                        },
                    );
                }
            }
        }
        let initial = capture::build(config.clone(), inputs.clone(), Vec::new(), cancel)?;
        // No-op audio never requires ownership of shared artwork or a live DJ
        // database. It cannot change any of their reference paths.
        for input in &mut inputs {
            let row = initial
                .rows
                .iter()
                .find(|row| row.track.id == input.id)
                .context("Incomplete organization scope")?;
            if row.outcome == RowOutcome::Ready {
                let source = &input.source.path.0;
                match self.organization_owned_sidecars(&input.id, source, cancel) {
                    Ok(sidecars) => {
                        let frozen = rows.get_mut(&input.id).unwrap().captured.as_mut().unwrap();
                        frozen.sidecar_parents =
                            sidecars.iter().map(|(_, parent)| parent.clone()).collect();
                        input.sidecars = sidecars.into_iter().map(|(sidecar, _)| sidecar).collect();
                    }
                    Err(error) => {
                        let public = &mut rows.get_mut(&input.id).unwrap().public;
                        public.outcome = "blocked".into();
                        public.reason = Some(format!("{error:#}"));
                    }
                }
            }
        }
        let preliminary = capture::build(config.clone(), inputs.clone(), Vec::new(), cancel)?;
        let inventory = super::inventory::inspect(&preliminary, cancel)?;
        let completed = capture::build(config, inputs, inventory.non_files, cancel)?;
        for planned in completed.rows {
            cancel.check()?;
            let row = rows
                .get_mut(&planned.track.id)
                .context("Incomplete organization row")?;
            let frozen = row.captured.as_mut().context("Missing captured source")?;
            frozen.files = planned.files;
            row.public.after_path = frozen
                .files
                .first()
                .map(|file| file.destination.0.to_string_lossy().into_owned())
                .or_else(|| {
                    if planned.outcome == RowOutcome::Unchanged {
                        Some(row.public.before_path.clone())
                    } else {
                        None
                    }
                });
            if row.public.outcome == "blocked" {
                continue;
            }
            let mut reason = None;
            let mut outcome = match planned.outcome {
                RowOutcome::Ready => "ready",
                RowOutcome::Unchanged => "unchanged",
                RowOutcome::IdenticalCopy => {
                    reason =
                        Some("Destination already exists; an existing copy is not adopted".into());
                    "blocked"
                }
                RowOutcome::Skipped { reason: why } => {
                    reason = Some(why);
                    "skipped"
                }
                RowOutcome::Blocked { reason: why } => {
                    reason = Some(why);
                    "blocked"
                }
            };
            if outcome == "ready" {
                let occupied = frozen.files.iter().any(|file| {
                    if file.destination == file.source.path {
                        return false;
                    }
                    std::iter::once(file.destination.0.as_path())
                        .chain(file.destination.0.ancestors().skip(1))
                        .any(|path| {
                            collision_key(path).is_ok_and(|key| inventory.occupied.contains(&key))
                        })
                });
                if occupied {
                    outcome = "blocked";
                    reason = Some("Destination already exists or has an occupied ancestor".into());
                }
                if frozen.files.iter().any(|file| {
                    cuerithm_organize::paths::is_within(
                        &self.data_dir().canonicalize().unwrap_or_default(),
                        &file.destination.0,
                    )
                }) {
                    outcome = "blocked";
                    reason = Some("Destination enters application data".into());
                }
            }
            if outcome == "blocked"
                && request.settings.collision == OrganizationCollisionPolicy::Skip
                && request.settings.organization_optional
                && reason.as_deref().is_some_and(|reason| {
                    reason.contains("Destination") || reason.contains("destination")
                })
            {
                outcome = "skipped";
            }
            if outcome == "ready"
                && request.settings.mode != OrganizationSettingMode::Copy
                && frozen.import_bindings_digest.is_some()
            {
                outcome = "blocked";
                reason=Some("Original imported library references have no qualified path repair adapter; choose Copy to preserve their paths".into());
            }
            if outcome == "ready" {
                match self.organization_reference_snapshot(&planned.track.id) {
                    Ok(references) => {
                        frozen.reference_digest = references.digest;
                        frozen.reference_bytes = references.estimated_bytes;
                        frozen.reference_storage = references.storage_paths;
                        row.public.references = references.references;
                        if !references.blockers.is_empty() {
                            outcome = "blocked";
                            reason = Some(references.blockers.join("; "));
                        }
                    }
                    Err(error) => {
                        outcome = "blocked";
                        reason = Some(format!("{error:#}"));
                    }
                }
            }
            row.public.outcome = outcome.into();
            row.public.reason = reason;
        }
        let ordered = ids
            .iter()
            .filter_map(|id| rows.remove(id))
            .collect::<Vec<_>>();
        for row in &ordered {
            if let Some(frozen) = &row.captured {
                Self::validate_frozen_track(&*self.conn()?, &row.public.track_id, frozen, None)?;
            }
        }
        let mut totals = OrganizationPreviewTotals {
            total: ordered.len(),
            ..Default::default()
        };
        for row in &ordered {
            match row.public.outcome.as_str() {
                "ready" => totals.ready += 1,
                "unchanged" => totals.unchanged += 1,
                "skipped" => totals.skipped += 1,
                _ => totals.blocked += 1,
            }
        }
        let (estimated_bytes, available_bytes) = self.organization_storage_estimate(&ordered)?;
        let id = uuid::Uuid::new_v4().to_string();
        let created = crate::model::now();
        let revision = format!(
            "{:x}",
            Sha256::digest(serde_json::to_vec(&(
                &request.settings,
                &root,
                &inventory.directories,
                &ordered
            ))?)
        );
        let public = OrganizationPreview {
            id: id.clone(),
            revision,
            created,
            can_apply: totals.ready > 0 && estimated_bytes <= available_bytes,
            totals,
            estimated_bytes: Some(estimated_bytes),
            available_bytes: Some(available_bytes),
        };
        let preparation_job_id = if allow_incoming {
            ensure!(
                ordered.len() == 1,
                "Incoming organization requires one logical preparation job"
            );
            Some(
                ordered[0]
                    .captured
                    .as_ref()
                    .and_then(|row| row.current_job.clone())
                    .context("Missing Incoming preparation identity")?,
            )
        } else {
            None
        };
        let plan = FrozenPlan {
            public: public.clone(),
            settings: request.settings,
            root,
            directories: inventory.directories,
            preparation_job_id,
        };
        let mut conn = self.conn()?;
        let tx = conn.transaction()?;
        tx.execute(
            "INSERT INTO organization_plans VALUES(?1,?2,?3,'preview',?4)",
            params![id, public.revision, serde_json::to_string(&plan)?, created],
        )?;
        for (ordinal, row) in ordered.iter().enumerate() {
            tx.execute(
                "INSERT INTO organization_plan_rows VALUES(?1,?2,?3,?4,?5,?6)",
                params![
                    row.public.id,
                    id,
                    row.public.track_id,
                    ordinal,
                    row.public.outcome,
                    serde_json::to_string(row)?
                ],
            )?;
        }
        tx.commit()?;
        Ok(public)
    }
    /// Refresh technical proofs after an approved import's library/tag effects.
    /// Paths and reference changes must remain exactly those already reviewed.
    pub(crate) fn rebase_import_organization(
        &self,
        id: &str,
        cancel: &CancellationToken,
    ) -> Result<OrganizationPreview> {
        let old = self.organization_plan(id)?;
        let rows = self.organization_plan_rows(id)?;
        ensure!(rows.len() == 1, "Import organization must own one item");
        if let Some(root) = &old.root {
            crate::organization::NativeParent::reopen(root)?;
        }
        for directory in &old.directories {
            crate::organization::NativeParent::reopen(&directory.authority)?;
        }
        let row = &rows[0];
        let frozen = row
            .captured
            .as_ref()
            .context("Missing reviewed organization proof")?;
        crate::organization::NativeParent::reopen(&frozen.source_parent)?;
        let preview = self.preview_import_organization(
            OrganizationPreviewRequest {
                scope: OrganizationScope::Tracks {
                    track_ids: vec![row.public.track_id.clone()],
                },
                settings: old.settings,
            },
            BTreeMap::from([(row.public.track_id.clone(), frozen.metadata.clone())]),
            cancel,
        )?;
        let fresh = self.organization_plan_rows(&preview.id)?;
        let fresh = &fresh[0];
        ensure!(
            preview.can_apply
                && fresh.public.before_path == row.public.before_path
                && fresh.public.after_path == row.public.after_path
                && serde_json::to_value(&fresh.public.references)?
                    == serde_json::to_value(&row.public.references)?
                && self.import_reference_refresh_same(
                    &frozen.reference_digest,
                    &fresh
                        .captured
                        .as_ref()
                        .context("Missing refreshed proof")?
                        .reference_digest,
                    &frozen.fingerprint.sha256,
                    &fresh
                        .captured
                        .as_ref()
                        .context("Missing refreshed proof")?
                        .fingerprint
                        .sha256
                )?,
            "Organization destination or reference changes differ from the reviewed effects; preview again"
        );
        Ok(preview)
    }
    pub fn organization_preview_page(
        &self,
        id: &str,
        offset: usize,
        limit: usize,
    ) -> Result<OrganizationPage> {
        self.organization_plan(id)?;
        let limit = limit.clamp(1, 1000);
        let conn = self.conn()?;
        let total = conn.query_row(
            "SELECT COUNT(*) FROM organization_plan_rows WHERE plan_id=?1",
            [id],
            |r| r.get::<_, usize>(0),
        )?;
        let mut query=conn.prepare("SELECT payload_json FROM organization_plan_rows WHERE plan_id=?1 ORDER BY ordinal LIMIT ?2 OFFSET ?3")?;
        let values = query
            .query_map(params![id, limit, offset], |r| r.get::<_, String>(0))?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        let rows = values
            .into_iter()
            .map(|value| Ok(serde_json::from_str::<FrozenRow>(&value)?.public))
            .collect::<Result<Vec<_>>>()?;
        Ok(OrganizationPage {
            rows,
            total,
            offset,
            limit,
        })
    }
    pub fn discard_organization_preview(&self, id: &str) -> Result<()> {
        let mut conn = self.conn()?;
        let tx = conn.transaction()?;
        let used: bool = tx.query_row(
            "SELECT EXISTS(SELECT 1 FROM organization_batches WHERE preview_id=?1)",
            [id],
            |r| r.get(0),
        )?;
        if !used {
            tx.execute("DELETE FROM organization_plan_rows WHERE plan_id=?1", [id])?;
            tx.execute("DELETE FROM organization_plans WHERE id=?1", [id])?;
        }
        tx.commit()?;
        Ok(())
    }
    pub(super) fn organization_plan(&self, id: &str) -> Result<FrozenPlan> {
        let text: Option<String> = self
            .conn()?
            .query_row(
                "SELECT payload_json FROM organization_plans WHERE id=?1",
                [id],
                |r| r.get(0),
            )
            .optional()?;
        Ok(serde_json::from_str(&text.context(
            "Organization preview expired; create a new preview",
        )?)?)
    }
    pub(super) fn organization_plan_rows(&self, id: &str) -> Result<Vec<FrozenRow>> {
        let values = {
            let conn = self.conn()?;
            let mut query = conn.prepare(
                "SELECT payload_json FROM organization_plan_rows WHERE plan_id=?1 ORDER BY ordinal",
            )?;
            query
                .query_map([id], |r| r.get::<_, String>(0))?
                .collect::<rusqlite::Result<Vec<_>>>()?
        };
        values
            .into_iter()
            .map(|value| Ok(serde_json::from_str(&value)?))
            .collect()
    }
}
