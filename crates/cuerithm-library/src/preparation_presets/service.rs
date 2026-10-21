use super::*;
use crate::{Library, model::now};
use anyhow::{Context, Result, ensure};
use rusqlite::{OptionalExtension, params};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct SavedPreset {
    pub id: String,
    pub revision: String,
    pub name: String,
    pub rules: PreparationPresetRules,
    pub connections: PreparationPresetConnections,
}
impl Library {
    pub(super) fn load_preparation_preset(&self, id: &str) -> Result<SavedPreset> {
        let json:String=self.conn()?.query_row("SELECT r.payload_json FROM preparation_presets p JOIN preparation_preset_revisions r ON r.preset_id=p.id AND r.revision=p.current_revision WHERE p.id=?1 AND p.deleted=0",[id],|r|r.get(0)).optional()?.context("Preparation preset is missing or deleted")?;
        ensure!(
            json.len() <= MAX_PREPARATION_PRESET_DOCUMENT_BYTES,
            "Stored preset exceeds bounded size"
        );
        let result: SavedPreset = serde_json::from_str(&json)?;
        ensure!(result.id == id, "Stored preset identity differs");
        super::portable::validate(&result.rules)?;
        Ok(result)
    }
    pub(super) fn preparation_preset_status(
        &self,
        rules: &PreparationPresetRules,
        connections: &PreparationPresetConnections,
    ) -> Result<(String, Vec<PreparationPresetIssue>)> {
        let mut issues = Vec::new();
        #[cfg(not(feature = "autotag"))]
        for (enabled, field) in [
            (rules.autotag_enabled, "autotag_enabled"),
            (rules.cleanup_enabled, "cleanup_enabled"),
        ] {
            if enabled {
                issues.push(PreparationPresetIssue{code:"unsupported".into(),field:field.into(),message:"This build cannot run automatic tagging/cleanup; the saved rules are preserved".into()});
            }
        }
        if matches!(
            rules.organization.mode,
            crate::OrganizationSettingMode::Move | crate::OrganizationSettingMode::Copy
        ) {
            match connections.destination_root_id.as_deref() {
                None => issues.push(PreparationPresetIssue {
                    code: "destination_required".into(),
                    field: "connections.destination_root_id".into(),
                    message: "Choose a destination folder on this computer".into(),
                }),
                Some(id) => {
                    if self.organization_root_authority(id).is_err() {
                        issues.push(PreparationPresetIssue{code:"destination_unavailable".into(),field:"connections.destination_root_id".into(),message:"Destination is offline or its native identity changed; reconnect the folder".into()});
                    }
                }
            }
        }
        let status = if issues.iter().any(|issue| issue.code == "unsupported") {
            "unsupported"
        } else if issues.is_empty() {
            "ready"
        } else {
            "needs_connection"
        };
        Ok((status.into(), issues))
    }
    pub fn preparation_preset(&self, id: &str) -> Result<PreparationPreset> {
        self.preparation_preset_public(self.load_preparation_preset(id)?)
    }
    fn preparation_preset_public(&self, preset: SavedPreset) -> Result<PreparationPreset> {
        let (status, issues) =
            self.preparation_preset_status(&preset.rules, &preset.connections)?;
        let count = self.conn()?.query_row(
            "SELECT COUNT(*) FROM preparation_preset_assignments WHERE preset_id=?1",
            [&preset.id],
            |r| r.get(0),
        )?;
        Ok(PreparationPreset {
            id: preset.id,
            revision: preset.revision,
            name: preset.name,
            rules: preset.rules,
            connections: preset.connections,
            status,
            issues,
            assigned_folder_count: count,
        })
    }
    pub fn preparation_presets(&self) -> Result<Vec<PreparationPresetSummary>> {
        let ids: Vec<String> = {
            let conn = self.conn()?;
            let mut q = conn.prepare(
                "SELECT id FROM preparation_presets WHERE deleted=0 ORDER BY name_key,id LIMIT 101",
            )?;
            q.query_map([], |r| r.get(0))?
                .collect::<rusqlite::Result<_>>()?
        };
        ensure!(ids.len() <= 100, "Preset limit exceeded");
        ids.into_iter()
            .map(|id| {
                let p = self.preparation_preset(&id)?;
                Ok(PreparationPresetSummary {
                    id: p.id,
                    revision: p.revision,
                    name: p.name,
                    status: p.status,
                    issues: p.issues,
                    assigned_folder_count: p.assigned_folder_count,
                })
            })
            .collect()
    }
    pub fn save_preparation_preset(
        &self,
        request: PreparationPresetSave,
    ) -> Result<PreparationPreset> {
        self.save_preparation_preset_with(request, || Ok(()))
    }
    pub(crate) fn save_preparation_preset_with(
        &self,
        request: PreparationPresetSave,
        after_commit: impl FnOnce() -> Result<()>,
    ) -> Result<PreparationPreset> {
        let name = super::portable::valid_name(&request.name)?;
        super::portable::validate(&request.rules)?;
        if let Some(root) = &request.connections.destination_root_id {
            ensure!(
                self.conn()?.query_row(
                    "SELECT EXISTS(SELECT 1 FROM organization_roots WHERE id=?1)",
                    [root],
                    |r| r.get::<_, bool>(0)
                )?,
                "Choose a native destination folder before connecting this preset"
            );
        }
        let creating = request.id.is_none();
        let id = request
            .id
            .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
        ensure!(id.len() <= 128, "Invalid preset ID");
        let revision = format!(
            "{:x}",
            Sha256::digest(serde_json::to_vec(&(
                &id,
                &name,
                &request.rules,
                &request.connections
            ))?)
        );
        let preset = SavedPreset {
            id: id.clone(),
            revision: revision.clone(),
            name: name.clone(),
            rules: request.rules,
            connections: request.connections,
        };
        let json = serde_json::to_string(&preset)?;
        ensure!(
            json.len() <= MAX_PREPARATION_PRESET_DOCUMENT_BYTES,
            "Preset exceeds the 1 MiB limit"
        );
        {
            let mut conn = self.conn()?;
            let tx = conn.transaction()?;
            if creating {
                ensure!(
                    request.expected_revision.is_none(),
                    "New presets cannot replace another revision"
                );
                let count: u64 = tx.query_row(
                    "SELECT COUNT(*) FROM preparation_presets WHERE deleted=0",
                    [],
                    |r| r.get(0),
                )?;
                ensure!(count < 100, "Keep at most 100 active presets");
                tx.execute("INSERT INTO preparation_presets(id,name,name_key,current_revision,created,updated) VALUES(?1,?2,?3,?4,?5,?5)",params![id,name,name.to_lowercase(),revision,now()]).context("Preset name is already in use")?;
            } else {
                let expected = request
                    .expected_revision
                    .context("Reload the preset before updating it")?;
                ensure!(tx.execute("UPDATE preparation_presets SET name=?1,name_key=?2,current_revision=?3,updated=?4 WHERE id=?5 AND current_revision=?6 AND deleted=0",params![name,name.to_lowercase(),revision,now(),id,expected]).context("Preset name is already in use")?==1,"Preset changed; reload before updating it");
            }
            tx.execute(
                "INSERT OR IGNORE INTO preparation_preset_revisions VALUES(?1,?2,?3,?4)",
                params![id, revision, json, now()],
            )?;
            tx.commit()?;
        }
        after_commit()?;
        self.preparation_preset_public(preset)
    }
    pub fn duplicate_preparation_preset(
        &self,
        id: &str,
        revision: &str,
        name: &str,
    ) -> Result<PreparationPreset> {
        let p = self.preparation_preset(id)?;
        ensure!(
            p.revision == revision,
            "Preset changed; reload before duplicating"
        );
        self.save_preparation_preset(PreparationPresetSave {
            id: None,
            expected_revision: None,
            name: name.into(),
            rules: p.rules,
            connections: p.connections,
        })
    }
    pub fn delete_preparation_preset(&self, id: &str, revision: &str) -> Result<()> {
        let mut conn = self.conn()?;
        let tx = conn.transaction()?;
        ensure!(
            !tx.query_row(
                "SELECT EXISTS(SELECT 1 FROM preparation_preset_assignments WHERE preset_id=?1)",
                [id],
                |r| r.get::<_, bool>(0)
            )?,
            "Remove folder assignments before deleting this preset"
        );
        ensure!(tx.execute("UPDATE preparation_presets SET deleted=1,updated=?1 WHERE id=?2 AND current_revision=?3 AND deleted=0",params![now(),id,revision])?==1,"Preset changed or was deleted; reload before deleting");
        tx.commit()?;
        Ok(())
    }
}
