use crate::{commands::AppError, host::Host};
use cuerithm_library::{PlaylistChange, TracklistOptions, TracklistResult, TracklistSaveRequest};
use std::sync::Arc;
use tauri::{Emitter, State};
#[tauri::command]
pub async fn match_tracklist(
    host: State<'_, Arc<Host>>,
    input: String,
    options: TracklistOptions,
) -> Result<TracklistResult, AppError> {
    let library = host.library.clone();
    tauri::async_runtime::spawn_blocking(move || library.match_tracklist(input, options))
        .await
        .map_err(|e| AppError::from(anyhow::anyhow!(e)))?
        .map_err(AppError::from)
}
#[tauri::command]
pub async fn save_tracklist_playlist(
    app: tauri::AppHandle,
    host: State<'_, Arc<Host>>,
    request: TracklistSaveRequest,
) -> Result<PlaylistChange, AppError> {
    let library = host.library.clone();
    let change =
        tauri::async_runtime::spawn_blocking(move || library.save_tracklist_playlist(request))
            .await
            .map_err(|e| AppError::from(anyhow::anyhow!(e)))?
            .map_err(AppError::from)?;
    let _ = app.emit(
        "library-change",
        serde_json::json!({"track_ids":[],"playlist_ids":change.affected_ids}),
    );
    Ok(change)
}
