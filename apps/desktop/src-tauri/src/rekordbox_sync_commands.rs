use crate::{
    commands::{AppError, work},
    host::Host,
};
use cuerithm_library::{
    RekordboxManualTarget, RekordboxSyncHistoryEntry, RekordboxSyncOutcome, RekordboxSyncPreflight,
    RekordboxSyncPreview, RekordboxSyncRequest, RekordboxSyncRuntime, RekordboxSyncTarget,
    SyncConfigurationBinding,
};
use std::sync::Arc;
use tauri::{Emitter, State};

type Managed<'a> = State<'a, Arc<Host>>;
type Ipc<T> = Result<T, AppError>;
#[tauri::command]
pub async fn configure_rekordbox_sync_target(
    host: Managed<'_>,
    request: RekordboxManualTarget,
) -> Ipc<RekordboxSyncTarget> {
    let library = host.library.clone();
    work(move || library.configure_rekordbox_sync_target(request))
        .await
        .map_err(native_error)
}

// Keep the shared IPC envelope while exposing the native workflow's actionable
// states. The SDK supplies already-redacted messages; never attach key sources.
fn native_error(mut error: AppError) -> AppError {
    if error.code != "operation_failed" {
        return error;
    }
    let message = error.message.to_ascii_lowercase();
    error.code = if message.contains("close the target dj software") {
        "target_busy"
    } else if message.contains("another rekordbox sync operation") {
        "native_sync_busy"
    } else if message.contains("restore refused") {
        "restore_conflict"
    } else if message.contains("recovery refused") {
        "recovery_conflict"
    } else if message.contains("not passed native write acceptance")
        || message.contains("no accepted native write capability")
        || message.contains("not supported for syncing changes")
    {
        "read_only_profile"
    } else if message.contains("discard the existing preview")
        || message.contains("recover the interrupted")
    {
        "recovery_required"
    } else if message.contains("create a new rekordbox preview")
        || message.contains("database changed after preview")
        || message.contains("destination changed")
        || message.contains("discover targets again")
    {
        "stale_preview"
    } else {
        "operation_failed"
    };
    error
}

async fn run<T: Send + 'static>(
    host: Arc<Host>,
    operation_id: String,
    f: impl FnOnce(&Host, &cuerithm_media::CancellationToken) -> anyhow::Result<T> + Send + 'static,
) -> Ipc<T> {
    let lease = host.maintenance.begin(&operation_id)?;
    work(move || {
        lease.token.check()?;
        let result = f(&host, &lease.token);
        drop(lease);
        result
    })
    .await
    .map_err(native_error)
}

#[tauri::command]
pub async fn discover_rekordbox_targets(host: Managed<'_>) -> Ipc<Vec<RekordboxSyncTarget>> {
    let library = host.library.clone();
    work(move || library.discover_rekordbox_targets())
        .await
        .map_err(native_error)
}

#[tauri::command]
pub async fn preflight_rekordbox_sync(
    host: Managed<'_>,
    operation_id: String,
    target_id: String,
    configuration: Option<SyncConfigurationBinding>,
) -> Ipc<RekordboxSyncPreflight> {
    run(host.inner().clone(), operation_id, move |host, cancel| {
        host.library
            .preflight_rekordbox_sync_configured(&target_id, configuration, cancel)
    })
    .await
}

#[tauri::command]
pub async fn preview_rekordbox_sync(
    host: Managed<'_>,
    operation_id: String,
    request: RekordboxSyncRequest,
) -> Ipc<RekordboxSyncPreview> {
    run(host.inner().clone(), operation_id, move |host, cancel| {
        let runtime = RekordboxSyncRuntime::current(Some(host.runtime.clone()))?;
        host.library
            .preview_rekordbox_sync(request, &runtime, cancel)
    })
    .await
}

#[tauri::command]
pub async fn discard_rekordbox_sync(host: Managed<'_>, id: String) -> Ipc<()> {
    let library = host.library.clone();
    work(move || library.discard_rekordbox_sync(&id))
        .await
        .map_err(native_error)
}

fn changed(app: &tauri::AppHandle) {
    let _ = app.emit(
        "library-change",
        serde_json::json!({"track_ids":[],"playlist_ids":[]}),
    );
}

#[tauri::command]
pub async fn apply_rekordbox_sync(
    host: Managed<'_>,
    app: tauri::AppHandle,
    operation_id: String,
    id: String,
) -> Ipc<RekordboxSyncOutcome> {
    let result = run(host.inner().clone(), operation_id, move |host, cancel| {
        host.library.apply_rekordbox_sync(&id, cancel)
    })
    .await?;
    changed(&app);
    Ok(result)
}

#[tauri::command]
pub async fn restore_rekordbox_sync(
    host: Managed<'_>,
    app: tauri::AppHandle,
    operation_id: String,
    id: String,
) -> Ipc<RekordboxSyncOutcome> {
    let result = run(host.inner().clone(), operation_id, move |host, cancel| {
        host.library.restore_rekordbox_sync(&id, cancel)
    })
    .await?;
    changed(&app);
    Ok(result)
}

#[tauri::command]
pub async fn recover_rekordbox_sync(
    host: Managed<'_>,
    app: tauri::AppHandle,
    operation_id: String,
    id: String,
) -> Ipc<RekordboxSyncOutcome> {
    let result = run(host.inner().clone(), operation_id, move |host, cancel| {
        host.library.recover_rekordbox_sync(&id, cancel)
    })
    .await?;
    changed(&app);
    Ok(result)
}

#[tauri::command]
pub async fn rekordbox_sync_history(host: Managed<'_>) -> Ipc<Vec<RekordboxSyncHistoryEntry>> {
    let library = host.library.clone();
    work(move || library.history_rekordbox_sync())
        .await
        .map_err(native_error)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_errors_preserve_messages_and_shared_error_codes() {
        for (message, code) in [
            ("Close the target DJ software before syncing", "target_busy"),
            (
                "Another rekordbox sync operation is running",
                "native_sync_busy",
            ),
            (
                "Native database changed after publication; restore refused",
                "restore_conflict",
            ),
            (
                "Native database changed during interruption; recovery refused",
                "recovery_conflict",
            ),
            (
                "This installed rekordbox application and database profile has not passed native write acceptance",
                "read_only_profile",
            ),
            (
                "This rekordbox library format is not supported for syncing changes",
                "read_only_profile",
            ),
            (
                "Discard the existing preview or recover the interrupted rekordbox operation first",
                "recovery_required",
            ),
            (
                "Library changed; create a new rekordbox preview",
                "stale_preview",
            ),
            ("Native database changed after preview", "stale_preview"),
            ("Unexpected disk error", "operation_failed"),
        ] {
            let error = native_error(AppError {
                code: "operation_failed",
                message: message.into(),
            });
            assert_eq!(error.code, code);
            assert_eq!(error.message, message);
        }
        assert_eq!(
            native_error(AppError {
                code: "cancelled",
                message: "Cancelled".into()
            })
            .code,
            "cancelled"
        );
    }
}
