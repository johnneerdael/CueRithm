//! Same-installation preparation-state snapshots. Music files are never copied.
use crate::{
    Library, SettingsSnapshot,
    model::{bytes_path, now, path_bytes},
};
use anyhow::{Context, Result, ensure};
use rusqlite::{
    Connection, OpenFlags,
    backup::{Backup, StepResult},
};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    fs::{self, File},
    io::Write,
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

const APPLICATION_ID: i64 = 1129665093;
const MAX_DATABASE: u64 = 1024 * 1024 * 1024;
const MAX_MANIFEST: u64 = 16 * 1024 * 1024;
const MAX_ENTRIES: usize = 50_000;
const MAX_DEPENDENCY_BYTES: u64 = 1024 * 1024 * 1024 * 1024;
const RESOURCE_DIRS: [&str; 4] = ["tagging", "cleanup", "maintenance", "sync"];
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LibraryBackupPreview {
    pub path: PathBuf,
    pub database_sha256: String,
    pub created: i64,
    pub tracks: u64,
    pub missing_sources: u64,
    pub schema: i64,
    pub music_included: bool,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LibraryRestoreRequest {
    pub id: String,
    pub created: i64,
    pub database_sha256: String,
    #[serde(default)]
    pub blocked_reason: Option<String>,
    #[serde(default)]
    pub mutation_started: bool,
    #[serde(default)]
    pub pre_restore: Option<PathBuf>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum LibraryRestoreStartup {
    NoPending,
    Applied {
        pre_restore: PathBuf,
    },
    BlockedBeforeMutation {
        request: LibraryRestoreRequest,
        reason: String,
    },
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Location {
    path: Vec<u8>,
    sha256: Option<String>,
    bytes: u64,
    kind: String,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Manifest {
    format: u32,
    application: String,
    version: String,
    platform: String,
    installation: Vec<u8>,
    installation_id: String,
    created: i64,
    schema: i64,
    database_sha256: String,
    music_included: bool,
    sources: Vec<Location>,
    models: Vec<Location>,
    resources: Vec<Location>,
}
fn root_identity(root: &Path) -> Result<Vec<u8>> {
    Ok(path_bytes(&root.canonicalize()?))
}
fn managed_directory(root: &Path) -> Result<PathBuf> {
    let path = root.join("library-backups");
    if let Ok(meta) = fs::symlink_metadata(&path) {
        ensure!(
            meta.is_dir() && !meta.file_type().is_symlink(),
            "Managed backup directory cannot be a symbolic link or non-directory"
        );
    }
    Ok(path)
}
fn installation_id(root: &Path, create: bool) -> Result<String> {
    let path = root.join(".library-installation-id");
    if create && !path.exists() {
        atomic_json(&path, &uuid::Uuid::new_v4().to_string())?;
    }
    regular(&path, 1024)?;
    let id: String = serde_json::from_slice(&fs::read(path)?)?;
    ensure!(
        uuid::Uuid::parse_str(&id).is_ok(),
        "Library installation identity is unavailable"
    );
    Ok(id)
}
fn file_lease(path: &Path, message: &str) -> Result<File> {
    ensure!(
        !fs::symlink_metadata(path).is_ok_and(|m| m.file_type().is_symlink()),
        "Lock cannot be a symbolic link"
    );
    let file = File::options()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(path)?;
    fs2::FileExt::try_lock_exclusive(&file).context(message.to_owned())?;
    Ok(file)
}
fn sync_lease(root: &Path) -> Result<File> {
    let directory = root.join("sync");
    fs::create_dir_all(&directory)?;
    ensure!(
        !fs::symlink_metadata(&directory)?.file_type().is_symlink(),
        "Sync directory cannot be a symbolic link"
    );
    file_lease(
        &directory.join("service.lock"),
        "Another DJ sync operation is running",
    )
}
fn readonly(path: &Path) -> Result<Connection> {
    regular(path, MAX_DATABASE)?;
    let conn = Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )?;
    conn.pragma_update(None, "trusted_schema", false)?;
    Ok(conn)
}
fn regular(path: &Path, max: u64) -> Result<()> {
    let meta = fs::symlink_metadata(path)?;
    ensure!(
        meta.is_file() && !meta.file_type().is_symlink(),
        "Backup entries must be regular files"
    );
    ensure!(meta.len() <= max, "Backup exceeds the supported size limit");
    Ok(())
}
fn schema(conn: &Connection) -> Result<i64> {
    let app: i64 = conn.query_row("PRAGMA application_id", [], |r| r.get(0))?;
    ensure!(app == APPLICATION_ID, "This is not a CueRithm library");
    let version: i64 = conn.query_row("PRAGMA user_version", [], |r| r.get(0))?;
    ensure!(
        (14..=15).contains(&version),
        "Unsupported library backup schema"
    );
    Ok(version)
}
fn quiet_database(conn: &Connection) -> Result<()> {
    let pending:i64=conn.query_row("SELECT (SELECT COUNT(*) FROM jobs WHERE state IN ('queued','running'))+(SELECT COUNT(*) FROM tag_writes WHERE state NOT IN ('committed','rolled_back') OR cleanup_complete=0)+(SELECT COUNT(*) FROM cleanup_writes WHERE state NOT IN ('committed','undone','rolled_back') OR cleanup_complete=0)+(SELECT COUNT(*) FROM maintenance_writes WHERE state NOT IN ('committed','undone','rolled_back') OR cleanup_complete=0)+(SELECT COUNT(*) FROM maintenance_scans WHERE (kind='unused_removal_operation' AND (json_extract(payload_json,'$.state') NOT IN ('committed','undone','rolled_back') OR COALESCE(json_extract(payload_json,'$.cleanup_complete'),0)=0)) OR (kind='unused_backup_disposal' AND (json_extract(payload_json,'$.result.state') NOT IN ('committed','rolled_back') OR COALESCE(json_extract(payload_json,'$.result.cleanup_pending'),1)=1)))",[],|r|r.get(0))?;
    ensure!(
        pending == 0,
        "Finish preparation and resolve tagging, cleanup or maintenance recovery before using a library backup"
    );
    Ok(())
}
fn settled(conn: &Connection, root: &Path) -> Result<()> {
    quiet_database(conn)?;
    // Every changed committed publication keeps its original bytes for recovery/Undo.
    for table in ["tag", "cleanup", "maintenance"] {
        let query = format!(
            "SELECT f.backup,f.before_hash FROM {table}_write_files f JOIN {table}_writes w ON w.id=f.write_id WHERE w.state='committed' AND f.before_hash IS NOT NULL AND f.before_hash<>f.after_hash"
        );
        let mut q = conn.prepare(&query)?;
        for row in q.query_map([], |r| {
            Ok((r.get::<_, Option<Vec<u8>>>(0)?, r.get::<_, String>(1)?))
        })? {
            let (path, hash) = row?;
            let path = bytes_path(&path.context("Recovery backup path is missing")?)?;
            let observed = location(&path)?;
            ensure!(
                observed.sha256.as_deref() == Some(hash.as_str()),
                "A retained recovery backup is missing or changed"
            );
        }
    }
    let mut q = conn.prepare(
        "SELECT payload_json FROM maintenance_scans WHERE kind='unused_removal_operation'",
    )?;
    for json in q.query_map([], |r| r.get::<_, String>(0))? {
        let op: serde_json::Value = serde_json::from_str(&json?)?;
        if op["state"] == "committed" && op["undone_by"].is_null() && op["backups_disposed"] != true
        {
            for entry in op["files"].as_array().context("Invalid removal journal")? {
                let bytes: Vec<u8> = serde_json::from_value(entry["backup"].clone())?;
                let saved = entry["entry"]["public"]["source_sha256"]
                    .as_str()
                    .context("Invalid removal recovery identity")?;
                ensure!(
                    location(&bytes_path(&bytes)?)?.sha256.as_deref() == Some(saved),
                    "An unused-file recovery backup is missing or changed"
                );
            }
        }
    }
    let sync = root.join("sync");
    if sync.exists() {
        for entry in fs::read_dir(sync)? {
            let entry = entry?;
            if uuid::Uuid::parse_str(&entry.file_name().to_string_lossy()).is_err() {
                continue;
            }
            ensure!(
                entry.file_type()?.is_dir(),
                "Invalid DJ sync recovery entry"
            );
            let transaction = cuerithm_dj_sync::transaction::FileTransaction::open(
                &root.join("sync"),
                &entry.file_name().to_string_lossy(),
            )?;
            use cuerithm_dj_sync::transaction::JournalState;
            let state = transaction.inspect()?.state;
            ensure!(
                matches!(state, JournalState::Applied | JournalState::Restored),
                "Resolve or discard pending DJ sync previews before using a library backup"
            );
            regular(&entry.path().join("library.json"), MAX_MANIFEST)?;
            let saved: serde_json::Value =
                serde_json::from_slice(&fs::read(entry.path().join("library.json"))?)?;
            ensure!(
                saved["error"].is_null()
                    && if state == JournalState::Applied {
                        saved["acknowledged"] == true
                    } else {
                        saved["restore_acknowledged"] == true
                    },
                "DJ sync recovery needs attention"
            );
        }
    }
    Ok(())
}
fn location(path: &Path) -> Result<Location> {
    let (sha256, bytes, kind) = match fs::symlink_metadata(path) {
        Ok(meta) => {
            ensure!(
                !meta.file_type().is_symlink(),
                "Symbolic links are not supported in library backup dependencies"
            );
            if meta.is_dir() {
                (None, 0, "directory")
            } else {
                ensure!(meta.is_file(), "Unsupported library dependency");
                ensure!(
                    meta.len() <= MAX_DEPENDENCY_BYTES,
                    "Library dependency exceeds size budget"
                );
                (Some(cuerithm_media::file_sha256(path)?), meta.len(), "file")
            }
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => (None, 0, "missing"),
        Err(e) => return Err(e.into()),
    };
    Ok(Location {
        path: path_bytes(path),
        sha256,
        bytes,
        kind: kind.into(),
    })
}
fn budget(items: &[Location]) -> Result<()> {
    ensure!(
        items.len() <= MAX_ENTRIES,
        "Too many library backup dependencies"
    );
    let bytes = items.iter().try_fold(0u64, |total, item| {
        total
            .checked_add(item.bytes)
            .context("Library backup size overflow")
    })?;
    ensure!(
        bytes <= MAX_DEPENDENCY_BYTES,
        "Library backup dependencies exceed size budget"
    );
    Ok(())
}
fn inventory(
    conn: &Connection,
    root: &Path,
) -> Result<(Vec<Location>, Vec<Location>, Vec<Location>)> {
    #[cfg(test)]
    inventory_phase_tests::run_hook();
    let mut sources = BTreeMap::new();
    let mut q = conn.prepare("SELECT path,source_hash FROM tracks ORDER BY id")?;
    for row in q.query_map([], |r| {
        Ok((r.get::<_, Vec<u8>>(0)?, r.get::<_, String>(1)?))
    })? {
        let (path, hash) = row?;
        let observed = location(&bytes_path(&path)?)?;
        ensure!(
            observed.kind == "missing" || observed.sha256.as_deref() == Some(hash.as_str()),
            "A source file changed since its saved preparation. Reprepare it before using this backup"
        );
        sources.insert(path, observed);
    }
    let sources = sources.into_values().collect::<Vec<_>>();
    budget(&sources)?;
    let mut models = BTreeMap::new();
    let mut q = conn.prepare("SELECT json FROM settings")?;
    for row in q.query_map([], |r| r.get::<_, String>(0))? {
        let saved: SettingsSnapshot = serde_json::from_str(&row?)?;
        let e = &saved.settings.engine;
        for path in [
            &e.beat_model,
            &e.mel_model,
            &e.key_model,
            &e.key_neural_model,
            &e.key_openkeyscan_model,
            &e.energy_model,
        ]
        .into_iter()
        .flatten()
        {
            let observed = location(path)?;
            ensure!(
                observed.kind == "file",
                "An analysis model required by saved settings is unavailable"
            );
            models.insert(observed.path.clone(), observed);
        }
    }
    let models = models.into_values().collect::<Vec<_>>();
    budget(&models)?;
    let mut resources = Vec::new();
    for name in RESOURCE_DIRS {
        let directory = root.join(name);
        if !directory.exists() {
            continue;
        }
        for entry in walkdir::WalkDir::new(&directory)
            .follow_links(false)
            .max_depth(32)
        {
            let entry = entry?;
            if entry.path() == root.join("sync/service.lock") {
                continue;
            }
            ensure!(
                entry.depth() < 32,
                "Recovery resource tree exceeds depth limit"
            );
            ensure!(
                resources.len() < MAX_ENTRIES,
                "Too many retained recovery resources"
            );
            resources.push(location(entry.path())?);
        }
    }
    resources.sort_by(|a, b| a.path.cmp(&b.path));
    budget(&resources)?;
    Ok((sources, models, resources))
}
fn copy_database(from: &Connection, destination: &Path) -> Result<()> {
    let mut to = Connection::open(destination)?;
    copy_connection(from, &mut to)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(destination, fs::Permissions::from_mode(0o600))?;
    }
    to.pragma_update(None, "journal_mode", "DELETE")?;
    to.close().map_err(|(_, e)| e)?;
    regular(destination, MAX_DATABASE)?;
    File::options()
        .read(true)
        .write(true)
        .open(destination)?
        .sync_all()?;
    Ok(())
}
fn copy_connection(from: &Connection, to: &mut Connection) -> Result<()> {
    let backup = Backup::new(from, to)?;
    let start = Instant::now();
    loop {
        match backup.step(256)? {
            StepResult::Done => break,
            StepResult::More => {}
            StepResult::Busy | StepResult::Locked => std::thread::sleep(Duration::from_millis(10)),
            _ => {}
        }
        ensure!(
            start.elapsed() < Duration::from_secs(60),
            "Library backup timed out; original database is preserved"
        );
    }
    Ok(())
}
fn atomic_json(path: &Path, value: &impl Serialize) -> Result<()> {
    let parent = path.parent().context("Missing backup parent")?;
    let mut temp = tempfile::NamedTempFile::new_in(parent)?;
    temp.write_all(&serde_json::to_vec_pretty(value)?)?;
    temp.as_file().sync_all()?;
    temp.persist(path).map_err(|e| e.error)?;
    sync_directory(parent)
}
fn sync_directory(path: &Path) -> Result<()> {
    #[cfg(unix)]
    File::open(path)?.sync_all()?;
    #[cfg(not(unix))]
    let _ = path;
    Ok(())
}
struct PreparedSnapshot {
    directory: tempfile::TempDir,
    manifest: Manifest,
}
fn prepare_snapshot(
    conn: &Connection,
    root: &Path,
    destination: &Path,
) -> Result<PreparedSnapshot> {
    settled(conn, root)?;
    let schema = schema(conn)?;
    let installation_id = installation_id(root, true)?;
    let (sources, models, resources) = inventory(conn, root)?;
    fs::create_dir_all(destination)?;
    ensure!(
        !fs::symlink_metadata(destination)?.file_type().is_symlink(),
        "Backup destination cannot be a symbolic link"
    );
    let temp = tempfile::Builder::new()
        .prefix(".cuerithm-backup-")
        .tempdir_in(destination)?;
    copy_database(conn, &temp.path().join("library.sqlite"))?;
    ensure!(
        (sources.clone(), models.clone(), resources.clone()) == inventory(conn, root)?,
        "Library dependencies changed while creating the backup; try again"
    );
    let created = now();
    let manifest = Manifest {
        format: 1,
        application: "CueRithm".into(),
        version: env!("CARGO_PKG_VERSION").into(),
        platform: std::env::consts::OS.into(),
        installation: root_identity(root)?,
        installation_id,
        created,
        schema,
        database_sha256: cuerithm_media::file_sha256(&temp.path().join("library.sqlite"))?,
        music_included: false,
        sources,
        models,
        resources,
    };
    atomic_json(&temp.path().join("manifest.json"), &manifest)?;
    regular(&temp.path().join("manifest.json"), MAX_MANIFEST)?;
    Ok(PreparedSnapshot {
        directory: temp,
        manifest,
    })
}
fn publish_snapshot(
    prepared: PreparedSnapshot,
    destination: &Path,
) -> Result<LibraryBackupPreview> {
    let path = destination.join(format!(
        "CueRithm-{}-{}",
        prepared.manifest.created,
        uuid::Uuid::new_v4()
    ));
    fs::rename(prepared.directory.path(), &path)?;
    sync_directory(destination)?;
    preview(&path, &prepared.manifest)
}
fn snapshot(conn: &Connection, root: &Path, destination: &Path) -> Result<LibraryBackupPreview> {
    publish_snapshot(prepare_snapshot(conn, root, destination)?, destination)
}
fn preview(path: &Path, m: &Manifest) -> Result<LibraryBackupPreview> {
    Ok(LibraryBackupPreview {
        path: path.to_path_buf(),
        database_sha256: m.database_sha256.clone(),
        created: m.created,
        tracks: m.sources.len() as u64,
        missing_sources: m.sources.iter().filter(|s| s.kind == "missing").count() as u64,
        schema: m.schema,
        music_included: false,
    })
}
fn inspect(bundle: &Path, root: &Path, current_schema: i64) -> Result<(Manifest, Connection)> {
    let meta = fs::symlink_metadata(bundle)?;
    ensure!(
        meta.is_dir() && !meta.file_type().is_symlink(),
        "Select a regular CueRithm backup folder"
    );
    let entries = fs::read_dir(bundle)?.collect::<std::io::Result<Vec<_>>>()?;
    ensure!(
        entries.len() == 2
            && entries.iter().all(|e| matches!(
                e.file_name().to_str(),
                Some("library.sqlite" | "manifest.json")
            )),
        "Backup contains unexpected entries"
    );
    regular(&bundle.join("manifest.json"), MAX_MANIFEST)?;
    regular(&bundle.join("library.sqlite"), MAX_DATABASE)?;
    let m: Manifest = serde_json::from_slice(&fs::read(bundle.join("manifest.json"))?)?;
    ensure!(
        m.format == 1 && m.application == "CueRithm" && !m.music_included,
        "Unsupported backup format"
    );
    ensure!(
        m.version == env!("CARGO_PKG_VERSION")
            && m.platform == std::env::consts::OS
            && m.installation == root_identity(root)?
            && m.installation_id == installation_id(root, false)?,
        "This backup requires the same CueRithm version, platform and library installation"
    );
    ensure!(
        cuerithm_media::file_sha256(&bundle.join("library.sqlite"))? == m.database_sha256,
        "Backup database hash does not match its manifest"
    );
    let conn = readonly(&bundle.join("library.sqlite"))?;
    ensure!(
        schema(&conn)? == current_schema && m.schema == current_schema,
        "Backup library schema differs from the current library"
    );
    let integrity: String = conn.query_row("PRAGMA quick_check", [], |r| r.get(0))?;
    ensure!(integrity == "ok", "Backup SQLite integrity check failed");
    let violations: bool = conn.prepare("PRAGMA foreign_key_check")?.exists([])?;
    ensure!(!violations, "Backup has broken library references");
    settled(&conn, root)?;
    let (sources, models, resources) = inventory(&conn, root)?;
    ensure!(
        m.sources == sources,
        "Backup source locations have changed. Music files and tag edits are not restored"
    );
    ensure!(
        m.models == models && m.resources == resources,
        "Retained recovery resources or models have changed; this database-only backup cannot be restored safely"
    );
    Ok((m, conn))
}
// This stamp covers mutations on the shared connection as well as commits by
// another SQLite connection. Application/schema identities are checked too; no
// file hashes or filesystem traversal occur while reading or comparing it.
#[derive(Debug, PartialEq, Eq)]
struct DatabaseStamp {
    total_changes: u64,
    data_version: i64,
    schema: i64,
    schema_cookie: i64,
    revision: u64,
}
impl DatabaseStamp {
    fn read(conn: &Connection) -> Result<Self> {
        Ok(Self {
            total_changes: conn.total_changes(),
            data_version: conn.query_row("PRAGMA data_version", [], |row| row.get(0))?,
            schema: schema(conn)?,
            schema_cookie: conn.query_row("PRAGMA schema_version", [], |row| row.get(0))?,
            revision: conn.query_row("SELECT value FROM meta WHERE key='revision'", [], |row| {
                row.get(0)
            })?,
        })
    }
}
struct CapturedDatabase {
    // Close the connection before deleting its private temporary directory.
    connection: Connection,
    _directory: tempfile::TempDir,
    stamp: DatabaseStamp,
}
impl Library {
    fn capture_backup_database(&self) -> Result<CapturedDatabase> {
        let directory = tempfile::Builder::new()
            .prefix("cuerithm-library-snapshot-")
            .tempdir()?;
        let path = directory.path().join("library.sqlite");
        let stamp = {
            let conn = self.conn()?;
            quiet_database(&conn)?;
            let stamp = DatabaseStamp::read(&conn)?;
            copy_database(&conn, &path)?;
            ensure!(
                stamp == DatabaseStamp::read(&conn)?,
                "Library changed while capturing its database; try again"
            );
            stamp
        };
        let connection = readonly(&path)?;
        Ok(CapturedDatabase {
            connection,
            _directory: directory,
            stamp,
        })
    }
    fn revalidate_backup_database(
        &self,
        captured: &CapturedDatabase,
    ) -> Result<std::sync::MutexGuard<'_, Connection>> {
        let conn = self.conn()?;
        ensure!(
            captured.stamp == DatabaseStamp::read(&conn)?,
            "Library changed while checking backup files; try again"
        );
        quiet_database(&conn)?;
        Ok(conn)
    }
    pub fn create_library_backup(&self, destination: &Path) -> Result<LibraryBackupPreview> {
        let _sync = sync_lease(self.data_dir())?;
        #[cfg(feature = "autotag")]
        let _gate = self
            .inner
            .tag_gate
            .lock()
            .map_err(|_| anyhow::anyhow!("Publication gate interrupted"))?;
        let captured = self.capture_backup_database()?;
        let prepared = prepare_snapshot(&captured.connection, self.data_dir(), destination)?;
        let _current = self.revalidate_backup_database(&captured)?;
        publish_snapshot(prepared, destination)
    }
    pub fn inspect_library_backup(&self, bundle: &Path) -> Result<LibraryBackupPreview> {
        let _sync = sync_lease(self.data_dir())?;
        #[cfg(feature = "autotag")]
        let _gate = self
            .inner
            .tag_gate
            .lock()
            .map_err(|_| anyhow::anyhow!("Publication gate interrupted"))?;
        let captured = self.capture_backup_database()?;
        settled(&captured.connection, self.data_dir())?;
        let (manifest, _) = inspect(bundle, self.data_dir(), captured.stamp.schema)?;
        let _current = self.revalidate_backup_database(&captured)?;
        preview(bundle, &manifest)
    }
    pub fn stage_library_restore(
        &self,
        bundle: &Path,
        sha256: &str,
    ) -> Result<LibraryRestoreRequest> {
        let root = self.data_dir();
        let _sync = sync_lease(root)?;
        #[cfg(feature = "autotag")]
        let _gate = self
            .inner
            .tag_gate
            .lock()
            .map_err(|_| anyhow::anyhow!("Publication gate interrupted"))?;
        let captured = self.capture_backup_database()?;
        settled(&captured.connection, root)?;
        let schema = captured.stamp.schema;
        let (m, _) = inspect(bundle, root, schema)?;
        ensure!(
            m.database_sha256 == sha256,
            "Backup selection changed; inspect it again"
        );
        let directory = managed_directory(root)?;
        fs::create_dir_all(&directory)?;
        ensure!(
            !directory.join("pending.json").exists(),
            "A library restore is already scheduled"
        );
        let request = LibraryRestoreRequest {
            id: uuid::Uuid::new_v4().to_string(),
            created: now(),
            database_sha256: sha256.into(),
            blocked_reason: None,
            mutation_started: false,
            pre_restore: None,
        };
        let staging = directory.join(&request.id);
        fs::create_dir(&staging)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&staging, fs::Permissions::from_mode(0o700))?;
        }
        for name in ["library.sqlite", "manifest.json"] {
            let target = staging.join(name);
            fs::copy(bundle.join(name), &target)?;
            File::options()
                .read(true)
                .write(true)
                .open(target)?
                .sync_all()?;
        }
        inspect(&staging, root, schema)?;
        sync_directory(&staging)?;
        let _current = self.revalidate_backup_database(&captured)?;
        atomic_json(&directory.join("pending.json"), &request)?;
        Ok(request)
    }
    pub fn retry_library_restore(&self) -> Result<LibraryRestoreRequest> {
        let root = self.data_dir();
        let _sync = sync_lease(root)?;
        #[cfg(feature = "autotag")]
        let _gate = self
            .inner
            .tag_gate
            .lock()
            .map_err(|_| anyhow::anyhow!("Publication gate interrupted"))?;
        let captured = self.capture_backup_database()?;
        settled(&captured.connection, root)?;
        let mut request = pending(root)?.context("No library restore is scheduled")?;
        ensure!(
            !request.mutation_started,
            "Restore recovery requires the retained pre-restore snapshot"
        );
        let directory = managed_directory(root)?;
        let (manifest, _) = inspect(&directory.join(&request.id), root, captured.stamp.schema)?;
        ensure!(
            manifest.database_sha256 == request.database_sha256,
            "Staged restore identity changed"
        );
        let _current = self.revalidate_backup_database(&captured)?;
        request.blocked_reason = None;
        atomic_json(&directory.join("pending.json"), &request)?;
        Ok(request)
    }
    pub fn pending_library_restore(&self) -> Result<Option<LibraryRestoreRequest>> {
        pending(self.data_dir())
    }
    pub fn cancel_library_restore(&self) -> Result<()> {
        let _sync = sync_lease(self.data_dir())?;
        let request = managed_directory(self.data_dir())?.join("pending.json");
        if request.exists() {
            regular(&request, MAX_MANIFEST)?;
            fs::remove_file(&request)?;
            sync_directory(request.parent().unwrap())?;
        }
        Ok(())
    }
}
fn pending(root: &Path) -> Result<Option<LibraryRestoreRequest>> {
    let path = managed_directory(root)?.join("pending.json");
    if !path.exists() {
        return Ok(None);
    }
    regular(&path, MAX_MANIFEST)?;
    let request: LibraryRestoreRequest = serde_json::from_slice(&fs::read(path)?)?;
    ensure!(
        uuid::Uuid::parse_str(&request.id).is_ok(),
        "Invalid staged library restore ID"
    );
    Ok(Some(request))
}
/// Call before `Library::open` or starting workers. A blocked preflight leaves the
/// original untouched and permits normal startup with a visible cancellable intent.
/// An error after durable mutation intent must stop startup for recovery.
pub fn apply_pending_library_restore(database: &Path) -> Result<LibraryRestoreStartup> {
    let root = database.parent().context("Library has no data directory")?;
    let Some(mut request) = pending(root)? else {
        return Ok(LibraryRestoreStartup::NoPending);
    };
    if let Some(reason) = request.blocked_reason.clone() {
        ensure!(
            !request.mutation_started,
            "Library restore stopped during database update; retain the pre-restore snapshot and stop startup"
        );
        return Ok(LibraryRestoreStartup::BlockedBeforeMutation { request, reason });
    }
    let mut lock = database.as_os_str().to_owned();
    lock.push(".lock");
    let _lease = file_lease(
        Path::new(&lock),
        "Close CueRithm before restoring this library",
    )?;
    let _sync = sync_lease(root)?;
    regular(database, MAX_DATABASE)?;
    let mut current = Connection::open(database)?;
    current.busy_timeout(Duration::from_secs(5))?;
    let directory = managed_directory(root)?;
    let validation = (|| -> Result<(Connection, PathBuf)> {
        settled(&current, root)?;
        let version = schema(&current)?;
        let (manifest, source) = inspect(&directory.join(&request.id), root, version)?;
        ensure!(
            manifest.database_sha256 == request.database_sha256,
            "Staged restore identity changed"
        );
        let preserved = if request.mutation_started {
            let saved = request
                .pre_restore
                .as_ref()
                .context("Pre-restore recovery snapshot is missing")?;
            ensure!(
                !saved
                    .components()
                    .any(|part| matches!(part, std::path::Component::ParentDir))
                    && saved.canonicalize()?.starts_with(directory.canonicalize()?),
                "Invalid pre-restore recovery location"
            );
            inspect(saved, root, version)?;
            saved.clone()
        } else {
            let pre = directory.join(format!(
                "pre-restore-{}-{}",
                request.id,
                uuid::Uuid::new_v4()
            ));
            let temporary = tempfile::Builder::new()
                .prefix(".pre-restore-")
                .tempdir_in(&directory)?;
            let saved = snapshot(&current, root, temporary.path())?;
            let name = saved
                .path
                .file_name()
                .context("Pre-restore snapshot has no name")?
                .to_owned();
            fs::rename(temporary.path(), &pre)?;
            sync_directory(&directory)?;
            let saved = pre.join(name);
            inspect(&saved, root, version)?;
            saved
        };
        Ok((source, preserved))
    })();
    let (source, preserved) = match validation {
        Ok(prepared) => prepared,
        Err(error) => {
            if request.mutation_started {
                return Err(error.context("Restore recovery stopped after database update intent; do not open this library until its pre-restore snapshot can be recovered"));
            }
            let reason = format!("{error:#}");
            request.blocked_reason = Some(reason.clone());
            atomic_json(&directory.join("pending.json"), &request)?;
            return Ok(LibraryRestoreStartup::BlockedBeforeMutation { request, reason });
        }
    };
    // Durable intent distinguishes a harmless failed preflight from a process crash
    // during SQLite's atomic update. Never replace its original recovery snapshot.
    request.pre_restore = Some(preserved.clone());
    request.mutation_started = true;
    atomic_json(&directory.join("pending.json"), &request)?;
    copy_connection(&source, &mut current)
        .context("Library restore stopped during database update; pre-restore snapshot retained")?;
    current.execute_batch("PRAGMA wal_checkpoint(TRUNCATE);")?;
    atomic_json(
        &directory.join(format!("applied-{}.json", request.id)),
        &request,
    )?;
    fs::remove_file(directory.join("pending.json"))?;
    sync_directory(&directory)?;
    Ok(LibraryRestoreStartup::Applied {
        pre_restore: preserved,
    })
}

