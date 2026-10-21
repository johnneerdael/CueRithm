use crate::{
    commands::{AppError, work},
    host::Host,
};
use cuerithm_library::*;
use std::sync::Arc;
use tauri::{Emitter, State};
type Managed<'a> = State<'a, Arc<Host>>;
type Ipc<T> = Result<T, AppError>;
fn changed(app: &tauri::AppHandle, result: &ManualPublicationResult) {
    let _ = app.emit(
        "library-change",
        serde_json::json!({"track_ids":result.track_ids,"playlist_ids":[]}),
    );
}
#[tauri::command]
pub async fn manual_publication_snapshot(
    host: Managed<'_>,
    track_id: String,
) -> Ipc<ManualPublicationSnapshot> {
    let lib = host.library.clone();
    work(move || lib.manual_publication_snapshot(&track_id)).await
}
#[tauri::command]
pub async fn preview_manual_publication(
    host: Managed<'_>,
    request: ManualPublicationRequest,
) -> Ipc<ManualPublicationPreview> {
    let lib = host.library.clone();
    work(move || lib.preview_manual_publication(request)).await
}
#[tauri::command]
pub async fn apply_manual_publication(
    host: Managed<'_>,
    app: tauri::AppHandle,
    preview_id: String,
    expected_revision: u64,
) -> Ipc<ManualPublicationResult> {
    let lib = host.library.clone();
    let result = work(move || lib.apply_manual_publication(&preview_id, expected_revision)).await?;
    changed(&app, &result);
    Ok(result)
}
#[tauri::command]
pub async fn manual_publication_history(
    host: Managed<'_>,
    track_id: String,
) -> Ipc<Vec<ManualPublicationHistory>> {
    let lib = host.library.clone();
    work(move || lib.manual_publication_history(&track_id)).await
}
#[tauri::command]
pub async fn undo_manual_publication(
    host: Managed<'_>,
    app: tauri::AppHandle,
    edit_id: String,
    expected_revision: u64,
) -> Ipc<ManualPublicationResult> {
    let lib = host.library.clone();
    let result = work(move || lib.undo_manual_publication(&edit_id, expected_revision)).await?;
    changed(&app, &result);
    Ok(result)
}
#[tauri::command]
pub async fn list_cue_templates(host: Managed<'_>) -> Ipc<Vec<SavedCueTemplate>> {
    let lib = host.library.clone();
    work(move || lib.cue_templates()).await
}
#[tauri::command]
pub async fn save_cue_template(
    host: Managed<'_>,
    id: Option<String>,
    name: String,
    template: camelot_core::lexicon_publication::CueTemplate,
    expected_revision: u64,
) -> Ipc<SavedCueTemplate> {
    let lib = host.library.clone();
    work(move || lib.save_cue_template(id.as_deref(), name, template, expected_revision)).await
}
#[tauri::command]
pub async fn delete_cue_template(host: Managed<'_>, id: String, expected_revision: u64) -> Ipc<()> {
    let lib = host.library.clone();
    work(move || lib.delete_cue_template(&id, expected_revision)).await
}
#[tauri::command]
pub async fn preview_cue_template(
    host: Managed<'_>,
    request: TemplatePublicationRequest,
) -> Ipc<ManualPublicationPreview> {
    let lib = host.library.clone();
    work(move || lib.preview_cue_template(request)).await
}
