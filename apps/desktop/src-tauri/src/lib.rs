mod builtin;
mod commands;
mod curation_commands;
mod exit;
mod exit_commands;
mod host;
mod maintenance;
mod maintenance_commands;
mod match_review_commands;
mod metadata_commands;
mod organization_commands;
mod preparation_preset_commands;
mod provider_commands;
mod sync_commands;
use anyhow::Result;
use std::{path::PathBuf, sync::Arc, time::Duration};
use tauri::{Emitter, Manager};

fn context() -> tauri::Context<tauri::Wry> {
    tauri::generate_context!()
}

/// Native automation shares packaged resources and provider identity without opening a window.
pub fn automation_context(
    library: &cuerithm_library::Library,
) -> Result<(
    cuerithm_media::MediaRuntime,
    Arc<cuerithm_autotag::CatalogRuntime>,
)> {
    let context = context();
    let resources =
        tauri::utils::platform::resource_dir(context.package_info(), &Default::default())?
            .join("media");
    let runtime = cuerithm_media::MediaRuntime::from_bundle(&resources)?;
    builtin::BuiltinModels::install(library.data_dir())?.configure_library(library)?;
    let providers = library.provider_runtime(&context.package_info().version.to_string())?;
    Ok((runtime, providers.runtime().clone()))
}

