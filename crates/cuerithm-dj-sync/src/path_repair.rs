//! Dedicated path-only reference updates; never invoke full track synchronization.
use crate::{
    DjSoftware, SyncTarget,
    transaction::{
        DirectoryEntry, FileJournal, FilePublication, FileTransaction, directory_manifest,
        file_hash,
    },
};
use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
};
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourcePathChange {
    pub track_id: String,
    #[serde(with = "crate::path_serde")]
    pub from: PathBuf,
    #[serde(with = "crate::path_serde")]
    pub to: PathBuf,
    /// Captured content at the managed destination; never changed by a source decision.
    pub source_sha256: String,
    /// A separately approved current original revision. Callers must bind this
    /// to an explicit durable decision and native source ownership proof.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub approved_from_sha256: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PathReferenceSnapshot {
    pub target: SyncTarget,
    #[serde(with = "crate::path_serde::map")]
    pub files: BTreeMap<PathBuf, Option<String>>,
    #[serde(with = "crate::path_serde::map")]
    pub directories: BTreeMap<PathBuf, Option<Vec<DirectoryEntry>>>,
    pub external_id: String,
    pub estimated_bytes: u64,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PathRepairPreview {
    pub id: String,
    pub snapshot: PathReferenceSnapshot,
    pub change: SourcePathChange,
    pub after_external_id: String,
}
fn resolved(target: &SyncTarget) -> Result<SyncTarget> {
    let mut target = target.clone();
    target.path = match target.software {
        DjSoftware::EngineDj => crate::engine_dj::resolve_database_path(&target.path)?,
        DjSoftware::VirtualDj if target.path.is_dir() => target.path.join("database.xml"),
        DjSoftware::DjayPro if target.path.is_dir() => target.path.join("MediaLibrary.db"),
        DjSoftware::Serato4 if target.path.is_dir() => target.path.join("root.sqlite"),
        DjSoftware::PioneerUsb => anyhow::bail!("Pioneer USB owns independent media copies"),
        DjSoftware::Rekordbox => {
            anyhow::bail!("Use the native rekordbox collection workflow for relocation")
        }
        _ => target.path,
    };
    ensure!(
        target.path.is_absolute(),
        "Native reference target must be absolute"
    );
    crate::transaction::safe_ancestors(&target.path)?;
    ensure!(
        file_hash(&target.path)?.is_some(),
        "Native reference database is unavailable"
    );
    Ok(target)
}
fn external_id(target: &SyncTarget, source: &Path) -> Result<String> {
    match target.software {
        DjSoftware::VirtualDj => crate::virtual_dj::reference_id(&target.path, source),
        DjSoftware::EngineDj => crate::engine_dj::reference_id(
            &target.path,
            target
                .path
                .parent()
                .and_then(Path::parent)
                .context("Engine Library root missing")?,
            source,
        ),
        DjSoftware::DjayPro => crate::djay_pro::reference_id(&target.path, source),
        DjSoftware::Serato4 => {
            crate::serato4::reference_id(&target.path, target.source_root.as_deref(), source)
        }
        DjSoftware::PioneerUsb => {
            anyhow::bail!("Independent device copy has no desktop source-path reference")
        }
        DjSoftware::Rekordbox => {
            anyhow::bail!("Use the native rekordbox collection workflow for relocation")
        }
    }
}
fn playlists(
    directory: &Path,
    files: &mut BTreeMap<PathBuf, Option<String>>,
    directories: &mut BTreeMap<PathBuf, Option<Vec<DirectoryEntry>>>,
) -> Result<()> {
    let manifest = directory_manifest(directory)?;
    directories.insert(directory.into(), manifest.clone());
    let Some(manifest) = manifest else {
        return Ok(());
    };
    ensure!(
        files.len() + manifest.len() <= 100_000,
        "VirtualDJ reference file budget exceeded"
    );
    for item in manifest {
        let path = directory.join(item.name);
        match item.kind {
            crate::transaction::DirectoryEntryKind::Directory => {
                playlists(&path, files, directories)?
            }
            crate::transaction::DirectoryEntryKind::File => {
                ensure!(
                    path.file_name()
                        .is_some_and(|s| s == "order" || s == ".DS_Store")
                        || path.extension().is_some_and(|s| s == "vdjfolder"),
                    "Unsupported managed VirtualDJ playlist file"
                );
                files.insert(path.clone(), file_hash(&path)?);
            }
            _ => anyhow::bail!("Managed VirtualDJ playlist contains an unsafe filesystem entry"),
        }
    }
    Ok(())
}
pub fn inspect_path_reference(target: &SyncTarget, source: &Path) -> Result<PathReferenceSnapshot> {
    let target = resolved(target)?;
    crate::discovery::ensure_closed(target.software)?;
    let mut files = BTreeMap::from([(target.path.clone(), file_hash(&target.path)?)]);
    let mut directories = BTreeMap::new();
    if target.software == DjSoftware::VirtualDj {
        playlists(
            &target
                .path
                .parent()
                .context("VirtualDJ home missing")?
                .join("MyLists/CueRithm.subfolders"),
            &mut files,
            &mut directories,
        )?;
    } else {
        for suffix in ["-wal", "-shm", "-journal"] {
            let mut path = target.path.as_os_str().to_owned();
            path.push(suffix);
            let path = PathBuf::from(path);
            files.insert(path.clone(), file_hash(&path)?);
        }
    }
    let external_id = external_id(&target, source)?;
    let bytes = files
        .iter()
        .try_fold(0_u64, |sum, (path, hash)| -> Result<u64> {
            let size = if hash.is_some() {
                fs::metadata(path)?.len()
            } else {
                0
            };
            sum.checked_add(size)
                .context("Native reference storage estimate overflow")
        })?;
    let estimated_bytes = bytes
        .checked_mul(4)
        .and_then(|n| n.checked_add(65536))
        .context("Native reference storage estimate overflow")?;
    let snapshot = PathReferenceSnapshot {
        target,
        files,
        directories,
        external_id,
        estimated_bytes,
    };
    validate_snapshot(&snapshot)?;
    Ok(snapshot)
}
fn validate_snapshot(snapshot: &PathReferenceSnapshot) -> Result<()> {
    for (path, hash) in &snapshot.files {
        crate::transaction::safe_ancestors(path)?;
        ensure!(
            file_hash(path)? == *hash,
            "Native reference target changed after preview"
        );
    }
    for (path, manifest) in &snapshot.directories {
        ensure!(
            directory_manifest(path)? == *manifest,
            "Native playlist hierarchy changed after preview"
        );
    }
    Ok(())
}
fn verify_audio(change: &SourcePathChange) -> Result<()> {
    ensure!(
        change.from != change.to,
        "Path-only repair requires a changed path"
    );
    for (path, expected) in [
        (
            &change.from,
            change
                .approved_from_sha256
                .as_deref()
                .unwrap_or(&change.source_sha256),
        ),
        (&change.to, change.source_sha256.as_str()),
    ] {
        ensure!(
            file_hash(path)?.as_deref() == Some(expected),
            "Organization source or destination bytes changed before reference repair"
        );
    }
    Ok(())
}
pub fn prepare_path_repair(
    root: &Path,
    snapshot: &PathReferenceSnapshot,
    change: &SourcePathChange,
) -> Result<PathRepairPreview> {
    validate_snapshot(snapshot)?;
    verify_audio(change)?;
    if snapshot.target.software == DjSoftware::Serato4 {
        crate::serato4::validate_reference_location(
            &change.from,
            &change.to,
            snapshot.target.source_root.as_deref(),
        )?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            ensure!(
                fs::metadata(&change.from)?.dev() == fs::metadata(&change.to)?.dev(),
                "Serato relocation crosses a location volume; migrate its location database explicitly"
            );
        }
        #[cfg(windows)]
        {
            ensure!(
                windows_volume(&change.from)? == windows_volume(&change.to)?,
                "Serato relocation crosses a location volume; migrate its location database explicitly"
            );
        }
    }
    crate::discovery::ensure_closed(snapshot.target.software)?;
    fs::create_dir_all(root)?;
    for path in [
        root,
        snapshot
            .target
            .path
            .parent()
            .context("Native database parent missing")?,
    ] {
        ensure!(
            fs2::available_space(path)? >= snapshot.estimated_bytes,
            "Insufficient disk space for native reference backup and staging"
        );
    }
    for path in [&change.from, &change.to] {
        ensure!(
            !same_file::is_same_file(path, &snapshot.target.path)?,
            "Native database aliases organization audio"
        );
    }
    let transaction = FileTransaction::create(root)?;
    let build = tempfile::tempdir_in(transaction.path())?;
    let staged = build.path().join("database");
    if snapshot.target.software == DjSoftware::VirtualDj {
        fs::copy(&snapshot.target.path, &staged)?;
    } else {
        let db = rusqlite::Connection::open_with_flags(
            &snapshot.target.path,
            rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
        )?;
        db.backup("main", &staged, None)?;
    }
    match snapshot.target.software {
        DjSoftware::VirtualDj => {
            let output = crate::virtual_dj::repair_reference_bytes(
                &fs::read(&staged)?,
                &change.from,
                &change.to,
                true,
            )?
            .context("VirtualDJ source reference vanished")?;
            fs::write(&staged, output)?;
        }
        DjSoftware::EngineDj => crate::engine_dj::repair_reference(
            &staged,
            snapshot
                .target
                .path
                .parent()
                .and_then(Path::parent)
                .context("Engine Library root missing")?,
            &change.from,
            &change.to,
            &snapshot.external_id,
        )?,
        DjSoftware::DjayPro => crate::djay_pro::repair_reference(
            &staged,
            &change.from,
            &change.to,
            &snapshot.external_id,
        )?,
        DjSoftware::Serato4 => crate::serato4::repair_reference(
            &staged,
            snapshot.target.source_root.as_deref(),
            &change.from,
            &change.to,
            &snapshot.external_id,
        )?,
        DjSoftware::PioneerUsb => anyhow::bail!("Independent device copy cannot be path-repaired"),
        DjSoftware::Rekordbox => {
            anyhow::bail!("Use the native rekordbox collection workflow for relocation")
        }
    }
    if snapshot.target.software != DjSoftware::VirtualDj {
        let db = rusqlite::Connection::open(&staged)?;
        db.pragma_update(None, "journal_mode", "DELETE")?;
    }
    let mut staged_target = snapshot.target.clone();
    staged_target.path = staged.clone();
    // Engine relative coordinates belong to the original library, not the private stage.
    let after_external_id = if snapshot.target.software == DjSoftware::EngineDj {
        crate::engine_dj::reference_id(
            &staged,
            snapshot
                .target
                .path
                .parent()
                .and_then(Path::parent)
                .unwrap(),
            &change.to,
        )?
    } else {
        external_id(&staged_target, &change.to)?
    };
    if snapshot.target.software != DjSoftware::VirtualDj {
        ensure!(
            after_external_id == snapshot.external_id,
            "Path repair changed native track identity"
        );
    }
    let mut publications = vec![FilePublication {
        target: snapshot.target.path.clone(),
        before_sha256: snapshot.files[&snapshot.target.path].clone(),
        staged: Some(staged),
    }];
    if snapshot.target.software == DjSoftware::VirtualDj {
        for (index, (path, hash)) in snapshot.files.iter().enumerate() {
            if path.extension().is_some_and(|s| s == "vdjfolder")
                && let Some(bytes) = crate::virtual_dj::repair_reference_bytes(
                    &fs::read(path)?,
                    &change.from,
                    &change.to,
                    false,
                )?
            {
                let stage = build.path().join(format!("playlist-{index}"));
                fs::write(&stage, bytes)?;
                publications.push(FilePublication {
                    target: path.clone(),
                    before_sha256: hash.clone(),
                    staged: Some(stage),
                });
            }
        }
    } else {
        for (path, hash) in &snapshot.files {
            if *path != snapshot.target.path && hash.is_some() {
                publications.push(FilePublication {
                    target: path.clone(),
                    before_sha256: hash.clone(),
                    staged: None,
                });
            }
        }
    }
    if snapshot.target.software != DjSoftware::VirtualDj {
        publications.sort_by_key(|file| u8::from(file.staged.is_some()));
    }
    validate_snapshot(snapshot)?;
    verify_audio(change)?;
    transaction.add_batch(&publications, &[])?;
    let preview = PathRepairPreview {
        id: transaction.inspect()?.id,
        snapshot: snapshot.clone(),
        change: change.clone(),
        after_external_id,
    };
    crate::transaction::durable_json(&transaction.path().join("path-repair.json"), &preview)?;
    Ok(preview)
}
fn saved(root: &Path, id: &str) -> Result<(FileTransaction, PathRepairPreview)> {
    let tx = FileTransaction::open(root, id)?;
    let preview = serde_json::from_reader(fs::File::open(tx.path().join("path-repair.json"))?)?;
    Ok((tx, preview))
}
/// Refresh only the original revision proof after a caller's durable explicit
/// decision. Frozen target evidence and managed content identity stay immutable.
pub fn refresh_path_repair_source_proof(
    root: &Path,
    id: &str,
    change: &SourcePathChange,
) -> Result<()> {
    let (transaction, mut preview) = saved(root, id)?;
    ensure!(
        preview.change.track_id == change.track_id
            && preview.change.from == change.from
            && preview.change.to == change.to
            && preview.change.source_sha256 == change.source_sha256,
        "A source decision cannot replace a native repair's captured path/content identity"
    );
    verify_audio(change)?;
    preview.change.approved_from_sha256 = change.approved_from_sha256.clone();
    crate::transaction::durable_json(&transaction.path().join("path-repair.json"), &preview)
}
pub fn apply_path_repair(root: &Path, id: &str) -> Result<FileJournal> {
    let (tx, preview) = saved(root, id)?;
    verify_audio(&preview.change)?;
    let changed: BTreeSet<_> = tx
        .inspect()?
        .changes
        .into_iter()
        .map(|c| c.target)
        .collect();
    let first = std::cell::Cell::new(true);
    tx.apply(|| {
        crate::discovery::ensure_closed(preview.snapshot.target.software)?;
        verify_audio(&preview.change)?;
        for (path, hash) in &preview.snapshot.files {
            if first.get() || !changed.contains(path) {
                ensure!(
                    file_hash(path)? == *hash,
                    "Native reference dependency changed"
                );
            }
        }
        first.set(false);
        for (path, manifest) in &preview.snapshot.directories {
            ensure!(
                directory_manifest(path)? == *manifest,
                "Native playlist hierarchy changed"
            );
        }
        Ok(())
    })?;
    verify_path_repair(root, id)
}
pub fn verify_path_repair(root: &Path, id: &str) -> Result<FileJournal> {
    let (tx, preview) = saved(root, id)?;
    crate::discovery::ensure_closed(preview.snapshot.target.software)?;
    let journal = tx.verify_applied()?;
    let changed: BTreeSet<_> = journal.changes.iter().map(|c| &c.target).collect();
    for (path, hash) in &preview.snapshot.files {
        if !changed.contains(path) {
            ensure!(
                file_hash(path)? == *hash,
                "Native reference dependency changed before acknowledgement"
            );
        }
    }
    for (path, manifest) in &preview.snapshot.directories {
        ensure!(
            directory_manifest(path)? == *manifest,
            "Native playlist hierarchy changed before acknowledgement"
        );
    }
    ensure!(
        external_id(&preview.snapshot.target, &preview.change.to)? == preview.after_external_id,
        "Repaired native reference changed before acknowledgement"
    );
    Ok(journal)
}

