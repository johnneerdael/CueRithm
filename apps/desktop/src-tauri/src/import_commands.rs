use crate::{
    commands::{AppError, work},
    host::Host,
};
use cuerithm_dj_sync::import::*;
use cuerithm_library::*;
use cuerithm_media::CancellationToken;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tauri::{Emitter, State};
mod selection;
pub use selection::*;
type Managed<'a> = State<'a, Arc<Host>>;
type Ipc<T> = Result<T, AppError>;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ImportMappingChoice {
    source_prefix: String,
    root_id: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NativeImportInspectRequest {
    selection_id: String,
    binding: SourceBindingChoice,
    capture_file_tags: bool,
    include_history: bool,
    include_queue: bool,
    path_mappings: Vec<ImportMappingChoice>,
}
#[derive(Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ImportEffectPageQuery {
    limit: u32,
    cursor: Option<String>,
}
impl Default for ImportEffectPageQuery {
    fn default() -> Self {
        Self {
            limit: 100,
            cursor: None,
        }
    }
}
#[derive(Clone, Serialize)]
struct Progress {
    operation_id: String,
    stage: &'static str,
    completed: u64,
    issue_summary: Option<&'static str>,
}

fn import_error(mut error: AppError) -> AppError {
    if error.code != "operation_failed" {
        return error;
    }
    let message = error.message.to_ascii_lowercase();
    error.code = if message.contains("expired")
        || message.contains("earlier runtime")
        || message.contains("changed")
        || message.contains("inspect again")
    {
        "stale_preview"
    } else if message.contains("busy") || message.contains("close the") {
        "source_busy"
    } else if message.contains("recover") || message.contains("already applied") {
        "recovery_required"
    } else if message.contains("unsupported")
        || message.contains("unknown")
        || message.contains("provision")
        || message.contains("supported rekordbox")
    {
        "unsupported_profile"
    } else {
        "operation_failed"
    };
    // Preserve actionable state without sending source paths or native acquisition details.
    error.message=match error.code {"stale_preview"=>"The source or library changed. Inspect it again before applying.","source_busy"=>"Close the source app and retry.","recovery_required"=>"Review the operation history and recover or undo the unfinished operation.","unsupported_profile"=>"This source profile or local access is unavailable. Select a supported source and installation.",_=>"Import could not complete. Review the source and operation history."}.into();
    error
}
fn leased<T>(
    lease: crate::maintenance::Lease,
    f: impl FnOnce(&CancellationToken) -> anyhow::Result<T>,
) -> anyhow::Result<T> {
    lease.token.check()?;
    f(&lease.token)
}
async fn run<T: Send + Serialize + 'static>(
    host: Arc<Host>,
    app: Option<tauri::AppHandle>,
    operation_id: String,
    f: impl FnOnce(&Host, &CancellationToken) -> anyhow::Result<T> + Send + 'static,
) -> Ipc<T> {
    let lease = host
        .maintenance
        .begin(&operation_id)
        .map_err(AppError::from)?;
    let emit = |stage, completed, issue_summary| {
        if let Some(app) = &app {
            let _ = app.emit(
                "library-import-progress",
                Progress {
                    operation_id: operation_id.clone(),
                    stage,
                    completed,
                    issue_summary,
                },
            );
        }
    };
    emit("running", 0, None);
    let result = work(move || leased(lease, |cancel| f(&host, cancel)))
        .await
        .map_err(import_error);
    let count = result
        .as_ref()
        .ok()
        .and_then(|v| serde_json::to_value(v).ok())
        .map(|v| {
            v.get("tracks_created")
                .or_else(|| v.get("processed"))
                .or_else(|| v.get("recovered"))
                .or_else(|| v.get("undone"))
                .or_else(|| v.get("ready"))
                .or_else(|| v.get("artifact_count"))
                .or_else(|| v.get("counts").and_then(|v| v.get("tracks")))
                .and_then(serde_json::Value::as_u64)
                .unwrap_or(0)
        })
        .unwrap_or(0);
    let stage = match &result {
        Ok(_) => "finished",
        Err(e) if e.code == "cancelled" => "cancelled",
        Err(_) => "stopped",
    };
    emit(
        stage,
        count,
        result
            .as_ref()
            .err()
            .map(|_| "Review source status and operation history"),
    );
    if let Some(app) = app {
        let _ = app.emit("library-change", serde_json::json!({"track_ids":[]}));
    }
    result
}
#[tauri::command]
pub async fn discover_import_sources(host: Managed<'_>) -> Ipc<Vec<ImportSourceCandidate>> {
    let library = host.library.clone();
    work(move || library.discover_import_sources())
        .await
        .map_err(import_error)
}
#[tauri::command]
pub async fn inspect_import(
    host: Managed<'_>,
    app: tauri::AppHandle,
    operation_id: String,
    request: NativeImportInspectRequest,
) -> Ipc<ImportInspection> {
    run(
        host.inner().clone(),
        Some(app),
        operation_id,
        move |host, cancel| {
            let selected = host.import_selections.get(&request.selection_id)?;
            let request = ImportInspectRequest {
                provider: selected.provider,
                selection: selected.selection,
                binding: request.binding,
                origin: None,
                path_mappings: host.import_selections.mappings(&request.path_mappings)?,
                capture_file_tags: request.capture_file_tags,
                include_history: request.include_history,
                include_queue: request.include_queue,
            };
            if let Some(reader) = selected.reader {
                host.library
                    .inspect_rekordbox_import(request, reader, cancel)
            } else {
                host.library.inspect_import(request, cancel)
            }
        },
    )
    .await
}
#[tauri::command]
pub async fn import_page(
    host: Managed<'_>,
    inspection_id: String,
    query: ImportPageQuery,
) -> Ipc<ImportPage> {
    let library = host.library.clone();
    work(move || library.import_page(&inspection_id, query))
        .await
        .map_err(import_error)
}
#[tauri::command]
pub async fn preview_import(
    host: Managed<'_>,
    app: tauri::AppHandle,
    operation_id: String,
    request: ImportPreviewRequest,
) -> Ipc<ImportPreview> {
    run(
        host.inner().clone(),
        Some(app),
        operation_id,
        move |host, cancel| host.library.preview_import(request, cancel),
    )
    .await
}
#[tauri::command]
pub async fn import_preview_page(
    host: Managed<'_>,
    preview_id: String,
    query: ImportEffectPageQuery,
) -> Ipc<ImportPreviewPage> {
    let library = host.library.clone();
    work(move || library.import_preview_page(&preview_id, query.cursor.as_deref(), query.limit))
        .await
        .map_err(import_error)
}
#[tauri::command]
pub async fn apply_import(
    host: Managed<'_>,
    app: tauri::AppHandle,
    operation_id: String,
    preview_id: String,
) -> Ipc<ImportApplyResult> {
    run(
        host.inner().clone(),
        Some(app),
        operation_id,
        move |host, cancel| host.library.apply_import(&preview_id, cancel),
    )
    .await
}
#[tauri::command]
pub async fn discard_import_preview(host: Managed<'_>, preview_id: String) -> Ipc<()> {
    let library = host.library.clone();
    work(move || library.discard_import_preview(&preview_id))
        .await
        .map_err(import_error)
}
#[tauri::command]
pub async fn inspect_import_refresh(
    host: Managed<'_>,
    app: tauri::AppHandle,
    operation_id: String,
    source_id: String,
) -> Ipc<ImportInspection> {
    run(
        host.inner().clone(),
        Some(app),
        operation_id,
        move |host, cancel| host.library.inspect_import_refresh(&source_id, cancel),
    )
    .await
}
#[tauri::command]
pub async fn import_operation_page(
    host: Managed<'_>,
    query: ImportOperationQuery,
) -> Ipc<ImportOperationPage> {
    let library = host.library.clone();
    work(move || library.import_operation_page(query))
        .await
        .map_err(import_error)
}
#[tauri::command]
pub async fn preview_import_effects(
    host: Managed<'_>,
    app: tauri::AppHandle,
    operation_id: String,
    id: String,
) -> Ipc<ImportEffectsPreview> {
    run(
        host.inner().clone(),
        Some(app),
        operation_id,
        move |host, cancel| host.library.preview_import_effects(&id, cancel),
    )
    .await
}
#[tauri::command]
pub async fn import_effects_page(
    host: Managed<'_>,
    preview_id: String,
    query: ImportEffectPageQuery,
) -> Ipc<ImportEffectsPage> {
    let library = host.library.clone();
    work(move || library.import_effects_page(&preview_id, query.cursor.as_deref(), query.limit))
        .await
        .map_err(import_error)
}
#[tauri::command]
pub async fn apply_import_effects(
    host: Managed<'_>,
    app: tauri::AppHandle,
    operation_id: String,
    preview_id: String,
) -> Ipc<ImportProcessingSummary> {
    run(
        host.inner().clone(),
        Some(app),
        operation_id,
        move |host, cancel| host.library.apply_import_effects(&preview_id, cancel),
    )
    .await
}
#[tauri::command]
pub async fn recover_import_operation(
    host: Managed<'_>,
    app: tauri::AppHandle,
    operation_id: String,
    id: String,
) -> Ipc<ImportRecoveryResult> {
    run(
        host.inner().clone(),
        Some(app),
        operation_id,
        move |host, cancel| host.library.recover_import_operation(&id, cancel),
    )
    .await
}
#[tauri::command]
pub async fn undo_import_operation(
    host: Managed<'_>,
    app: tauri::AppHandle,
    operation_id: String,
    id: String,
) -> Ipc<ImportUndoResult> {
    run(
        host.inner().clone(),
        Some(app),
        operation_id,
        move |host, cancel| host.library.undo_import_operation(&id, cancel),
    )
    .await
}
#[tauri::command]
pub async fn imported_item(host: Managed<'_>, id: String) -> Ipc<ImportedItemDetail> {
    let library = host.library.clone();
    work(move || library.imported_item(&id))
        .await
        .map_err(import_error)
}
#[tauri::command]
pub async fn imported_collection_page(
    host: Managed<'_>,
    id: String,
    query: ImportEffectPageQuery,
) -> Ipc<ImportedCollectionPage> {
    let library = host.library.clone();
    work(move || library.imported_collection_page(&id, query.cursor.as_deref(), query.limit))
        .await
        .map_err(import_error)
}
#[tauri::command]
pub async fn run_import_processing(
    host: Managed<'_>,
    app: tauri::AppHandle,
    operation_id: String,
) -> Ipc<ImportProcessingSummary> {
    run(
        host.inner().clone(),
        Some(app),
        operation_id,
        move |host, cancel| host.import_processing.run_pending(cancel),
    )
    .await
}

