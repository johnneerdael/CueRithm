use super::*;
use anyhow::{Context, ensure};
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
    sync::Mutex,
};
use tauri_plugin_dialog::DialogExt;

#[derive(Default)]
pub struct ImportSelections {
    selected: Mutex<BTreeMap<String, SelectedSource>>,
    mappings: Mutex<BTreeMap<String, PathBuf>>,
}
#[derive(Clone)]
pub struct SelectedSource {
    pub provider: SourceProvider,
    pub selection: ImportSourceSelection,
    pub reader: Option<Arc<cuerithm_dj_sync::import::readers::rekordbox::RekordboxReader>>,
}
#[derive(Serialize)]
pub struct NativeImportSelection {
    selection_id: String,
    provider: SourceProvider,
    display_location: String,
    artifacts: Vec<String>,
    artifact_count: usize,
}
impl ImportSelections {
    fn insert(&self, source: SelectedSource) -> anyhow::Result<NativeImportSelection> {
        let mut selected = self
            .selected
            .lock()
            .map_err(|_| anyhow::anyhow!("Source selection interrupted"))?;
        ensure!(
            selected.len() < 64,
            "Too many source selections; restart the application to clear them"
        );
        let selection_id = uuid();
        let result = NativeImportSelection {
            selection_id: selection_id.clone(),
            provider: source.provider,
            display_location: source.selection.root.to_string_lossy().into_owned(),
            artifact_count: source.selection.artifacts.len(),
            artifacts: source
                .selection
                .artifacts
                .iter()
                .take(20)
                .map(|a| a.id.clone())
                .collect(),
        };
        selected.insert(selection_id, source);
        Ok(result)
    }
    pub fn get(&self, id: &str) -> anyhow::Result<SelectedSource> {
        self.selected
            .lock()
            .map_err(|_| anyhow::anyhow!("Source selection interrupted"))?
            .get(id)
            .cloned()
            .context("Native source selection expired; choose the source again")
    }
    pub fn mappings(&self, request: &[ImportMappingChoice]) -> anyhow::Result<Vec<PathMapping>> {
        ensure!(request.len() <= 64, "Too many native path mappings");
        let roots = self
            .mappings
            .lock()
            .map_err(|_| anyhow::anyhow!("Path mapping selection interrupted"))?;
        request
            .iter()
            .map(|choice| {
                ensure!(
                    choice.source_prefix.len() <= 4096,
                    "Source prefix exceeds limit"
                );
                Ok(PathMapping {
                    source_prefix: choice.source_prefix.clone(),
                    root_id: choice.root_id.clone(),
                    local_root: roots
                        .get(&choice.root_id)
                        .cloned()
                        .context("Choose the local mapping folder again")?,
                })
            })
            .collect()
    }
}
fn uuid() -> String {
    uuid::Uuid::new_v4().to_string()
}

