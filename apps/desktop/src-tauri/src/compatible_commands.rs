use crate::{commands::AppError, host::Host};
use cuerithm_library::{
    CompatibleTrackRequest, CompatibleTrackResult, PlaylistChange, PreparedPlaylistSaveRequest,
};
use std::sync::Arc;
use tauri::{Emitter, State};

#[tauri::command]
pub async fn find_compatible_tracks(
    host: State<'_, Arc<Host>>,
    request: CompatibleTrackRequest,
) -> Result<CompatibleTrackResult, AppError> {
    let library = host.library.clone();
    tauri::async_runtime::spawn_blocking(move || library.find_compatible_tracks(request))
        .await
        .map_err(|error| AppError::from(anyhow::anyhow!(error)))?
        .map_err(AppError::from)
}

#[tauri::command]
pub async fn save_compatible_playlist(
    host: State<'_, Arc<Host>>,
    app: tauri::AppHandle,
    request: PreparedPlaylistSaveRequest,
) -> Result<PlaylistChange, AppError> {
    let library = host.library.clone();
    let change =
        tauri::async_runtime::spawn_blocking(move || library.save_prepared_playlist(request))
            .await
            .map_err(|error| AppError::from(anyhow::anyhow!(error)))?
            .map_err(AppError::from)?;
    let _ = app.emit(
        "library-change",
        serde_json::json!({"track_ids":[],"playlist_ids":change.affected_ids}),
    );
    Ok(change)
}
