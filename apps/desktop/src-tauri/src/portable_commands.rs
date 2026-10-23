use crate::{
    commands::{AppError, work},
    host::Host,
};
use anyhow::{Context, Result, ensure};
use cuerithm_library::{
    Library, LibraryGeneration,
    backup::{self, ArchiveLimits, SpoolArchive},
};
use cuerithm_media::{CancellationToken, file_sha256_cancellable};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::{Path, PathBuf},
    sync::{Arc, Mutex, atomic::Ordering},
};
use tauri::{Emitter, Manager, State};
type Managed<'a> = State<'a, Arc<Host>>;
type Ipc<T> = Result<T, AppError>;
#[derive(Default)]
pub struct PortableState {
    previews: Mutex<BTreeMap<String, Preview>>,
    pub gate: Mutex<()>,
}
#[derive(Clone)]
struct Preview {
    path: PathBuf,
    digest: String,
    time_zone: String,
    created: i64,
    revision: u64,
    active: String,
    lexicon: bool,
    losses: Vec<String>,
}
#[derive(Clone, Serialize)]
pub struct Progress<'a> {
    pub task_id: &'a str,
    pub phase: &'a str,
    pub completed: usize,
    pub total: Option<usize>,
    pub bytes: u64,
}
pub(crate) fn progress(
    app: &tauri::AppHandle,
    id: &str,
    phase: &str,
    completed: usize,
    total: Option<usize>,
) {
    let _ = app.emit(
        "portable-progress",
        Progress {
            task_id: id,
            phase,
            completed,
            total,
            bytes: 0,
        },
    );
}
#[derive(Serialize)]
pub struct ExportResult {
    path: PathBuf,
    tracks: usize,
    size_bytes: u64,
}
#[derive(Serialize)]
pub struct Root {
    id: String,
    label: String,
    source_path: String,
    track_count: usize,
}
#[derive(Serialize)]
pub struct Inspection {
    preview_id: String,
    source_format: String,
    source_profile: String,
    created: Option<i64>,
    tracks: usize,
    playlists: usize,
    cues: usize,
    roots: Vec<Root>,
    warnings: Vec<String>,
    losses: Vec<String>,
    source_digest: String,
}
#[derive(Deserialize)]
pub struct RootMapping {
    root_id: String,
    destination: Option<PathBuf>,
}
#[derive(Deserialize)]
pub struct RestoreRequest {
    preview_id: String,
    source_digest: String,
    root_mappings: Vec<RootMapping>,
    acknowledge_losses: bool,
    replace_generation: String,
}
#[derive(Serialize)]
pub struct RestoreResult {
    pub active_generation: String,
    pub retained_generation: String,
    pub tracks: usize,
    pub unresolved_tracks: usize,
    pub warnings: Vec<String>,
    pub restart_required: bool,
}
#[derive(Serialize)]
pub struct Generations {
    active: String,
    retained: Vec<LibraryGeneration>,
}
fn reporting<'a>(app: &'a tauri::AppHandle, id: &'a str) -> impl Fn(&str, u64) -> Result<()> + 'a {
    let last = std::cell::Cell::new(std::time::Instant::now() - std::time::Duration::from_secs(1));
    move |phase, bytes| {
        if last.get().elapsed() >= std::time::Duration::from_millis(250) {
            let (phase, is_bytes) = match phase {
                "write" => ("Writing archive", true),
                "read" => ("Verifying archive", true),
                "extract" => ("Reading source archive", true),
                "source_digest" => ("Checking source archive", true),
                "write_optional" => ("Writing optional library data", true),
                "materialize" => ("Opening source library", false),
                "settings" => ("Reading library settings", false),
                "source_history" => ("Reading source history", false),
                "resize_snapshot_page" => ("Collecting large track records", false),
                "validate_tree" => ("Checking playlist folders", false),
                "read_tables" => ("Validating source library", true),
                "tracks" | "project" => ("Collecting tracks", false),
                "mapping_roots" => ("Grouping music folders", false),
                "project_shared" => ("Collecting playlists and settings", false),
                "validate_relationships" => ("Checking library relationships", false),
                "cues" => ("Reading saved cues", false),
                "tempo_markers" => ("Reading tempo markers", false),
                "playlists" => ("Reading playlists", false),
                "snapshot" => ("Snapshotting library", false),
                "validate" | "validate_summary" | "validate_selection" => {
                    ("Checking library data", false)
                }
                "history" => ("Reading saved history", false),
                "restore_tracks" => ("Restoring track data", false),
                "restore_domains" => ("Restoring playlists and settings", false),
                "publish" => ("Publishing verified archive", false),
                _ => ("Processing library data", false),
            };
            let _ = app.emit(
                "portable-progress",
                Progress {
                    task_id: id,
                    phase,
                    completed: usize::try_from(bytes).unwrap_or(usize::MAX),
                    total: None,
                    bytes: if is_bytes { bytes } else { 0 },
                },
            );
            last.set(std::time::Instant::now());
        }
        Ok(())
    }
}
pub(crate) fn export(
    host: &Host,
    destination: &Path,
    include_history: bool,
    cancel: &CancellationToken,
) -> Result<ExportResult> {
    export_with_progress(host, destination, include_history, cancel, &|_, _| Ok(()))
}
fn export_with_progress(
    host: &Host,
    destination: &Path,
    include_history: bool,
    cancel: &CancellationToken,
    control: &dyn Fn(&str, u64) -> Result<()>,
) -> Result<ExportResult> {
    cancel.check()?;
    let tick = |phase: &str, bytes: u64| {
        cancel.check()?;
        control(phase, bytes)
    };
    let archive = host
        .library
        .portable_snapshot_spooled(&ArchiveLimits::default(), &tick)?;
    if !include_history {
        archive.retain_selected_history()?;
    }
    let tracks = usize::try_from(archive.count("tracks")?)?;
    cancel.check()?;
    backup::write_portable_spooled(destination, &archive, &ArchiveLimits::default(), &tick)?;
    Ok(ExportResult {
        path: destination.into(),
        tracks,
        size_bytes: std::fs::metadata(destination)?.len(),
    })
}
#[cfg(test)]
fn read(
    path: &Path,
    time_zone: &str,
    lexicon: bool,
    cancel: &CancellationToken,
) -> Result<(SpoolArchive, Vec<String>, Vec<String>)> {
    read_with_progress(path, time_zone, lexicon, cancel, &|_, _| Ok(()))
}
fn read_with_progress(
    path: &Path,
    time_zone: &str,
    lexicon: bool,
    cancel: &CancellationToken,
    control: &dyn Fn(&str, u64) -> Result<()>,
) -> Result<(SpoolArchive, Vec<String>, Vec<String>)> {
    let tick = |phase: &str, bytes: u64| {
        cancel.check()?;
        control(phase, bytes)
    };
    if lexicon {
        let preview =
            backup::read_lexicon_spooled(path, time_zone, &ArchiveLimits::default(), &tick)?;
        let losses = preview.report.excluded.iter().map(exclusion).collect();
        Ok((preview.archive, preview.report.warnings, losses))
    } else {
        let a = backup::read_portable_spooled(path, &ArchiveLimits::default(), &tick)?;
        let losses = a.manifest.excluded.iter().map(exclusion).collect();
        Ok((a, vec![], losses))
    }
}
fn exclusion(v: &serde_json::Value) -> String {
    format!(
        "{}: {}",
        v["domain"].as_str().unwrap_or("Excluded data"),
        v["reason"].as_str().unwrap_or("Not restored")
    )
}
fn encoded_display(value: &serde_json::Value) -> String {
    serde_json::from_value::<cuerithm_library::EncodedPath>(value.clone())
        .ok()
        .and_then(|p| {
            let bytes = p.bytes().ok()?;
            if p.encoding == cuerithm_library::PathEncoding::Base64Utf16le {
                Some(String::from_utf16_lossy(
                    &bytes
                        .as_chunks::<2>()
                        .0
                        .iter()
                        .map(|b| u16::from_le_bytes([b[0], b[1]]))
                        .collect::<Vec<_>>(),
                ))
            } else {
                Some(String::from_utf8_lossy(&bytes).into())
            }
        })
        .unwrap_or_else(|| "Opaque source path".into())
}
fn roots(a: &SpoolArchive) -> Result<Vec<Root>> {
    let mut out = BTreeMap::<String, Root>::new();
    a.for_each("roots", &mut |r| {
        out.insert(
            r.id.clone(),
            Root {
                id: r.id,
                label: r.value["label"].as_str().unwrap_or("Music folder").into(),
                source_path: encoded_display(&r.value["source_root"]),
                track_count: 0,
            },
        );
        Ok(())
    })?;
    a.for_each("tracks", &mut |track| {
        if let Some(locator) = a.record(
            "locators",
            track.value["locator_id"]
                .as_str()
                .context("Missing locator ID")?,
        )? && let Some(root) = locator.value["root_id"]
            .as_str()
            .and_then(|id| out.get_mut(id))
        {
            root.track_count += 1;
        }
        Ok(())
    })?;
    Ok(out.into_values().collect())
}
fn cue_count(a: &SpoolArchive) -> Result<usize> {
    let mut count = 0;
    a.for_each("musical_revisions", &mut |r| {
        if r.value["selected"] == true {
            count += r.value["cues"].as_array().map_or(0, Vec::len);
        }
        Ok(())
    })?;
    Ok(count)
}
#[tauri::command]
pub async fn portable_export(
    host: Managed<'_>,
    app: tauri::AppHandle,
    task_id: String,
    destination: PathBuf,
    include_history: bool,
) -> Ipc<ExportResult> {
    let host = host.inner().clone();
    let lease = host.maintenance.begin(&task_id)?;
    work(move || {
        let _gate = host
            .portable
            .gate
            .lock()
            .map_err(|_| anyhow::anyhow!("Library operation interrupted"))?;
        let _operation = host.library.exclusive_operation()?;
        host.library.ensure_portable_operation_safe()?;
        progress(&app, &task_id, "Saving library data", 0, None);
        let r = export_with_progress(
            &host,
            &destination,
            include_history,
            &lease.token,
            &reporting(&app, &task_id),
        );
        drop(lease);
        r
    })
    .await
}
#[tauri::command]
pub async fn portable_inspect(
    host: Managed<'_>,
    app: tauri::AppHandle,
    task_id: String,
    path: PathBuf,
    import_time_zone: String,
) -> Ipc<Inspection> {
    let host = host.inner().clone();
    let lease = host.maintenance.begin(&task_id)?;
    work(move || {
        lease.token.check()?;
        progress(&app, &task_id, "Validating archive", 0, None);
        let digest = file_sha256_cancellable(&path, &lease.token)?;
        let lexicon = backup::detect_archive_format(&path, &ArchiveLimits::default())?
            == backup::ArchiveFormat::Lexicon;
        let (a, warnings, losses) = read_with_progress(
            &path,
            &import_time_zone,
            lexicon,
            &lease.token,
            &reporting(&app, &task_id),
        )?;
        lease.token.check()?;
        ensure!(
            file_sha256_cancellable(&path, &lease.token)? == digest,
            "Source archive changed during inspection"
        );
        let id = uuid::Uuid::new_v4().to_string();
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_millis()
            .min(i64::MAX as u128) as i64;
        let result = Inspection {
            preview_id: id.clone(),
            source_format: if lexicon { "lexicon" } else { "cuerithm" }.into(),
            source_profile: if lexicon {
                "lexicon-main-db-v26"
            } else {
                "cuerithm-library-v1"
            }
            .into(),
            created: chrono::DateTime::parse_from_rfc3339(&a.manifest.created_at)
                .ok()
                .map(|d| d.timestamp_millis()),
            tracks: usize::try_from(a.count("tracks")?)?,
            playlists: usize::try_from(a.count("playlists")?)?,
            cues: cue_count(&a)?,
            roots: roots(&a)?,
            warnings,
            losses: losses.clone(),
            source_digest: digest.clone(),
        };
        let mut previews = host
            .portable
            .previews
            .lock()
            .map_err(|_| anyhow::anyhow!("Preview state interrupted"))?;
        previews.retain(|_, p| now - p.created < 86_400_000);
        ensure!(
            previews.len() < 16,
            "Close existing archive previews before opening more"
        );
        previews.insert(
            id,
            Preview {
                path,
                digest,
                time_zone: import_time_zone,
                created: now,
                revision: host.library.revision()?,
                active: host.libraries.active_id()?,
                lexicon,
                losses,
            },
        );
        Ok(result)
    })
    .await
}
fn resolve_roots(
    library: &Library,
    a: &SpoolArchive,
    mappings: &[RootMapping],
    host: &Host,
    cancel: &CancellationToken,
) -> Result<(usize, Vec<String>)> {
    let mut seen = BTreeSet::new();
    let mut mappings_by_id = BTreeMap::new();
    for m in mappings {
        ensure!(seen.insert(&m.root_id), "Duplicate root mapping");
        ensure!(
            a.record("roots", &m.root_id)?.is_some(),
            "Unknown root mapping"
        );
        if let Some(path) = &m.destination {
            ensure!(
                path.is_absolute() && path.is_dir(),
                "Destination root is unavailable"
            );
            for ancestor in path.ancestors() {
                ensure!(
                    !std::fs::symlink_metadata(ancestor)?
                        .file_type()
                        .is_symlink(),
                    "Root contains a symbolic-link parent"
                );
            }
            mappings_by_id.insert(&m.root_id, path.canonicalize()?);
        }
    }
    let mut connected = 0;
    let mut warnings = vec![];
    let mut bound_files: BTreeMap<u64, Vec<PathBuf>> = BTreeMap::new();
    a.for_each("tracks",&mut |track| {
        cancel.check()?;
        let locator = a.record("locators",track.value["locator_id"].as_str().context("Missing locator ID")?)?.context("Locator disappeared")?;
        let Some(root_id) = locator.value["root_id"].as_str() else {
            return Ok(());
        };
        let Some(root) = mappings_by_id.get(&root_id.to_string()) else {
            return Ok(());
        };
        let portable = library
            .source_locator(&track.id)?
            .context("No portable locator")?;
        let Some(components) = portable.components else {
            return Ok(());
        };
        let mut target = root.clone();
        let mut representable = true;
        for component in components {
            let bytes = component.bytes()?;
            #[cfg(unix)]
            {
                if component.encoding == cuerithm_library::PathEncoding::Base64Utf16le {
                    let units = bytes
                        .as_chunks::<2>().0.iter()
                        .map(|b| u16::from_le_bytes([b[0], b[1]]))
                        .collect::<Vec<_>>();
                    if let Ok(s) = String::from_utf16(&units) {
                        target.push(s);
                    } else {
                        representable = false;
                        break;
                    }
                } else {
                    use std::os::unix::ffi::OsStringExt;
                    target.push(std::ffi::OsString::from_vec(bytes));
                }
            }
            #[cfg(windows)]
            {
                if !component.windows_filename_compatible()? {representable=false;break;}
                if component.encoding == cuerithm_library::PathEncoding::Base64Bytes {
                    if let Ok(s) = String::from_utf8(bytes) {
                        target.push(s);
                    } else {
                        representable = false;
                        break;
                    }
                } else if component.encoding == cuerithm_library::PathEncoding::Base64Utf16le {
                    use std::os::windows::ffi::OsStringExt;
                    target.push(std::ffi::OsString::from_wide(
                        &bytes
                            .as_chunks::<2>().0.iter()
                            .map(|b| u16::from_le_bytes([b[0], b[1]]))
                            .collect::<Vec<_>>(),
                    ));
                } else {
                    target.push(std::str::from_utf8(&bytes)?);
                }
            }
        }
        if !representable || !target.is_file() {
            return Ok(());
        }
        let mut safe = true;
        for ancestor in target.ancestors() {
            if std::fs::symlink_metadata(ancestor).is_ok_and(|m| m.file_type().is_symlink()) {
                safe = false;
                break;
            }
        }
        if !safe || !target.canonicalize()?.starts_with(root) {
            warnings.push("A destination alias was left unresolved.".into());
            return Ok(());
        }
        use std::hash::{Hash,Hasher};
        let mut hasher=std::collections::hash_map::DefaultHasher::new();same_file::Handle::from_path(&target)?.hash(&mut hasher);let identity=hasher.finish();
        if bound_files.get(&identity).is_some_and(|paths|paths.iter().any(|p|same_file::is_same_file(p,&target).unwrap_or(false)))
        {
            warnings.push("Destination collision left one track unresolved.".into());
            return Ok(());
        }
        let hash = file_sha256_cancellable(&target, cancel)?;
        if let Some(expected) = track.value["content"]["sha256"].as_str()
            && hash != expected {
                warnings.push("A changed destination file was left unresolved; use Relocate for an explicit replacement.".into());
                return Ok(());
            }
        if host.runtime.probe_cancellable(&target, cancel).is_err() {
            cancel.check()?;
            warnings.push("Unsupported destination audio was left unresolved.".into());
            return Ok(());
        }
        ensure!(
            file_sha256_cancellable(&target, cancel)? == hash,
            "Destination changed during binding"
        );
        let canonical_target=target.canonicalize()?;
        ensure!(canonical_target==target,"Destination parent changed during binding");
        library.bind_portable_track_in_root(&track.id,&canonical_target,root,&hash,cancel)?;
        bound_files.entry(identity).or_default().push(target);
        connected += 1;Ok(())
    })?;
    warnings.sort();
    warnings.dedup();
    Ok((connected, warnings))
}
fn request_restart(app: &tauri::AppHandle, host: &Arc<Host>) -> Result<()> {
    host.restart_requested.store(true, Ordering::Release);
    match host.exit.request(!app.webview_windows().is_empty())? {
        crate::exit::ExitAction::Notify(id) => {
            app.emit("app-exit-requested", serde_json::json!({"request_id":id}))?;
        }
        crate::exit::ExitAction::Shutdown => crate::start_shutdown(app.clone(), host.clone()),
        crate::exit::ExitAction::Waiting => {}
    }
    Ok(())
}
#[tauri::command]
pub async fn portable_restore(
    host: Managed<'_>,
    app: tauri::AppHandle,
    task_id: String,
    request: RestoreRequest,
) -> Ipc<RestoreResult> {
    let host = host.inner().clone();
    let lease = host.maintenance.begin(&task_id)?;
    let restart_host = host.clone();
    let restart_app = app.clone();
    let result = work(move || {
        let _gate = host
            .portable
            .gate
            .lock()
            .map_err(|_| anyhow::anyhow!("Library operation interrupted"))?;
        let p = host
            .portable
            .previews
            .lock()
            .map_err(|_| anyhow::anyhow!("Preview state interrupted"))?
            .get(&request.preview_id)
            .cloned()
            .context("Preview expired; inspect again")?;
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_millis() as i64;
        ensure!(
            now - p.created < 86_400_000
                && host.library.revision()? == p.revision
                && host.libraries.active_id()? == p.active
                && request.replace_generation == p.active,
            "Library changed; inspect again"
        );
        ensure!(
            p.digest == request.source_digest
                && file_sha256_cancellable(&p.path, &lease.token)? == p.digest,
            "Source backup changed; inspect again"
        );
        ensure!(
            p.losses.is_empty() || request.acknowledge_losses,
            "Acknowledge compatibility losses before replacing the library"
        );
        progress(&app, &task_id, "Building replacement library", 0, None);
        let (a, mut warnings, _) = read_with_progress(
            &p.path,
            &p.time_zone,
            p.lexicon,
            &lease.token,
            &reporting(&app, &task_id),
        )?;
        lease.token.check()?;
        ensure!(
            file_sha256_cancellable(&p.path, &lease.token)? == p.digest,
            "Source backup changed while staging; inspect again"
        );
        let stage = host.libraries.create_staging()?;
        let built = (|| -> Result<RestoreResult> {
            let library = Library::open(&stage.database)?;
            let report = reporting(&app, &task_id);
            library.restore_spooled(&a, &ArchiveLimits::default(), &|phase, count| {
                lease.token.check()?;
                report(phase, count)
            })?;
            let (connected, issues) =
                resolve_roots(&library, &a, &request.root_mappings, &host, &lease.token)?;
            warnings.extend(issues);
            drop(library);
            progress(
                &app,
                &task_id,
                "Verifying replacement library",
                usize::try_from(a.count("tracks")?)?,
                Some(usize::try_from(a.count("tracks")?)?),
            );
            let seal = host.libraries.seal(&stage.id)?;
            lease.token.check()?;
            let _operation = host.library.exclusive_operation()?;
            host.library.ensure_portable_operation_safe()?;
            ensure!(
                host.library.revision()? == p.revision,
                "Library changed while staging; inspect again"
            );
            host.libraries.schedule(&stage.id, &p.active, &seal)?;
            host.portable
                .previews
                .lock()
                .map_err(|_| anyhow::anyhow!("Preview state interrupted"))?
                .remove(&request.preview_id);
            Ok(RestoreResult {
                active_generation: stage.id.clone(),
                retained_generation: p.active,
                tracks: usize::try_from(a.count("tracks")?)?,
                unresolved_tracks: usize::try_from(a.count("tracks")?)? - connected,
                warnings,
                restart_required: true,
            })
        })();
        if built.is_err() {
            let _ = std::fs::remove_dir_all(stage.database.parent().unwrap());
        }
        drop(lease);
        built
    })
    .await?;
    request_restart(&restart_app, &restart_host)?;
    Ok(result)
}
#[tauri::command]
pub async fn portable_generations(host: Managed<'_>) -> Ipc<Generations> {
    let host = host.inner().clone();
    work(move || {
        Ok(Generations {
            active: host.libraries.active_id()?,
            retained: host.libraries.retained()?,
        })
    })
    .await
}
#[tauri::command]
pub async fn portable_activate_generation(
    host: Managed<'_>,
    app: tauri::AppHandle,
    task_id: String,
    generation_id: String,
    expected_active: String,
) -> Ipc<RestoreResult> {
    let host = host.inner().clone();
    let lease = host.maintenance.begin(&task_id)?;
    let restart_host = host.clone();
    let result = work(move || {
        let _gate = host
            .portable
            .gate
            .lock()
            .map_err(|_| anyhow::anyhow!("Library operation interrupted"))?;
        let _operation = host.library.exclusive_operation()?;
        host.library.ensure_portable_operation_safe()?;
        lease.token.check()?;
        ensure!(
            host.libraries.active_id()? == expected_active,
            "Active library changed"
        );
        ensure!(
            host.libraries
                .retained()?
                .iter()
                .any(|g| g.id == generation_id),
            "Retained generation not found"
        );
        let seal = host.libraries.seal(&generation_id)?;
        let (tracks, unresolved_tracks) = host.libraries.track_counts(&generation_id)?;
        lease.token.check()?;
        host.libraries
            .schedule(&generation_id, &expected_active, &seal)?;
        Ok(RestoreResult {
            active_generation: generation_id,
            retained_generation: expected_active,
            tracks: usize::try_from(tracks)?,
            unresolved_tracks: usize::try_from(unresolved_tracks)?,
            warnings: vec![
                "Returning to a retained library restores its local configuration.".into(),
            ],
            restart_required: true,
        })
    })
    .await?;
    request_restart(&app, &restart_host)?;
    Ok(result)
}
#[tauri::command]
pub fn portable_cancel(host: Managed<'_>, task_id: String) -> Ipc<bool> {
    host.maintenance.cancel(&task_id).map_err(Into::into)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn frontend_restore_request_preserves_unmapped_roots_and_exact_preview_identity() {
        let request: RestoreRequest = serde_json::from_value(json!({
            "preview_id":"review-1","source_digest":"digest-1",
            "root_mappings":[{"root_id":"offline","destination":null},{"root_id":"music","destination":"/music"}],
            "acknowledge_losses":true,"replace_generation":"current-generation"
        })).unwrap();
        assert_eq!(request.preview_id, "review-1");
        assert_eq!(request.source_digest, "digest-1");
        assert_eq!(request.replace_generation, "current-generation");
        assert!(request.root_mappings[0].destination.is_none());
        assert_eq!(
            request.root_mappings[1].destination.as_deref(),
            Some(Path::new("/music"))
        );
        assert!(request.acknowledge_losses);
        assert!(
            serde_json::from_value::<RestoreRequest>(json!({"preview_id":"review-1"})).is_err()
        );
    }

    #[test]
    fn progress_and_restart_receipt_match_the_frontend_wire_contract() {
        let progress = serde_json::to_value(Progress {
            task_id: "task",
            phase: "Validating archive",
            completed: 0,
            total: Some(0),
            bytes: 12,
        })
        .unwrap();
        assert_eq!(
            progress,
            json!({"task_id":"task","phase":"Validating archive","completed":0,"total":0,"bytes":12})
        );
        let result = serde_json::to_value(RestoreResult {
            active_generation: "next".into(),
            retained_generation: "current".into(),
            tracks: 0,
            unresolved_tracks: 0,
            warnings: vec![],
            restart_required: true,
        })
        .unwrap();
        assert_eq!(result["restart_required"], true);
        assert_eq!(result["tracks"], 0);
        assert_eq!(result["active_generation"], "next");
        assert_eq!(result["retained_generation"], "current");
    }

    #[test]
    fn empty_portable_archives_work_with_zip_extension_and_do_not_need_import_timezone() {
        let folder = tempfile::tempdir().unwrap();
        let path = folder.path().join("renamed.zip");
        let library = Library::open(&folder.path().join("library.sqlite")).unwrap();
        let archive = library.portable_snapshot().unwrap();
        backup::write_portable_archive(&path, &archive, &ArchiveLimits::default()).unwrap();
        assert_eq!(
            backup::detect_archive_format(&path, &ArchiveLimits::default()).unwrap(),
            backup::ArchiveFormat::Portable
        );
        let (readback, _, _) = read(
            &path,
            "not-a-timezone",
            false,
            &CancellationToken::default(),
        )
        .unwrap();
        assert_eq!(readback.count("tracks").unwrap(), 0);
        let observed = std::cell::Cell::new(0u64);
        read_with_progress(
            &path,
            "UTC",
            false,
            &CancellationToken::default(),
            &|_, bytes| {
                observed.set(observed.get().max(bytes));
                Ok(())
            },
        )
        .unwrap();
        assert!(
            observed.get() > 0,
            "Reader must publish advancing byte counts even for an empty track library"
        );
        assert!(roots(&readback).unwrap().is_empty());
        assert_eq!(cue_count(&readback).unwrap(), 0);
        assert_eq!(readback.count("settings").unwrap(), 1);
    }

    #[test]
    fn cancelled_portable_reads_fail_before_returning_an_inspection() {
        let folder = tempfile::tempdir().unwrap();
        let path = folder.path().join("library.cuerithm");
        let library = Library::open(&folder.path().join("library.sqlite")).unwrap();
        backup::write_portable_archive(
            &path,
            &library.portable_snapshot().unwrap(),
            &ArchiveLimits::default(),
        )
        .unwrap();
        let cancel = CancellationToken::default();
        cancel.cancel();
        assert!(read(&path, "UTC", false, &cancel).is_err());
    }

    #[test]
    fn source_path_display_decodes_foreign_utf16_without_host_parsing() {
        use base64::Engine;
        let bytes = "D:\\Music\\House"
            .encode_utf16()
            .flat_map(u16::to_le_bytes)
            .collect::<Vec<_>>();
        let encoded = json!({"encoding":"base64-utf16le","data":base64::engine::general_purpose::STANDARD.encode(bytes)});
        assert_eq!(encoded_display(&encoded), "D:\\Music\\House");
        assert_eq!(
            encoded_display(&json!({"encoding":"base64-utf16le","data":"invalid"})),
            "Opaque source path"
        );
    }
}
