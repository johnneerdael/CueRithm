//! Native commands for the rekordbox synchronization screen. Every step runs under the maintenance task
//! lease, so it can be cancelled and never overlaps another maintenance operation.
use crate::{
    commands::{AppError, work},
    host::Host,
};
use cuerithm_library::*;
use cuerithm_media::CancellationToken;
use cuerithm_rekordbox::{
    CollectionReadOptions, DatabaseKey, DiscoveryPaths, NativeKeySource, ReadOnlyDatabase,
    WRITE_PROFILES, discover_targets, resolve_database_key,
};
use serde::{Deserialize, Serialize};
use std::{path::PathBuf, sync::Arc};
use tauri::State;
type Managed<'a> = State<'a, Arc<Host>>;
type Ipc<T> = Result<T, AppError>;

async fn run<T: Send + 'static>(
    host: Arc<Host>,
    operation_id: String,
    f: impl FnOnce(&Host, &CancellationToken) -> anyhow::Result<T> + Send + 'static,
) -> Ipc<T> {
    let lease = host.maintenance.begin(&operation_id)?;
    work(move || {
        lease.token.check()?;
        let result = f(&host, &lease.token);
        drop(lease);
        result
    })
    .await
}

/// The installed library's path and its runtime-only key, derived from local files only.
fn context(host: &Host, cancel: &CancellationToken) -> anyhow::Result<(PathBuf, DatabaseKey)> {
    use anyhow::Context;
    let paths = DiscoveryPaths::current()?;
    let found = discover_targets(&paths, cancel)?;
    let target = found
        .installations
        .first()
        .context("rekordbox is not installed")?;
    let database = found
        .database
        .context("No rekordbox library was found")?
        .path;
    let source = NativeKeySource::for_installation(target, paths.platform)?;
    let key = resolve_database_key(
        &source,
        &paths.agent_options_file()?,
        Default::default(),
        cancel,
    )?;
    let _ = host;
    Ok((database, key))
}
fn work_dir(host: &Host, name: &str) -> anyhow::Result<PathBuf> {
    let dir = host.library.data_dir().join("rekordbox").join(name);
    std::fs::create_dir_all(&dir)?;
    Ok(dir)
}

#[derive(Serialize)]
pub struct Installation {
    pub version: String,
    pub major: u32,
}
#[derive(Serialize)]
pub struct Preflight {
    pub platform_supported: bool,
    pub installations: Vec<Installation>,
    pub database_path: Option<String>,
    pub database_state: Option<String>,
    pub key_ready: bool,
    pub key_error: Option<String>,
    pub schema_fingerprint: Option<String>,
    /// The library's schema is one the writer is verified against.
    pub write_supported: bool,
    pub write_profiles: Vec<&'static str>,
    pub targets: Vec<NativeTargetState>,
}
#[tauri::command]
pub async fn rekordbox_preflight(host: Managed<'_>) -> Ipc<Preflight> {
    let host = host.inner().clone();
    work(move || {
        let cancel = CancellationToken::default();
        let paths = DiscoveryPaths::current()?;
        let found = discover_targets(&paths, &cancel)?;
        let mut result = Preflight {
            platform_supported: found.platform_supported,
            installations: found
                .installations
                .iter()
                .map(|i| Installation {
                    version: i.version.clone(),
                    major: i.major,
                })
                .collect(),
            database_path: found
                .database
                .as_ref()
                .map(|d| d.path.display().to_string()),
            database_state: found
                .database
                .as_ref()
                .map(|d| format!("{:?}", d.state).to_lowercase()),
            key_ready: false,
            key_error: None,
            schema_fingerprint: None,
            write_supported: false,
            write_profiles: WRITE_PROFILES.iter().map(|p| p.name).collect(),
            targets: host.library.native_targets()?,
        };
        if found.database.is_some() && !found.installations.is_empty() {
            match context(&host, &cancel) {
                Ok((path, key)) => {
                    result.key_ready = true;
                    // Schema inspection is read-only and does not take the library's lock.
                    match ReadOnlyDatabase::open(&path, &key).and_then(|d| d.schema()) {
                        Ok(schema) => {
                            result.write_supported = WRITE_PROFILES
                                .iter()
                                .any(|p| p.schema_fingerprint == schema.fingerprint);
                            result.schema_fingerprint = Some(schema.fingerprint[..12].to_owned());
                        }
                        Err(error) => result.key_error = Some(format!("{error:#}")),
                    }
                }
                Err(error) => result.key_error = Some(format!("{error:#}")),
            }
        }
        Ok(result)
    })
    .await
}

