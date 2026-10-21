use super::{
    journal::*,
    native::*,
    publish::{Boundary, verify_destination},
    *,
};
use anyhow::{Context, Result, ensure};
use rusqlite::{OptionalExtension, params};
use std::collections::HashSet;
use std::path::{Path, PathBuf};

impl Library {
    /// Freeze a complete caller-authorized audio/owned-sidecar scope. The first
    /// source must be this track's current native path and exact content hash.
    pub fn plan_organization(&self, request: OrganizationRequest) -> Result<String> {
        let _gate = self
            .inner
            .organization_gate
            .lock()
            .map_err(|_| anyhow::anyhow!("Organization gate interrupted"))?;
        self.plan_organization_inner(request, None, None)
    }
    pub(crate) fn plan_organization_for_row(
        &self,
        request: OrganizationRequest,
        row_id: &str,
    ) -> Result<String> {
        let _gate = self
            .inner
            .organization_gate
            .lock()
            .map_err(|_| anyhow::anyhow!("Organization gate interrupted"))?;
        self.plan_organization_inner(request, None, Some(row_id.into()))
    }
    fn plan_organization_inner(
        &self,
        request: OrganizationRequest,
        undo_of: Option<String>,
        batch_row_id: Option<String>,
    ) -> Result<String> {
        ensure!(
            !request.files.is_empty() && request.files.len() <= 256,
            "Organization requires one audio file and at most 255 owned sidecars"
        );
        Parent::capture(&request.source_root)?;
        Parent::capture(&request.destination_root)?;
        let (current_path, current_hash): (Vec<u8>, String) = self.conn()?.query_row(
            "SELECT path,source_hash FROM tracks WHERE id=?1",
            [&request.track_id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )?;
        ensure!(
            crate::model::bytes_path(&current_path)? == request.files[0].0,
            "Track path changed before organization planning"
        );
        let id = uuid::Uuid::new_v4().to_string();
        let mut files = Vec::new();
        let mut sources = HashSet::new();
        let mut targets = HashSet::new();
        let mut target_keys = HashSet::new();
        let mut identities = HashSet::new();
        for (index, (source, destination)) in request.files.iter().enumerate() {
            ensure!(
                source.starts_with(&request.source_root)
                    && destination.starts_with(&request.destination_root),
                "Organization leaves an authorized root"
            );
            ensure!(
                sources.insert(source.clone()) && targets.insert(destination.clone()),
                "Duplicate organization source or destination"
            );
            ensure!(
                target_keys.insert(
                    cuerithm_organize::paths::collision_key(destination)
                        .map_err(|error| anyhow::anyhow!(error))?
                ),
                "Organization contains case-equivalent destinations"
            );
            let source_folder = source.parent().context("Missing source folder")?;
            let destination_folder = destination.parent().context("Missing destination folder")?;
            if request.mode == OrganizationMode::Rename {
                ensure!(
                    source_folder == destination_folder,
                    "Rename must stay in the same folder"
                );
            }
            let (parent, source_parent) = Parent::capture(source_folder)?;
            let source_name = Path::new(source.file_name().context("Missing source file name")?);
            let original = parent.fingerprint(source_name)?;
            // Two names can open one native file (case aliases, hard links), and
            // cleanup of the first leaves the second without a source.
            ensure!(
                identities.insert((original.identity.volume, original.identity.object)),
                "Organization lists the same native source file more than once"
            );
            if index == 0 {
                ensure!(
                    original.sha256 == current_hash,
                    "Track content changed before organization planning"
                );
            }
            let (target, destination_parent) =
                capture_destination_parent(destination_folder, &request.destination_root)?;
            let target_name = Path::new(
                destination
                    .file_name()
                    .context("Missing destination file name")?,
            );
            let existing = if destination_parent.path.path()? == destination_folder {
                target.optional(target_name)?
            } else {
                None
            };
            let unchanged = source == destination;
            let case_only = !unchanged
                && source_folder == destination_folder
                && source_name
                    .to_str()
                    .zip(target_name.to_str())
                    .is_some_and(|(a, b)| a.to_lowercase() == b.to_lowercase())
                && existing.as_ref().is_some_and(|f| matches(f, &original));
            ensure!(
                request.mode != OrganizationMode::Copy || !case_only,
                "Copy cannot retain an original and publish to its case alias; choose Rename"
            );
            ensure!(
                !case_only || parent.single_link(source_name)?,
                "A hard-linked destination is not a case-only rename"
            );
            ensure!(
                existing.is_none() || unchanged || case_only,
                "Organization destination already exists"
            );
            let stage_name = format!(".cuerithm-{id}-{index}.stage");
            let quarantine_name = format!(".cuerithm-{id}-{index}.original");
            ensure!(
                target.optional(Path::new(&stage_name))?.is_none()
                    && parent.optional(Path::new(&quarantine_name))?.is_none(),
                "Organization recovery path already exists"
            );
            files.push(OrganizationFile {
                source: NativePath::new(source),
                destination: NativePath::new(destination),
                source_parent,
                destination_parent,
                original,
                staged: None,
                stage_complete: false,
                abandoned_stages: Vec::new(),
                stage_name,
                quarantine_name,
                unchanged,
                case_only,
                published: false,
                quarantined: false,
            });
        }
        ensure!(
            request.files.iter().all(|(_, d)| !sources.contains(d)
                || request.files.iter().any(|(s, d2)| s == d && d == d2)
                || files
                    .iter()
                    .any(|f| f.case_only && f.destination.path().ok().as_ref() == Some(d))),
            "Organization contains a source/destination cycle"
        );
        let phase = if files.iter().all(|f| f.unchanged) {
            OrganizationPhase::Unchanged
        } else {
            OrganizationPhase::Planned
        };
        let record = OrganizationRecord {
            id: id.clone(),
            track_id: request.track_id,
            mode: request.mode,
            phase,
            files,
            created_directories: Vec::new(),
            error: None,
            undo_of,
            batch_row_id,
        };
        self.insert_organization(&record)?;
        Ok(id)
    }
    fn insert_organization(&self, record: &OrganizationRecord) -> Result<()> {
        let mut conn = self.conn()?;
        let tx = conn.transaction()?;
        let now = crate::model::now();
        tx.execute("INSERT INTO organization_operations(id,track_id,state,payload_json,created,updated) VALUES(?1,?2,?3,?4,?5,?5)",params![record.id,record.track_id,record.phase.key(),serde_json::to_string(record)?,now])?;
        if record.phase != OrganizationPhase::Unchanged {
            // Destinations also carry a conservative case/Unicode-equivalent key so
            // two active plans cannot claim spellings of one name before either publishes.
            let mut reserved = std::collections::BTreeMap::new();
            for file in &record.files {
                reserved
                    .entry(&file.source.0)
                    .or_insert((&file.source, false));
                reserved
                    .entry(&file.destination.0)
                    .or_insert((&file.destination, true))
                    .1 = true;
            }
            for (bytes, (path, destination)) in reserved {
                let key = if destination {
                    let key = cuerithm_organize::paths::collision_key(&path.path()?)
                        .map_err(|error| anyhow::anyhow!(error))?;
                    let owner: Option<String> = tx
                        .query_row(
                            "SELECT operation_id FROM organization_reservations WHERE collision_key=?1 AND operation_id!=?2 LIMIT 1",
                            params![key, record.id],
                            |r| r.get(0),
                        )
                        .optional()?;
                    ensure!(
                        owner.is_none(),
                        "Another active organization already reserves this destination or a case-equivalent spelling of it"
                    );
                    Some(key)
                } else {
                    None
                };
                tx.execute("INSERT INTO organization_reservations(path,collision_key,operation_id) VALUES(?1,?2,?3)",params![bytes,key,record.id])?;
            }
        }
        if let Some(row) = &record.batch_row_id {
            ensure!(tx.execute("UPDATE organization_batch_rows SET operation_id=?1 WHERE id=?2 AND track_id=?3 AND operation_id IS NULL",params![record.id,row,record.track_id])?==1,"Batch row already owns an organization operation");
            self.organization_validate_operation_tx(&tx, record)?;
        }
        tx.commit()?;
        Ok(())
    }
    /// Run until publication needs reference acknowledgement, or finish cleanup
    /// after acknowledgement. An error preserves every original/recovery asset.
    pub fn execute_organization(&self, id: &str) -> Result<OrganizationRecord> {
        self.execute_organization_with(id, |_| Ok(()))
    }
    pub(crate) fn execute_organization_with(
        &self,
        id: &str,
        boundary: impl FnMut(Boundary) -> Result<()>,
    ) -> Result<OrganizationRecord> {
        self.execute_organization_with_guard(id, None, boundary)
    }
    pub fn execute_organization_checked(
        &self,
        id: &str,
        guard: &crate::OrganizationReferenceGuard,
    ) -> Result<OrganizationRecord> {
        self.execute_organization_with_guard(id, Some(guard), |_| Ok(()))
    }
    pub(crate) fn execute_organization_with_checked(
        &self,
        id: &str,
        guard: &crate::OrganizationReferenceGuard,
        boundary: impl FnMut(Boundary) -> Result<()>,
    ) -> Result<OrganizationRecord> {
        self.execute_organization_with_guard(id, Some(guard), boundary)
    }
    fn execute_organization_with_guard(
        &self,
        id: &str,
        guard: Option<&crate::OrganizationReferenceGuard>,
        boundary: impl FnMut(Boundary) -> Result<()>,
    ) -> Result<OrganizationRecord> {
        let _gate = self
            .inner
            .organization_gate
            .lock()
            .map_err(|_| anyhow::anyhow!("Organization gate interrupted"))?;
        #[cfg(feature = "autotag")]
        let _tag_gate = self
            .inner
            .tag_gate
            .lock()
            .map_err(|_| anyhow::anyhow!("Publication gate interrupted"))?;
        let mut record = self.organization_record(id)?;
        if matches!(
            record.phase,
            OrganizationPhase::Planned
                | OrganizationPhase::Publishing
                | OrganizationPhase::AwaitingReferences
                | OrganizationPhase::Cleanup
        ) {
            let mut conn = self.conn()?;
            let tx = conn.transaction()?;
            self.organization_validate_operation_tx(&tx, &record)?;
            tx.commit()?;
        }
        if let Some(guard) = guard {
            ensure!(
                guard.operation_id() == id,
                "Reference guard belongs to another organization operation"
            );
        }
        if record.phase == OrganizationPhase::Cleanup {
            let owned = self.organization_service_owned(id)?
                || self.organization_has_required_references(id)?;
            ensure!(
                !owned || guard.is_some(),
                "Verified current reference receipts are required before source cleanup"
            );
            if let Some(guard) = guard {
                self.validate_live_organization_reference_guard(&record, guard)?;
            }
        }
        record.error = None;
        let result = self.publish_organization(&mut record, boundary);
        if let Err(error) = result {
            record.error = Some(format!("{error:#}"));
            self.save_organization(&record)?;
            return Err(error);
        }
        Ok(record)
    }
    /// Commit the library path only AFTER required native-reference repair.
    /// PR2 owns that caller contract. This transaction never sets a track Ready.
    pub fn acknowledge_organization_references(&self, id: &str) -> Result<OrganizationRecord> {
        self.acknowledge_organization_references_inner(id, None)
    }
    pub fn acknowledge_organization_references_checked(
        &self,
        id: &str,
        guard: &crate::OrganizationReferenceGuard,
    ) -> Result<OrganizationRecord> {
        self.acknowledge_organization_references_inner(id, Some(guard))
    }
    fn acknowledge_organization_references_inner(
        &self,
        id: &str,
        guard: Option<&crate::OrganizationReferenceGuard>,
    ) -> Result<OrganizationRecord> {
        let _gate = self
            .inner
            .organization_gate
            .lock()
            .map_err(|_| anyhow::anyhow!("Organization gate interrupted"))?;
        #[cfg(feature = "autotag")]
        let _tag_gate = self
            .inner
            .tag_gate
            .lock()
            .map_err(|_| anyhow::anyhow!("Publication gate interrupted"))?;
        let mut record = self.organization_record(id)?;
        ensure!(
            record.phase == OrganizationPhase::AwaitingReferences && record.error.is_none(),
            "Organization has not completed verified publication"
        );
        for file in &record.files {
            verify_destination(file)?;
        }
        ensure!(
            guard.is_some()
                || (!self.organization_service_owned(id)?
                    && !self.organization_has_required_references(id)?),
            "Service-owned organization requires a checked reference acknowledgement"
        );
        if let Some(guard) = guard {
            self.validate_organization_reference_source_proof(&record, &guard.source_proof)?;
            self.validate_live_organization_reference_guard(&record, guard)?;
        }
        let main = &record.files[0];
        let mut conn = self.conn()?;
        let tx = conn.transaction()?;
        self.organization_validate_operation_tx(&tx, &record)?;
        if let Some(guard) = guard {
            self.organization_references_ack_tx(&tx, &record, guard)?;
        }
        let changed = tx.execute(
            "UPDATE tracks SET path=?1,updated=?2 WHERE id=?3 AND path=?4 AND source_hash=?5",
            params![
                main.destination.0,
                crate::model::now(),
                record.track_id,
                main.source.0,
                main.original.sha256
            ],
        )?;
        ensure!(
            changed == 1,
            "Track changed before organization path commit"
        );
        self.complete_organization_copy_conflict_tx(&tx, &record, guard.map(|g| &g.source_proof))?;
        if let Some(previous) = &record.undo_of {
            tx.execute(
                "UPDATE organization_alternates SET state='retired' WHERE operation_id=?1",
                [previous],
            )?;
        }
        for file in record.files.iter().skip(1) {
            tx.execute("UPDATE organization_owned_sidecars SET path=?1 WHERE track_id=?2 AND path=?3 AND source_hash=?4",params![file.destination.0,record.track_id,file.source.0,file.original.sha256])?;
        }
        record.phase = OrganizationPhase::Cleanup;
        tx.execute("UPDATE organization_operations SET state='cleanup',payload_json=?1,error=NULL,updated=?2 WHERE id=?3",params![serde_json::to_string(&record)?,crate::model::now(),id])?;
        Self::bump(&tx)?;
        tx.commit()?;
        Ok(record)
    }
    pub(crate) fn verify_organization_publication(&self, id: &str) -> Result<OrganizationRecord> {
        let _gate = self
            .inner
            .organization_gate
            .lock()
            .map_err(|_| anyhow::anyhow!("Organization gate interrupted"))?;
        #[cfg(feature = "autotag")]
        let _tag_gate = self
            .inner
            .tag_gate
            .lock()
            .map_err(|_| anyhow::anyhow!("Publication gate interrupted"))?;
        let record = self.organization_record(id)?;
        ensure!(
            record.phase == OrganizationPhase::AwaitingReferences && record.error.is_none(),
            "Organization publication is not ready for reference repair"
        );
        for file in &record.files {
            verify_destination(file)?;
        }
        Ok(record)
    }
    pub(crate) fn verify_organization_reference_destinations(
        record: &OrganizationRecord,
    ) -> Result<()> {
        for file in &record.files {
            verify_destination(file)?;
        }
        Ok(())
    }
    pub(crate) fn finish_organization(&self, record: &OrganizationRecord) -> Result<()> {
        let mut conn = self.conn()?;
        let tx = conn.transaction()?;
        tx.execute("UPDATE organization_operations SET state=?1,payload_json=?2,error=NULL,updated=?3 WHERE id=?4",params![record.phase.key(),serde_json::to_string(record)?,crate::model::now(),record.id])?;
        tx.execute(
            "DELETE FROM organization_reservations WHERE operation_id=?1",
            [&record.id],
        )?;
        if let Some(previous) = &record.undo_of
            && record.phase == OrganizationPhase::Complete
        {
            let mut old = {
                let json: String = tx.query_row(
                    "SELECT payload_json FROM organization_operations WHERE id=?1",
                    [previous],
                    |r| r.get(0),
                )?;
                serde_json::from_str::<OrganizationRecord>(&json)?
            };
            old.phase = OrganizationPhase::Undone;
            tx.execute("UPDATE organization_operations SET state='undone',payload_json=?1,updated=?2 WHERE id=?3",params![serde_json::to_string(&old)?,crate::model::now(),previous])?;
        }
        tx.commit()?;
        Ok(())
    }
    pub fn cancel_unstarted_organization(&self, id: &str) -> Result<()> {
        let _gate = self
            .inner
            .organization_gate
            .lock()
            .map_err(|_| anyhow::anyhow!("Organization gate interrupted"))?;
        let mut record = self.organization_record(id)?;
        ensure!(
            matches!(
                record.phase,
                OrganizationPhase::Planned | OrganizationPhase::Publishing
            ) && record.files.iter().all(|f| !f.published && !f.quarantined),
            "Partially published operations require recovery, not cancellation"
        );
        for file in &record.files {
            if let Some(stage) = &file.staged {
                // A crash can leave publication/quarantine on disk before its
                // journal flag. Never release that ownership on flags alone.
                let parent = Parent::reopen(&file.destination_parent)?;
                let destination = file.destination.path()?;
                let actual = parent.optional(Path::new(
                    destination
                        .file_name()
                        .context("Missing destination name")?,
                ))?;
                ensure!(
                    !actual.is_some_and(|actual| actual.identity == stage.identity),
                    "Publication reached disk; recover before cancellation"
                );
                if file.case_only {
                    let parent = Parent::reopen(&file.source_parent)?;
                    ensure!(
                        parent.optional(Path::new(&file.quarantine_name))?.is_none(),
                        "Original quarantine reached disk; recover before cancellation"
                    );
                }
            }
        }
        record.phase = OrganizationPhase::Cancelled;
        record.error = None;
        self.finish_organization(&record)
    }
    /// Produce a new guarded reverse operation; references still require the
    /// normal acknowledgement before the current managed file is quarantined.
    pub fn plan_organization_undo(&self, id: &str) -> Result<String> {
        self.plan_organization_undo_inner(id, None)
    }
    pub(crate) fn plan_organization_undo_for_row(&self, id: &str, row_id: &str) -> Result<String> {
        self.plan_organization_undo_inner(id, Some(row_id.into()))
    }
    fn plan_organization_undo_inner(
        &self,
        id: &str,
        batch_row_id: Option<String>,
    ) -> Result<String> {
        let _gate = self
            .inner
            .organization_gate
            .lock()
            .map_err(|_| anyhow::anyhow!("Organization gate interrupted"))?;
        let record = self.organization_record(id)?;
        ensure!(
            record.phase == OrganizationPhase::Complete
                && record.error.is_none()
                && record.undo_of.is_none(),
            "Only complete original operations can be undone"
        );
        for file in &record.files {
            Parent::reopen(&file.source_parent)?;
            verify_destination(file)?;
        }
        if record.mode != OrganizationMode::Copy {
            let files = record
                .files
                .iter()
                .map(|f| Ok((f.destination.path()?, f.source.path()?)))
                .collect::<Result<Vec<_>>>()?;
            let source_root = common_parent(files.iter().map(|p| p.0.as_path()))?;
            let destination_root = common_parent(files.iter().map(|p| p.1.as_path()))?;
            let inverse_id = self.plan_organization_inner(
                OrganizationRequest {
                    track_id: record.track_id,
                    mode: OrganizationMode::Move,
                    source_root,
                    destination_root,
                    files,
                },
                Some(id.into()),
                batch_row_id,
            )?;
            return Ok(inverse_id);
        }
        let inverse_id = uuid::Uuid::new_v4().to_string();
        let mut files = Vec::new();
        for (index, file) in record.files.iter().enumerate() {
            let parent = Parent::reopen(&file.source_parent)?;
            let original = file.source.path()?;
            let actual = parent.fingerprint(Path::new(
                original.file_name().context("Missing original name")?,
            ))?;
            ensure!(
                matches(&actual, &file.original),
                "Original copy source changed; Undo cannot overwrite or rebind it"
            );
            let managed = file.staged.clone().unwrap_or_else(|| file.original.clone());
            files.push(OrganizationFile {
                source: file.destination.clone(),
                destination: file.source.clone(),
                source_parent: file.destination_parent.clone(),
                destination_parent: file.source_parent.clone(),
                original: managed,
                staged: Some(actual),
                stage_complete: true,
                abandoned_stages: Vec::new(),
                stage_name: format!(".cuerithm-{inverse_id}-{index}.stage"),
                quarantine_name: format!(".cuerithm-{inverse_id}-{index}.original"),
                unchanged: file.unchanged,
                case_only: false,
                published: true,
                quarantined: false,
            });
        }
        self.insert_organization(&OrganizationRecord {
            id: inverse_id.clone(),
            track_id: record.track_id,
            mode: OrganizationMode::Move,
            phase: OrganizationPhase::AwaitingReferences,
            files,
            created_directories: Vec::new(),
            error: None,
            undo_of: Some(id.into()),
            batch_row_id,
        })?;
        Ok(inverse_id)
    }
}
fn common_parent<'a>(paths: impl Iterator<Item = &'a Path>) -> Result<PathBuf> {
    let mut paths = paths;
    let mut root = paths
        .next()
        .context("Empty organization scope")?
        .parent()
        .context("Missing parent")?
        .to_path_buf();
    for path in paths {
        while !path.starts_with(&root) {
            ensure!(root.pop(), "Paths have no common native root");
        }
    }
    Ok(root)
}

fn capture_destination_parent(path: &Path, root: &Path) -> Result<(Parent, Authority)> {
    let mut existing = path.to_path_buf();
    loop {
        match Parent::capture(&existing) {
            Ok(parent) => return Ok(parent),
            Err(error)
                if error
                    .downcast_ref::<std::io::Error>()
                    .is_some_and(|e| e.kind() == std::io::ErrorKind::NotFound) =>
            {
                ensure!(
                    existing != root && existing.pop() && existing.starts_with(root),
                    "Destination root is unavailable"
                );
            }
            Err(error) => return Err(error),
        }
    }
}
