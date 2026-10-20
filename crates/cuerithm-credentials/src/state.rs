use crate::ProviderId;
use serde::{Deserialize, Serialize};
use uuid::Uuid;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StateError {
    Unavailable,
    Corrupt,
    Conflict,
    UnsafePath,
    Busy,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MutationKind {
    Save,
    Remove,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MutationState {
    pub generation: Uuid,
    pub pending: Option<MutationKind>,
}
pub trait StateStore: Send + Sync + 'static {
    fn claim_authority(&self) -> Result<Box<dyn Send + Sync>, StateError>;
    fn load_or_create(&self, provider: ProviderId) -> Result<MutationState, StateError>;
    fn compare_exchange(
        &self,
        provider: ProviderId,
        expected: &MutationState,
        next: &MutationState,
    ) -> Result<(), StateError>;
}

/// Private non-secret marker state under explicitly supplied application data.
pub struct FileStateStore {
    directory: std::path::PathBuf,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct StateFile {
    schema: u32,
    credentials: std::collections::BTreeMap<ProviderId, MutationState>,
}
impl FileStateStore {
    pub fn open(app_data: &std::path::Path) -> Result<Self, StateError> {
        let app_metadata =
            std::fs::symlink_metadata(app_data).map_err(|_| StateError::Unavailable)?;
        if !app_metadata.is_dir() || app_metadata.file_type().is_symlink() {
            return Err(StateError::UnsafePath);
        }
        let app_data = app_data
            .canonicalize()
            .map_err(|_| StateError::Unavailable)?;
        let directory = app_data.join("credentials-state");
        let mut builder = std::fs::DirBuilder::new();
        #[cfg(unix)]
        {
            use std::os::unix::fs::DirBuilderExt;
            builder.mode(0o700);
        }
        match builder.create(&directory) {
            Ok(()) => sync_directory(&app_data)?,
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(_) => return Err(StateError::Unavailable),
        }
        let store = Self { directory };
        store.check_directory()?;
        Ok(store)
    }
    fn check_directory(&self) -> Result<(), StateError> {
        let m = std::fs::symlink_metadata(&self.directory).map_err(|_| StateError::Unavailable)?;
        if !m.is_dir() || unsafe_entry(&m) {
            return Err(StateError::UnsafePath);
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            if m.permissions().mode() & 0o077 != 0 {
                return Err(StateError::UnsafePath);
            }
        }
        Ok(())
    }
    fn open_lock(&self, name: &str) -> Result<std::fs::File, StateError> {
        self.check_directory()?;
        let path = self.directory.join(name);
        let mut options = std::fs::OpenOptions::new();
        options.read(true).write(true).create(true).truncate(false);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600).custom_flags(libc::O_NOFOLLOW);
        }
        #[cfg(windows)]
        {
            use std::os::windows::fs::OpenOptionsExt;
            options.custom_flags(0x00200000);
        }
        let file = options.open(path).map_err(|_| StateError::Unavailable)?;
        if !file
            .metadata()
            .map_err(|_| StateError::Unavailable)?
            .is_file()
            || unsafe_entry(&file.metadata().map_err(|_| StateError::Unavailable)?)
        {
            return Err(StateError::UnsafePath);
        }
        Ok(file)
    }
    fn locked(&self) -> Result<std::fs::File, StateError> {
        let file = self.open_lock("state.lock")?;
        file.lock().map_err(|_| StateError::Unavailable)?;
        Ok(file)
    }
    fn read(&self) -> Result<StateFile, StateError> {
        use std::io::Read;
        let path = self.directory.join("state.json");
        match std::fs::symlink_metadata(&path) {
            Ok(m) if !m.is_file() || unsafe_entry(&m) => return Err(StateError::UnsafePath),
            Ok(_) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                return Ok(StateFile {
                    schema: 1,
                    credentials: Default::default(),
                });
            }
            Err(_) => return Err(StateError::Unavailable),
        }
        let mut options = std::fs::OpenOptions::new();
        options.read(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.custom_flags(libc::O_NOFOLLOW);
        }
        #[cfg(windows)]
        {
            use std::os::windows::fs::OpenOptionsExt;
            options.custom_flags(0x00200000);
        }
        let file = options.open(path).map_err(|_| StateError::Unavailable)?;
        let metadata = file.metadata().map_err(|_| StateError::Unavailable)?;
        if !metadata.is_file() || unsafe_entry(&metadata) {
            return Err(StateError::UnsafePath);
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            if metadata.permissions().mode() & 0o077 != 0 {
                return Err(StateError::UnsafePath);
            }
        }
        let mut bytes = Vec::new();
        file.take(65537)
            .read_to_end(&mut bytes)
            .map_err(|_| StateError::Unavailable)?;
        if bytes.len() > 65536 {
            return Err(StateError::Corrupt);
        }
        let state: StateFile = serde_json::from_slice(&bytes).map_err(|_| StateError::Corrupt)?;
        if state.schema != 1 || state.credentials.values().any(|v| v.generation.is_nil()) {
            return Err(StateError::Corrupt);
        }
        Ok(state)
    }
    fn write(&self, state: &StateFile) -> Result<(), StateError> {
        self.check_directory()?;
        let mut temporary = tempfile::NamedTempFile::new_in(&self.directory)
            .map_err(|_| StateError::Unavailable)?;
        serde_json::to_writer(&mut temporary, state).map_err(|_| StateError::Unavailable)?;
        temporary
            .as_file()
            .sync_all()
            .map_err(|_| StateError::Unavailable)?;
        temporary
            .persist(self.directory.join("state.json"))
            .map_err(|_| StateError::Unavailable)?;
        sync_directory(&self.directory)
    }
}
impl StateStore for FileStateStore {
    fn claim_authority(&self) -> Result<Box<dyn Send + Sync>, StateError> {
        let file = self.open_lock("authority.lock")?;
        file.try_lock().map_err(|_| StateError::Busy)?;
        Ok(Box::new(file))
    }

    fn load_or_create(&self, provider: ProviderId) -> Result<MutationState, StateError> {
        let _guard = self.locked()?;
        let mut state = self.read()?;
        if let Some(value) = state.credentials.get(&provider) {
            return Ok(value.clone());
        }
        let value = MutationState {
            generation: Uuid::new_v4(),
            pending: None,
        };
        state.credentials.insert(provider, value.clone());
        self.write(&state)?;
        Ok(value)
    }
    fn compare_exchange(
        &self,
        provider: ProviderId,
        expected: &MutationState,
        next: &MutationState,
    ) -> Result<(), StateError> {
        let _guard = self.locked()?;
        let mut state = self.read()?;
        if state.credentials.get(&provider) != Some(expected) {
            return Err(StateError::Conflict);
        }
        if next.generation.is_nil() {
            return Err(StateError::Corrupt);
        }
        state.credentials.insert(provider, next.clone());
        self.write(&state)
    }
}
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
fn sync_directory(path: &std::path::Path) -> Result<(), StateError> {
    #[cfg(unix)]
    {
        std::fs::File::open(path)
            .and_then(|file| file.sync_all())
            .map_err(|_| StateError::Unavailable)
    }
    // Windows flushes the staged file before atomic replacement. There is no portable directory fsync.
    #[cfg(not(unix))]
    {
        let _ = path;
        Ok(())
    }
}