#[tauri::command]
pub async fn retry_import_processing(
    host: Managed<'_>,
    app: tauri::AppHandle,
    operation_id: String,
    id: String,
) -> Ipc<ImportProcessingSummary> {
    run(
        host.inner().clone(),
        Some(app),
        operation_id,
        move |host, cancel| host.import_processing.retry_pending(&id, cancel),
    )
    .await
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cancelled_inspection_releases_lease() {
        let tasks = Arc::new(crate::maintenance::MaintenanceTasks::default());
        let lease = tasks.begin("scan").unwrap();
        assert!(tasks.begin("scan").is_err());
        tasks.cancel("scan").unwrap();
        assert!(leased(lease, |_| Ok(())).is_err());
        assert!(tasks.begin("next").is_ok());
    }
    #[test]
    fn import_errors_keep_typed_states_and_redact_native_context() {
        for (message, code) in [
            ("Import preview expired; inspect again", "stale_preview"),
            (
                "Unsupported source profile /private/music",
                "unsupported_profile",
            ),
            ("Source is busy; close the source app", "source_busy"),
            (
                "Effects already applied or require recovery",
                "recovery_required",
            ),
        ] {
            let error = import_error(anyhow::anyhow!(message).into());
            assert_eq!(error.code, code);
            assert!(!error.message.contains("/private"));
        }
    }
}
