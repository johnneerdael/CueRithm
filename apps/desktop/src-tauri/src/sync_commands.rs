use crate::{
    commands::{AppError, work},
    host::Host,
};
use cuerithm_dj_sync::{SyncPreview, discovery::DiscoveredTarget, transaction::FileJournal};
use cuerithm_library::{DjSyncHistoryEntry, DjSyncRequest};
use std::{path::PathBuf, sync::Arc};
use tauri::{Emitter, Manager, State};
type Managed<'a> = State<'a, Arc<Host>>;
type Ipc<T> = Result<T, AppError>;

fn mounted_drives() -> anyhow::Result<Vec<PathBuf>> {
    #[cfg(windows)]
    {
        Ok(('A'..='Z')
            .map(|letter| PathBuf::from(format!("{letter}:\\")))
            .filter(|path| path.is_dir())
            .collect())
    }
    #[cfg(not(windows))]
    {
        let mut paths = vec![];
        for parent in [
            PathBuf::from("/Volumes"),
            PathBuf::from("/media"),
            PathBuf::from("/run/media"),
            PathBuf::from("/mnt"),
        ] {
            if !parent.exists() {
                continue;
            }
            for entry in std::fs::read_dir(&parent)? {
                let entry = entry?;
                if entry.path().is_dir() {
                    paths.push(entry.path());
                }
            }
        }
        #[cfg(target_os = "linux")]
        {
            let nested = paths.clone();
            for parent in nested {
                for entry in std::fs::read_dir(&parent)? {
                    let entry = entry?;
                    if entry.path().is_dir() {
                        paths.push(entry.path());
                    }
                }
            }
        }
        Ok(paths)
    }
}
#[tauri::command]
pub async fn discover_dj_targets(app: tauri::AppHandle) -> Ipc<Vec<DiscoveredTarget>> {
    let home = app.path().home_dir().map_err(|e| AppError {
        code: "discovery_failed",
        message: e.to_string(),
    })?;
    let music = app
        .path()
        .audio_dir()
        .unwrap_or_else(|_| home.join("Music"));
    let documents = app
        .path()
        .document_dir()
        .unwrap_or_else(|_| home.join("Documents"));
    work(move || {
        Ok(cuerithm_dj_sync::discovery::discover(
            &home,
            &music,
            &documents,
            &mounted_drives()?,
        ))
    })
    .await
}
#[tauri::command]
pub async fn preview_dj_sync(host: Managed<'_>, request: DjSyncRequest) -> Ipc<SyncPreview> {
    let host = host.inner().clone();
    work(move || {
        host.library
            .preview_dj_sync_with_runtime(request, &host.runtime)
    })
    .await
}
#[tauri::command]
pub async fn apply_dj_sync(
    host: Managed<'_>,
    app: tauri::AppHandle,
    id: String,
) -> Ipc<FileJournal> {
    let library = host.library.clone();
    let result = work(move || library.apply_dj_sync(&id)).await?;
    let _ = app.emit(
        "library-change",
        serde_json::json!({"track_ids":[],"playlist_ids":[]}),
    );
    Ok(result)
}
#[tauri::command]
pub async fn restore_dj_sync(
    host: Managed<'_>,
    app: tauri::AppHandle,
    id: String,
) -> Ipc<FileJournal> {
    let library = host.library.clone();
    let result = work(move || library.restore_dj_sync(&id)).await?;
    let _ = app.emit(
        "library-change",
        serde_json::json!({"track_ids":[],"playlist_ids":[]}),
    );
    Ok(result)
}
#[tauri::command]
pub async fn dj_sync_history(host: Managed<'_>) -> Ipc<Vec<DjSyncHistoryEntry>> {
    let library = host.library.clone();
    work(move || library.history_dj_sync()).await
}
