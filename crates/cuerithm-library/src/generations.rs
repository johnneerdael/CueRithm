//! Closed-generation activation. Scheduling never changes the running library.
//! Startup owns the installation lease and switches before any worker is started.
use anyhow::{Context, Result, ensure};
use rusqlite::{Connection, OpenFlags};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    fs::{self, File},
    io::{Read, Write},
    path::{Path, PathBuf},
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LibraryGeneration {
    pub id: String,
    pub created: i64,
    pub tracks: u64,
    pub database: PathBuf,
}
#[derive(Clone)]
pub struct LibraryGenerations {
    root: PathBuf,
}
#[derive(Serialize, Deserialize, Clone)]
#[serde(deny_unknown_fields)]
struct Pointer {
    generation: String,
    checksum: String,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Activation {
    old: String,
    new: String,
    seal: String,
    phase: String,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct GenerationSeal {
    generation: String,
    sha256: String,
    created: i64,
}
impl Pointer {
    fn new(generation: &str) -> Self {
        Self {
            generation: generation.into(),
            checksum: format!(
                "{:x}",
                Sha256::digest(format!("cuerithm-active-v1:{generation}"))
            ),
        }
    }
    fn valid(&self) -> bool {
        Self::new(&self.generation).checksum == self.checksum
    }
}
fn regular(path: &Path) -> Result<()> {
    let m = fs::symlink_metadata(path)?;
    ensure!(
        m.is_file() && !m.file_type().is_symlink(),
        "Generation entry is not a regular file"
    );
    Ok(())
}
fn digest(path: &Path) -> Result<String> {
    regular(path)?;
    let mut f = File::open(path)?;
    let mut h = Sha256::new();
    let mut buf = [0; 65536];
    loop {
        let n = f.read(&mut buf)?;
        if n == 0 {
            break;
        }
        h.update(&buf[..n]);
    }
    Ok(format!("{:x}", h.finalize()))
}
fn lease_database(path: &Path) -> Result<File> {
    let mut name = path.as_os_str().to_owned();
    name.push(".lock");
    let name = PathBuf::from(name);
    if name.exists() {
        regular(&name)?;
    }
    let f = File::options()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(name)?;
    fs2::FileExt::try_lock_exclusive(&f).context("Library still has an open owner")?;
    Ok(f)
}
fn sync_dir(path: &Path) -> Result<()> {
    #[cfg(unix)]
    File::open(path)?.sync_all()?;
    #[cfg(not(unix))]
    let _ = path;
    Ok(())
}
fn atomic_json(path: &Path, value: &impl Serialize) -> Result<()> {
    let parent = path.parent().context("Missing pointer directory")?;
    let mut tmp = tempfile::NamedTempFile::new_in(parent)?;
    tmp.write_all(&serde_json::to_vec(value)?)?;
    tmp.as_file().sync_all()?;
    #[cfg(unix)]
    tmp.persist(path).map_err(|e| e.error)?;
    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStrExt;
        use windows_sys::Win32::Storage::FileSystem::{
            MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH, MoveFileExW,
        };
        let from: Vec<u16> = tmp
            .path()
            .as_os_str()
            .encode_wide()
            .chain(Some(0))
            .collect();
        let to: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
        ensure!(
            unsafe {
                MoveFileExW(
                    from.as_ptr(),
                    to.as_ptr(),
                    MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
                )
            } != 0,
            "Cannot replace active library pointer: {}",
            std::io::Error::last_os_error()
        );
    }
    sync_dir(parent)
}
impl LibraryGenerations {
    pub fn new(root: &Path) -> Result<Self> {
        fs::create_dir_all(root)?;
        let root = root.canonicalize()?;
        let libraries = root.join("libraries");
        if libraries.exists() {
            ensure!(
                !fs::symlink_metadata(&libraries)?.file_type().is_symlink(),
                "Generation directory cannot be a symlink"
            );
        }
        fs::create_dir_all(&libraries)?;
        Ok(Self { root })
    }
    pub fn root(&self) -> &Path {
        &self.root
    }
    pub fn database_path(&self, id: &str) -> Result<PathBuf> {
        if id == "legacy" {
            return Ok(self.root.join("library.sqlite"));
        }
        ensure!(
            uuid::Uuid::parse_str(id).is_ok(),
            "Invalid generation identity"
        );
        let dir = self.root.join("libraries").join(id);
        if dir.exists() {
            ensure!(
                fs::symlink_metadata(&dir)?.is_dir()
                    && !fs::symlink_metadata(&dir)?.file_type().is_symlink(),
                "Unsafe generation directory"
            );
        }
        Ok(dir.join("library.sqlite"))
    }
    fn pointer(&self) -> PathBuf {
        self.root.join("active-library.json")
    }
    fn journal(&self) -> PathBuf {
        self.root.join("library-activation.json")
    }
    pub fn active_id(&self) -> Result<String> {
        if !self.pointer().exists() {
            return Ok("legacy".into());
        }
        regular(&self.pointer())?;
        ensure!(
            fs::metadata(self.pointer())?.len() <= 4096,
            "Invalid active pointer length"
        );
        let p: Pointer = serde_json::from_slice(&fs::read(self.pointer())?)?;
        ensure!(p.valid(), "Invalid active library pointer checksum");
        regular(&self.database_path(&p.generation)?)?;
        Ok(p.generation)
    }
    fn read_seal(&self, id: &str) -> Result<GenerationSeal> {
        ensure!(id != "legacy", "Legacy generation has no staging seal");
        let path = self
            .database_path(id)?
            .with_file_name("generation-seal.json");
        regular(&path)?;
        ensure!(
            fs::metadata(&path)?.len() <= 4096,
            "Invalid generation seal length"
        );
        let seal: GenerationSeal = serde_json::from_slice(&fs::read(path)?)?;
        ensure!(
            seal.generation == id
                && seal.sha256.len() == 64
                && seal.sha256.bytes().all(|b| b.is_ascii_hexdigit()),
            "Invalid generation seal"
        );
        Ok(seal)
    }
    pub fn create_staging(&self) -> Result<LibraryGeneration> {
        let id = uuid::Uuid::new_v4().to_string();
        let database = self.database_path(&id)?;
        fs::create_dir(database.parent().unwrap())?;
        sync_dir(&self.root.join("libraries"))?;
        Ok(LibraryGeneration {
            id,
            created: crate::now(),
            tracks: 0,
            database,
        })
    }
    pub fn seal(&self, id: &str) -> Result<String> {
        let path = self.database_path(id)?;
        regular(&path)?;
        let _lease = lease_database(&path)?;
        let conn = Connection::open(&path)?;
        conn.busy_timeout(std::time::Duration::from_secs(2))?;
        let checkpoint: (i64, i64, i64) =
            conn.query_row("PRAGMA wal_checkpoint(TRUNCATE)", [], |r| {
                Ok((r.get(0)?, r.get(1)?, r.get(2)?))
            })?;
        ensure!(checkpoint.0 == 0, "Staging database still has open writers");
        let integrity: String = conn.query_row("PRAGMA integrity_check", [], |r| r.get(0))?;
        ensure!(integrity == "ok", "Staging integrity check failed");
        let fks: i64 =
            conn.query_row("SELECT COUNT(*) FROM pragma_foreign_key_check", [], |r| {
                r.get(0)
            })?;
        ensure!(fks == 0, "Staging foreign key check failed");
        conn.close().map_err(|(_, e)| e)?;
        File::open(&path)?.sync_all()?;
        sync_dir(path.parent().unwrap())?;
        self.check_wal(&path)?;
        let sha256 = digest(&path)?;
        if id != "legacy" {
            atomic_json(
                &path.with_file_name("generation-seal.json"),
                &GenerationSeal {
                    generation: id.into(),
                    sha256: sha256.clone(),
                    created: crate::now(),
                },
            )?;
        }
        Ok(sha256)
    }
    fn check_wal(&self, path: &Path) -> Result<()> {
        let mut wal = path.as_os_str().to_owned();
        wal.push("-wal");
        match fs::symlink_metadata(PathBuf::from(wal)) {
            Ok(m) => ensure!(
                m.is_file() && !m.file_type().is_symlink() && m.len() == 0,
                "Sealed generation has unsafe or uncheckpointed WAL"
            ),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(e.into()),
        }
        Ok(())
    }
    pub fn schedule(&self, id: &str, expected_active: &str, seal: &str) -> Result<()> {
        let installation = File::options()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(self.root.join("generation-switch.lock"))?;
        fs2::FileExt::try_lock_exclusive(&installation)
            .context("Another library switch is running")?;
        ensure!(
            !self.root.join("library-backups/pending.json").exists(),
            "Finish or cancel the scheduled library backup restore first"
        );
        ensure!(
            self.active_id()? == expected_active,
            "Active library changed; preview again"
        );
        ensure!(id != expected_active, "Generation is already active");
        let path = self.database_path(id)?;
        if id != "legacy" {
            ensure!(
                self.read_seal(id)?.sha256 == seal,
                "Generation seal does not match reviewed database"
            );
        }
        regular(&self.database_path(expected_active)?)?;
        self.check_wal(&path)?;
        ensure!(
            digest(&path)? == seal,
            "Staged generation changed; preview again"
        );
        ensure!(
            !self.journal().exists(),
            "Another library activation is scheduled"
        );
        atomic_json(
            &self.root.join("last-good-library.json"),
            &Pointer::new(expected_active),
        )?;
        atomic_json(
            &self.journal(),
            &Activation {
                old: expected_active.into(),
                new: id.into(),
                seal: seal.into(),
                phase: "validated".into(),
            },
        )
    }
    /// Cancel only a scheduled restart; a switch already in progress must finish
    /// or roll back through startup recovery.
    pub fn cancel_scheduled(&self) -> Result<bool> {
        let installation = File::options()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(self.root.join("generation-switch.lock"))?;
        fs2::FileExt::try_lock_exclusive(&installation)
            .context("A library switch is already running")?;
        if !self.journal().exists() {
            return Ok(false);
        }
        regular(&self.journal())?;
        ensure!(
            fs::metadata(self.journal())?.len() <= 16384,
            "Activation journal exceeds limit"
        );
        let j: Activation = serde_json::from_slice(&fs::read(self.journal())?)?;
        ensure!(
            j.phase == "validated" && self.active_id()? == j.old,
            "Library activation has already started"
        );
        fs::remove_file(self.journal())?;
        sync_dir(&self.root)?;
        Ok(true)
    }
    /// Called only before constructing Host, hence no active process DB handles.
    pub fn open_active(&self) -> Result<crate::Library> {
        let mut installation = File::options()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(self.root.join("generation-switch.lock"))?;
        fs2::FileExt::try_lock_exclusive(&installation)
            .context("Another library switch is running")?;
        let mut active = match self.active_id() {
            Ok(id) => id,
            Err(error) => {
                let last_good = self.root.join("last-good-library.json");
                regular(&last_good)?;
                ensure!(
                    fs::metadata(&last_good)?.len() <= 4096,
                    "Invalid last-good pointer length"
                );
                let p: Pointer = serde_json::from_slice(
                    &fs::read(self.root.join("last-good-library.json"))
                        .context(error.to_string())?,
                )?;
                ensure!(p.valid(), "Last-good pointer is invalid");
                regular(&self.database_path(&p.generation)?)?;
                atomic_json(&self.pointer(), &p)?;
                p.generation
            }
        };
        if self.journal().exists() {
            regular(&self.journal())?;
            ensure!(
                fs::metadata(self.journal())?.len() <= 16384,
                "Activation journal exceeds limit"
            );
            let mut j: Activation = serde_json::from_slice(&fs::read(self.journal())?)?;
            ensure!(
                j.old != j.new && matches!(j.phase.as_str(), "validated" | "switched" | "opened"),
                "Invalid activation journal phase"
            );
            let old = self.database_path(&j.old)?;
            regular(&old)?;
            let new = self.database_path(&j.new)?;
            ensure!(
                active == j.old || active == j.new,
                "Activation journal belongs to another active library"
            );
            // Acquire both old/new library leases before switching. Another
            // running process prevents startup activation rather than sharing it.
            let old_lease = lease_database(&old)?;
            let new_lease = if regular(&new).is_ok() {
                Some(lease_database(&new)?)
            } else {
                None
            };
            let validated = new_lease.is_some()
                && self.check_wal(&new).is_ok()
                && digest(&new).is_ok_and(|h| h == j.seal)
                && (j.new == "legacy"
                    || self
                        .read_seal(&j.new)
                        .is_ok_and(|seal| seal.sha256 == j.seal));
            if validated {
                j.phase = "switched".into();
                atomic_json(&self.journal(), &j)?;
                atomic_json(&self.pointer(), &Pointer::new(&j.new))?;
                active = j.new.clone();
            } else {
                atomic_json(&self.pointer(), &Pointer::new(&j.old))?;
                active = j.old.clone();
            }
            drop(new_lease);
            drop(old_lease);
            match open_generation(self, &self.database_path(&active)?) {
                Ok(library) => {
                    j.phase = "opened".into();
                    atomic_json(&self.root.join("last-library-activation.json"), &j)?;
                    fs::remove_file(self.journal())?;
                    sync_dir(&self.root)?;
                    return Ok(library);
                }
                Err(error) if active == j.new => {
                    atomic_json(&self.pointer(), &Pointer::new(&j.old))?;
                    let library = open_generation(self, &old).context(format!(
                        "New library could not open ({error}); rollback failed"
                    ))?;
                    fs::remove_file(self.journal())?;
                    sync_dir(&self.root)?;
                    return Ok(library);
                }
                Err(error) => return Err(error),
            }
        }
        let library = open_generation(self, &self.database_path(&active)?)?;
        let _ = &mut installation;
        Ok(library)
    }
    pub fn retained(&self) -> Result<Vec<LibraryGeneration>> {
        let active = self.active_id()?;
        let mut ids = vec!["legacy".to_owned()];
        for entry in fs::read_dir(self.root.join("libraries"))? {
            let entry = entry?;
            if entry.file_type()?.is_dir() {
                ids.push(entry.file_name().to_string_lossy().into());
            }
        }
        let mut out = vec![];
        for id in ids {
            if id == active {
                continue;
            }
            let Ok(database) = self.database_path(&id) else {
                continue;
            };
            let observed = (|| -> Result<(u64, i64)> {
                regular(&database)?;
                if id != "legacy" {
                    self.read_seal(&id)?;
                }
                let conn = Connection::open_with_flags(
                    &database,
                    OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
                )?;
                conn.pragma_update(None, "trusted_schema", false)?;
                let app: i64 = conn.query_row("PRAGMA application_id", [], |r| r.get(0))?;
                ensure!(app == 1129665093, "Not a CueRithm generation");
                let integrity: String = conn.query_row("PRAGMA quick_check", [], |r| r.get(0))?;
                ensure!(integrity == "ok", "Damaged retained generation");
                let tracks = conn.query_row("SELECT COUNT(*) FROM tracks", [], |r| r.get(0))?;
                let created = if id != "legacy" {
                    self.read_seal(&id)?.created
                } else {
                    fs::metadata(&database)?
                        .modified()?
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_millis()
                        .min(i64::MAX as u128) as i64
                };
                Ok((tracks, created))
            })();
            let Ok((tracks, created)) = observed else {
                continue;
            };
            out.push(LibraryGeneration {
                id,
                created,
                tracks,
                database,
            });
        }
        out.sort_by(|a, b| b.created.cmp(&a.created).then(a.id.cmp(&b.id)));
        Ok(out)
    }
}

fn open_generation(manager: &LibraryGenerations, path: &Path) -> Result<crate::Library> {
    crate::library_backup::apply_pending_in_installation(path, manager.root(), &|data| {
        let id = if data == manager.root() {
            "legacy"
        } else {
            data.file_name()
                .and_then(|id| id.to_str())
                .context("Invalid restored generation")?
        };
        ensure!(
            manager.database_path(id)?.parent() == Some(data),
            "Invalid restored data directory"
        );
        if id != "legacy" {
            atomic_json(
                &data.join("generation-seal.json"),
                &GenerationSeal {
                    generation: id.into(),
                    sha256: digest(&data.join("library.sqlite"))?,
                    created: crate::now(),
                },
            )?;
        }
        if manager.active_id()? != id {
            atomic_json(
                &manager.root.join("last-good-library.json"),
                &Pointer::new(&manager.active_id()?),
            )?;
            atomic_json(&manager.pointer(), &Pointer::new(id))?;
        }
        Ok(())
    })?;
    let active = manager.database_path(&manager.active_id()?)?;
    crate::Library::open_in_installation(&active, manager.root())
}