fn eligible(provider: SourceProvider, relative: &Path) -> bool {
    let name = relative
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or_default();
    let extension = relative
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    match provider {
        SourceProvider::Serato3 => {
            matches!(name, "database V2" | "neworder.pref")
                || matches!(extension.as_str(), "crate" | "scrate")
        }
        SourceProvider::Serato4 => extension == "sqlite",
        SourceProvider::VirtualDj => {
            name.eq_ignore_ascii_case("database.xml")
                || matches!(extension.as_str(), "m3u" | "m3u8" | "vdjfolder")
        }
        SourceProvider::EngineDj => matches!(name, "m.db" | "stm.db" | "hm.db"),
        SourceProvider::DjayPro => {
            matches!(name, "MediaLibrary.db" | "database2" | "Music.library")
        }
        SourceProvider::PioneerUsb => {
            relative
                .components()
                .next()
                .is_some_and(|v| matches!(v.as_os_str().to_str(), Some("PIONEER" | ".PIONEER")))
                && matches!(extension.as_str(), "pdb" | "db" | "dat" | "ext" | "2ex")
        }
        SourceProvider::DjStudio => {
            relative == Path::new("Database/studio.db")
                || relative.components().nth(1).is_some_and(|v| {
                    matches!(
                        v.as_os_str().to_str(),
                        Some(
                            "audio-library-table"
                                | "track-structures-table"
                                | "projects-table"
                                | "projects-meta-table"
                                | "mix-data-table"
                                | "automation-data-table"
                        )
                    )
                })
        }
        _ => false,
    }
}
fn descend(provider: SourceProvider, relative: &Path) -> bool {
    if provider == SourceProvider::DjStudio {
        return relative == Path::new("Database")
            || relative.starts_with("Database")
                && relative.components().nth(1).is_some_and(|v| {
                    matches!(
                        v.as_os_str().to_str(),
                        Some(
                            "audio-library-table"
                                | "track-structures-table"
                                | "projects-table"
                                | "projects-meta-table"
                                | "mix-data-table"
                                | "automation-data-table"
                        )
                    )
                });
    }
    if provider == SourceProvider::PioneerUsb {
        return relative
            .components()
            .next()
            .is_some_and(|v| matches!(v.as_os_str().to_str(), Some("PIONEER" | ".PIONEER")));
    }
    let name = relative
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or_default();
    !matches!(
        name,
        "Cache"
            | "cache"
            | "Contents"
            | "Music"
            | "Audio"
            | "node_modules"
            | "Local Storage"
            | "Session Storage"
            | "Cookies"
            | "GPUCache"
            | ".git"
    )
}
fn artifact(root: &Path, relative: PathBuf, id: String) -> anyhow::Result<ArtifactRequest> {
    let extension = relative
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    let name = relative
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or_default();
    let kind = if matches!(extension.as_str(), "sqlite" | "sqlite3" | "db" | "mikdb")
        || matches!(name, "database2" | "Music.library")
    {
        ArtifactKind::Sqlite
    } else {
        ArtifactKind::StableFile
    };
    let request = ArtifactRequest {
        id,
        relative_path: relative,
        kind,
        selected_root: None,
    };
    // Run the shared traversal/symlink validation before minting native authority.
    SourceCaptureRequest::from_selected_root(SourceProvider::M3u, root, vec![request.clone()])?;
    Ok(request)
}
fn inventory_source(
    root: &Path,
    provider: SourceProvider,
    cancel: &CancellationToken,
) -> anyhow::Result<Vec<ArtifactRequest>> {
    let mut pending = vec![(root.to_path_buf(), 0usize)];
    let mut artifacts = Vec::new();
    let mut visited = 0;
    while let Some((directory, depth)) = pending.pop() {
        cancel.check()?;
        ensure!(depth <= 128, "Source hierarchy exceeds128 levels");
        for entry in fs::read_dir(directory)? {
            cancel.check()?;
            visited += 1;
            ensure!(visited <= 2_000_000, "Source inventory exceeds limit");
            let entry = entry?;
            let path = entry.path();
            let relative = path.strip_prefix(root)?.to_path_buf();
            let metadata = fs::symlink_metadata(&path)?;
            if metadata.file_type().is_symlink() {
                continue;
            }
            if metadata.is_dir() && descend(provider, &relative) {
                pending.push((path, depth + 1));
            } else if metadata.is_file() && eligible(provider, &relative) {
                ensure!(
                    artifacts.len() < 100_000,
                    "Selected source exceeds artifact limit"
                );
                let id = relative
                    .to_str()
                    .context("Source artifact name is not Unicode")?
                    .replace('\\', "/");
                artifacts.push(artifact(root, relative, id)?);
            }
        }
    }
    ensure!(
        !artifacts.is_empty(),
        "No supported music-library artifacts in the selected folder"
    );
    artifacts.sort_by(|a, b| a.id.cmp(&b.id));
    Ok(artifacts)
}
#[tauri::command]
pub async fn choose_import_source(
    host: Managed<'_>,
    app: tauri::AppHandle,
    provider: SourceProvider,
    operation_id: String,
) -> Ipc<Option<NativeImportSelection>> {
    let handle = app.clone();
    run(
        host.inner().clone(),
        Some(handle),
        operation_id,
        move |host, cancel| {
            ensure!(
                provider != SourceProvider::Rekordbox,
                "Select a discovered/configured rekordbox installation"
            );
            let folder = matches!(
                provider,
                SourceProvider::Serato3
                    | SourceProvider::Serato4
                    | SourceProvider::VirtualDj
                    | SourceProvider::EngineDj
                    | SourceProvider::DjayPro
                    | SourceProvider::PioneerUsb
                    | SourceProvider::DjStudio
            );
            let chosen = if folder {
                app.dialog().file().blocking_pick_folder()
            } else {
                app.dialog().file().blocking_pick_file()
            };
            let Some(chosen) = chosen else {
                return Ok(None);
            };
            let path = chosen.into_path()?.canonicalize()?;
            let (root, artifacts) = if folder {
                let artifacts = inventory_source(&path, provider, cancel)?;
                (path, artifacts)
            } else {
                let root = path
                    .parent()
                    .context("Source file parent missing")?
                    .to_path_buf();
                let relative = PathBuf::from(path.file_name().context("Source file name missing")?);
                let id = relative
                    .to_str()
                    .context("Source file name is not Unicode")?
                    .to_owned();
                let one = artifact(&root, relative, id)?;
                (root, vec![one])
            };
            host.import_selections
                .insert(SelectedSource {
                    provider,
                    selection: ImportSourceSelection { root, artifacts },
                    reader: None,
                })
                .map(Some)
        },
    )
    .await
}
#[tauri::command]
pub async fn choose_rekordbox_import_source(
    host: Managed<'_>,
    operation_id: String,
    target_id: String,
) -> Ipc<NativeImportSelection> {
    run(
        host.inner().clone(),
        None,
        operation_id,
        move |host, cancel| {
            let (selection, reader) = host
                .library
                .prepare_rekordbox_import_source(&target_id, cancel)?;
            host.import_selections.insert(SelectedSource {
                provider: SourceProvider::Rekordbox,
                selection,
                reader: Some(reader),
            })
        },
    )
    .await
}
#[tauri::command]
pub async fn add_import_artifacts(
    host: Managed<'_>,
    app: tauri::AppHandle,
    selection_id: String,
    operation_id: String,
) -> Ipc<Option<NativeImportSelection>> {
    let handle = app.clone();
    run(
        host.inner().clone(),
        Some(handle),
        operation_id,
        move |host, cancel| {
            let mut source = host.import_selections.get(&selection_id)?;
            ensure!(
                matches!(
                    source.provider,
                    SourceProvider::VirtualDj | SourceProvider::Serato3 | SourceProvider::Serato4
                ),
                "Additional native volumes are unsupported for this provider"
            );
            let Some(files) = app.dialog().file().blocking_pick_files() else {
                return Ok(None);
            };
            for file in files {
                cancel.check()?;
                let path = file.into_path()?.canonicalize()?;
                let root = path.parent().context("Native file parent missing")?;
                let relative = PathBuf::from(path.file_name().context("Native file name missing")?);
                ensure!(
                    eligible(source.provider, &relative),
                    "Unsupported additional native artifact"
                );
                let id = format!(
                    "selected-volume-{}/{}",
                    source.selection.artifacts.len(),
                    relative.to_string_lossy()
                );
                let one = artifact(root, relative, id.clone())?.with_selected_root(&id, root);
                ensure!(
                    source.selection.artifacts.len() < 100_000,
                    "Too many selected artifacts"
                );
                source.selection.artifacts.push(one);
            }
            host.import_selections.insert(source).map(Some)
        },
    )
    .await
}
#[derive(Serialize)]
pub struct NativeMappingSelection {
    root_id: String,
    display_location: String,
}
#[tauri::command]
pub async fn choose_import_mapping(
    host: Managed<'_>,
    app: tauri::AppHandle,
) -> Ipc<Option<NativeMappingSelection>> {
    let host = host.inner().clone();
    work(move || {
        let Some(folder) = app.dialog().file().blocking_pick_folder() else {
            return Ok(None);
        };
        let path = folder.into_path()?.canonicalize()?;
        let root_id = uuid();
        let mut roots = host
            .import_selections
            .mappings
            .lock()
            .map_err(|_| anyhow::anyhow!("Mapping selection interrupted"))?;
        ensure!(roots.len() < 64, "Too many selected mapping roots");
        let display_location = path.to_string_lossy().into_owned();
        roots.insert(root_id.clone(), path);
        Ok(Some(NativeMappingSelection {
            root_id,
            display_location,
        }))
    })
    .await
    .map_err(import_error)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn engine_inventory_includes_streaming_and_history_databases() {
        let root = tempfile::tempdir().unwrap();
        for name in ["m.db", "stm.db", "hm.db", "p.db"] {
            let path = root.path().join("Engine Library/Database2").join(name);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, b"authored").unwrap();
        }
        let artifacts = inventory_source(
            root.path(),
            SourceProvider::EngineDj,
            &CancellationToken::default(),
        )
        .unwrap();
        let names: std::collections::BTreeSet<_> = artifacts
            .iter()
            .map(|a| a.relative_path.file_name().unwrap().to_str().unwrap())
            .collect();
        assert_eq!(names, ["m.db", "stm.db", "hm.db"].into());
    }
    #[test]
    fn mikdb_selection_uses_coherent_sqlite_acquisition() {
        let root = tempfile::tempdir().unwrap();
        let request = artifact(
            root.path(),
            "Collection11.mikdb".into(),
            "Collection11.mikdb".into(),
        )
        .unwrap();
        assert!(matches!(request.kind, ArtifactKind::Sqlite));
    }
    #[test]
    fn studio_inventory_never_reads_browser_or_account_storage() {
        let root = tempfile::tempdir().unwrap();
        for path in [
            "Database/studio.db",
            "Database/audio-library-table/a/invented",
            "Database/track-structures-table/a/invented/fixed",
            "Database/account-table/secret",
            "Cache/Database/audio-library-table.json",
            "Local Storage/secret",
        ] {
            let path = root.path().join(path);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, b"invented").unwrap();
        }
        let artifacts = inventory_source(
            root.path(),
            SourceProvider::DjStudio,
            &CancellationToken::default(),
        )
        .unwrap();
        assert_eq!(artifacts.len(), 3);
        assert!(
            artifacts
                .iter()
                .all(|a| !a.id.contains("secret") && !a.id.starts_with("Cache"))
        );
    }
}