pub fn diagnostics() -> Result<()> {
    let context = context();
    let folder = tauri::utils::platform::resource_dir(context.package_info(), &Default::default())?
        .join("media");
    let runtime = cuerithm_media::MediaRuntime::from_bundle(&folder)?;
    builtin::BuiltinModels::verify()?;
    println!(
        "{}",
        serde_json::json!({"application":"Cuerithm","version":context.package_info().version.to_string(),"decoder":runtime.identity,"resources_verified":true,"built_in_analysis_verified":true})
    );
    Ok(())
}
pub(crate) fn start_shutdown(app: tauri::AppHandle, host: Arc<host::Host>) {
    if !host.alive.swap(false, std::sync::atomic::Ordering::Relaxed) {
        return;
    }
    host.maintenance.stop();
    std::thread::spawn(move || {
        if let Ok(mut player) = host.player.lock() {
            let _ = player.pause();
        }
        if let Ok(worker) = host.curation_worker.lock() {
            worker.request_shutdown();
        }
        host.preparation.shutdown();
        if let Ok(mut worker) = host.curation_worker.lock() {
            worker.shutdown();
        }
        host.smart_playlists.shutdown();
        host.maintenance.wait_until_idle();
        app.exit(0);
    });
}
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let root = if let Some(root) = std::env::var_os("CUERITHM_DATA_DIR") {
                PathBuf::from(root)
            } else {
                app.path().app_data_dir()?
            };
            let runtime = if cfg!(debug_assertions) {
                if let Some(runtime) = std::env::var_os("CUERITHM_MEDIA_RUNTIME") {
                    PathBuf::from(runtime)
                } else {
                    app.path().resource_dir()?.join("media")
                }
            } else {
                app.path().resource_dir()?.join("media")
            };
            let initial_folder = app
                .path()
                .audio_dir()
                .ok()
                .map(|p| p.join("Cuerithm/Incoming"));
            let host = Arc::new(host::Host::open_with_release(
                &root,
                &runtime,
                initial_folder,
                &app.package_info().version.to_string(),
            )?);
            let events = host.preparation.subscribe();
            let handle = app.handle().clone();
            std::thread::spawn(move || {
                for event in events {
                    let settled = matches!(
                        event.state.as_str(),
                        "completed" | "attention" | "superseded"
                    );
                    let _ = handle.emit("preparation", event);
                    if settled {
                        // Incoming overrides can settle after their invoke has
                        // returned. Refresh saved receipts and access projections
                        // without treating unrelated reviews as content changes.
                        let _ = handle.emit("metadata-edit-change", ());
                        let _ = handle.emit("catalog-access-change", ());
                    }
                }
            });
            let events = host.curation.subscribe();
            let handle = app.handle().clone();
            std::thread::spawn(move || {
                for event in events {
                    let track_ids = event
                        .track_id
                        .as_ref()
                        .map(|id| vec![id.clone()])
                        .unwrap_or_default();
                    let _ = handle.emit("curation-change", &event);
                    let _ = handle.emit("metadata-edit-change", ());
                    let _ =
                        handle.emit("library-change", serde_json::json!({"track_ids":track_ids}));
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
            provider_commands::provider_setup_statuses,
            provider_commands::save_provider_credential,
            provider_commands::remove_provider_credential,
            provider_commands::test_provider_credential,
            provider_commands::open_catalog_source,
            curation_commands::curation_catalog,
            curation_commands::edit_curation_taxonomy,
            curation_commands::curation_snapshot,
            curation_commands::update_curation,
            curation_commands::reexport_curation,
            curation_commands::curation_pending_page,
            curation_commands::curation_history_page,
            curation_commands::curation_publication,
            curation_commands::retry_curation,
            curation_commands::discard_curation,
            curation_commands::undo_curation,
            preparation_preset_commands::preparation_presets,
            preparation_preset_commands::preparation_preset,
            preparation_preset_commands::save_preparation_preset,
            preparation_preset_commands::duplicate_preparation_preset,
            preparation_preset_commands::delete_preparation_preset,
            preparation_preset_commands::preview_preparation_preset_import,
            preparation_preset_commands::import_preparation_preset,
            preparation_preset_commands::export_preparation_preset,
            preparation_preset_commands::project_preparation_preset,
            preparation_preset_commands::overlay_preparation_preset,
            preparation_preset_commands::preparation_preset_folders,
            preparation_preset_commands::choose_preparation_preset_import,
            preparation_preset_commands::save_preparation_preset_export,
            exit_commands::register_exit_guard,
            exit_commands::unregister_exit_guard,
            exit_commands::complete_exit,
            exit_commands::cancel_exit,
            metadata_commands::metadata_editor_snapshot,
            metadata_commands::create_metadata_draft,
            metadata_commands::metadata_draft,
            metadata_commands::metadata_drafts,
            metadata_commands::update_metadata_draft,
            metadata_commands::preview_metadata_edits,
            metadata_commands::apply_metadata_edits,
            metadata_commands::recover_metadata_edits,
            metadata_commands::undo_metadata_edits,
            metadata_commands::discard_metadata_draft,
            metadata_commands::metadata_edit_history,
            metadata_commands::metadata_edit_history_page,
            metadata_commands::metadata_edit_batch,
            metadata_commands::choose_metadata_artwork,
            metadata_commands::extract_metadata_artwork,
            metadata_commands::metadata_artwork_asset,
            match_review_commands::track_match_review,
            match_review_commands::preview_match_review,
            match_review_commands::apply_match_review,
            match_review_commands::requery_matches,
            match_review_commands::match_review_artwork,
            organization_commands::organization_roots,
            organization_commands::choose_organization_root,
            organization_commands::organization_example,
            organization_commands::preview_organization,
            organization_commands::organization_preview_page,
            organization_commands::discard_organization_preview,
            organization_commands::apply_organization,
            organization_commands::cancel_organization,
            organization_commands::cancel_organization_batch,
            organization_commands::organization_history,
            organization_commands::organization_batch_page,
            organization_commands::recover_organization,
            organization_commands::undo_organization,
            organization_commands::organization_alternates,
            organization_commands::resolve_organization_alternate,
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
            commands::reset_catalog_access,
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
                match host.exit.request(!app.webview_windows().is_empty()) {
                    Ok(exit::ExitAction::Notify(id)) => {
                        if app
                            .emit("app-exit-requested", serde_json::json!({"request_id":id}))
                            .is_err()
                        {
                            let _ = host.exit.cancel(&id);
                        }
                    }
                    Ok(exit::ExitAction::Shutdown) => {
                        start_shutdown(app.clone(), host.inner().clone())
                    }
                    Ok(exit::ExitAction::Waiting) => {}
                    Err(error) => {
                        let _ = app.emit("app-exit-error", error.to_string());
                    }
                }
            }
        });
}
