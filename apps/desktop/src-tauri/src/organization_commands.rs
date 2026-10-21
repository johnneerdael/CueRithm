use crate::{
    commands::{AppError, work},
    host::Host,
};
use cuerithm_library::*;
use cuerithm_media::CancellationToken;
use std::sync::Arc;
use tauri::{Emitter, State};
use tauri_plugin_dialog::DialogExt;

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

fn changed(app: &tauri::AppHandle) {
    let _ = app.emit("library-change", serde_json::json!({"track_ids":[]}));
    let _ = app.emit("organization-change", ());
}

#[tauri::command]
pub async fn organization_roots(host: Managed<'_>) -> Ipc<Vec<OrganizationRoot>> {
    let lib = host.library.clone();
    work(move || lib.organization_roots()).await
}

#[tauri::command]
pub async fn choose_organization_root(
    host: Managed<'_>,
    app: tauri::AppHandle,
) -> Ipc<Option<OrganizationRoot>> {
    let lib = host.library.clone();
    work(move || {
        // Keep the native path entirely in Rust, including non-Unicode filenames.
        let Some(folder) = app.dialog().file().blocking_pick_folder() else {
            return Ok(None);
        };
        lib.register_organization_root(folder.into_path()?)
            .map(Some)
    })
    .await
}

#[tauri::command]
pub async fn organization_example(
    host: Managed<'_>,
    settings: OrganizationSettings,
) -> Ipc<OrganizationExample> {
    let lib = host.library.clone();
    work(move || lib.organization_example(settings)).await
}

#[tauri::command]
pub async fn preview_organization(
    host: Managed<'_>,
    operation_id: String,
    request: OrganizationPreviewRequest,
) -> Ipc<OrganizationPreview> {
    run(host.inner().clone(), operation_id, move |host, cancel| {
        host.library.preview_organization(request, cancel)
    })
    .await
}

#[tauri::command]
pub async fn organization_preview_page(
    host: Managed<'_>,
    id: String,
    offset: usize,
    limit: usize,
) -> Ipc<OrganizationPage> {
    let lib = host.library.clone();
    work(move || lib.organization_preview_page(&id, offset, limit)).await
}

#[tauri::command]
pub async fn discard_organization_preview(host: Managed<'_>, id: String) -> Ipc<()> {
    let lib = host.library.clone();
    work(move || lib.discard_organization_preview(&id)).await
}

#[tauri::command]
pub async fn apply_organization(
    host: Managed<'_>,
    app: tauri::AppHandle,
    operation_id: String,
    preview_id: String,
    revision: String,
) -> Ipc<OrganizationBatch> {
    let result = run(host.inner().clone(), operation_id, move |host, cancel| {
        host.library
            .apply_organization_preview(&preview_id, &revision, cancel)
    })
    .await;
    // An error can follow partial publication; refresh durable history either way.
    changed(&app);
    result
}

#[tauri::command]
pub fn cancel_organization(host: Managed<'_>, operation_id: String) -> Ipc<bool> {
    host.maintenance.cancel(&operation_id).map_err(Into::into)
}

#[tauri::command]
pub async fn cancel_organization_batch(
    host: Managed<'_>,
    app: tauri::AppHandle,
    id: String,
) -> Ipc<()> {
    let lib = host.library.clone();
    let result = work(move || lib.cancel_organization_batch(&id)).await;
    changed(&app);
    result
}

#[tauri::command]
pub async fn organization_history(host: Managed<'_>) -> Ipc<Vec<OrganizationBatch>> {
    let lib = host.library.clone();
    work(move || lib.organization_batches()).await
}

#[tauri::command]
pub async fn organization_batch_page(
    host: Managed<'_>,
    id: String,
    offset: usize,
    limit: usize,
) -> Ipc<OrganizationPage> {
    let lib = host.library.clone();
    work(move || lib.organization_batch_page(&id, offset, limit)).await
}

#[tauri::command]
pub async fn recover_organization(
    host: Managed<'_>,
    app: tauri::AppHandle,
    operation_id: String,
    id: String,
) -> Ipc<OrganizationBatch> {
    let result = run(host.inner().clone(), operation_id, move |host, cancel| {
        host.library.recover_organization_batch(&id, cancel)
    })
    .await;
    changed(&app);
    result
}

#[tauri::command]
pub async fn undo_organization(
    host: Managed<'_>,
    app: tauri::AppHandle,
    operation_id: String,
    id: String,
) -> Ipc<OrganizationBatch> {
    let result = run(host.inner().clone(), operation_id, move |host, cancel| {
        host.library.undo_organization_batch(&id, cancel)
    })
    .await;
    changed(&app);
    result
}

#[tauri::command]
pub async fn organization_alternates(host: Managed<'_>) -> Ipc<Vec<OrganizationAlternate>> {
    let lib = host.library.clone();
    work(move || lib.organization_alternates()).await
}

#[tauri::command]
pub async fn resolve_organization_alternate(
    host: Managed<'_>,
    app: tauri::AppHandle,
    operation_id: String,
    id: String,
    resolution: OrganizationAlternateResolution,
) -> Ipc<()> {
    let result = run(host.inner().clone(), operation_id, move |host, cancel| {
        host.library
            .resolve_organization_alternate_cancellable(&id, resolution, cancel)
            .map(|_| ())
    })
    .await;
    changed(&app);
    result
}
