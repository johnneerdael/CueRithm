use crate::host::{Detail, Host};
use cuerithm_library::*;
use serde::Serialize;
use std::{path::PathBuf, sync::Arc};
use tauri::State;
type Managed<'a> = State<'a, Arc<Host>>;
#[derive(Debug, Serialize)]
pub struct AppError {
    pub code: &'static str,
    pub message: String,
}
impl From<anyhow::Error> for AppError {
    fn from(error: anyhow::Error) -> Self {
        let message = format!("{error:#}");
        let code = if message.contains("Stale cursor") {
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
async fn work<T: Send + 'static>(f: impl FnOnce() -> anyhow::Result<T> + Send + 'static) -> Ipc<T> {
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
pub async fn get_settings(host: Managed<'_>) -> Ipc<SettingsSnapshot> {
    let lib = host.library.clone();
    work(move || lib.settings()).await
}
#[tauri::command]
pub async fn save_settings(
    host: Managed<'_>,
    mut settings: PreparationSettings,
) -> Ipc<SettingsSnapshot> {
    let host = host.inner().clone();
    work(move || {
        host.builtin.apply(&mut settings);
        host.library.save_settings(settings)?;
        host.preparation.wake();
        let _ = host.preparation.retry_attention();
        host.library.settings()
    })
    .await
}
#[tauri::command]
pub async fn enqueue_paths(host: Managed<'_>, paths: Vec<PathBuf>) -> Ipc<Vec<JobId>> {
    let host = host.inner().clone();
    work(move || host.preparation.enqueue_paths(&paths)).await
}
#[tauri::command]
pub async fn retry_job(host: Managed<'_>, job_id: String) -> Ipc<()> {
    let host = host.inner().clone();
    work(move || {
        host.preparation
            .retry(job_id, Some(host.library.settings()?.revision))
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
