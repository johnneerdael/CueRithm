use crate::{
    commands::{AppError, work},
    host::Host,
};
use anyhow::{Context, ensure};
use cuerithm_library::*;
use std::{
    io::{Read, Write},
    path::Path,
    sync::Arc,
};
use tauri::{Emitter, State};
use tauri_plugin_dialog::DialogExt;

type Managed<'a> = State<'a, Arc<Host>>;
type Ipc<T> = Result<T, AppError>;

fn read_import_document(library: &Library, path: &Path) -> anyhow::Result<String> {
    let mut bytes = Vec::new();
    std::fs::File::open(path)?
        .take(MAX_PREPARATION_PRESET_DOCUMENT_BYTES as u64 + 1)
        .read_to_end(&mut bytes)?;
    ensure!(
        bytes.len() <= MAX_PREPARATION_PRESET_DOCUMENT_BYTES,
        "Preparation preset exceeds 1 MiB"
    );
    let document = String::from_utf8(bytes).context("Preparation preset must be UTF-8 JSON")?;
    library
        .preview_preparation_preset_import(&document)
        .map_err(|_| {
            anyhow::anyhow!(
                "Selected file is not a supported preparation preset; check its format, version and fields"
            )
        })?;
    Ok(document)
}

async fn mutate<T: Send + 'static>(
    host: Arc<Host>,
    app: tauri::AppHandle,
    operation: impl FnOnce(&Library) -> anyhow::Result<T> + Send + 'static,
) -> Ipc<T> {
    let result = work(move || operation(&host.library)).await;
    let _ = app.emit("preparation-presets-change", ());
    result
}

