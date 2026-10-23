//! Maintenance -> Missing & broken: remove records of unavailable tracks and
//! find moved files. Each is preview, apply, history and undo.
use crate::{
    commands::{AppError, work},
    host::Host,
};
use cuerithm_library::*;
use cuerithm_media::CancellationToken;
use std::{path::PathBuf, sync::Arc};
use tauri::{Emitter, State};
type Managed<'a> = State<'a, Arc<Host>>;
type Ipc<T> = Result<T, AppError>;

async fn run<T: Send + 'static>(
    host: Arc<Host>,
    operation_id: String,
    f: impl FnOnce(&Host, &CancellationToken) -> anyhow::Result<T> + Send + 'static,
) -> Ipc<T> {
    let lease = host.maintenance.begin(&operation_id)?;
    work(move || {
        lease.token.check()?;
        let result = f(&host, &lease.token);
        drop(lease);
        result
    })
    .await
}
/// The library and the other Maintenance tools learn that records changed.
/// `readable` names the tracks that exist afterwards and can be re-read.
fn repaired(app: &tauri::AppHandle, entry: &MissingRepairEntry, readable: bool) {
    let track_ids: Vec<&str> = if readable {
        entry.tracks.iter().map(|t| t.id.as_str()).collect()
    } else {
        vec![]
    };
    let _ = app.emit(
        "library-change",
        serde_json::json!({"track_ids":track_ids,"playlist_ids":entry.playlist_ids}),
    );
    let _ = app.emit(
        "maintenance-change",
        serde_json::json!({"review_id":null,"stamp":entry.stamp}),
    );
}

#[tauri::command]
pub async fn maintenance_preview_missing_removal(
    host: Managed<'_>,
    operation_id: String,
    scan_id: String,
    selected: Vec<String>,
) -> Ipc<MissingPreview> {
    run(host.inner().clone(), operation_id, move |host, cancel| {
        host.library
            .preview_missing_removal(&scan_id, &selected, cancel)
    })
    .await
}
#[tauri::command]
pub async fn maintenance_apply_missing_removal(
    host: Managed<'_>,
    app: tauri::AppHandle,
    operation_id: String,
    id: String,
) -> Ipc<MissingRepairEntry> {
    let entry = run(host.inner().clone(), operation_id, move |host, cancel| {
        host.library.apply_missing_removal(&id, cancel)
    })
    .await?;
    repaired(&app, &entry, false);
    Ok(entry)
}
#[tauri::command]
pub async fn maintenance_preview_missing_relink(
    host: Managed<'_>,
    app: tauri::AppHandle,
    operation_id: String,
    scan_id: String,
    selected: Vec<String>,
    locations: Vec<PathBuf>,
) -> Ipc<MissingPreview> {
    let progress_id = operation_id.clone();
    run(host.inner().clone(), operation_id, move |host, cancel| {
        let progress = move |value| {
            let _ = app.emit(
                "maintenance-progress",
                serde_json::json!({"operation_id":progress_id,"kind":"broken","progress":value}),
            );
        };
        host.library
            .preview_missing_relink(&scan_id, &selected, &locations, cancel, progress)
    })
    .await
}
#[tauri::command]
pub async fn maintenance_apply_missing_relink(
    host: Managed<'_>,
    app: tauri::AppHandle,
    operation_id: String,
    id: String,
) -> Ipc<MissingRepairEntry> {
    let entry = run(host.inner().clone(), operation_id, move |host, cancel| {
        host.library.apply_missing_relink(&id, cancel)
    })
    .await?;
    repaired(&app, &entry, true);
    Ok(entry)
}
#[tauri::command]
pub async fn maintenance_missing_repair_history(
    host: Managed<'_>,
) -> Ipc<Vec<MissingRepairHistory>> {
    let lib = host.library.clone();
    work(move || lib.missing_repair_history()).await
}
#[tauri::command]
pub async fn maintenance_undo_missing_repair(
    host: Managed<'_>,
    app: tauri::AppHandle,
    operation_id: String,
    id: String,
) -> Ipc<MissingRepairEntry> {
    let entry = run(host.inner().clone(), operation_id, move |host, cancel| {
        host.library.undo_missing_repair(&id, cancel)
    })
    .await?;
    // Undo puts removed tracks back and old paths back, so every track can be re-read.
    repaired(&app, &entry, true);
    Ok(entry)
}
#[tauri::command]
pub async fn maintenance_discard_missing_repair_undo(
    host: Managed<'_>,
    operation_id: String,
    id: String,
) -> Ipc<MissingRepairHistory> {
    run(host.inner().clone(), operation_id, move |host, _| {
        host.library.discard_missing_repair_undo(&id)
    })
    .await
}
