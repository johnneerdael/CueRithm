use anyhow::{Context, Result, ensure};
use cuerithm_media::CancellationToken;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    fs::File,
    hash::{Hash, Hasher},
    io::Read,
    path::{Path, PathBuf},
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct MediaGuard {
    #[serde(with = "cuerithm_dj_sync::path_serde")]
    pub path: PathBuf,
    pub sha256: Option<String>,
    pub object: Option<String>,
    pub bytes: Option<u64>,
}
struct IdentityHasher(Sha256);
impl Hasher for IdentityHasher {
    fn write(&mut self, bytes: &[u8]) {
        self.0.update(bytes)
    }
    fn finish(&self) -> u64 {
        let bytes = self.0.clone().finalize();
        u64::from_le_bytes(bytes[..8].try_into().expect("SHA digest length"))
    }
}
fn identity(file: &File) -> Result<String> {
    let handle = same_file::Handle::from_file(file.try_clone()?)?;
    let mut digest = IdentityHasher(Sha256::new());
    handle.hash(&mut digest);
    Ok(format!("{:x}", digest.0.finalize()))
}
impl MediaGuard {
    pub fn capture(path: &Path, cancel: &CancellationToken) -> Result<Self> {
        cancel.check()?;
        let mut guard = Self {
            path: path.into(),
            sha256: None,
            object: None,
            bytes: None,
        };
        let mut file = match File::open(path) {
            Ok(file) => file,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(guard),
            Err(e) => return Err(e).context("Cannot inspect selected media"),
        };
        let before = file.metadata()?;
        ensure!(before.is_file(), "Selected media is not a regular file");
        ensure!(
            before.len() <= 8 * 1024 * 1024 * 1024,
            "Selected media exceeds import byte limit"
        );
        let object = identity(&file)?;
        let mut digest = Sha256::new();
        let mut bytes = 0u64;
        let mut buffer = [0u8; 128 * 1024];
        loop {
            cancel.check()?;
            let n = file.read(&mut buffer)?;
            if n == 0 {
                break;
            }
            bytes = bytes
                .checked_add(n as u64)
                .context("Media byte count overflow")?;
            ensure!(
                bytes <= 8 * 1024 * 1024 * 1024,
                "Selected media grew beyond import limit"
            );
            digest.update(&buffer[..n]);
        }
        let after = file.metadata()?;
        ensure!(
            bytes == before.len()
                && after.len() == before.len()
                && after.modified()? == before.modified()?,
            "Media changed during import preview"
        );
        let current = File::open(path)?;
        ensure!(
            identity(&current)? == object,
            "Media replaced during import preview"
        );
        guard.sha256 = Some(format!("{:x}", digest.finalize()));
        guard.object = Some(object);
        guard.bytes = Some(bytes);
        Ok(guard)
    }
    pub fn validate(&self, cancel: &CancellationToken) -> Result<()> {
        ensure!(
            &Self::capture(&self.path, cancel)? == self,
            "Media changed after import preview"
        );
        Ok(())
    }
}
