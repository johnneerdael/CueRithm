use crate::{commands::AppError, host::Host};
use std::sync::{Arc, atomic::Ordering};
use tauri::State;
type Managed<'a> = State<'a, Arc<Host>>;
type Ipc<T> = Result<T, AppError>;

#[tauri::command]
pub fn register_exit_guard(host: Managed<'_>) -> Ipc<Option<String>> {
    if !host.alive.load(Ordering::Relaxed) {
        return Err(anyhow::anyhow!("Application is closing").into());
    }
    host.exit.register().map_err(Into::into)
}
#[tauri::command]
pub fn unregister_exit_guard(host: Managed<'_>) -> Ipc<()> {
    host.exit.unregister().map_err(Into::into)
}
#[tauri::command]
pub fn complete_exit(host: Managed<'_>, app: tauri::AppHandle, request_id: String) -> Ipc<()> {
    host.exit.complete(&request_id)?;
    crate::start_shutdown(app, host.inner().clone());
    Ok(())
}
#[tauri::command]
pub fn cancel_exit(host: Managed<'_>, request_id: String) -> Ipc<bool> {
    host.exit.cancel(&request_id).map_err(Into::into)
}
