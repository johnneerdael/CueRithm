use crate::{
    commands::{AppError, work},
    host::Host,
    metadata_commands,
};
use cuerithm_library::{
    ProviderCredentialId, ProviderCredentialInput, ProviderCredentialRef, ProviderCredentialStatus,
    ProviderSetupStatus,
};
use std::sync::Arc;
use tauri::{Emitter, State};
type Managed<'a> = State<'a, Arc<Host>>;
type Ipc<T> = Result<T, AppError>;
fn changed(app: &tauri::AppHandle) {
    let _ = app.emit("catalog-access-change", ());
}
#[tauri::command]
pub async fn provider_setup_statuses(host: Managed<'_>) -> Ipc<Vec<ProviderSetupStatus>> {
    let service = host.provider_setup.clone();
    work(move || service.statuses()).await
}
#[tauri::command]
pub async fn save_provider_credential(
    host: Managed<'_>,
    app: tauri::AppHandle,
    expected: ProviderCredentialRef,
    input: ProviderCredentialInput,
) -> Ipc<ProviderCredentialStatus> {
    let service = host.provider_setup.clone();
    let result = work(move || service.save(&expected, input)).await;
    changed(&app);
    result
}
#[tauri::command]
pub async fn remove_provider_credential(
    host: Managed<'_>,
    app: tauri::AppHandle,
    provider: ProviderCredentialId,
) -> Ipc<ProviderCredentialStatus> {
    let service = host.provider_setup.clone();
    let result = work(move || service.remove(provider)).await;
    changed(&app);
    result
}
#[tauri::command]
pub async fn test_provider_credential(
    host: Managed<'_>,
    app: tauri::AppHandle,
    operation_id: String,
    expected: ProviderCredentialRef,
) -> Ipc<ProviderSetupStatus> {
    let result = metadata_commands::run(host.inner().clone(), operation_id, move |host, cancel| {
        host.provider_setup.test(&expected, cancel)
    })
    .await;
    changed(&app);
    result
}
