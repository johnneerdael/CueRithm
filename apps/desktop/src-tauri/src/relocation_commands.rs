use crate::{
    commands::{AppError, work},
    host::Host,
    portable_commands::{self, progress},
};
use anyhow::{Result, ensure};
use cuerithm_library::{RelocationChoice, RelocationPlan, RelocationRequest, RelocationResult};
use serde::Deserialize;
use std::{
    path::PathBuf,
    sync::{Arc, atomic::Ordering},
};
use tauri::{Emitter, State};
type Managed<'a> = State<'a, Arc<Host>>;
type Ipc<T> = Result<T, AppError>;
#[derive(Deserialize)]
pub struct BackupChoice {
    enabled: bool,
    destination: Option<PathBuf>,
}
#[tauri::command]
pub async fn relocation_plan(
    host: Managed<'_>,
    app: tauri::AppHandle,
    task_id: String,
    request: RelocationRequest,
) -> Ipc<RelocationPlan> {
    let host = host.inner().clone();
    let lease = host.maintenance.begin(&task_id)?;
    work(move || {
        host.library
            .plan_relocation(request, &host.runtime, &lease.token, |p| {
                progress(&app, &task_id, &p.phase, p.completed, p.total)
            })
    })
    .await
}
#[tauri::command]
pub async fn relocation_continue(
    host: Managed<'_>,
    app: tauri::AppHandle,
    task_id: String,
    plan_id: String,
) -> Ipc<RelocationPlan> {
    let host = host.inner().clone();
    let lease = host.maintenance.begin(&task_id)?;
    work(move || {
        host.library
            .continue_relocation(&plan_id, &host.runtime, &lease.token, |p| {
                progress(&app, &task_id, &p.phase, p.completed, p.total)
            })
    })
    .await
}
fn unload(host: &Host) -> Result<()> {
    host.generation.fetch_add(1, Ordering::SeqCst);
    host.player
        .lock()
        .map_err(|_| anyhow::anyhow!("Player interrupted"))?
        .unload()?;
    *host
        .loaded
        .lock()
        .map_err(|_| anyhow::anyhow!("Playback cache interrupted"))? = None;
    Ok(())
}
fn changed(app: &tauri::AppHandle) {
    let _ = app.emit("library-change", serde_json::json!({"track_ids":[]}));
}
#[tauri::command]
pub async fn relocation_apply(
    host: Managed<'_>,
    app: tauri::AppHandle,
    task_id: String,
    plan_id: String,
    choices: Vec<RelocationChoice>,
    backup: BackupChoice,
) -> Ipc<RelocationResult> {
    let host = host.inner().clone();
    let lease = host.maintenance.begin(&task_id)?;
    work(move || {
        let _gate = host
            .portable
            .gate
            .lock()
            .map_err(|_| anyhow::anyhow!("Library operation interrupted"))?;
        let _operation = host.library.exclusive_operation()?;
        lease.token.check()?;
        let before = host.library.revision()?;
        let preview = host.library.relocation_plan(&plan_id)?;
        ensure!(preview.revision == before, "Library changed; preview again");
        let saved = if backup.enabled {
            let folder = backup
                .destination
                .as_ref()
                .ok_or_else(|| anyhow::anyhow!("Choose a preflight backup folder"))?;
            ensure!(folder.is_dir(), "Backup destination is unavailable");
            let path = folder.join(format!(
                "CueRithm-before-relocation-{}.cuerithm",
                uuid::Uuid::new_v4()
            ));
            progress(&app, &task_id, "Saving preflight backup", 0, None);
            portable_commands::export(&host, &path, true, &lease.token)?;
            Some(path)
        } else {
            None
        };
        ensure!(
            host.library.revision()? == before,
            "Library changed during backup; preview again"
        );
        lease.token.check()?;
        unload(&host)?;
        progress(
            &app,
            &task_id,
            "Reconnecting reviewed tracks",
            0,
            Some(choices.len()),
        );
        let r = host.library.apply_relocation(
            &plan_id,
            &choices,
            &host.runtime,
            &lease.token,
            saved,
        )?;
        changed(&app);
        Ok(r)
    })
    .await
}
#[tauri::command]
pub async fn relocation_undo(
    host: Managed<'_>,
    app: tauri::AppHandle,
    task_id: String,
    operation_id: String,
) -> Ipc<RelocationResult> {
    let host = host.inner().clone();
    let lease = host.maintenance.begin(&task_id)?;
    work(move || {
        let _gate = host
            .portable
            .gate
            .lock()
            .map_err(|_| anyhow::anyhow!("Library operation interrupted"))?;
        let _operation = host.library.exclusive_operation()?;
        unload(&host)?;
        let r = host.library.undo_relocation(&operation_id, &lease.token)?;
        changed(&app);
        Ok(r)
    })
    .await
}
#[tauri::command]
pub async fn relocation_history(
    host: Managed<'_>,
) -> Ipc<Vec<cuerithm_library::RelocationHistory>> {
    let host = host.inner().clone();
    work(move || host.library.relocation_history()).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn frontend_single_file_selection_does_not_become_a_root_or_enqueue_request() {
        let request: RelocationRequest = serde_json::from_value(json!({
            "mode":"selection","track_ids":["track-a"],"search_root":null,
            "replacement_file":"/music/replacement.mp3","source_root":null,
            "destination_root":null,"extension":null,"all_tracks":false,
            "include_unavailable":false
        }))
        .unwrap();
        assert_eq!(request.mode, "selection");
        assert_eq!(request.track_ids, vec!["track-a"]);
        assert_eq!(
            request.replacement_file.as_deref(),
            Some(Path::new("/music/replacement.mp3"))
        );
        assert!(!request.all_tracks);
        assert!(!request.include_unavailable);
    }
    #[test]
    fn frontend_collision_decision_and_backup_opt_out_deserialize_without_defaults() {
        let choices: Vec<RelocationChoice> = serde_json::from_value(json!([
            {"track_id":"track-a","candidate_id":"candidate-a","survivor":"existing"}
        ]))
        .unwrap();
        assert_eq!(choices[0].survivor, "existing");
        let backup: BackupChoice =
            serde_json::from_value(json!({"enabled":false,"destination":null})).unwrap();
        assert!(!backup.enabled);
        assert!(backup.destination.is_none());
        assert!(serde_json::from_value::<BackupChoice>(json!({"destination":null})).is_err());
    }
    use std::path::Path;
}

#[tauri::command]
pub async fn relocation_roots(
    host: Managed<'_>,
) -> Ipc<Vec<cuerithm_library::RelocationSourceRoot>> {
    let host = host.inner().clone();
    work(move || host.library.relocation_roots()).await
}
