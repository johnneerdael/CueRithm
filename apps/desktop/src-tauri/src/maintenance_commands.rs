use crate::{
    commands::{AppError, work},
    host::Host,
};
use cuerithm_library::*;
use cuerithm_media::CancellationToken;
use std::sync::Arc;
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

#[tauri::command]
pub fn maintenance_cancel(host: Managed<'_>, operation_id: String) -> Ipc<bool> {
    host.maintenance.cancel(&operation_id).map_err(Into::into)
}

#[tauri::command]
pub async fn maintenance_scan_duplicates(
    host: Managed<'_>,
    app: tauri::AppHandle,
    operation_id: String,
    request: DuplicateScanRequest,
) -> Ipc<DuplicateScanReport> {
    let progress_id = operation_id.clone();
    run(host.inner().clone(),operation_id,move |host,cancel| {
        let progress=move |value| { let _=app.emit("maintenance-progress",serde_json::json!({"operation_id":progress_id,"kind":"duplicates","progress":value})); };
        host.library.scan_duplicates_with_progress(request,&host.runtime,cancel,progress)
    }).await
}

#[tauri::command]
pub async fn maintenance_scan_unused(
    host: Managed<'_>,
    app: tauri::AppHandle,
    operation_id: String,
    request: UnusedScanRequest,
) -> Ipc<UnusedScanReport> {
    let progress_id = operation_id.clone();
    run(host.inner().clone(), operation_id, move |host, cancel| {
        let progress = move |value| {
            let _ = app.emit(
                "maintenance-progress",
                serde_json::json!({"operation_id":progress_id,"kind":"unused","progress":value}),
            );
        };
        host.library
            .scan_unused_files_with_progress(request, cancel, progress)
    })
    .await
}

#[tauri::command]
pub async fn maintenance_scan_broken(
    host: Managed<'_>,
    app: tauri::AppHandle,
    operation_id: String,
    request: BrokenScanRequest,
) -> Ipc<BrokenScanReport> {
    let progress_id = operation_id.clone();
    run(host.inner().clone(), operation_id, move |host, cancel| {
        let progress = move |value| {
            let _ = app.emit(
                "maintenance-progress",
                serde_json::json!({"operation_id":progress_id,"kind":"broken","progress":value}),
            );
        };
        host.library
            .scan_broken_tracks_with_progress(request, &host.runtime, cancel, progress)
    })
    .await
}

#[tauri::command]
pub async fn maintenance_duplicate_scan(
    host: Managed<'_>,
    operation_id: String,
    id: String,
) -> Ipc<DuplicateScanReport> {
    run(host.inner().clone(), operation_id, move |host, cancel| {
        host.library.duplicate_scan_cancellable(&id, cancel)
    })
    .await
}

#[tauri::command]
pub async fn maintenance_unused_scan(
    host: Managed<'_>,
    operation_id: String,
    id: String,
) -> Ipc<UnusedScanReport> {
    run(host.inner().clone(), operation_id, move |host, cancel| {
        host.library.unused_scan_cancellable(&id, cancel)
    })
    .await
}

#[tauri::command]
pub async fn maintenance_broken_scan(
    host: Managed<'_>,
    operation_id: String,
    id: String,
) -> Ipc<BrokenScanReport> {
    run(host.inner().clone(), operation_id, move |host, cancel| {
        host.library.broken_scan_cancellable(&id, cancel)
    })
    .await
}

#[tauri::command]
pub async fn maintenance_export_unused(
    host: Managed<'_>,
    operation_id: String,
    id: String,
) -> Ipc<String> {
    run(host.inner().clone(), operation_id, move |host, cancel| {
        host.library.unused_scan_export_cancellable(&id, cancel)
    })
    .await
}

#[tauri::command]
pub async fn maintenance_begin_review(
    host: Managed<'_>,
    operation_id: String,
    id: String,
) -> Ipc<DuplicateReview> {
    run(host.inner().clone(), operation_id, move |host, cancel| {
        host.library.begin_duplicate_review(&id, cancel)
    })
    .await
}

#[tauri::command]
pub async fn maintenance_get_review(
    host: Managed<'_>,
    operation_id: String,
    id: String,
) -> Ipc<DuplicateReview> {
    run(host.inner().clone(), operation_id, move |host, cancel| {
        host.library.duplicate_review(&id, cancel)
    })
    .await
}

#[tauri::command]
pub async fn maintenance_edit_review(
    host: Managed<'_>,
    app: tauri::AppHandle,
    operation_id: String,
    id: String,
    expected_revision: u64,
    edit: DuplicateReviewEdit,
) -> Ipc<DuplicateReview> {
    let review = run(host.inner().clone(), operation_id, move |host, cancel| {
        host.library
            .edit_duplicate_review(&id, expected_revision, edit, cancel)
    })
    .await?;
    let _ = app.emit(
        "maintenance-change",
        serde_json::json!({"review_id":review.id,"stamp":review.stamp}),
    );
    Ok(review)
}
#[tauri::command]
pub async fn maintenance_ignores(host: Managed<'_>) -> Ipc<DuplicateIgnores> {
    let lib = host.library.clone();
    work(move || lib.duplicate_ignores()).await
}
#[tauri::command]
pub async fn maintenance_set_ignores(
    host: Managed<'_>,
    app: tauri::AppHandle,
    track_ids: Vec<String>,
    ignored: bool,
    expected: MaintenanceStamp,
) -> Ipc<DuplicateIgnores> {
    let lib = host.library.clone();
    let snapshot = work(move || lib.set_duplicate_ignores(track_ids, ignored, expected)).await?;
    let _ = app.emit(
        "maintenance-change",
        serde_json::json!({"review_id":null,"stamp":snapshot.stamp}),
    );
    Ok(snapshot)
}

#[tauri::command]
pub async fn maintenance_preview_merge(
    host: Managed<'_>,
    operation_id: String,
    review_id: String,
    expected_revision: u64,
    groups: Vec<String>,
) -> Ipc<DuplicateMergePreview> {
    run(host.inner().clone(), operation_id, move |host, cancel| {
        host.library
            .preview_duplicate_merge(&review_id, expected_revision, groups, cancel)
    })
    .await
}
#[tauri::command]
pub async fn maintenance_merge_history(host: Managed<'_>) -> Ipc<Vec<DuplicateMergeHistory>> {
    let lib = host.library.clone();
    work(move || lib.duplicate_merge_history()).await
}