#[derive(Serialize)]
pub struct RekordboxPlaylist {
    pub id: String,
    pub parent_id: Option<String>,
    pub name: String,
    pub is_folder: bool,
    pub tracks: usize,
}
#[tauri::command]
pub async fn rekordbox_playlists(
    host: Managed<'_>,
    operation_id: String,
) -> Ipc<Vec<RekordboxPlaylist>> {
    run(host.inner().clone(), operation_id, move |host, cancel| {
        let (live, key) = context(host, cancel)?;
        let scratch = work_dir(host, "work")?;
        let snapshot = ReadOnlyDatabase::open(&live, &key)?.capture(&scratch, &key, cancel)?;
        let collection = ReadOnlyDatabase::open(&snapshot.path, &key)?
            .read_collection(CollectionReadOptions::default(), cancel)?;
        Ok(collection
            .playlists
            .iter()
            .filter(|p| p.deleted == Some(false) && matches!(p.attribute, 0 | 1))
            .map(|p| RekordboxPlaylist {
                id: p.id.clone(),
                parent_id: p.parent_id.clone(),
                name: p.name.clone(),
                is_folder: p.attribute == 1,
                tracks: if p.attribute == 1 {
                    0
                } else {
                    collection.playlist_local_track_ids(&p.id).len()
                },
            })
            .collect())
    })
    .await
}

#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ImportScope {
    All,
    Playlist { id: String },
}
/// Link tracks from the installed rekordbox library. New tracks join Incoming for the usual preparation;
/// Cuerithm owns every writable field, the way a primary library does, and an empty Cuerithm value never
/// replaces rekordbox's.
#[tauri::command]
pub async fn rekordbox_import(
    host: Managed<'_>,
    operation_id: String,
    scope: ImportScope,
) -> Ipc<NativeImportReport> {
    run(host.inner().clone(), operation_id, move |host, cancel| {
        let (live, key) = context(host, cancel)?;
        let scratch = work_dir(host, "work")?;
        let snapshot = ReadOnlyDatabase::open(&live, &key)?.capture(&scratch, &key, cancel)?;
        let collection = ReadOnlyDatabase::open(&snapshot.path, &key)?
            .read_collection(CollectionReadOptions::default(), cancel)?;
        let ids: std::collections::BTreeSet<String> = match scope {
            ImportScope::All => collection
                .tracks
                .iter()
                .filter(|t| t.importable_local())
                .map(|t| t.identity.content_id.clone())
                .collect(),
            ImportScope::Playlist { id } => collection
                .playlist_local_track_ids(&id)
                .into_iter()
                .collect(),
        };
        anyhow::ensure!(
            !ids.is_empty(),
            "There are no importable tracks in that selection"
        );
        host.library.import_rekordbox_collection(
            &collection,
            NativeImportOptions {
                settings_revision: host.library.settings()?.revision,
                content_ids: Some(ids),
                initial_owners: native_writable_fields()
                    .into_iter()
                    .map(|f| (f.to_owned(), NativeFieldOwner::Local))
                    .collect(),
            },
            cancel,
        )
    })
    .await
}

#[tauri::command]
pub async fn rekordbox_prepare(
    host: Managed<'_>,
    operation_id: String,
    add_new_tracks: Option<bool>,
) -> Ipc<NativeSyncPrepared> {
    let options = NativeSyncOptions {
        add_new_tracks: add_new_tracks.unwrap_or(false),
    };
    run(host.inner().clone(), operation_id, move |host, cancel| {
        let (live, key) = context(host, cancel)?;
        host.library
            .native_sync_prepare(&live, &key, &work_dir(host, "work")?, options, cancel)
    })
    .await
}

#[derive(Serialize)]
pub struct ApplyOutcome {
    pub run: Option<NativeSyncRun>,
    /// Set when rekordbox changed a track or playlist after it was read; nothing was written.
    pub conflicts: Vec<NativeSyncConflict>,
}
#[tauri::command]
pub async fn rekordbox_apply(
    host: Managed<'_>,
    operation_id: String,
    add_new_tracks: Option<bool>,
) -> Ipc<ApplyOutcome> {
    let options = NativeSyncOptions {
        add_new_tracks: add_new_tracks.unwrap_or(false),
    };
    run(host.inner().clone(), operation_id, move |host, cancel| {
        let (live, key) = context(host, cancel)?;
        match host.library.native_sync_apply(
            &live,
            &key,
            &work_dir(host, "work")?,
            &work_dir(host, "backups")?,
            options,
            cancel,
        ) {
            Ok(run) => Ok(ApplyOutcome {
                run: Some(run),
                conflicts: vec![],
            }),
            Err(error) => match native_sync_conflicts(&error) {
                Some(conflicts) => Ok(ApplyOutcome {
                    run: None,
                    conflicts,
                }),
                None => Err(error),
            },
        }
    })
    .await
}

#[tauri::command]
pub async fn rekordbox_history(host: Managed<'_>) -> Ipc<Vec<NativeSyncRun>> {
    let lib = host.library.clone();
    work(move || lib.native_sync_history()).await
}
