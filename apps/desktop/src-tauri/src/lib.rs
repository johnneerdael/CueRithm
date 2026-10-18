mod builtin;
mod commands;
mod host;
use anyhow::Result;
use std::{path::PathBuf, sync::Arc, time::Duration};
use tauri::{Emitter, Manager};

fn context() -> tauri::Context<tauri::Wry> {
    tauri::generate_context!()
}

pub fn diagnostics() -> Result<()> {
    let context = context();
    let folder = tauri::utils::platform::resource_dir(context.package_info(), &Default::default())?
        .join("media");
    let runtime = cuerithm_media::MediaRuntime::from_bundle(&folder)?;
    builtin::BuiltinModels::verify()?;
    println!(
        "{}",
        serde_json::json!({"application":"Cuerithm","version":env!("CARGO_PKG_VERSION"),"decoder":runtime.identity,"resources_verified":true,"built_in_analysis_verified":true})
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
            commands::metadata_fields,
            commands::select_track_ids,
            commands::list_jobs,
            commands::get_track,
            commands::get_artwork,
            commands::get_settings,
            commands::tagging_catalog,
            commands::tagging_storage,
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
        .build(context())
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