#[cfg(test)]
mod inventory_phase_tests {
    use super::*;
    use std::cell::RefCell;
    thread_local! {
        static HOOK: RefCell<Option<Box<dyn FnOnce()>>> = const { RefCell::new(None) };
    }
    pub(super) fn run_hook() {
        let hook = HOOK.with(|hook| hook.borrow_mut().take());
        if let Some(hook) = hook {
            hook();
        }
    }
    fn during_inventory(action: impl FnOnce() + 'static) {
        HOOK.with(|hook| *hook.borrow_mut() = Some(Box::new(action)));
    }
    fn fixture() -> (tempfile::TempDir, Library) {
        let directory = tempfile::tempdir().unwrap();
        let library = Library::open(&directory.path().join("app/library.sqlite")).unwrap();
        let path = directory.path().join("authored.wav");
        fs::write(&path, b"authored source bytes").unwrap();
        library.conn().unwrap().execute("INSERT INTO tracks(id,path,title,source_hash,state,created,updated) VALUES('fixture',?1,'Authored',?2,'ready',0,0)",rusqlite::params![path_bytes(&path),cuerithm_media::file_sha256(&path).unwrap()]).unwrap();
        (directory, library)
    }
    fn query_during_inventory(library: &Library) {
        let library = library.clone();
        during_inventory(move || {
            // Inventory stays paused until an actual query on another thread
            // completes. try_lock makes the old retained-lock behavior fail
            // immediately instead of relying on sleeps or a timing threshold.
            std::thread::spawn(move || {
                let connection = library
                    .inner
                    .conn
                    .try_lock()
                    .expect("Source inventory must leave ordinary library queries available");
                let expected: u64 = connection
                    .query_row("SELECT value FROM meta WHERE key='revision'", [], |r| {
                        r.get(0)
                    })
                    .unwrap();
                drop(connection);
                assert_eq!(library.revision().unwrap(), expected);
            })
            .join()
            .unwrap();
        });
    }
    #[test]
    fn backup_inventory_keeps_library_queries_available() {
        let (d, library) = fixture();
        query_during_inventory(&library);
        library
            .create_library_backup(&d.path().join("backups"))
            .unwrap();
    }
    #[test]
    fn inspect_inventory_keeps_library_queries_available() {
        let (d, library) = fixture();
        let backup = library
            .create_library_backup(&d.path().join("backups"))
            .unwrap();
        query_during_inventory(&library);
        library.inspect_library_backup(&backup.path).unwrap();
    }
    #[test]
    fn stage_inventory_keeps_library_queries_available() {
        let (d, library) = fixture();
        let backup = library
            .create_library_backup(&d.path().join("backups"))
            .unwrap();
        query_during_inventory(&library);
        library
            .stage_library_restore(&backup.path, &backup.database_sha256)
            .unwrap();
    }
    #[test]
    fn retry_inventory_keeps_library_queries_available() {
        let (d, library) = fixture();
        let backup = library
            .create_library_backup(&d.path().join("backups"))
            .unwrap();
        library
            .stage_library_restore(&backup.path, &backup.database_sha256)
            .unwrap();
        query_during_inventory(&library);
        library.retry_library_restore().unwrap();
    }
    fn edit_during_inventory(library: &Library) {
        let library = library.clone();
        during_inventory(move || {
            let connection = library
                .inner
                .conn
                .try_lock()
                .expect("Editing during inventory must not block on its global connection");
            connection
                .execute(
                    "UPDATE tracks SET title='Edited during backup inventory' WHERE id='fixture'",
                    [],
                )
                .unwrap();
        });
    }
    #[test]
    fn backup_refuses_to_publish_after_a_library_edit_during_inventory() {
        let (d, library) = fixture();
        let destination = d.path().join("backups");
        edit_during_inventory(&library);
        let error = library.create_library_backup(&destination).unwrap_err();
        assert!(error.to_string().contains("Library changed"));
        assert_eq!(fs::read_dir(destination).unwrap().count(), 0);
    }
    #[test]
    fn inspection_refuses_a_stale_preview_after_a_library_edit() {
        let (d, library) = fixture();
        let backup = library
            .create_library_backup(&d.path().join("backups"))
            .unwrap();
        edit_during_inventory(&library);
        assert!(
            library
                .inspect_library_backup(&backup.path)
                .unwrap_err()
                .to_string()
                .contains("Library changed")
        );
    }
    #[test]
    fn staging_does_not_publish_intent_after_a_library_edit() {
        let (d, library) = fixture();
        let backup = library
            .create_library_backup(&d.path().join("backups"))
            .unwrap();
        edit_during_inventory(&library);
        assert!(
            library
                .stage_library_restore(&backup.path, &backup.database_sha256)
                .unwrap_err()
                .to_string()
                .contains("Library changed")
        );
        assert!(library.pending_library_restore().unwrap().is_none());
    }
    #[test]
    fn retry_does_not_clear_a_blocked_intent_after_a_library_edit() {
        let (d, library) = fixture();
        let backup = library
            .create_library_backup(&d.path().join("backups"))
            .unwrap();
        let mut request = library
            .stage_library_restore(&backup.path, &backup.database_sha256)
            .unwrap();
        request.blocked_reason = Some("Original blocked reason".into());
        atomic_json(
            &library.data_dir().join("library-backups/pending.json"),
            &request,
        )
        .unwrap();
        edit_during_inventory(&library);
        assert!(
            library
                .retry_library_restore()
                .unwrap_err()
                .to_string()
                .contains("Library changed")
        );
        assert_eq!(
            library
                .pending_library_restore()
                .unwrap()
                .unwrap()
                .blocked_reason
                .as_deref(),
            Some("Original blocked reason")
        );
    }
    #[test]
    fn external_sqlite_edits_during_inventory_invalidate_the_capture() {
        let (d, library) = fixture();
        let path = library.data_dir().join("library.sqlite");
        let original_changes = library.conn().unwrap().total_changes();
        during_inventory(move || {
            Connection::open(path)
                .unwrap()
                .execute(
                    "UPDATE tracks SET title='Independent connection edit' WHERE id='fixture'",
                    [],
                )
                .unwrap();
        });
        assert!(
            library
                .create_library_backup(&d.path().join("backups"))
                .unwrap_err()
                .to_string()
                .contains("Library changed")
        );
        assert_eq!(library.conn().unwrap().total_changes(), original_changes);
    }
}
