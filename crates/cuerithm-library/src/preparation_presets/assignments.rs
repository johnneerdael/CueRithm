use super::*;
use crate::{
    Library, PreparationSettings,
    model::{bytes_path, now, path_bytes},
    organization::{Authority, NativeParent},
};
use anyhow::{Context, Result, ensure};
use rusqlite::{OptionalExtension, params};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
#[derive(Clone)]
pub(super) struct FolderState {
    pub id: String,
    pub path: PathBuf,
    pub scope: PathBuf,
    pub current: Option<Authority>,
    pub saved: Option<Authority>,
    pub active: bool,
    pub error: Option<String>,
    pub revision: Option<String>,
    pub choice: PreparationFolderChoice,
}
struct PreparedChange {
    folder: FolderState,
    expected: Option<String>,
    choice: PreparationFolderChoice,
    preset_revision: Option<String>,
}
pub(crate) struct PreparedPreparationAssignments {
    changes: Vec<PreparedChange>,
}
fn issue(code: &str, message: impl Into<String>) -> PreparationPresetIssue {
    PreparationPresetIssue {
        code: code.into(),
        field: "folder".into(),
        message: message.into(),
    }
}
impl Library {
    pub(super) fn preparation_folder_states(
        &self,
        settings: &PreparationSettings,
    ) -> Result<Vec<FolderState>> {
        let mut result = Vec::new();
        let mut seen = std::collections::BTreeSet::new();
        for requested in &settings.watch_folders {
            let path = std::path::absolute(requested)?;
            let bytes = path_bytes(&path);
            if !seen.insert(bytes.clone()) {
                continue;
            }
            let id = format!("folder-{:x}", Sha256::digest(&bytes));
            let (current, error) = match NativeParent::capture(&path) {
                Ok((_, authority)) => (Some(authority), None),
                Err(error) => (
                    None,
                    Some(format!(
                        "Folder is unavailable or contains a link; choose its native folder again: {error:#}"
                    )),
                ),
            };
            let encoded = current.as_ref().map(serde_json::to_string).transpose()?;
            self.conn()?.execute("INSERT OR IGNORE INTO preparation_preset_folders(id,path,authority_json,created) VALUES(?1,?2,?3,?4)",params![id,bytes,encoded,now()])?;
            let (saved,revision,choice):(Option<String>,Option<String>,Option<String>)=self.conn()?.query_row("SELECT f.authority_json,a.revision,a.choice_json FROM preparation_preset_folders f LEFT JOIN preparation_preset_assignments a ON a.folder_id=f.id WHERE f.id=?1 AND f.path=?2",params![id,bytes],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?)))?;
            result.push(FolderState {
                id,
                path: path.clone(),
                scope: path.canonicalize().unwrap_or(path),
                current,
                saved: saved.map(|json| serde_json::from_str(&json)).transpose()?,
                active: true,
                error,
                revision,
                choice: choice
                    .map(|json| serde_json::from_str(&json))
                    .transpose()?
                    .unwrap_or(PreparationFolderChoice::Inherit),
            });
        }
        type InactiveAssignment = (String, Vec<u8>, Option<String>, String, String);
        let inactive: Vec<InactiveAssignment> = {
            let conn = self.conn()?;
            let mut q=conn.prepare("SELECT f.id,f.path,f.authority_json,a.revision,a.choice_json FROM preparation_preset_folders f JOIN preparation_preset_assignments a ON a.folder_id=f.id ORDER BY f.id LIMIT 257")?;
            q.query_map([], |r| {
                Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?))
            })?
            .collect::<rusqlite::Result<_>>()?
        };
        ensure!(
            inactive.len() <= 256,
            "Too many retained folder assignments; remove inactive assignments"
        );
        for (id, bytes, saved, revision, choice) in inactive {
            if result.iter().any(|folder| folder.id == id) {
                continue;
            }
            let path = bytes_path(&bytes)?;
            result.push(FolderState{id,path:path.clone(),scope:path,current:None,saved:saved.map(|json|serde_json::from_str(&json)).transpose()?,active:false,error:Some("This folder is no longer watched; clear its assignment or add the watch folder again".into()),revision:Some(revision),choice:serde_json::from_str(&choice)?});
        }
        Ok(result)
    }

    pub(super) fn preparation_routing_folders(
        &self,
        settings: &PreparationSettings,
        source: &Path,
    ) -> Result<Vec<FolderState>> {
        let watched = settings
            .watch_folders
            .iter()
            .map(|path| {
                let path = std::path::absolute(path)?;
                Ok(format!("folder-{:x}", Sha256::digest(path_bytes(&path))))
            })
            .collect::<Result<std::collections::BTreeSet<_>>>()?;
        type RoutingFolder = (
            String,
            Vec<u8>,
            Option<String>,
            String,
            String,
            Option<String>,
        );
        let rows: Vec<RoutingFolder> = {
            let conn = self.conn()?;
            let mut q=conn.prepare("SELECT f.id,f.path,f.authority_json,a.revision,a.choice_json,a.preset_id FROM preparation_preset_folders f JOIN preparation_preset_assignments a ON a.folder_id=f.id ORDER BY f.id LIMIT 257")?;
            q.query_map([], |r| {
                Ok((
                    r.get(0)?,
                    r.get(1)?,
                    r.get(2)?,
                    r.get(3)?,
                    r.get(4)?,
                    r.get(5)?,
                ))
            })?
            .collect::<rusqlite::Result<_>>()?
        };
        ensure!(rows.len() <= 256, "Too many folder assignments");
        let mut result = Vec::new();
        for (id, path, saved, revision, choice, preset_id) in rows {
            if !watched.contains(&id) {
                continue;
            }
            let path = bytes_path(&path)?;
            let scope = path.canonicalize().unwrap_or_else(|_| path.clone());
            if !source.starts_with(&scope) {
                continue;
            }
            let choice: PreparationFolderChoice = serde_json::from_str(&choice)?;
            ensure!(
                match &choice {
                    PreparationFolderChoice::Preset { preset_id: id } =>
                        preset_id.as_ref() == Some(id),
                    _ => preset_id.is_none(),
                },
                "Stored preset assignment identity differs"
            );
            if matches!(choice, PreparationFolderChoice::Inherit) {
                continue;
            }
            let (current, error) = match NativeParent::capture(&path) {
                Ok((_, authority)) => (Some(authority), None),
                Err(error) => (
                    None,
                    Some(format!("Assigned native folder is unavailable: {error:#}")),
                ),
            };
            result.push(FolderState {
                id,
                path,
                scope,
                current,
                saved: saved.map(|json| serde_json::from_str(&json)).transpose()?,
                active: true,
                error,
                revision: Some(revision),
                choice,
            });
        }
        Ok(result)
    }
    pub(super) fn select_preparation_folder<'a>(
        folders: &'a [FolderState],
        path: &Path,
    ) -> Result<Option<&'a FolderState>> {
        let mut matching: Vec<_> = folders
            .iter()
            .filter(|folder| {
                folder.active
                    && !matches!(folder.choice, PreparationFolderChoice::Inherit)
                    && path.starts_with(&folder.scope)
            })
            .collect();
        matching.sort_by(|a, b| {
            b.scope
                .components()
                .count()
                .cmp(&a.scope.components().count())
                .then_with(|| a.id.cmp(&b.id))
        });
        if let Some(first) = matching.first() {
            ensure!(
                matching
                    .get(1)
                    .is_none_or(|other| other.scope.components().count()
                        != first.scope.components().count()),
                "Multiple assignments name the same native folder; clear the duplicate"
            );
        }
        Ok(matching.into_iter().next())
    }
    pub(super) fn verify_preparation_folder(folder: &FolderState) -> Result<&Authority> {
        ensure!(folder.active, "Assigned watch folder is inactive");
        let saved = folder
            .saved
            .as_ref()
            .context("Reconnect this folder assignment before preparing files")?;
        let current = folder.current.as_ref().with_context(|| {
            folder
                .error
                .clone()
                .unwrap_or_else(|| "Assigned folder is unavailable".into())
        })?;
        ensure!(
            saved.ancestors == current.ancestors,
            "Assigned folder identity changed; save its assignment to explicitly reconnect it"
        );
        NativeParent::reopen(saved)?;
        Ok(saved)
    }
    pub fn preparation_preset_folders_for_settings(
        &self,
        settings: &PreparationSettings,
    ) -> Result<Vec<PreparationFolderAssignment>> {
        let folders = self.preparation_folder_states(settings)?;
        let mut result = Vec::new();
        for folder in &folders {
            let mut entry = PreparationFolderAssignment {
                folder_id: folder.id.clone(),
                display_path: folder.path.to_string_lossy().into_owned(),
                revision: folder.revision.clone(),
                choice: folder.choice.clone(),
                effective_preset_id: None,
                effective_preset_revision: None,
                matched_folder_id: None,
                status: "ready".into(),
                issues: vec![],
            };
            if !folder.active {
                entry.status = "inactive".into();
                entry.issues.push(issue(
                    "inactive_folder",
                    folder.error.clone().unwrap_or_default(),
                ));
                result.push(entry);
                continue;
            }
            let effective = (|| -> Result<()> {
                if folder.current.is_none() {
                    anyhow::bail!(
                        "{}",
                        folder.error.as_deref().unwrap_or("Folder is unavailable")
                    );
                }
                if let Some(selected) = Self::select_preparation_folder(&folders, &folder.scope)? {
                    Self::verify_preparation_folder(selected)?;
                    entry.matched_folder_id = Some(selected.id.clone());
                    if let PreparationFolderChoice::Preset { preset_id } = &selected.choice {
                        let preset = self.preparation_preset(preset_id)?;
                        entry.effective_preset_id = Some(preset.id);
                        entry.effective_preset_revision = Some(preset.revision);
                        entry.status = preset.status;
                        entry.issues = preset.issues;
                    }
                }
                Ok(())
            })();
            if let Err(error) = effective {
                entry.status = "needs_connection".into();
                entry
                    .issues
                    .push(issue("folder_unavailable", format!("{error:#}")));
            }
            if folder
                .saved
                .as_ref()
                .zip(folder.current.as_ref())
                .is_some_and(|(saved, current)| saved.ancestors != current.ancestors)
            {
                entry.status = "needs_connection".into();
                entry.issues.push(issue("folder_identity_changed","This watch folder names a different native directory. Explicitly reconnect its current choice before assigning it."));
            }
            result.push(entry);
        }
        Ok(result)
    }
    pub(crate) fn prepare_preparation_assignment_changes(
        &self,
        settings: &PreparationSettings,
        changes: &[PreparationFolderChange],
    ) -> Result<PreparedPreparationAssignments> {
        ensure!(changes.len() <= 256, "Too many folder assignment changes");
        let folders = self.preparation_folder_states(settings)?;
        let mut seen = std::collections::BTreeSet::new();
        let mut prepared = Vec::new();
        for change in changes {
            ensure!(
                seen.insert(&change.folder_id),
                "Duplicate folder assignment change"
            );
            let folder = folders
                .iter()
                .find(|folder| folder.id == change.folder_id)
                .context("Reload watched folders before assigning a preset")?
                .clone();
            ensure!(
                folder.revision == change.expected_revision,
                "Folder assignment changed; reload before saving"
            );
            if !matches!(change.choice, PreparationFolderChoice::Inherit) {
                ensure!(
                    folder.active && folder.current.is_some(),
                    "Choose an available native watch folder before assigning it"
                );
            }
            let preset_revision =
                if let PreparationFolderChoice::Preset { preset_id } = &change.choice {
                    let preset = self.preparation_preset(preset_id)?;
                    ensure!(
                        preset.status == "ready",
                        "Reconnect the preset before assigning it to a watch folder"
                    );
                    Some(preset.revision)
                } else {
                    None
                };
            prepared.push(PreparedChange {
                folder,
                expected: change.expected_revision.clone(),
                choice: change.choice.clone(),
                preset_revision,
            });
        }
        for (a_index, a) in folders.iter().enumerate() {
            let a_choice = changes
                .iter()
                .find(|change| change.folder_id == a.id)
                .map(|change| &change.choice)
                .unwrap_or(&a.choice);
            if !a.active || matches!(a_choice, PreparationFolderChoice::Inherit) {
                continue;
            }
            for b in &folders[a_index + 1..] {
                let b_choice = changes
                    .iter()
                    .find(|change| change.folder_id == b.id)
                    .map(|change| &change.choice)
                    .unwrap_or(&b.choice);
                if b.active
                    && !matches!(b_choice, PreparationFolderChoice::Inherit)
                    && a.current
                        .as_ref()
                        .zip(b.current.as_ref())
                        .is_some_and(|(a, b)| a.ancestors == b.ancestors)
                {
                    anyhow::bail!(
                        "Multiple watch-folder aliases refer to the same native folder; retain one assignment"
                    );
                }
            }
        }
        Ok(PreparedPreparationAssignments { changes: prepared })
    }
    pub(crate) fn apply_preparation_assignment_changes_tx(
        tx: &rusqlite::Transaction<'_>,
        prepared: &PreparedPreparationAssignments,
    ) -> Result<()> {
        for change in &prepared.changes {
            let current: Option<String> = tx
                .query_row(
                    "SELECT revision FROM preparation_preset_assignments WHERE folder_id=?1",
                    [&change.folder.id],
                    |r| r.get(0),
                )
                .optional()?;
            ensure!(
                current == change.expected,
                "Folder assignment changed; reload before saving"
            );
            let saved = change
                .folder
                .saved
                .as_ref()
                .map(serde_json::to_string)
                .transpose()?;
            let actual: Option<String> = tx.query_row(
                "SELECT authority_json FROM preparation_preset_folders WHERE id=?1 AND path=?2",
                params![change.folder.id, path_bytes(&change.folder.path)],
                |row| row.get(0),
            )?;
            ensure!(
                actual == saved,
                "Folder registration changed; reload before reconnecting"
            );
            if matches!(change.choice, PreparationFolderChoice::Inherit) {
                if let Some(authority) = &change.folder.current {
                    ensure!(tx.execute("UPDATE preparation_preset_folders SET authority_json=?1 WHERE id=?2 AND path=?3 AND authority_json IS ?4",params![serde_json::to_string(authority)?,change.folder.id,path_bytes(&change.folder.path),saved])?==1,"Folder registration changed");
                }
                tx.execute(
                    "DELETE FROM preparation_preset_assignments WHERE folder_id=?1",
                    [&change.folder.id],
                )?;
                continue;
            }
            let preset_id = if let PreparationFolderChoice::Preset { preset_id } = &change.choice {
                let revision:String=tx.query_row("SELECT current_revision FROM preparation_presets WHERE id=?1 AND deleted=0",[preset_id],|r|r.get(0)).optional()?.context("Assigned preset was deleted")?;
                ensure!(
                    Some(&revision) == change.preset_revision.as_ref(),
                    "Assigned preset changed; reload before saving"
                );
                Some(preset_id)
            } else {
                None
            };
            if current.is_none() {
                ensure!(
                    tx.query_row(
                        "SELECT COUNT(*) FROM preparation_preset_assignments",
                        [],
                        |row| row.get::<_, u64>(0)
                    )? < 256,
                    "Keep at most 256 folder assignments; clear inactive assignments first"
                );
            }
            let authority = change
                .folder
                .current
                .as_ref()
                .context("Missing native folder authority")?;
            let encoded = serde_json::to_string(authority)?;
            let choice = serde_json::to_string(&change.choice)?;
            ensure!(tx.execute("UPDATE preparation_preset_folders SET authority_json=?1 WHERE id=?2 AND path=?3",params![encoded,change.folder.id,path_bytes(&change.folder.path)])?==1,"Watch folder registration changed");
            let revision = format!(
                "{:x}",
                Sha256::digest(serde_json::to_vec(&(
                    &change.folder.id,
                    &change.choice,
                    authority
                ))?)
            );
            tx.execute("INSERT INTO preparation_preset_assignments(folder_id,revision,choice_json,preset_id,updated) VALUES(?1,?2,?3,?4,?5) ON CONFLICT(folder_id) DO UPDATE SET revision=excluded.revision,choice_json=excluded.choice_json,preset_id=excluded.preset_id,updated=excluded.updated",params![change.folder.id,revision,choice,preset_id,now()])?;
        }
        Ok(())
    }
}
