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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub environment: Option<cuerithm_rekordbox::TargetEnvironmentSnapshot>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub roots: Vec<cuerithm_rekordbox::TargetPathSnapshot>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        with = "crate::path_serde::option"
    )]
    pub database: Option<PathBuf>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        with = "crate::path_serde::option"
    )]
    pub reference_root: Option<PathBuf>,
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
fn reference_location(
    target: &SyncTarget,
    source: &Path,
    resolver: &cuerithm_rekordbox::TargetPathResolver<'_>,
) -> Result<(PathBuf, Option<PathBuf>)> {
    if target.software == DjSoftware::VirtualDj {
        let native = crate::destination_reference(&resolver.to_native(source)?);
        let primary = crate::sync::windows_volume_root(&crate::destination_reference(
            &resolver.to_native(&target.path)?,
        ));
        if let Some(root) = crate::sync::windows_volume_root(&native).filter(|root| {
            primary
                .as_ref()
                .is_some_and(|primary| !primary.eq_ignore_ascii_case(root))
        }) {
            let path = resolver.to_host(Path::new(&format!(
                "{}/VirtualDJ/database.xml",
                root.trim_end_matches('/')
            )))?;
            ensure!(
                path.is_file(),
                "Initialize the destination drive in VirtualDJ before organizing its tracks"
            );
            return Ok((fs::canonicalize(path)?, None));
        }
    }
    if target.software == DjSoftware::Serato4 && target.source_root.is_none() {
        return crate::serato4::location_for_track_with_resolver(
            target
                .path
                .parent()
                .context("Serato local library missing")?,
            source,
            resolver,
        );
    }
    Ok((target.path.clone(), target.source_root.clone()))
}
fn engine_root(
    target: &SyncTarget,
    resolver: &cuerithm_rekordbox::TargetPathResolver<'_>,
) -> Result<PathBuf> {
    Ok(crate::destination_reference(
        &resolver.to_native(
            target
                .path
                .parent()
                .and_then(Path::parent)
                .context("Engine Library root missing")?,
        )?,
    ))
}
fn external_id_at(
    target: &SyncTarget,
    database: &Path,
    root: Option<&Path>,
    source: &Path,
    resolver: &cuerithm_rekordbox::TargetPathResolver<'_>,
) -> Result<String> {
    let native = crate::destination_reference(&resolver.to_native(source)?);
    match target.software {
        DjSoftware::VirtualDj => {
            crate::virtual_dj::reference_id_with_resolver(database, source, resolver)
        }
        DjSoftware::EngineDj => {
            crate::engine_dj::reference_id(database, &engine_root(target, resolver)?, &native)
        }
        DjSoftware::DjayPro => {
            crate::djay_pro::reference_id_with_resolver(database, source, resolver)
        }
        DjSoftware::Serato4 => {
            crate::serato4::reference_id_with_resolver(database, root, source, resolver)
        }
        DjSoftware::PioneerUsb => {
            anyhow::bail!("Independent device copy has no desktop source-path reference")
        }
        DjSoftware::Mixxx => anyhow::bail!("Mixxx path repair is not wired yet"),
        DjSoftware::Rekordbox => {
            anyhow::bail!("Use the native rekordbox collection workflow for relocation")
        }
    }
}
fn external_id(
    target: &SyncTarget,
    source: &Path,
    resolver: &cuerithm_rekordbox::TargetPathResolver<'_>,
) -> Result<String> {
    let (database, root) = reference_location(target, source, resolver)?;
    external_id_at(target, &database, root.as_deref(), source, resolver)
}
fn snapshot_database(snapshot: &PathReferenceSnapshot) -> &Path {
    snapshot
        .database
        .as_deref()
        .unwrap_or(&snapshot.target.path)
}
fn verify_context(snapshot: &PathReferenceSnapshot) -> Result<()> {
    if let Some(environment) = &snapshot.environment {
        ensure!(
            environment.environment == snapshot.target.environment,
            "Native reference environment changed after preview"
        );
        environment.verify()?;
    } else {
        ensure!(
            snapshot.target.environment.is_native(),
            "Native reference snapshot has no captured Windows context"
        );
    }
    for root in &snapshot.roots {
        root.verify()?;
    }
    Ok(())
}
fn virtualdj_databases(
    target: &SyncTarget,
    resolver: &cuerithm_rekordbox::TargetPathResolver<'_>,
) -> Result<BTreeSet<PathBuf>> {
    let mut databases = BTreeSet::from([target.path.clone()]);
    if let cuerithm_rekordbox::TargetEnvironment::WindowsCompatibility { mappings, .. } =
        &target.environment
    {
        for mapping in mappings {
            if let Some(root) = crate::sync::windows_volume_root(Path::new(&mapping.windows_root))
                && let Ok(path) = resolver.to_host(Path::new(&format!(
                    "{}/VirtualDJ/database.xml",
                    root.trim_end_matches('/')
                )))
                && path.is_file()
            {
                databases.insert(fs::canonicalize(path)?);
            }
        }
    }
    #[cfg(windows)]
    if target.environment.is_native() {
        for drive in b'A'..=b'Z' {
            let path = PathBuf::from(format!("{}:\\VirtualDJ\\database.xml", char::from(drive)));
            if path.is_file() {
                databases.insert(fs::canonicalize(path)?);
            }
        }
    }
    Ok(databases)
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
                if directory.file_name().is_none_or(|name| name != "Filters") {
                    playlists(&path, files, directories)?;
                }
            }
            crate::transaction::DirectoryEntryKind::File => {
                if directory.file_name().is_some_and(|name| name == "Filters")
                    && path
                        .file_name()
                        .is_none_or(|name| !name.to_string_lossy().starts_with("CueRithm-"))
                {
                    continue;
                }
                ensure!(
                    path.file_name()
                        .is_some_and(|s| s == "order" || s == ".DS_Store")
                        || path
                            .extension()
                            .is_some_and(|s| s == "vdjfolder" || s == "m3u"),
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
    let environment = target.environment.freeze()?;
    let resolver = target.environment.resolver()?;
    let (database, reference_root) = reference_location(&target, source, &resolver)?;
    let mut files = BTreeMap::from([
        (target.path.clone(), file_hash(&target.path)?),
        (database.clone(), file_hash(&database)?),
    ]);
    let mut directories = BTreeMap::new();
    if target.software == DjSoftware::VirtualDj {
        for database in virtualdj_databases(&target, &resolver)? {
            files.insert(database.clone(), file_hash(&database)?);
        }
        let home = target.path.parent().context("VirtualDJ home missing")?;
        for root in crate::virtual_dj::playlist_roots(home) {
            playlists(&root, &mut files, &mut directories)?;
        }
    } else {
        for suffix in ["-wal", "-shm", "-journal"] {
            let mut path = database.as_os_str().to_owned();
            path.push(suffix);
            let path = PathBuf::from(path);
            files.insert(path.clone(), file_hash(&path)?);
        }
    }
    let roots = files
        .keys()
        .filter_map(|path| {
            path.parent()
                .filter(|parent| parent.is_dir())
                .map(Path::to_owned)
        })
        .collect::<BTreeSet<_>>()
        .into_iter()
        .map(|path| cuerithm_rekordbox::TargetPathSnapshot::capture(&path))
        .collect::<Result<Vec<_>>>()?;
    let external_id = external_id(&target, source, &resolver)?;
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
    resolver.verify()?;
    drop(resolver);
    let snapshot = PathReferenceSnapshot {
        environment: Some(environment),
        roots,
        database: (database != target.path).then_some(database),
        reference_root,
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
    verify_context(snapshot)?;
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
    let resolver = snapshot.target.environment.resolver()?;
    if snapshot.target.software == DjSoftware::Serato4 {
        let root = snapshot
            .reference_root
            .as_ref()
            .or(snapshot.target.source_root.as_ref())
            .map(|root| resolver.to_native(root))
            .transpose()?;
        crate::serato4::validate_reference_location(
            &resolver.to_native(&change.from)?,
            &resolver.to_native(&change.to)?,
            root.as_deref(),
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
    resolver.verify()?;
    drop(resolver);
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
        for (native, hash) in &snapshot.files {
            if hash.is_some() {
                ensure!(
                    !same_file::is_same_file(path, native)?,
                    "Native database or playlist aliases organization audio"
                );
            }
        }
    }
    let transaction = FileTransaction::create(root)?;
    let build = tempfile::tempdir_in(transaction.path())?;
    let staged = build.path().join("database");
    if snapshot.target.software == DjSoftware::VirtualDj {
        fs::copy(snapshot_database(snapshot), &staged)?;
    } else {
        let db = rusqlite::Connection::open_with_flags(
            snapshot_database(snapshot),
            rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
        )?;
        db.backup("main", &staged, None)?;
    }
    // Journal creation and SQLite backup can change mapped directory entries.
    // Begin the next read-only resolution phase only after those owned writes.
    let resolver = snapshot.target.environment.resolver()?;
    let native_from = crate::destination_reference(&resolver.to_native(&change.from)?);
    let native_to = crate::destination_reference(&resolver.to_native(&change.to)?);
    let mut extra_publications = Vec::new();
    let destination_location = if snapshot.target.software == DjSoftware::VirtualDj {
        reference_location(&snapshot.target, &change.to, &resolver)?.0
    } else {
        snapshot_database(snapshot).to_owned()
    };
    match snapshot.target.software {
        DjSoftware::VirtualDj if destination_location != snapshot_database(snapshot) => {
            ensure!(
                snapshot.files.contains_key(&destination_location),
                "Destination VirtualDJ drive changed since inspection; preview again"
            );
            let (from, to) = crate::virtual_dj::move_reference_between_databases(
                &fs::read(&staged)?,
                &fs::read(&destination_location)?,
                &change.from,
                &change.to,
                &resolver,
            )?;
            fs::write(&staged, from)?;
            let destination_stage = build.path().join("destination-database");
            fs::write(&destination_stage, to)?;
            extra_publications.push(FilePublication {
                target: destination_location.clone(),
                before_sha256: snapshot.files[&destination_location].clone(),
                staged: Some(destination_stage),
            });
        }
        DjSoftware::VirtualDj => {
            let output = crate::virtual_dj::repair_reference_bytes_with_resolver(
                &fs::read(&staged)?,
                &change.from,
                &change.to,
                true,
                &resolver,
            )?
            .context("VirtualDJ source reference vanished")?;
            fs::write(&staged, output)?;
        }
        DjSoftware::EngineDj => crate::engine_dj::repair_reference(
            &staged,
            &engine_root(&snapshot.target, &resolver)?,
            &native_from,
            &native_to,
            &snapshot.external_id,
        )?,
        DjSoftware::DjayPro => crate::djay_pro::repair_reference_with_resolver(
            &staged,
            &change.from,
            &change.to,
            &snapshot.external_id,
            &resolver,
        )?,
        DjSoftware::Serato4 => crate::serato4::repair_reference_with_resolver(
            &staged,
            snapshot
                .reference_root
                .as_deref()
                .or(snapshot.target.source_root.as_deref()),
            &change.from,
            &change.to,
            &snapshot.external_id,
            &resolver,
        )?,
        DjSoftware::PioneerUsb => anyhow::bail!("Independent device copy cannot be path-repaired"),
        DjSoftware::Mixxx => anyhow::bail!("Mixxx path repair is not wired yet"),
        DjSoftware::Rekordbox => {
            anyhow::bail!("Use the native rekordbox collection workflow for relocation")
        }
    }
    if snapshot.target.software != DjSoftware::VirtualDj {
        let db = rusqlite::Connection::open(&staged)?;
        db.pragma_update(None, "journal_mode", "DELETE")?;
    }
    let after_database = if let Some(publication) = extra_publications.first() {
        publication.staged.as_deref().unwrap()
    } else {
        &staged
    };
    let after_external_id = external_id_at(
        &snapshot.target,
        after_database,
        snapshot
            .reference_root
            .as_deref()
            .or(snapshot.target.source_root.as_deref()),
        &change.to,
        &resolver,
    )?;
    if snapshot.target.software != DjSoftware::VirtualDj {
        ensure!(
            after_external_id == snapshot.external_id,
            "Path repair changed native track identity"
        );
    }
    let mut publications = vec![FilePublication {
        target: snapshot_database(snapshot).to_owned(),
        before_sha256: snapshot.files[snapshot_database(snapshot)].clone(),
        staged: Some(staged),
    }];
    publications.extend(extra_publications);
    if snapshot.target.software == DjSoftware::VirtualDj {
        for (index, (path, hash)) in snapshot.files.iter().enumerate() {
            let bytes = if path.extension().is_some_and(|extension| extension == "m3u") {
                crate::virtual_dj::repair_legacy_playlist_bytes(
                    &fs::read(path)?,
                    &change.from,
                    &change.to,
                    &resolver,
                )?
            } else if path
                .extension()
                .is_some_and(|extension| extension == "vdjfolder")
            {
                crate::virtual_dj::repair_reference_bytes_with_resolver(
                    &fs::read(path)?,
                    &change.from,
                    &change.to,
                    false,
                    &resolver,
                )?
            } else {
                None
            };
            if let Some(bytes) = bytes {
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
            if *path != snapshot_database(snapshot)
                && crate::sync::is_sqlite_sidecar(path)
                && hash.is_some()
            {
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
    resolver.verify()?;
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
        verify_context(&preview.snapshot)?;
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
    verify_context(&preview.snapshot)?;
    let resolver = preview.snapshot.target.environment.resolver()?;
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
        external_id(&preview.snapshot.target, &preview.change.to, &resolver)?
            == preview.after_external_id,
        "Repaired native reference changed before acknowledgement"
    );
    resolver.verify()?;
    Ok(journal)
}

/// Restore only known before/after bytes from this repair's own backups.
pub fn restore_path_repair(root: &Path, id: &str) -> Result<FileJournal> {
    let (tx, preview) = saved(root, id)?;
    verify_audio(&preview.change)?;
    if tx.inspect()?.state != crate::transaction::JournalState::Restored {
        tx.restore(|| {
            verify_context(&preview.snapshot)?;
            crate::discovery::ensure_closed(preview.snapshot.target.software)?;
            verify_audio(&preview.change)?;
            Ok(())
        })?;
    }
    let journal = tx.verify_restored()?;
    let resolver = preview.snapshot.target.environment.resolver()?;
    verify_context(&preview.snapshot)?;
    crate::discovery::ensure_closed(preview.snapshot.target.software)?;
    ensure!(
        external_id(&preview.snapshot.target, &preview.change.from, &resolver)?
            == preview.snapshot.external_id,
        "Restored native source identity changed"
    );
    resolver.verify()?;
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
