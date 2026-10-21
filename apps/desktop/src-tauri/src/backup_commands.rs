use crate::{
    commands::{AppError, work},
    host::Host,
};
use cuerithm_library::{LibraryBackupPreview, LibraryRestoreRequest};
use std::{path::PathBuf, sync::Arc};
use tauri::State;
type Managed<'a> = State<'a, Arc<Host>>;
type Ipc<T> = Result<T, AppError>;
async fn run<T: Send + 'static>(
    host: Arc<Host>,
    f: impl FnOnce(&Host) -> anyhow::Result<T> + Send + 'static,
) -> Ipc<T> {
    let lease = host.maintenance.begin("library-backup")?;
    work(move || {
        lease.token.check()?;
        let result = f(&host);
        drop(lease);
        result
    })
    .await
}
#[tauri::command]
pub async fn create_library_backup(
    host: Managed<'_>,
    destination: PathBuf,
) -> Ipc<LibraryBackupPreview> {
    run(host.inner().clone(), move |h| {
        h.library.create_library_backup(&destination)
    })
    .await
}
#[tauri::command]
pub async fn inspect_library_backup(
    host: Managed<'_>,
    bundle: PathBuf,
) -> Ipc<LibraryBackupPreview> {
    run(host.inner().clone(), move |h| {
        h.library.inspect_library_backup(&bundle)
    })
    .await
}
#[tauri::command]
pub async fn stage_library_restore(
    host: Managed<'_>,
    bundle: PathBuf,
    sha256: String,
) -> Ipc<LibraryRestoreRequest> {
    run(host.inner().clone(), move |h| {
        h.library.stage_library_restore(&bundle, &sha256)
    })
    .await
}
#[tauri::command]
pub async fn pending_library_restore(host: Managed<'_>) -> Ipc<Option<LibraryRestoreRequest>> {
    let host = host.inner().clone();
    work(move || host.library.pending_library_restore()).await
}
#[tauri::command]
pub async fn cancel_library_restore(host: Managed<'_>) -> Ipc<()> {
    run(host.inner().clone(), move |h| {
        h.library.cancel_library_restore()
    })
    .await
}

#[tauri::command]
pub async fn retry_library_restore(host: Managed<'_>) -> Ipc<LibraryRestoreRequest> {
    run(host.inner().clone(), move |h| {
        h.library.retry_library_restore()
    })
    .await
}