/// Restore only known before/after bytes from this repair's own backups.
pub fn restore_path_repair(root: &Path, id: &str) -> Result<FileJournal> {
    let (tx, preview) = saved(root, id)?;
    verify_audio(&preview.change)?;
    if tx.inspect()?.state != crate::transaction::JournalState::Restored {
        tx.restore(|| {
            crate::discovery::ensure_closed(preview.snapshot.target.software)?;
            verify_audio(&preview.change)?;
            Ok(())
        })?;
    }
    let journal = tx.verify_restored()?;
    crate::discovery::ensure_closed(preview.snapshot.target.software)?;
    ensure!(
        external_id(&preview.snapshot.target, &preview.change.from)?
            == preview.snapshot.external_id,
        "Restored native source identity changed"
    );
    Ok(journal)
}

#[cfg(windows)]
fn windows_volume(path: &Path) -> Result<u32> {
    use std::os::windows::io::AsRawHandle;
    #[repr(C)]
    struct Info {
        attributes: u32,
        creation: [u32; 2],
        access: [u32; 2],
        write: [u32; 2],
        volume: u32,
        size_high: u32,
        size_low: u32,
        links: u32,
        index_high: u32,
        index_low: u32,
    }
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn GetFileInformationByHandle(handle: *mut std::ffi::c_void, info: *mut Info) -> i32;
    }
    let file = fs::File::open(path)?;
    let mut info = std::mem::MaybeUninit::<Info>::uninit();
    ensure!(
        unsafe { GetFileInformationByHandle(file.as_raw_handle(), info.as_mut_ptr()) } != 0,
        "Serato media volume identity unavailable"
    );
    Ok(unsafe { info.assume_init() }.volume)
}
