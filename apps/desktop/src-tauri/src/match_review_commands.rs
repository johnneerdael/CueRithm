use crate::{
    commands::{AppError, work},
    host::Host,
    metadata_commands::{library_changed, run},
};
use cuerithm_library::*;
use cuerithm_media::CancellationToken;
use std::sync::Arc;
use tauri::{Emitter, State};
type Managed<'a> = State<'a, Arc<Host>>;
type Ipc<T> = Result<T, AppError>;

// Access refresh is independent of track content and durable editor drafts.
// Never let an availability-read failure replace the provider operation result.
async fn run_provider<T: Send + 'static>(
    app: tauri::AppHandle,
    host: Arc<Host>,
    operation_id: String,
    f: impl FnOnce(&Host, &CancellationToken) -> anyhow::Result<T> + Send + 'static,
) -> Ipc<T> {
    run(host, operation_id, move |host, cancel| {
        let access = || {
            host.library.tagging_catalog().ok().map(|catalog| {
                catalog
                    .access
                    .into_iter()
                    .map(|source| (source.id, source.requires_solver))
                    .collect::<Vec<_>>()
            })
        };
        let before = access();
        let result = f(host, cancel);
        if before != access() {
            let _ = app.emit("catalog-access-change", ());
        }
        result
    })
    .await
}

#[tauri::command]
pub async fn track_match_review(
    host: Managed<'_>,
    track_id: String,
) -> Ipc<Option<MatchReviewState>> {
    let lib = host.library.clone();
    work(move || lib.track_match_review(&track_id)).await
}
#[tauri::command]
pub async fn preview_match_review(
    host: Managed<'_>,
    app: tauri::AppHandle,
    operation_id: String,
    request: MatchReviewRequest,
) -> Ipc<MatchReviewPreview> {
    run_provider(
        app.clone(),
        host.inner().clone(),
        operation_id,
        move |host, cancel| host.preparation.preview_match_review(request, cancel),
    )
    .await
}
#[tauri::command]
pub async fn apply_match_review(
    host: Managed<'_>,
    app: tauri::AppHandle,
    operation_id: String,
    preview_id: String,
    revision: String,
) -> Ipc<MatchReviewApplyResult> {
    let result = run_provider(
        app.clone(),
        host.inner().clone(),
        operation_id,
        move |host, cancel| {
            host.preparation
                .apply_match_review(&preview_id, &revision, cancel)
        },
    )
    .await;
    library_changed(&app);
    let _ = app.emit("match-review-change", ());
    result
}
#[tauri::command]
pub async fn requery_matches(
    host: Managed<'_>,
    app: tauri::AppHandle,
    operation_id: String,
    track_id: String,
    generation: String,
) -> Ipc<MatchReviewState> {
    let result = run_provider(
        app.clone(),
        host.inner().clone(),
        operation_id,
        move |host, cancel| {
            host.preparation
                .requery_matches(&track_id, &generation, cancel)
        },
    )
    .await;
    let _ = app.emit("match-review-change", ());
    result
}
#[tauri::command]
pub async fn match_review_artwork(
    host: Managed<'_>,
    app: tauri::AppHandle,
    operation_id: String,
    preview_id: String,
    revision: String,
) -> Ipc<MetadataArtworkAsset> {
    run_provider(
        app.clone(),
        host.inner().clone(),
        operation_id,
        move |host, cancel| {
            host.preparation
                .review_artwork_asset(&preview_id, &revision, cancel)
        },
    )
    .await
}
