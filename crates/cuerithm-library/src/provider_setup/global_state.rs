//! One application authority for all library windows and headless processes.
use anyhow::{Context, Result, ensure};
use cuerithm_autotag::{
    CachedResponse, CatalogCredentials, CatalogIdentity, CatalogRuntime, CatalogState, RateClaim,
    RateObservation, StartDecision,
};
use rusqlite::Connection;
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    sync::{Arc, Mutex, OnceLock, Weak},
};

pub(super) struct ApplicationCatalogState {
    root: Option<PathBuf>,
    connection: Mutex<Option<Connection>>,
}
impl ApplicationCatalogState {
    fn at(root: Option<PathBuf>) -> Arc<Self> {
        Arc::new(Self {
            root,
            connection: Mutex::new(None),
        })
    }
    fn with<T>(&self, op: impl FnOnce(&mut Connection) -> Result<T>) -> Result<T> {
        let mut slot = self
            .connection
            .lock()
            .map_err(|_| anyhow::anyhow!("Application catalog state interrupted"))?;
        if slot.is_none() {
            let root = self
                .root
                .as_ref()
                .context("Application catalog directory unavailable")?;
            *slot = Some(open_connection(root)?);
        }
        op(slot
            .as_mut()
            .context("Application catalog state unavailable")?)
    }
    pub(super) fn projection(&self, service: &str) -> Result<crate::catalog_state::RateProjection> {
        self.with(|conn| crate::catalog_state::projection(conn, service))
    }
}
impl CatalogState for ApplicationCatalogState {
    fn claim_start(&self, c: &RateClaim) -> Result<StartDecision> {
        self.with(|conn| crate::catalog_state::claim_start(conn, c))
    }
    fn observe(&self, o: &RateObservation) -> Result<()> {
        self.with(|conn| crate::catalog_state::observe(conn, o))
    }
    fn cache_get(&self, k: &str, now: i64) -> Result<Option<CachedResponse>> {
        self.with(|conn| crate::catalog_state::cache_get(conn, k, now))
    }
    fn cache_put(&self, k: &str, v: &CachedResponse) -> Result<()> {
        self.with(|conn| crate::catalog_state::cache_put(conn, k, v))
    }
}
fn schema() -> Result<&'static str> {
    include_str!("../../migrations/023_catalog_runtime.sql")
        .split_once("ALTER TABLE")
        .map(|(tables, _)| tables)
        .context("Catalog schema boundary missing")
}
const GLOBAL_APPLICATION_ID: i64 = 0x43554341;
const GLOBAL_SCHEMA_VERSION: i64 = 1;
fn unsafe_entry(metadata: &std::fs::Metadata) -> bool {
    if metadata.file_type().is_symlink() {
        return true;
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        if metadata.file_attributes() & 0x400 != 0 {
            return true;
        }
    }
    false
}
pub(super) fn prepare_root(root: &Path) -> Result<()> {
    ensure!(
        root.is_absolute(),
        "Application catalog directory must be absolute"
    );
    let mut path = PathBuf::new();
    for component in root.components() {
        match component {
            std::path::Component::ParentDir | std::path::Component::CurDir => {
                anyhow::bail!("Application catalog directory is not normalized")
            }
            std::path::Component::Prefix(_) => {
                path.push(component);
                continue;
            }
            _ => path.push(component),
        }
        match std::fs::symlink_metadata(&path) {
            Ok(metadata) => ensure!(
                metadata.is_dir() && !unsafe_entry(&metadata),
                "Application catalog directory contains an alias"
            ),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                let mut builder = std::fs::DirBuilder::new();
                #[cfg(unix)]
                {
                    use std::os::unix::fs::DirBuilderExt;
                    builder.mode(0o700);
                }
                match builder.create(&path) {
                    Ok(()) => {}
                    Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {}
                    Err(e) => return Err(e.into()),
                }
                let metadata = std::fs::symlink_metadata(&path)?;
                ensure!(
                    metadata.is_dir() && !unsafe_entry(&metadata),
                    "Application catalog directory changed"
                );
            }
            Err(e) => return Err(e.into()),
        }
    }
    let metadata = std::fs::symlink_metadata(root)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::{MetadataExt, PermissionsExt};
        ensure!(
            metadata.uid() == unsafe { libc::geteuid() }
                && metadata.permissions().mode() & 0o077 == 0,
            "Application catalog directory is not private"
        );
    }
    ensure!(
        metadata.is_dir() && !unsafe_entry(&metadata),
        "Application catalog directory is not private"
    );
    Ok(())
}
fn check_file(path: &Path) -> Result<()> {
    let m = std::fs::symlink_metadata(path)?;
    ensure!(
        m.is_file() && !unsafe_entry(&m),
        "Application catalog file is not regular"
    );
    #[cfg(unix)]
    {
        use std::os::unix::fs::{MetadataExt, PermissionsExt};
        ensure!(
            m.uid() == unsafe { libc::geteuid() }
                && m.nlink() == 1
                && m.permissions().mode() & 0o077 == 0,
            "Application catalog file is not private"
        );
    }
    Ok(())
}
fn open_connection(root: &Path) -> Result<Connection> {
    prepare_root(root)?;
    let path = root.join("catalog.sqlite3");
    let mut options = std::fs::OpenOptions::new();
    options.read(true).write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600).custom_flags(libc::O_NOFOLLOW);
    }
    match options.open(&path) {
        Ok(file) => file.sync_all()?,
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {}
        Err(e) => return Err(e.into()),
    }
    check_file(&path)?;
    for suffix in ["-wal", "-shm", "-journal"] {
        let sidecar = root.join(format!("catalog.sqlite3{suffix}"));
        match std::fs::symlink_metadata(&sidecar) {
            Ok(_) => check_file(&sidecar)?,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(e.into()),
        }
    }
    let mut conn = Connection::open_with_flags(
        &path,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_WRITE
            | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX
            | rusqlite::OpenFlags::SQLITE_OPEN_NOFOLLOW,
    )?;
    conn.busy_timeout(std::time::Duration::from_millis(250))?;
    conn.pragma_update(None, "synchronous", "FULL")?;
    {
        let tx = conn.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        let version: i64 = tx.query_row("PRAGMA user_version", [], |r| r.get(0))?;
        let application: i64 = tx.query_row("PRAGMA application_id", [], |r| r.get(0))?;
        if version == 0 {
            let tables:i64=tx.query_row("SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%'",[],|r|r.get(0))?;
            ensure!(
                application == 0 && tables == 0,
                "Unrecognized application catalog state"
            );
            tx.execute_batch(schema()?)?;
            tx.pragma_update(None, "application_id", GLOBAL_APPLICATION_ID)?;
            tx.pragma_update(None, "user_version", GLOBAL_SCHEMA_VERSION)?;
        } else {
            ensure!(
                version == GLOBAL_SCHEMA_VERSION && application == GLOBAL_APPLICATION_ID,
                "Application catalog state requires another application version"
            );
        }
        tx.commit()?;
    }
    conn.pragma_update(None, "journal_mode", "WAL")?;
    conn.prepare("SELECT manual_hold_observed_ms FROM catalog_rate_state LIMIT 0")?;
    prepare_root(root)?;
    check_file(&path)?;
    #[cfg(unix)]
    {
        std::fs::File::open(root)?.sync_all()?;
    }
    Ok(conn)
}

