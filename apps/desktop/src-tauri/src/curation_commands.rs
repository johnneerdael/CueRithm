use crate::{
    commands::{AppError, work},
    host::Host,
    metadata_commands,
};
use cuerithm_library::*;
use std::sync::Arc;
use tauri::State;

type Managed<'a> = State<'a, Arc<Host>>;
type Ipc<T> = Result<T, AppError>;

#[tauri::command]
pub async fn curation_catalog(host: Managed<'_>) -> Ipc<CurationCatalog> {
    let service = host.curation.clone();
    work(move || service.catalog()).await
}
#[tauri::command]
pub async fn edit_curation_taxonomy(
    host: Managed<'_>,
    request: CurationTaxonomyRequest,
) -> Ipc<CurationTaxonomyReceipt> {
    let service = host.curation.clone();
    work(move || service.edit_taxonomy(request)).await
}
#[tauri::command]
pub async fn curation_snapshot(host: Managed<'_>, track_ids: Vec<String>) -> Ipc<CurationSnapshot> {
    let service = host.curation.clone();
    work(move || service.snapshot(&track_ids)).await
}
#[tauri::command]
pub async fn update_curation(
    host: Managed<'_>,
    request: CurationUpdateRequest,
) -> Ipc<CurationReceipt> {
    let service = host.curation.clone();
    work(move || service.update(request)).await
}
#[tauri::command]
pub async fn reexport_curation(
    host: Managed<'_>,
    request: CurationReexportRequest,
) -> Ipc<CurationReceipt> {
    let service = host.curation.clone();
    work(move || service.reexport(request)).await
}
#[tauri::command]
pub async fn curation_pending_page(
    host: Managed<'_>,
    offset: usize,
    limit: usize,
) -> Ipc<CurationPendingPage> {
    let service = host.curation.clone();
    work(move || service.pending_page(offset, limit)).await
}
#[tauri::command]
pub async fn curation_history_page(
    host: Managed<'_>,
    offset: usize,
    limit: usize,
) -> Ipc<CurationHistoryPage> {
    let service = host.curation.clone();
    work(move || service.history_page(offset, limit)).await
}
#[tauri::command]
pub async fn curation_publication(host: Managed<'_>, id: String) -> Ipc<CurationPublication> {
    let service = host.curation.clone();
    work(move || service.publication(&id)).await
}
#[tauri::command]
pub async fn retry_curation(
    host: Managed<'_>,
    operation_id: String,
    track_id: String,
    expected_revision: u64,
) -> Ipc<CurationTrackState> {
    metadata_commands::run(host.inner().clone(), operation_id, move |host, cancel| {
        let result = host.curation.retry(&track_id, expected_revision, cancel);
        host.preparation.wake();
        result
    })
    .await
}
#[tauri::command]
pub async fn discard_curation(
    host: Managed<'_>,
    track_id: String,
    expected_revision: u64,
) -> Ipc<CurationTrackState> {
    let service = host.curation.clone();
    work(move || service.discard(&track_id, expected_revision)).await
}
#[tauri::command]
pub async fn undo_curation(
    host: Managed<'_>,
    operation_id: String,
    publication_id: String,
) -> Ipc<CurationPublication> {
    metadata_commands::run(host.inner().clone(), operation_id, move |host, cancel| {
        host.curation.undo(&publication_id, cancel)
    })
    .await
}
