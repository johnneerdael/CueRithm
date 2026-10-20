mod builtin;
mod commands;
mod host;
mod maintenance;
mod maintenance_commands;
mod sync_commands;
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
            sync_commands::discover_dj_targets,
            sync_commands::preview_dj_sync,
            sync_commands::discard_dj_sync,
            sync_commands::apply_dj_sync,
            sync_commands::restore_dj_sync,
            sync_commands::dj_sync_history,
            maintenance_commands::maintenance_apply_merge,
            maintenance_commands::maintenance_undo_merge,
            maintenance_commands::maintenance_recovery,
            maintenance_commands::maintenance_retry_recovery,
            maintenance_commands::maintenance_preview_merge,
            maintenance_commands::maintenance_merge_history,
            maintenance_commands::maintenance_cancel,
            maintenance_commands::maintenance_scan_duplicates,
            maintenance_commands::maintenance_scan_unused,
            maintenance_commands::maintenance_scan_broken,
            maintenance_commands::maintenance_duplicate_scan,
            maintenance_commands::maintenance_unused_scan,
            maintenance_commands::maintenance_broken_scan,
            maintenance_commands::maintenance_export_unused,
            maintenance_commands::maintenance_preview_unused_removal,
            maintenance_commands::maintenance_apply_unused_removal,
            maintenance_commands::maintenance_restore_unused_removal,
            maintenance_commands::maintenance_unused_removal_history,
            maintenance_commands::maintenance_retry_unused_recovery,
            maintenance_commands::maintenance_preview_backup_disposal,
            maintenance_commands::maintenance_dispose_backups,
            maintenance_commands::maintenance_backup_disposal_history,
            maintenance_commands::maintenance_retry_backup_disposal,
            maintenance_commands::maintenance_begin_review,
            maintenance_commands::maintenance_get_review,
            maintenance_commands::maintenance_edit_review,
            maintenance_commands::maintenance_ignores,
            maintenance_commands::maintenance_set_ignores,
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
            commands::cleanup_catalog,
            commands::preview_cleanup,
            commands::cleanup_history,
            commands::apply_cleanup,
            commands::undo_cleanup,
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
            commands::player_status,
            commands::playlists_workspace,
            commands::edit_playlist,
            commands::undo_playlist,
            commands::smart_field_catalog,
            commands::preview_smart_list,
            commands::save_smart_playlist,
            commands::preview_playlist_tool,
            commands::apply_playlist_tool,
            commands::preview_smart_generation,
            commands::apply_smart_generation,
            commands::get_playlist_preferences,
            commands::resolve_playlist_calendar_day,
            commands::save_playlist_preferences,
            commands::playlist_members
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
                    host.maintenance.stop();
                    let host = host.inner().clone();
                    let app = app.clone();
                    std::thread::spawn(move || {
                        if let Ok(mut player) = host.player.lock() {
                            let _ = player.pause();
                        }
                        host.preparation.shutdown();
                        host.smart_playlists.shutdown();
                        host.maintenance.wait_until_idle();
                        app.exit(0);
                    });
                }
            }
        });
}