#[tauri::command]
pub async fn preparation_presets(host: Managed<'_>) -> Ipc<Vec<PreparationPresetSummary>> {
    let library = host.library.clone();
    work(move || library.preparation_presets()).await
}
#[tauri::command]
pub async fn preparation_preset(host: Managed<'_>, id: String) -> Ipc<PreparationPreset> {
    let library = host.library.clone();
    work(move || library.preparation_preset(&id)).await
}
#[tauri::command]
pub async fn save_preparation_preset(
    host: Managed<'_>,
    app: tauri::AppHandle,
    request: PreparationPresetSave,
) -> Ipc<PreparationPreset> {
    mutate(host.inner().clone(), app, move |library| {
        library.save_preparation_preset(request)
    })
    .await
}
#[tauri::command]
pub async fn duplicate_preparation_preset(
    host: Managed<'_>,
    app: tauri::AppHandle,
    id: String,
    revision: String,
    name: String,
) -> Ipc<PreparationPreset> {
    mutate(host.inner().clone(), app, move |library| {
        library.duplicate_preparation_preset(&id, &revision, &name)
    })
    .await
}
#[tauri::command]
pub async fn delete_preparation_preset(
    host: Managed<'_>,
    app: tauri::AppHandle,
    id: String,
    revision: String,
) -> Ipc<()> {
    mutate(host.inner().clone(), app, move |library| {
        library.delete_preparation_preset(&id, &revision)
    })
    .await
}
#[tauri::command]
pub async fn preview_preparation_preset_import(
    host: Managed<'_>,
    document: String,
) -> Ipc<PreparationPresetImportPreview> {
    let library = host.library.clone();
    work(move || library.preview_preparation_preset_import(&document)).await
}
#[tauri::command]
pub async fn import_preparation_preset(
    host: Managed<'_>,
    app: tauri::AppHandle,
    document: String,
    name_override: Option<String>,
) -> Ipc<PreparationPreset> {
    mutate(host.inner().clone(), app, move |library| {
        library.import_preparation_preset(&document, name_override.as_deref())
    })
    .await
}
#[tauri::command]
pub async fn export_preparation_preset(
    host: Managed<'_>,
    id: String,
    revision: String,
) -> Ipc<PreparationPresetExport> {
    let library = host.library.clone();
    work(move || library.export_preparation_preset(&id, &revision)).await
}
#[tauri::command]
pub async fn project_preparation_preset(
    host: Managed<'_>,
    settings: PreparationSettings,
) -> Ipc<PreparationPresetProjection> {
    let library = host.library.clone();
    work(move || library.project_preparation_preset(&settings)).await
}
#[tauri::command]
pub async fn overlay_preparation_preset(
    host: Managed<'_>,
    settings: PreparationSettings,
    id: String,
    revision: String,
) -> Ipc<PreparationSettings> {
    let library = host.library.clone();
    work(move || library.overlay_preparation_preset(&settings, &id, &revision)).await
}
#[tauri::command]
pub async fn preparation_preset_folders(
    host: Managed<'_>,
    settings: PreparationSettings,
) -> Ipc<Vec<PreparationFolderAssignment>> {
    let library = host.library.clone();
    work(move || library.preparation_preset_folders_for_settings(&settings)).await
}
#[tauri::command]
pub async fn choose_preparation_preset_import(
    host: Managed<'_>,
    app: tauri::AppHandle,
) -> Ipc<Option<String>> {
    let library = host.library.clone();
    work(move || {
        let Some(file) = app
            .dialog()
            .file()
            .add_filter("Preparation preset", &["json"])
            .blocking_pick_file()
        else {
            return Ok(None);
        };
        // Only a validated preset document crosses the bridge, never a picked
        // filesystem path or arbitrary selected file contents.
        read_import_document(&library, &file.into_path()?).map(Some)
    })
    .await
}
#[tauri::command]
pub async fn save_preparation_preset_export(
    host: Managed<'_>,
    app: tauri::AppHandle,
    id: String,
    revision: String,
) -> Ipc<bool> {
    let library = host.library.clone();
    work(move || {
        let export = library.export_preparation_preset(&id, &revision)?;
        ensure!(
            export.document.len() <= MAX_PREPARATION_PRESET_DOCUMENT_BYTES,
            "Preparation preset exceeds 1 MiB"
        );
        let Some(file) = app
            .dialog()
            .file()
            .add_filter("Preparation preset", &["json"])
            .set_file_name("Preparation preset.json")
            .blocking_save_file()
        else {
            return Ok(false);
        };
        let path = file.into_path()?;
        let parent = path.parent().context("Missing preset export folder")?;
        let mut output = tempfile::NamedTempFile::new_in(parent)?;
        output.write_all(export.document.as_bytes())?;
        output.as_file().sync_all()?;
        output.persist(&path).map_err(|error| error.error)?;
        Ok(true)
    })
    .await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selected_nonpreset_contents_never_cross_the_bridge_or_appear_in_errors() {
        let temp = tempfile::tempdir().unwrap();
        let library = Library::open(&temp.path().join("library.sqlite")).unwrap();
        let path = temp.path().join("selected.json");
        let private_value = "do-not-expose-this-test-value";
        std::fs::write(&path, format!("{{\"api_key\":\"{private_value}\"}}")).unwrap();
        let error = read_import_document(&library, &path).unwrap_err();
        assert!(!format!("{error:#}").contains(private_value));
        std::fs::write(
            &path,
            serde_json::to_vec(&serde_json::json!({
                "format": "cuerithm-preparation-preset",
                "version": 1,
                "name": "Selected document",
                "rules": { "organization": { "mode": private_value } }
            }))
            .unwrap(),
        )
        .unwrap();
        let error = read_import_document(&library, &path).unwrap_err();
        assert!(
            !format!("{error:#}").contains(private_value),
            "Rejected enum values from a selected file must not appear in IPC errors"
        );
        std::fs::write(&path, vec![b'x'; MAX_PREPARATION_PRESET_DOCUMENT_BYTES + 1]).unwrap();
        assert!(
            read_import_document(&library, &path)
                .unwrap_err()
                .to_string()
                .contains("exceeds 1 MiB")
        );
    }
}
