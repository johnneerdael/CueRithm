//! Named target setup only. Saving a configuration never starts publication.
use crate::{Library, RekordboxSyncTarget};
use anyhow::{Context, Result, ensure};
use cuerithm_dj_sync::{DjSoftware, SyncTarget};
use rusqlite::{OptionalExtension, params};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum SavedSyncTarget {
    DjLibrary { target: SyncTarget },
    Rekordbox { target: RekordboxSyncTarget },
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncConfigurationBinding {
    pub id: String,
    pub revision: u64,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncTargetConfiguration {
    pub id: String,
    pub name: String,
    pub revision: u64,
    pub target: SavedSyncTarget,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SaveSyncTargetConfiguration {
    pub id: Option<String>,
    pub expected_revision: Option<u64>,
    pub name: String,
    pub target: SavedSyncTarget,
}
fn checked_id(id: &str) -> Result<()> {
    ensure!(
        uuid::Uuid::parse_str(id)?.to_string() == id,
        "Invalid saved target identity"
    );
    Ok(())
}
fn normalize(mut value: SavedSyncTarget) -> Result<SavedSyncTarget> {
    match &mut value {
        SavedSyncTarget::DjLibrary { target } => {
            ensure!(
                target.software != DjSoftware::Rekordbox,
                "Use the native rekordbox setup"
            );
            ensure!(
                target.path.is_absolute(),
                "Library location must be absolute"
            );
            *target = crate::dj_sync_service::canonical_target(target.clone())?;
            target.environment.validate()?;
        }
        SavedSyncTarget::Rekordbox { target } => {
            target.environment.validate()?;
            ensure!(
                !target.id.is_empty() && matches!(target.installation.major, 6 | 7),
                "Configure a supported rekordbox 6 or 7 installation first"
            );
            for path in [
                &target.database,
                &target.options_file,
                &target.installation.executable,
            ] {
                ensure!(
                    path.is_absolute() && path.is_file(),
                    "Selected rekordbox file is unavailable"
                );
            }
            ensure!(
                target.asset_root.is_absolute(),
                "rekordbox asset root must be absolute"
            );
            // The manual builder owns native identity and canonical location checks.
            // Never derive or store a key during configuration.
        }
    }
    Ok(value)
}
impl Library {
    pub fn list_sync_target_configurations(&self) -> Result<Vec<SyncTargetConfiguration>> {
        let connection = self.conn()?;
        let mut query = connection.prepare("SELECT id,name,revision,target_json FROM sync_target_configurations ORDER BY name COLLATE NOCASE,id")?;
        let rows = query.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, u64>(2)?,
                row.get::<_, String>(3)?,
            ))
        })?;
        rows.map(|row| {
            let (id, name, revision, json) = row?;
            Ok(SyncTargetConfiguration {
                id,
                name,
                revision,
                target: serde_json::from_str(&json)?,
            })
        })
        .collect()
    }
    pub fn save_sync_target_configuration(
        &self,
        request: SaveSyncTargetConfiguration,
    ) -> Result<SyncTargetConfiguration> {
        let name = request.name.trim();
        ensure!(
            !name.is_empty() && name.chars().count() <= 100 && !name.chars().any(char::is_control),
            "Give the saved setup a name of 1–100 characters without control characters"
        );
        ensure!(
            request.id.is_some() == request.expected_revision.is_some(),
            "Saved setup revision is required for an update"
        );
        let target = normalize(match request.target {
            SavedSyncTarget::Rekordbox { target } => SavedSyncTarget::Rekordbox {
                target: self.configure_rekordbox_sync_target(crate::RekordboxManualTarget {
                    executable: target.installation.executable,
                    version_hint: Some(target.installation.version),
                    database: target.database,
                    options_file: target.options_file,
                    asset_root: target.asset_root,
                    environment: target.environment,
                })?,
            },
            other => other,
        })?;
        let json = serde_json::to_string(&target)?;
        ensure!(json.len() <= 1024 * 1024, "Saved target setup is too large");
        let _guard = crate::dj_sync_service::service_lock(&self.data_dir().join("sync"))?;
        let mut connection = self.conn()?;
        let transaction =
            connection.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        let (id, revision) = if let Some(id) = request.id {
            checked_id(&id)?;
            let previous: Option<u64> = transaction
                .query_row(
                    "SELECT revision FROM sync_target_configurations WHERE id=?1",
                    [&id],
                    |row| row.get(0),
                )
                .optional()?;
            ensure!(
                previous == request.expected_revision,
                "Saved target setup changed; reload it before saving"
            );
            let revision = previous
                .context("Saved target setup was removed")?
                .checked_add(1)
                .context("Saved target revision exhausted")?;
            ensure!(
                revision <= i64::MAX as u64,
                "Saved target revision exhausted"
            );
            transaction.execute("UPDATE sync_target_configurations SET name=?2,revision=?3,target_json=?4,updated=?5 WHERE id=?1", params![id,name,revision,json,crate::model::now()])?;
            (id, revision)
        } else {
            let id = uuid::Uuid::new_v4().to_string();
            transaction.execute("INSERT INTO sync_target_configurations(id,name,revision,target_json,updated) VALUES(?1,?2,1,?3,?4)", params![id,name,json,crate::model::now()])?;
            (id, 1)
        };
        transaction.commit()?;
        Ok(SyncTargetConfiguration {
            id,
            name: name.to_owned(),
            revision,
            target,
        })
    }
    pub fn delete_sync_target_configuration(&self, id: &str, expected_revision: u64) -> Result<()> {
        checked_id(id)?;
        let _guard = crate::dj_sync_service::service_lock(&self.data_dir().join("sync"))?;
        ensure!(
            self.conn()?.execute(
                "DELETE FROM sync_target_configurations WHERE id=?1 AND revision=?2",
                params![id, expected_revision]
            )? == 1,
            "Saved target setup changed or was removed; reload it before deleting"
        );
        Ok(())
    }
    pub fn verify_sync_target_configuration(
        &self,
        binding: &SyncConfigurationBinding,
        target: &SavedSyncTarget,
    ) -> Result<()> {
        checked_id(&binding.id)?;
        let stored: Option<(u64, String)> = self
            .conn()?
            .query_row(
                "SELECT revision,target_json FROM sync_target_configurations WHERE id=?1",
                [&binding.id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()?;
        let (revision, json) =
            stored.context("Saved target setup was removed; create a new preview")?;
        ensure!(
            revision == binding.revision
                && serde_json::from_str::<serde_json::Value>(&json)?
                    == serde_json::to_value(target)?,
            "Saved target setup changed; create a new preview"
        );
        Ok(())
    }
    pub(crate) fn configured_rekordbox_targets(&self) -> Result<Vec<RekordboxSyncTarget>> {
        Ok(self
            .list_sync_target_configurations()?
            .into_iter()
            .filter_map(|saved| match saved.target {
                SavedSyncTarget::Rekordbox { target } => Some(target),
                _ => None,
            })
            .collect())
    }
}
