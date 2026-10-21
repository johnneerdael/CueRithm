use super::*;
use crate::{
    JobId, JobRun, Library, SettingsSnapshot,
    model::now,
    organization::{Authority, NativeParent},
};
use anyhow::{Context, Result, ensure};
use rusqlite::{OptionalExtension, params};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
impl Library {
    pub fn current_preparation_snapshot_for_path(&self, path: &Path) -> Result<SettingsSnapshot> {
        self.current_preparation_snapshot_for_path_with(path, || Ok(()))
    }
    pub(crate) fn current_preparation_snapshot_for_path_with(
        &self,
        path: &Path,
        mut after_read: impl FnMut() -> Result<()>,
    ) -> Result<SettingsSnapshot> {
        let source = path
            .canonicalize()
            .context("Incoming source is unavailable")?;
        ensure!(
            source.is_file(),
            "Only audio files can resolve preparation rules"
        );
        for _ in 0..3 {
            let token = {
                let conn = self.conn()?;
                routing_token(&conn)?
            };
            let base = self.settings()?;
            after_read()?;
            let folders = self.preparation_routing_folders(&base.settings, &source)?;
            let Some(folder) = Self::select_preparation_folder(&folders, &source)? else {
                let conn = self.conn()?;
                if routing_token(&conn)? != token {
                    continue;
                }
                return Ok(base);
            };
            let (_source_parent, source_authority) =
                NativeParent::capture(source.parent().context("Missing source parent")?)?;
            let authority = Self::verify_preparation_folder(folder)?.clone();
            ensure!(
                source_authority.ancestors.starts_with(&authority.ancestors),
                "Source no longer belongs to the assigned native folder"
            );
            let (settings, preset_id, preset_revision, preset_name) =
                if let PreparationFolderChoice::Preset { preset_id } = &folder.choice {
                    let preset = self.preparation_preset(preset_id)?;
                    ensure!(
                        preset.status == "ready",
                        "Assigned preset '{}' is unavailable: {}",
                        preset.name,
                        preset
                            .issues
                            .iter()
                            .map(|issue| issue.message.as_str())
                            .collect::<Vec<_>>()
                            .join("; ")
                    );
                    (
                        super::portable::overlay(
                            &base.settings,
                            &preset.rules,
                            &preset.connections,
                        )?,
                        Some(preset.id),
                        Some(preset.revision),
                        Some(preset.name),
                    )
                } else {
                    (base.settings.clone(), None, None, None)
                };
            settings.validate()?;
            let capture = PreparationPresetCapture {
                preset_id,
                preset_revision,
                preset_name,
                assignment_id: Some(folder.id.clone()),
                assignment_revision: folder.revision.clone(),
                base_settings_revision: base.revision,
            };
            let revision = format!(
                "{:x}",
                Sha256::digest(serde_json::to_vec(
                    &serde_json::json!({"settings":settings,"models":base.models,"preset_capture":capture})
                )?)
            );
            let snapshot = SettingsSnapshot {
                revision: revision.clone(),
                settings,
                models: base.models,
                preset_capture: Some(capture),
            };
            NativeParent::reopen(&authority)?;
            let encoded = serde_json::to_string(&snapshot)?;
            let authority = serde_json::to_string(&authority)?;
            {
                let mut conn = self.conn()?;
                let tx = conn.transaction()?;
                if routing_token(&tx)? != token {
                    continue;
                }
                tx.execute(
                    "INSERT OR IGNORE INTO settings VALUES(?1,?2,?3)",
                    params![revision, encoded, now()],
                )?;
                tx.execute(
                    "INSERT OR IGNORE INTO preparation_preset_capture_authorities VALUES(?1,?2)",
                    params![revision, authority],
                )?;
                let saved:String=tx.query_row("SELECT authority_json FROM preparation_preset_capture_authorities WHERE settings_revision=?1",[&revision],|r|r.get(0))?;
                ensure!(
                    saved == authority,
                    "Captured folder proof changed; all work retained"
                );
                tx.commit()?;
            }
            return Ok(snapshot);
        }
        anyhow::bail!("Preparation rules changed repeatedly during capture; retry admission")
    }
    /// Current rules govern a fresh retry. An already accepted operation keeps its
    /// captured policy when those rules are unavailable or would invalidate its
    /// evidence; only the existing safe unpublished naming correction may rebind.
    pub(crate) fn current_preparation_retry_revision(
        &self,
        job: &str,
    ) -> Result<Option<crate::SettingsRevision>> {
        let (path, revision, current_incoming): (Vec<u8>, String, bool) = self.conn()?.query_row(
            "SELECT t.path,j.settings_revision,t.current_job=j.id AND t.state='incoming' AND t.archived=0 FROM jobs j JOIN tracks t ON t.id=j.track_id WHERE j.id=?1",
            [job],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )?;
        let owned = current_incoming && self.has_preparation_checkpoint(job)?;
        #[cfg(feature = "autotag")]
        let owned = owned || (current_incoming && self.conn()?.query_row(
            "SELECT EXISTS(SELECT 1 FROM preparation_review_decisions WHERE job_id=?1 AND state IN ('pending','claimed','applied')) OR EXISTS(SELECT 1 FROM metadata_edit_overrides WHERE job_id=?1 AND state IN ('queued','claimed'))",
            [job],
            |row| row.get::<_,bool>(0),
        )?);
        let next =
            match self.current_preparation_snapshot_for_path(&crate::model::bytes_path(&path)?) {
                Ok(next) => next,
                Err(_) if owned => return Ok(None),
                Err(error) => return Err(error),
            };
        if owned {
            let captured = self.settings_revision(&revision)?;
            if !Self::settings_equal_except_solver(&captured, &next)? {
                let mut naming_only = captured;
                naming_only.settings.organization = next.settings.organization.clone();
                if !Self::settings_equal_except_solver(&naming_only, &next)? {
                    return Ok(None);
                }
                let bound:bool=self.conn()?.query_row("SELECT EXISTS(SELECT 1 FROM preparation_checkpoints WHERE job_id=?1 AND state!='complete' AND (state!='prepared' OR batch_id IS NOT NULL))",[job],|row|row.get(0))?;
                if bound {
                    return Ok(None);
                }
            }
        }
        Ok(Some(next.revision))
    }
    pub fn enqueue_with_current_preparation(
        &self,
        paths: &[PathBuf],
        discovered: bool,
    ) -> Result<Vec<JobId>> {
        let base = self.settings()?.revision;
        let mut ids = Vec::new();
        let mut errors = Vec::new();
        for path in paths {
            match self.enqueue_sources_with_current_rules(
                std::slice::from_ref(path),
                base.clone(),
                discovered,
            ) {
                Ok(next) => ids.extend(next),
                Err(error) => errors.push(format!("{}: {error:#}", path.display())),
            }
        }
        ensure!(errors.is_empty(), "{}", errors.join("\n"));
        Ok(ids)
    }
    pub(crate) fn verify_preparation_preset_source(&self, job: &JobRun) -> Result<()> {
        if job.snapshot.preset_capture.is_none() {
            return Ok(());
        }
        let json:String=self.conn()?.query_row("SELECT authority_json FROM preparation_preset_capture_authorities WHERE settings_revision=?1",[&job.settings_revision],|r|r.get(0)).optional()?.context("Captured preset folder proof is missing; retain this job for review")?;
        let authority: Authority = serde_json::from_str(&json)?;
        let _root = NativeParent::reopen(&authority).context(
            "Captured preset folder changed; reconnect and explicitly retry current rules",
        )?;
        let (_parent, source) =
            NativeParent::capture(job.path.parent().context("Missing source parent")?)?;
        ensure!(
            source.ancestors.starts_with(&authority.ancestors),
            "Source left its captured preset folder"
        );
        Ok(())
    }
}

fn routing_token(conn: &rusqlite::Connection) -> Result<String> {
    let selected: String = conn.query_row(
        "SELECT revision FROM selected_settings WHERE singleton=1",
        [],
        |row| row.get(0),
    )?;
    type RoutingTokenRow = (
        String,
        String,
        String,
        Vec<u8>,
        Option<String>,
        Option<String>,
        Option<bool>,
    );
    let mut query=conn.prepare("SELECT a.folder_id,a.revision,a.choice_json,f.path,f.authority_json,p.current_revision,p.deleted FROM preparation_preset_assignments a JOIN preparation_preset_folders f ON f.id=a.folder_id LEFT JOIN preparation_presets p ON p.id=a.preset_id ORDER BY a.folder_id LIMIT 257")?;
    let rows: Vec<RoutingTokenRow> = query
        .query_map([], |row| {
            Ok((
                row.get(0)?,
                row.get(1)?,
                row.get(2)?,
                row.get(3)?,
                row.get(4)?,
                row.get(5)?,
                row.get(6)?,
            ))
        })?
        .collect::<rusqlite::Result<_>>()?;
    ensure!(rows.len() <= 256, "Too many folder assignments");
    Ok(format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(&(selected, rows))?)
    ))
}
