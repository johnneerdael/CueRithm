use crate::host::{Detail, Host};
use cuerithm_library::*;
use serde::Serialize;
use std::{path::PathBuf, sync::Arc};
use tauri::{Emitter, State};
type Managed<'a> = State<'a, Arc<Host>>;
#[derive(Debug, Serialize)]
pub struct AppError {
    pub code: &'static str,
    pub message: String,
}
impl From<anyhow::Error> for AppError {
    fn from(error: anyhow::Error) -> Self {
        let message = format!("{error:#}");
        let code = if error.downcast_ref::<cuerithm_media::MediaTaskError>()
            == Some(&cuerithm_media::MediaTaskError::Cancelled)
        {
            "cancelled"
        } else if message.contains("Another maintenance operation") {
            "maintenance_busy"
        } else if message.contains("Stale cursor") {
            "stale_cursor"
        } else if message.contains("superseded") {
            "superseded"
        } else {
            "operation_failed"
        };
        Self { code, message }
    }
}
type Ipc<T> = Result<T, AppError>;
#[derive(Serialize)]
pub struct PlaylistResponse {
    #[serde(flatten)]
    workspace: PlaylistWorkspace,
    service_error: Option<String>,
}
#[tauri::command]
pub async fn playlists_workspace(host: Managed<'_>) -> Ipc<PlaylistResponse> {
    let host = host.inner().clone();
    work(move || {
        Ok(PlaylistResponse {
            workspace: host.library.playlist_workspace()?,
            service_error: host.smart_playlists.last_error()?,
        })
    })
    .await
}
fn playlist_changed(app: &tauri::AppHandle, change: &PlaylistChange) {
    let _ = app.emit(
        "library-change",
        serde_json::json!({"track_ids":[],"playlist_ids":change.affected_ids}),
    );
}
#[tauri::command]
pub async fn edit_playlist(
    host: Managed<'_>,
    app: tauri::AppHandle,
    edit: PlaylistEdit,
    expected_revision: u64,
) -> Ipc<PlaylistChange> {
    let lib = host.library.clone();
    let result = work(move || lib.apply_playlist_edit(edit, expected_revision)).await?;
    playlist_changed(&app, &result);
    Ok(result)
}
#[tauri::command]
pub async fn undo_playlist(
    host: Managed<'_>,
    app: tauri::AppHandle,
    edit_id: String,
    expected_revision: u64,
) -> Ipc<PlaylistChange> {
    let lib = host.library.clone();
    let result = work(move || lib.undo_playlist_edit(&edit_id, expected_revision)).await?;
    playlist_changed(&app, &result);
    Ok(result)
}
#[tauri::command]
pub async fn smart_field_catalog(host: Managed<'_>) -> Ipc<SmartFieldCatalog> {
    let lib = host.library.clone();
    work(move || lib.smart_field_catalog()).await
}
#[tauri::command]
pub async fn preview_smart_list(
    host: Managed<'_>,
    definition: SmartDefinition,
    scope: SmartPreviewScope,
) -> Ipc<SmartPreview> {
    let lib = host.library.clone();
    work(move || {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_millis()
            .min(i64::MAX as u128) as i64;
        lib.preview_smart_list(definition, scope, now)
    })
    .await
}
#[tauri::command]
pub async fn save_smart_playlist(
    host: Managed<'_>,
    app: tauri::AppHandle,
    preview_id: String,
    playlist_id: Option<String>,
    name: String,
    parent_id: Option<String>,
    expected_revision: u64,
) -> Ipc<PlaylistChange> {
    let lib = host.library.clone();
    let result = work(move || {
        lib.save_smart_preview(
            &preview_id,
            playlist_id.as_deref(),
            name,
            parent_id,
            expected_revision,
        )
    })
    .await?;
    playlist_changed(&app, &result);
    Ok(result)
}
#[tauri::command]
pub async fn preview_playlist_tool(
    host: Managed<'_>,
    request: PlaylistToolRequest,
) -> Ipc<PlaylistToolPreview> {
    let lib = host.library.clone();
    work(move || lib.preview_playlist_tool(request)).await
}
#[tauri::command]
pub async fn apply_playlist_tool(
    host: Managed<'_>,
    app: tauri::AppHandle,
    preview_id: String,
    expected_revision: u64,
) -> Ipc<PlaylistChange> {
    let lib = host.library.clone();
    let result = work(move || lib.apply_playlist_tool(&preview_id, expected_revision)).await?;
    playlist_changed(&app, &result);
    Ok(result)
}
#[tauri::command]
pub async fn preview_smart_generation(
    host: Managed<'_>,
    request: SmartGenerationRequest,
) -> Ipc<GenerationPreview> {
    let lib = host.library.clone();
    work(move || lib.preview_smart_generation(request)).await
}
#[tauri::command]
pub async fn apply_smart_generation(
    host: Managed<'_>,
    app: tauri::AppHandle,
    preview_id: String,
    expected_revision: u64,
) -> Ipc<PlaylistChange> {
    let lib = host.library.clone();
    let result = work(move || lib.apply_smart_generation(&preview_id, expected_revision)).await?;
    playlist_changed(&app, &result);
    Ok(result)
}
#[tauri::command]
pub async fn get_playlist_preferences(host: Managed<'_>) -> Ipc<PlaylistPreferencesSnapshot> {
    let lib = host.library.clone();
    work(move || lib.get_playlist_preferences()).await
}
#[tauri::command]
pub async fn resolve_playlist_calendar_day(
    host: Managed<'_>,
    day: String,
    expected_revision: u64,
) -> Ipc<i64> {
    let lib = host.library.clone();
    work(move || lib.resolve_playlist_calendar_day(&day, expected_revision)).await
}
#[tauri::command]
pub async fn save_playlist_preferences(
    host: Managed<'_>,
    app: tauri::AppHandle,
    preferences: PlaylistPreferences,
    expected_revision: u64,
) -> Ipc<PlaylistPreferencesSnapshot> {
    let lib = host.library.clone();
    let result =
        work(move || lib.save_playlist_preferences(preferences, expected_revision)).await?;
    let _ = app.emit(
        "library-change",
        serde_json::json!({"track_ids":[],"playlist_ids":[]}),
    );
    Ok(result)
}
#[tauri::command]
pub async fn playlist_members(
    host: Managed<'_>,
    playlist_id: String,
    manual: bool,
) -> Ipc<Vec<String>> {
    let lib = host.library.clone();
    work(move || {
        if manual {
            lib.manual_playlist_members(&playlist_id)
        } else {
            lib.playlist_members(&playlist_id)
        }
    })
    .await
}
pub(crate) async fn work<T: Send + 'static>(
    f: impl FnOnce() -> anyhow::Result<T> + Send + 'static,
) -> Ipc<T> {
    tauri::async_runtime::spawn_blocking(f)
        .await
        .map_err(|e| AppError {
            code: "worker_failed",
            message: e.to_string(),
        })?
        .map_err(Into::into)
}
#[derive(Serialize)]
pub struct LibraryState {
    counts: LibraryCounts,
    revision: u64,
    jobs: Vec<JobSummary>,
    settings: SettingsSnapshot,
    service: ServiceStatus,
}
#[tauri::command]
pub async fn library_state(host: Managed<'_>) -> Ipc<LibraryState> {
    let host = host.inner().clone();
    work(move || {
        Ok(LibraryState {
            counts: host.library.counts()?,
            revision: host.library.revision()?,
            jobs: host.library.jobs()?,
            settings: host.library.settings()?,
            service: host.preparation.status(),
        })
    })
    .await
}
#[tauri::command]
pub async fn list_tracks(host: Managed<'_>, query: TrackQuery) -> Ipc<TrackPage> {
    let lib = host.library.clone();
    work(move || lib.tracks(query)).await
}
#[tauri::command]
pub async fn select_track_ids(host: Managed<'_>, query: TrackQuery) -> Ipc<Vec<TrackId>> {
    let lib = host.library.clone();
    work(move || lib.track_ids(query)).await
}
#[tauri::command]
pub async fn metadata_fields(host: Managed<'_>) -> Ipc<Vec<String>> {
    let host = host.inner().clone();
    work(move || host.metadata_fields()).await
}
#[tauri::command]
pub async fn list_jobs(host: Managed<'_>) -> Ipc<Vec<JobSummary>> {
    let lib = host.library.clone();
    work(move || lib.jobs()).await
}
#[tauri::command]
pub async fn get_track(host: Managed<'_>, track_id: String) -> Ipc<Detail> {
    let host = host.inner().clone();
    work(move || host.detail(&track_id)).await
}
#[tauri::command]
pub async fn get_artwork(
    host: Managed<'_>,
    track_id: String,
    source_sha256: String,
) -> Ipc<Option<String>> {
    let host = host.inner().clone();
    work(move || host.artwork(&track_id, &source_sha256)).await
}
#[tauri::command]
pub async fn tagging_storage(host: Managed<'_>) -> Ipc<TaggingStorage> {
    let lib = host.library.clone();
    work(move || lib.tagging_storage()).await
}
#[tauri::command]
pub async fn tagging_catalog(host: Managed<'_>) -> Ipc<cuerithm_autotag::TaggingCatalog> {
    let library = host.library.clone();
    work(move || library.tagging_catalog()).await
}
#[tauri::command]
pub async fn reset_catalog_access(
    host: Managed<'_>,
    app: tauri::AppHandle,
) -> Ipc<cuerithm_autotag::TaggingCatalog> {
    let library = host.library.clone();
    let result = work(move || {
        library.reset_catalog_access()?;
        library.tagging_catalog()
    })
    .await;
    let _ = app.emit("catalog-access-change", ());
    result
}
#[tauri::command]
pub async fn cleanup_catalog(
    host: Managed<'_>,
    config: cuerithm_cleanup::CleanupConfig,
) -> Ipc<CleanupCatalog> {
    let lib = host.library.clone();
    work(move || lib.cleanup_catalog(&config)).await
}
#[tauri::command]
pub async fn preview_cleanup(
    host: Managed<'_>,
    scope: CleanupScope,
    config: cuerithm_cleanup::CleanupConfig,
) -> Ipc<CleanupPreview> {
    let service = CleanupService::new(host.library.clone(), host.runtime.clone());
    work(move || service.preview_cleanup(scope, config)).await
}
#[tauri::command]
pub async fn cleanup_history(host: Managed<'_>) -> Ipc<Vec<CleanupHistory>> {
    let service = CleanupService::new(host.library.clone(), host.runtime.clone());
    work(move || service.cleanup_history()).await
}
#[tauri::command]
pub async fn apply_cleanup(
    host: Managed<'_>,
    app: tauri::AppHandle,
    plan_id: String,
    proposal_ids: Vec<String>,
) -> Ipc<CleanupBatchResult> {
    let service = CleanupService::new(host.library.clone(), host.runtime.clone());
    let result = work(move || service.apply_cleanup(&plan_id, &proposal_ids)).await?;
    let _ = app.emit(
        "library-change",
        serde_json::json!({"track_ids": result.applied}),
    );
    Ok(result)
}
#[tauri::command]
pub async fn undo_cleanup(
    host: Managed<'_>,
    app: tauri::AppHandle,
    batch_id: String,
) -> Ipc<CleanupBatchResult> {
    let service = CleanupService::new(host.library.clone(), host.runtime.clone());
    let result = work(move || service.undo_cleanup(&batch_id)).await?;
    let _ = app.emit(
        "library-change",
        serde_json::json!({"track_ids": result.applied}),
    );
    Ok(result)
}
#[tauri::command]
pub async fn get_settings(host: Managed<'_>) -> Ipc<SettingsSnapshot> {
    let lib = host.library.clone();
    work(move || lib.settings()).await
}
#[tauri::command]
pub async fn save_settings(
    host: Managed<'_>,
    app: tauri::AppHandle,
    mut settings: PreparationSettings,
    folder_changes: Option<Vec<PreparationFolderChange>>,
    expected_revision: Option<String>,
) -> Ipc<SettingsSnapshot> {
    let host = host.inner().clone();
    let result = work(move || {
        host.builtin.apply(&mut settings);
        let revision = if let Some(changes) = folder_changes {
            let revision = expected_revision.as_deref().ok_or_else(|| {
                anyhow::anyhow!("Reload settings before saving folder assignments")
            })?;
            host.library
                .save_settings_with_preparation_assignments(settings, revision, &changes)?
        } else {
            host.library.save_settings(settings)?
        };
        host.preparation.wake();
        let _ = host.preparation.retry_attention();
        host.library.settings_revision(&revision)
    })
    .await;
    let _ = app.emit("catalog-access-change", ());
    let _ = app.emit("preparation-presets-change", ());
    result
}
#[tauri::command]
pub async fn enqueue_paths(host: Managed<'_>, paths: Vec<PathBuf>) -> Ipc<Vec<JobId>> {
    let host = host.inner().clone();
    work(move || host.preparation.enqueue_paths(&paths)).await
}
#[tauri::command]
pub async fn retry_job(
    host: Managed<'_>,
    job_id: String,
    use_captured_settings: Option<bool>,
) -> Ipc<()> {
    let host = host.inner().clone();
    work(move || {
        let revision = if use_captured_settings.unwrap_or(false) {
            None
        } else {
            Some(host.library.settings()?.revision)
        };
        host.preparation.retry(job_id, revision)
    })
    .await
}
#[tauri::command]
pub async fn reprepare_track(host: Managed<'_>, track_id: String) -> Ipc<()> {
    let host = host.inner().clone();
    work(move || host.preparation.reprepare(track_id)).await
}
#[tauri::command]
pub async fn archive_track(host: Managed<'_>, track_id: String, archived: bool) -> Ipc<()> {
    let lib = host.library.clone();
    work(move || lib.set_archived(&track_id, archived)).await
}
#[tauri::command]
pub async fn get_waveform(
    host: Managed<'_>,
    track_id: String,
    bins: usize,
) -> Ipc<waveform::Waveform> {
    let host = host.inner().clone();
    work(move || host.waveform(&track_id, bins)).await
}
#[tauri::command]
pub async fn player_load(host: Managed<'_>, track_id: String) -> Ipc<Detail> {
    let host = host.inner().clone();
    let generation = host.begin_load();
    work(move || host.load(track_id, generation)).await
}
#[tauri::command]
pub async fn player_play(host: Managed<'_>) -> Ipc<()> {
    let host = host.inner().clone();
    work(move || {
        host.player
            .lock()
            .map_err(|_| anyhow::anyhow!("Player interrupted"))?
            .play()
    })
    .await
}
#[tauri::command]
pub async fn player_pause(host: Managed<'_>) -> Ipc<()> {
    let host = host.inner().clone();
    work(move || {
        host.player
            .lock()
            .map_err(|_| anyhow::anyhow!("Player interrupted"))?
            .pause()
    })
    .await
}
#[tauri::command]
pub async fn player_seek(host: Managed<'_>, seconds: f64) -> Ipc<()> {
    let host = host.inner().clone();
    work(move || {
        host.player
            .lock()
            .map_err(|_| anyhow::anyhow!("Player interrupted"))?
            .seek(seconds)
    })
    .await
}
#[tauri::command]
pub async fn player_volume(host: Managed<'_>, volume: f32) -> Ipc<()> {
    let host = host.inner().clone();
    work(move || {
        host.player
            .lock()
            .map_err(|_| anyhow::anyhow!("Player interrupted"))?
            .set_volume(volume)
    })
    .await
}
#[tauri::command]
pub fn player_status(host: Managed<'_>) -> Ipc<playback::PlaybackStatus> {
    Ok(host
        .player
        .lock()
        .map_err(|_| AppError {
            code: "player_unavailable",
            message: "Player interrupted".into(),
        })?
        .status())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bridge_errors_and_query_enums_have_stable_json_contracts() {
        let query: TrackQuery = serde_json::from_value(
            serde_json::json!({"view":"incoming","sort":"bpm","descending":true,"limit":200}),
        )
        .unwrap();
        assert!(matches!(query.view, TrackView::Incoming));
        let error: AppError = anyhow::anyhow!("Stale cursor; refresh the library").into();
        assert_eq!(serde_json::to_value(error).unwrap()["code"], "stale_cursor");
        assert!(
            serde_json::from_value::<TrackQuery>(serde_json::json!({"view":"unknown"})).is_err()
        );
    }
}
