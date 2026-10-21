use crate::{
    commands::{AppError, work},
    host::Host,
};
use anyhow::{Context, ensure};
use base64::Engine as _;
use cuerithm_autotag::ArtworkIdentity;
use cuerithm_library::*;
use cuerithm_media::CancellationToken;
use std::{io::Read, sync::Arc};
use tauri::{Emitter, State};
use tauri_plugin_dialog::DialogExt;

type Managed<'a> = State<'a, Arc<Host>>;
type Ipc<T> = Result<T, AppError>;

fn service(host: &Host) -> MetadataEditService {
    MetadataEditService::new(host.library.clone(), host.runtime.clone())
}

pub(crate) async fn run<T: Send + 'static>(
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

fn drafts_changed(app: &tauri::AppHandle) {
    let _ = app.emit("metadata-edit-change", ());
}
pub(crate) fn library_changed(app: &tauri::AppHandle) {
    let _ = app.emit("library-change", serde_json::json!({"track_ids":[]}));
    drafts_changed(app);
}

#[tauri::command]
pub async fn metadata_editor_snapshot(
    host: Managed<'_>,
    operation_id: String,
    track_ids: Vec<String>,
) -> Ipc<MetadataEditorSnapshot> {
    run(host.inner().clone(), operation_id, move |host, cancel| {
        service(host).snapshot_cancellable(&track_ids, cancel)
    })
    .await
}
#[tauri::command]
pub async fn create_metadata_draft(
    host: Managed<'_>,
    app: tauri::AppHandle,
    operation_id: String,
    track_ids: Vec<String>,
) -> Ipc<MetadataDraft> {
    let result = run(host.inner().clone(), operation_id, move |host, cancel| {
        service(host).create_draft_cancellable(&track_ids, cancel)
    })
    .await;
    drafts_changed(&app);
    result
}
#[tauri::command]
pub async fn metadata_draft(host: Managed<'_>, id: String) -> Ipc<MetadataDraft> {
    let host = host.inner().clone();
    work(move || service(&host).draft(&id)).await
}
#[tauri::command]
pub async fn metadata_drafts(host: Managed<'_>) -> Ipc<Vec<MetadataDraft>> {
    let host = host.inner().clone();
    work(move || service(&host).drafts()).await
}
#[tauri::command]
pub async fn update_metadata_draft(
    host: Managed<'_>,
    app: tauri::AppHandle,
    id: String,
    expected_revision: u64,
    patch: MetadataEditorPatch,
) -> Ipc<MetadataDraft> {
    let host = host.inner().clone();
    let result = work(move || service(&host).update_draft(&id, expected_revision, patch)).await;
    drafts_changed(&app);
    result
}
#[tauri::command]
pub async fn preview_metadata_edits(
    host: Managed<'_>,
    operation_id: String,
    id: String,
    revision: u64,
) -> Ipc<MetadataEditPreview> {
    run(host.inner().clone(), operation_id, move |host, cancel| {
        service(host).preview(&id, revision, cancel)
    })
    .await
}
#[tauri::command]
pub async fn apply_metadata_edits(
    host: Managed<'_>,
    app: tauri::AppHandle,
    operation_id: String,
    preview_id: String,
) -> Ipc<MetadataEditBatch> {
    let result = run(host.inner().clone(), operation_id, move |host, cancel| {
        let result = service(host).apply(&preview_id, cancel);
        // Incoming decisions are queued atomically by the service. Wake them
        // without performing a second retry or changing their captured owner.
        host.preparation.wake();
        result
    })
    .await;
    library_changed(&app);
    result
}
#[tauri::command]
pub async fn recover_metadata_edits(
    host: Managed<'_>,
    app: tauri::AppHandle,
    operation_id: String,
    batch_id: String,
) -> Ipc<MetadataEditBatch> {
    let result = run(host.inner().clone(), operation_id, move |host, cancel| {
        let result = service(host).recover(&batch_id, cancel);
        host.preparation.wake();
        result
    })
    .await;
    library_changed(&app);
    result
}
#[tauri::command]
pub async fn undo_metadata_edits(
    host: Managed<'_>,
    app: tauri::AppHandle,
    operation_id: String,
    batch_id: String,
) -> Ipc<MetadataEditBatch> {
    let result = run(host.inner().clone(), operation_id, move |host, cancel| {
        service(host).undo(&batch_id, cancel)
    })
    .await;
    library_changed(&app);
    result
}
#[tauri::command]
pub async fn discard_metadata_draft(
    host: Managed<'_>,
    app: tauri::AppHandle,
    id: String,
    revision: u64,
) -> Ipc<()> {
    let host = host.inner().clone();
    let result = work(move || service(&host).discard_draft(&id, revision)).await;
    drafts_changed(&app);
    result
}
#[tauri::command]
pub async fn metadata_edit_history(host: Managed<'_>) -> Ipc<Vec<MetadataEditBatch>> {
    let host = host.inner().clone();
    work(move || service(&host).history()).await
}
#[tauri::command]
pub async fn metadata_edit_history_page(
    host: Managed<'_>,
    offset: usize,
    limit: usize,
) -> Ipc<MetadataEditHistoryPage> {
    let host = host.inner().clone();
    work(move || service(&host).history_page(offset, limit)).await
}
#[tauri::command]
pub async fn metadata_edit_batch(host: Managed<'_>, id: String) -> Ipc<MetadataEditBatch> {
    let host = host.inner().clone();
    work(move || service(&host).batch(&id)).await
}

#[tauri::command]
pub async fn choose_metadata_artwork(
    host: Managed<'_>,
    app: tauri::AppHandle,
) -> Ipc<Option<MetadataArtworkAsset>> {
    let host = host.inner().clone();
    work(move || {
        let Some(file) = app
            .dialog()
            .file()
            .add_filter("Artwork", &["jpg", "jpeg", "png", "webp"])
            .blocking_pick_file()
        else {
            return Ok(None);
        };
        let path = file.into_path()?;
        // This path comes only from the native picker. Bound the read before
        // passing bytes to the codec-aware image validator/private asset store.
        let mut bytes = Vec::new();
        std::fs::File::open(path)?
            .take(8 * 1024 * 1024 + 1)
            .read_to_end(&mut bytes)?;
        ensure!(bytes.len() <= 8 * 1024 * 1024, "Artwork exceeds 8 MiB");
        service(&host).import_artwork(bytes).map(Some)
    })
    .await
}
#[tauri::command]
pub async fn extract_metadata_artwork(
    host: Managed<'_>,
    operation_id: String,
    track_id: String,
    source_sha256: String,
    identity: ArtworkIdentity,
) -> Ipc<MetadataArtworkAsset> {
    run(host.inner().clone(), operation_id, move |host, cancel| {
        service(host).extract_artwork_cancellable(&track_id, &source_sha256, identity, cancel)
    })
    .await
}
#[tauri::command]
pub async fn metadata_artwork_asset(host: Managed<'_>, asset_id: String) -> Ipc<String> {
    let host = host.inner().clone();
    work(move || {
        let bytes = service(&host).artwork_bytes(&asset_id)?;
        ensure!(bytes.len() <= 8 * 1024 * 1024, "Artwork exceeds 8 MiB");
        let mime = cuerithm_media::artwork_mime(&bytes).context("Unsupported artwork image")?;
        Ok(format!(
            "data:{mime};base64,{}",
            base64::engine::general_purpose::STANDARD.encode(bytes)
        ))
    })
    .await
}
