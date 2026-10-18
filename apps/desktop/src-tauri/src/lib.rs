mod commands;
mod host;
use anyhow::{Context, Result};
use std::{path::PathBuf, sync::Arc, time::Duration};
use tauri::{Emitter, Manager};

pub fn diagnostics() -> Result<()> {
    let exe = std::env::current_exe()?;
    let parent = exe.parent().context("Missing executable directory")?;
    let folder = if parent.file_name().is_some_and(|n| n == "MacOS") {
        parent.join("../Resources/media")
    } else {
        parent.join("media")
    };
    let runtime = cuerithm_media::MediaRuntime::from_bundle(&folder)?;
    println!(
        "{}",
        serde_json::json!({"application":"Cuerithm","version":env!("CARGO_PKG_VERSION"),"decoder":runtime.identity,"resources_verified":true})
    );
    Ok(())
}
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let root = std::env::var_os("CUERITHM_DATA_DIR")
                .map(PathBuf::from)
                .unwrap_or(app.path().app_data_dir()?);
            let runtime = if cfg!(debug_assertions) {
                std::env::var_os("CUERITHM_MEDIA_RUNTIME")
                    .map(PathBuf::from)
                    .unwrap_or(app.path().resource_dir()?.join("media"))
            } else {
                app.path().resource_dir()?.join("media")
            };
            let initial_folder = app
                .path()
                .audio_dir()
                .ok()
                .map(|p| p.join("Cuerithm/Incoming"));
            let host = Arc::new(host::Host::open(&root, &runtime, initial_folder)?);
            let events = host.preparation.subscribe();
            let handle = app.handle().clone();
            std::thread::spawn(move || {
                for event in events {
                    let _ = handle.emit("preparation", event);
                }
            });
            let state = host.clone();
            let handle = app.handle().clone();
            std::thread::spawn(move || {
                let mut sequence = 0u64;
                while state.alive.load(std::sync::atomic::Ordering::Relaxed) {
                    sequence += 1;
                    if let Ok(player) = state.player.lock() {
                        let _ = handle.emit(
                            "playback",
                            serde_json::json!({"sequence":sequence,"status":player.status()}),
                        );
                    }
                    std::thread::sleep(Duration::from_millis(100));
                }
            });
            app.manage(host);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::library_state,
            commands::list_tracks,
            commands::list_jobs,
            commands::get_track,
            commands::get_settings,
            commands::save_settings,
            commands::enqueue_paths,
            commands::retry_job,
            commands::reprepare_track,
            commands::archive_track,
            commands::get_waveform,
            commands::player_load,
            commands::player_play,
            commands::player_pause,
            commands::player_seek,
            commands::player_volume,
            commands::player_status
        ])
        .build(tauri::generate_context!())
        .expect("Create Cuerithm desktop")
        .run(|app, event| {
            if let tauri::RunEvent::ExitRequested {
                code: None, api, ..
            } = event
                && let Some(host) = app.try_state::<Arc<host::Host>>()
            {
                api.prevent_exit();
                if host.alive.swap(false, std::sync::atomic::Ordering::Relaxed) {
                    let host = host.inner().clone();
                    let app = app.clone();
                    std::thread::spawn(move || {
                        if let Ok(mut player) = host.player.lock() {
                            let _ = player.pause();
                        }
                        host.preparation.shutdown();
                        app.exit(0);
                    });
                }
            }
        });
}
