use super::{ImportLimits, SourceProvider};
use anyhow::{Context, Result, bail, ensure};
use cuerithm_media::CancellationToken;
use rusqlite::{
    Connection, OpenFlags,
    backup::{Backup, StepResult},
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeSet,
    fs::{self, File},
    hash::{Hash, Hasher},
    io::{Read, Write},
    path::{Component, Path, PathBuf},
    time::{Duration, Instant, SystemTime},
};

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ArtifactKind {
    Sqlite,
    StableFile,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArtifactRequest {
    pub id: String,
    #[serde(with = "crate::path_serde")]
    pub relative_path: PathBuf,
    pub kind: ArtifactKind,
}
impl ArtifactRequest {
    pub fn sqlite(path: &str) -> Self {
        Self {
            id: path.into(),
            relative_path: path.into(),
            kind: ArtifactKind::Sqlite,
        }
    }
    pub fn stable(path: &str) -> Self {
        Self {
            id: path.into(),
            relative_path: path.into(),
            kind: ArtifactKind::StableFile,
        }
    }
}
/// Constructed by backend/native selection, never from a client-supplied preview path.
#[derive(Debug, Clone)]
pub struct SourceCaptureRequest {
    pub provider: SourceProvider,
    root: PathBuf,
    artifacts: Vec<ArtifactRequest>,
}
fn safe_path(root: &Path, relative: &Path) -> Result<PathBuf> {
    ensure!(
        !relative.as_os_str().is_empty() && !relative.is_absolute(),
        "Expected relative source artifact"
    );
    let mut path = root.to_path_buf();
    for component in relative.components() {
        let Component::Normal(name) = component else {
            bail!("Source artifact traversal is not allowed");
        };
        path.push(name);
        match fs::symlink_metadata(&path) {
            Ok(meta) => ensure!(
                !meta.file_type().is_symlink(),
                "Source artifact symlinks require explicit native resolution"
            ),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
        }
    }
    ensure!(
        path.starts_with(root),
        "Source artifact escapes selected root"
    );
    Ok(path)
}
impl SourceCaptureRequest {
    pub fn from_selected_root(
        provider: SourceProvider,
        root: &Path,
        mut artifacts: Vec<ArtifactRequest>,
    ) -> Result<Self> {
        let root = root.canonicalize().context("Source root is unavailable")?;
        ensure!(
            root.is_dir() && !artifacts.is_empty(),
            "Choose a source root and artifacts"
        );
        let mut ids = BTreeSet::new();
        for artifact in &artifacts {
            ensure!(
                !artifact.id.is_empty() && ids.insert(&artifact.id),
                "Duplicate source artifact ID"
            );
            safe_path(&root, &artifact.relative_path)?;
        }
        artifacts.sort_by(|a, b| a.id.cmp(&b.id));
        Ok(Self {
            provider,
            root,
            artifacts,
        })
    }
    pub fn root(&self) -> &Path {
        &self.root
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
struct Fingerprint {
    object_identity: String,
    sha256: String,
    bytes: u64,
    modified: Option<SystemTime>,
}
struct ObjectIdentityHasher(Sha256);
impl Hasher for ObjectIdentityHasher {
    fn write(&mut self, bytes: &[u8]) {
        self.0.update(bytes);
    }
    fn finish(&self) -> u64 {
        u64::from_le_bytes(
            self.0.clone().finalize()[..8]
                .try_into()
                .expect("eight digest bytes"),
        )
    }
}
fn object_identity(file: &File) -> Result<String> {
    // same-file's pinned Hash implementation encodes native dev/inode or
    // volume/file-index equality keys. Capture/drop handles here rather than
    // retaining one descriptor per source artifact in a large library.
    let first = same_file::Handle::from_file(file.try_clone()?)?;
    let second = same_file::Handle::from_file(file.try_clone()?)?;
    ensure!(
        first == second,
        "Source filesystem does not expose stable file identity"
    );
    let mut state = ObjectIdentityHasher(Sha256::new());
    first.hash(&mut state);
    Ok(format!("{:x}", state.0.finalize()))
}
fn fingerprint(
    path: &Path,
    max_bytes: u64,
    cancel: &CancellationToken,
) -> Result<Option<Fingerprint>> {
    cancel.check()?;
    let mut file = match File::open(path) {
        Ok(f) => f,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e.into()),
    };
    let before = file.metadata()?;
    let identity = object_identity(&file)?;
    ensure!(
        before.is_file() && before.len() <= max_bytes,
        "Source artifact exceeds capture budget or is not a file"
    );
    let mut hash = Sha256::new();
    let mut read = 0u64;
    let mut buffer = [0u8; 65536];
    loop {
        cancel.check()?;
        let n = file.read(&mut buffer)?;
        if n == 0 {
            break;
        }
        read = read
            .checked_add(n as u64)
            .context("Capture size overflow")?;
        ensure!(read <= max_bytes, "Capture size limit exceeded");
        hash.update(&buffer[..n]);
    }
    let after = file.metadata()?;
    ensure!(
        read == before.len()
            && after.len() == before.len()
            && after.modified().ok() == before.modified().ok(),
        "Source changed while fingerprinting"
    );
    Ok(Some(Fingerprint {
        object_identity: identity,
        sha256: format!("{:x}", hash.finalize()),
        bytes: read,
        modified: before.modified().ok(),
    }))
}
#[derive(Debug)]
struct SourceGuard {
    relative: PathBuf,
    fingerprint: Option<Fingerprint>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnapshotArtifact {
    pub id: String,
    pub kind: ArtifactKind,
    pub sha256: String,
    pub bytes: u64,
}
/// Owns private snapshot files; dropping it releases only that private workspace.
#[derive(Debug)]
pub struct ImportSnapshot {
    workspace: tempfile::TempDir,
    root: PathBuf,
    guards: Vec<SourceGuard>,
    pub provider: SourceProvider,
    pub artifacts: Vec<SnapshotArtifact>,
    pub snapshot_hash: String,
    paths: Vec<PathBuf>,
    source_paths: Vec<PathBuf>,
}
impl ImportSnapshot {
    /// Original selected context for interpreting relative references. Parsers
    /// read artifact bytes exclusively through `artifact`, never through here.
    pub fn original_artifact_path(&self, id: &str) -> Result<&Path> {
        let index = self
            .artifacts
            .iter()
            .position(|a| a.id == id)
            .context("Unknown snapshot artifact")?;
        Ok(&self.source_paths[index])
    }
    pub fn source_root(&self) -> &Path {
        &self.root
    }
    pub fn artifact(&self, id: &str) -> Result<&Path> {
        let index = self
            .artifacts
            .iter()
            .position(|a| a.id == id)
            .context("Unknown snapshot artifact")?;
        // Paths are generated inside the private directory; never append source keys.
        Ok(self.paths()[index].as_path())
    }
    fn paths(&self) -> &[PathBuf] {
        &self.paths
    }
    pub fn validate_source(&self, cancel: &CancellationToken) -> Result<()> {
        for guard in &self.guards {
            let path = safe_path(&self.root, &guard.relative)?;
            let current = fingerprint(
                &path,
                guard.fingerprint.as_ref().map_or(0, |f| f.bytes),
                cancel,
            )?;
            ensure!(
                current == guard.fingerprint,
                "Source changed after snapshot; inspect again"
            );
        }
        Ok(())
    }
    pub fn private_root(&self) -> &Path {
        self.workspace.path()
    }
}
fn backup_sqlite(
    source: &Path,
    destination: &Path,
    limits: &ImportLimits,
    cancel: &CancellationToken,
) -> Result<()> {
    let source = Connection::open_with_flags(
        source,
        OpenFlags::SQLITE_OPEN_READ_ONLY
            | OpenFlags::SQLITE_OPEN_NO_MUTEX
            | OpenFlags::SQLITE_OPEN_NOFOLLOW,
    )?;
    source.busy_timeout(Duration::from_millis(50))?;
    let mut target = Connection::open(destination)?;
    let started = Instant::now();
    {
        let backup = Backup::new(&source, &mut target)?;
        loop {
            cancel.check()?;
            ensure!(
                destination.metadata()?.len() <= limits.snapshot_bytes,
                "Snapshot size limit exceeded"
            );
            ensure!(
                started.elapsed() < Duration::from_secs(120),
                "Source backup timed out"
            );
            match backup.step(64)? {
                StepResult::Done => break,
                StepResult::More => {}
                StepResult::Busy | StepResult::Locked => {
                    ensure!(
                        started.elapsed() < Duration::from_secs(2),
                        "Source is busy; close the source app and retry"
                    );
                    std::thread::sleep(Duration::from_millis(10));
                }
                _ => bail!("Unknown SQLite backup state"),
            }
        }
    }
    target.close().map_err(|(_, e)| e)?;
    File::open(destination)?.sync_all()?;
    Ok(())
}
pub fn capture_source(
    request: &SourceCaptureRequest,
    workspace: &Path,
    limits: &ImportLimits,
    cancel: &CancellationToken,
) -> Result<ImportSnapshot> {
    capture_source_with_sqlite(request, workspace, limits, cancel, backup_sqlite)
}
pub(crate) fn capture_source_with_sqlite(
    request: &SourceCaptureRequest,
    workspace: &Path,
    limits: &ImportLimits,
    cancel: &CancellationToken,
    mut backup: impl FnMut(&Path, &Path, &ImportLimits, &CancellationToken) -> Result<()>,
) -> Result<ImportSnapshot> {
    cancel.check()?;
    ensure!(workspace.is_dir(), "Import workspace is unavailable");
    let owned = tempfile::Builder::new()
        .prefix("library-import-")
        .tempdir_in(workspace)?;
    let mut guards = Vec::new();
    let mut total = 0u64;
    for artifact in &request.artifacts {
        let mut paths = vec![artifact.relative_path.clone()];
        if matches!(artifact.kind, ArtifactKind::Sqlite) {
            for suffix in ["-wal", "-journal"] {
                let mut p = artifact.relative_path.as_os_str().to_os_string();
                p.push(suffix);
                paths.push(p.into());
            }
        }
        for relative in paths {
            let state = fingerprint(
                &safe_path(&request.root, &relative)?,
                limits.snapshot_bytes,
                cancel,
            )?;
            if relative == artifact.relative_path {
                ensure!(state.is_some(), "Source artifact is missing");
            }
            if let Some(state) = &state {
                total = total
                    .checked_add(state.bytes)
                    .context("Capture size overflow")?;
                ensure!(
                    total <= limits.snapshot_bytes,
                    "Snapshot size limit exceeded"
                );
            }
            guards.push(SourceGuard {
                relative,
                fingerprint: state,
            });
        }
    }
    let mut captured = Vec::new();
    let mut paths = Vec::new();
    let mut copied = 0u64;
    for (index, artifact) in request.artifacts.iter().enumerate() {
        cancel.check()?;
        let input = safe_path(&request.root, &artifact.relative_path)?;
        let path = owned.path().join(format!("artifact-{index}"));
        match artifact.kind {
            ArtifactKind::Sqlite => backup(&input, &path, limits, cancel)?,
            ArtifactKind::StableFile => {
                let mut input = File::open(input)?;
                let mut output = File::create(&path)?;
                let mut buffer = [0; 65536];
                loop {
                    cancel.check()?;
                    let n = input.read(&mut buffer)?;
                    if n == 0 {
                        break;
                    }
                    copied = copied
                        .checked_add(n as u64)
                        .context("Capture size overflow")?;
                    ensure!(
                        copied <= limits.snapshot_bytes,
                        "Snapshot size limit exceeded"
                    );
                    output.write_all(&buffer[..n])?;
                }
                output.sync_all()?;
            }
        }
        let state = fingerprint(&path, limits.snapshot_bytes, cancel)?
            .context("Snapshot artifact disappeared")?;
        captured.push(SnapshotArtifact {
            id: artifact.id.clone(),
            kind: artifact.kind,
            sha256: state.sha256,
            bytes: state.bytes,
        });
        paths.push(path);
    }
    let snapshot_hash = format!("{:x}", Sha256::digest(serde_json::to_vec(&captured)?));
    let snapshot = ImportSnapshot {
        workspace: owned,
        root: request.root.clone(),
        guards,
        provider: request.provider,
        artifacts: captured,
        snapshot_hash,
        paths,
        source_paths: request
            .artifacts
            .iter()
            .map(|a| request.root.join(&a.relative_path))
            .collect(),
    };
    snapshot.validate_source(cancel)?;
    Ok(snapshot)
}