struct Shared {
    runtime: Weak<CatalogRuntime>,
    state: Weak<ApplicationCatalogState>,
}
fn shared() -> &'static Mutex<BTreeMap<Option<PathBuf>, Shared>> {
    static SHARED: OnceLock<Mutex<BTreeMap<Option<PathBuf>, Shared>>> = OnceLock::new();
    SHARED.get_or_init(|| Mutex::new(BTreeMap::new()))
}
fn root() -> Option<PathBuf> {
    dirs::data_local_dir().map(|p| p.join("com.cuerithm.app").join("catalog-runtime"))
}
pub(super) fn runtime(
    release: &str,
) -> Result<(Arc<CatalogRuntime>, Arc<ApplicationCatalogState>)> {
    let root = root();
    let marker = root.clone();
    runtime_at(root, release, move || match marker {
        Some(root) => Ok(
            Arc::new(super::native::NativeCredentialAuthority::new(root))
                as Arc<dyn CatalogCredentials>,
        ),
        None => Ok(Arc::new(UnavailableCredentials)),
    })
}
fn runtime_at(
    root: Option<PathBuf>,
    release: &str,
    credentials: impl FnOnce() -> Result<Arc<dyn CatalogCredentials>>,
) -> Result<(Arc<CatalogRuntime>, Arc<ApplicationCatalogState>)> {
    let mut shared = shared()
        .lock()
        .map_err(|_| anyhow::anyhow!("Application catalog authority interrupted"))?;
    shared.retain(|_, v| v.runtime.strong_count() > 0);
    if let Some(entry) = shared.get(&root)
        && let (Some(runtime), Some(state)) = (entry.runtime.upgrade(), entry.state.upgrade())
    {
        ensure!(
            runtime.identity().user_agent() == CatalogIdentity::new(release)?.user_agent(),
            "Application release identity changed"
        );
        return Ok((runtime, state));
    }
    let state = ApplicationCatalogState::at(root.clone());
    let runtime = Arc::new(CatalogRuntime::new(
        CatalogIdentity::new(release)?,
        state.clone(),
        credentials()?,
    )?);
    shared.insert(
        root,
        Shared {
            runtime: Arc::downgrade(&runtime),
            state: Arc::downgrade(&state),
        },
    );
    Ok((runtime, state))
}
pub(super) fn state_for(
    runtime: &Arc<CatalogRuntime>,
) -> Result<Option<Arc<ApplicationCatalogState>>> {
    let shared = shared()
        .lock()
        .map_err(|_| anyhow::anyhow!("Application catalog authority interrupted"))?;
    Ok(shared.values().find_map(|entry| {
        entry
            .runtime
            .upgrade()
            .filter(|r| Arc::ptr_eq(r, runtime))
            .and_then(|_| entry.state.upgrade())
    }))
}
struct UnavailableCredentials;
impl CatalogCredentials for UnavailableCredentials {
    fn status(
        &self,
        _: cuerithm_credentials::ProviderId,
    ) -> Result<cuerithm_credentials::CredentialStatus> {
        anyhow::bail!("Application credential directory unavailable")
    }
    fn save(
        &self,
        _: &cuerithm_credentials::CredentialRef,
        _: cuerithm_credentials::Secret,
    ) -> Result<cuerithm_credentials::CredentialStatus> {
        anyhow::bail!("Application credential directory unavailable")
    }
    fn remove(
        &self,
        _: cuerithm_credentials::ProviderId,
    ) -> Result<cuerithm_credentials::CredentialStatus> {
        anyhow::bail!("Application credential directory unavailable")
    }
    fn with_secret(
        &self,
        _: &cuerithm_credentials::CredentialRef,
        _: &mut dyn FnMut(&str, cuerithm_credentials::Revocation) -> Result<()>,
    ) -> Result<()> {
        anyhow::bail!("Application credential directory unavailable")
    }
    fn record_verification(
        &self,
        _: &cuerithm_credentials::CredentialRef,
        _: cuerithm_credentials::Verification,
    ) -> Result<cuerithm_credentials::CredentialStatus> {
        anyhow::bail!("Application credential directory unavailable")
    }
}
#[cfg(test)]
mod tests;
