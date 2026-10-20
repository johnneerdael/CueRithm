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
#[cfg(not(windows))]
mod mounts;

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
        Ok(mounts::scan(
            &[
                PathBuf::from("/Volumes"),
                PathBuf::from("/media"),
                PathBuf::from("/run/media"),
                PathBuf::from("/mnt"),
            ],
            cfg!(target_os = "linux"),
        )?)
    }
}

fn ipc_targets(targets: Vec<DiscoveredTarget>) -> Vec<DiscoveredTarget> {
    // The destination picker is a text input. Raw source paths stay lossless in
    // private evidence, but a raw destination tag cannot fill this string field.
    targets
        .into_iter()
        .filter(|target| target.path.to_str().is_some())
        .collect()
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
        Ok(ipc_targets(cuerithm_dj_sync::discovery::discover(
            &home,
            &music,
            &documents,
            &mounted_drives()?,
        )))
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
pub async fn discard_dj_sync(host: Managed<'_>, id: String) -> Ipc<()> {
    let library = host.library.clone();
    work(move || library.discard_dj_sync(&id)).await
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn discovery_keeps_the_frontend_destination_path_contract_as_text() {
        #[cfg(unix)]
        let raw = {
            use std::os::unix::ffi::OsStringExt;
            PathBuf::from(std::ffi::OsString::from_vec(b"/volumes/raw-\xff".to_vec()))
        };
        #[cfg(windows)]
        let raw = {
            use std::os::windows::ffi::OsStringExt;
            PathBuf::from(std::ffi::OsString::from_wide(&[0x43, 0x3a, 0x5c, 0xd800]))
        };
        assert!(raw.to_str().is_none());
        let targets = ipc_targets(vec![
            DiscoveredTarget {
                software: cuerithm_dj_sync::DjSoftware::EngineDj,
                path: raw,
            },
            DiscoveredTarget {
                software: cuerithm_dj_sync::DjSoftware::EngineDj,
                path: "/volumes/Engine Library/Database2/m.db".into(),
            },
        ]);
        let json = serde_json::to_value(targets).unwrap();
        assert_eq!(json.as_array().unwrap().len(), 1);
        assert_eq!(json[0]["path"], "/volumes/Engine Library/Database2/m.db");
    }
}
