mod build;
use crate::{
    DjSoftware, SyncLibrary, SyncOptions,
    discovery::ensure_closed,
    transaction::{FileJournal, FileTransaction, file_hash},
    validate_library,
};
use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};
pub type PlaylistOwnershipContexts =
    BTreeMap<PathBuf, (Option<PathBuf>, crate::ownership::PlaylistOwnershipRequest)>;
pub type PlaylistOwnershipReports =
    BTreeMap<PathBuf, (Option<PathBuf>, crate::ownership::PlaylistOwnershipReport)>;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncTarget {
    pub software: DjSoftware,
    #[serde(
        default,
        skip_serializing_if = "cuerithm_rekordbox::TargetEnvironment::is_native"
    )]
    pub environment: cuerithm_rekordbox::TargetEnvironment,
    /// Native database path; Pioneer USB uses its drive root.
    #[serde(with = "crate::path_serde")]
    pub path: PathBuf,
    /// Original external/network volume root for a Serato location database.
    #[serde(default, with = "crate::path_serde::option")]
    pub source_root: Option<PathBuf>,
}
impl SyncTarget {
    /// The selected installation is a preview precondition, not a library identity.
    pub fn identity_target(&self) -> Self {
        let mut target = self.clone();
        if let cuerithm_rekordbox::TargetEnvironment::WindowsCompatibility {
            installation, ..
        } = &mut target.environment
        {
            *installation = None;
        }
        target
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncPreview {
    pub id: String,
    pub target: SyncTarget,
    pub tracks: usize,
    pub playlists: usize,
    pub bytes_to_write: u64,
    pub files: Vec<crate::transaction::FileChange>,
    pub warnings: Vec<String>,
    #[serde(default)]
    pub directories: Vec<crate::transaction::DirectoryChange>,
    #[serde(default)]
    pub deleted_playlists: usize,
    #[serde(default)]
    pub retained_external_playlists: usize,
    #[serde(default)]
    pub renames: Vec<crate::transaction::CaseRenameChange>,
}
#[derive(Serialize, Deserialize)]
struct PreparedRequest {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    related_roots: Vec<cuerithm_rekordbox::TargetPathSnapshot>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    target_root: Option<cuerithm_rekordbox::TargetPathSnapshot>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    selected_target: Option<SyncTarget>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    environment: Option<cuerithm_rekordbox::TargetEnvironmentSnapshot>,
    target: SyncTarget,
    #[serde(with = "crate::path_serde::map")]
    sources: BTreeMap<PathBuf, Option<String>>,
    preview: SyncPreview,
    #[serde(with = "crate::path_serde::map")]
    preconditions: BTreeMap<PathBuf, Option<String>>,
    #[serde(default, with = "crate::path_serde::rooted_map")]
    ownership: PlaylistOwnershipReports,
}

fn remember(path: &Path, before: &mut BTreeMap<PathBuf, Option<String>>) -> Result<()> {
    if !before.contains_key(path) {
        before.insert(path.into(), file_hash(path)?);
    }
    Ok(())
}
fn copy_file(
    source: &Path,
    stage: &Path,
    before: &mut BTreeMap<PathBuf, Option<String>>,
) -> Result<()> {
    remember(source, before)?;
    fs::create_dir_all(stage.parent().unwrap())?;
    fs::copy(source, stage)?;
    Ok(())
}
fn copy_tree(
    source: &Path,
    stage: &Path,
    before: &mut BTreeMap<PathBuf, Option<String>>,
    directories: &mut BTreeMap<PathBuf, Option<Vec<crate::transaction::DirectoryEntry>>>,
) -> Result<()> {
    directories.insert(
        source.into(),
        crate::transaction::directory_manifest(source)?,
    );
    if !source.exists() {
        return Ok(());
    }
    ensure!(
        !fs::symlink_metadata(source)?.file_type().is_symlink(),
        "Sync folder cannot be a symbolic link"
    );
    fs::create_dir_all(stage)?;
    let mut entries = fs::read_dir(source)?.collect::<std::io::Result<Vec<_>>>()?;
    entries.sort_by_key(|e| e.file_name());
    for entry in entries {
        let target = stage.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_tree(&entry.path(), &target, before, directories)?;
        } else {
            copy_file(&entry.path(), &target, before)?;
        }
    }
    Ok(())
}
fn sqlite_snapshot(
    source: &Path,
    stage: &Path,
    before: &mut BTreeMap<PathBuf, Option<String>>,
) -> Result<()> {
    remember(source, before)?;
    fs::create_dir_all(stage.parent().unwrap())?;
    for suffix in ["-wal", "-shm", "-journal"] {
        let mut name = source.as_os_str().to_owned();
        name.push(suffix);
        let path = PathBuf::from(name);
        remember(&path, before)?;
    }
    let connection =
        rusqlite::Connection::open_with_flags(source, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)?;
    connection.busy_timeout(std::time::Duration::from_secs(5))?;
    connection.backup("main", stage, None)?;
    Ok(())
}
fn list_files(root: &Path) -> Result<Vec<PathBuf>> {
    if !root.exists() {
        return Ok(vec![]);
    }
    let mut files = vec![];
    for entry in fs::read_dir(root)? {
        let entry = entry?;
        ensure!(
            !entry.file_type()?.is_symlink(),
            "Staged export contains a symbolic link"
        );
        if entry.file_type()?.is_dir() {
            files.extend(list_files(&entry.path())?);
        } else {
            files.push(entry.path());
        }
    }
    files.sort();
    Ok(files)
}

type DirectorySnapshots = BTreeMap<PathBuf, Option<Vec<crate::transaction::DirectoryEntry>>>;
fn stage_directory_plan(
    stage: &Path,
    destination: &Path,
    captured: Option<&DirectorySnapshots>,
) -> Result<Vec<crate::transaction::DirectoryChange>> {
    let mut pending = vec![stage.to_path_buf()];
    let mut result = Vec::new();
    while let Some(directory) = pending.pop() {
        let target = destination.join(directory.strip_prefix(stage)?);
        let before_manifest = if let Some(captured) = captured {
            captured.get(&target).cloned().flatten()
        } else {
            crate::transaction::directory_manifest(&target)?
        };
        result.push(crate::transaction::DirectoryChange {
            target,
            before_manifest,
            after_exists: true,
        });
        for entry in fs::read_dir(directory)? {
            let entry = entry?;
            ensure!(
                !entry.file_type()?.is_symlink(),
                "Staged directory contains a symbolic link"
            );
            if entry.file_type()?.is_dir() {
                pending.push(entry.path());
            }
        }
    }
    result.sort_by_key(|d| d.target.clone());
    Ok(result)
}
fn usb_cleanup_candidates(
    root: &Path,
    files: &[PathBuf],
    replacements: &BTreeMap<PathBuf, PathBuf>,
    protected: &std::collections::BTreeSet<PathBuf>,
) -> Result<std::collections::BTreeSet<PathBuf>> {
    let catalog_exists = |folder: &str| {
        ["export.pdb", "exportLibrary.db"]
            .iter()
            .any(|file| root.join(folder).join("rekordbox").join(file).is_file())
    };
    ensure!(
        !(catalog_exists("PIONEER") && catalog_exists(".PIONEER")),
        "This drive contains two Pioneer library catalogs; disable unused file cleanup to preserve both"
    );
    let mut deletions = std::collections::BTreeSet::new();
    for path in files {
        let relative = path.strip_prefix(root)?;
        let managed = relative.starts_with("Contents")
            || relative.starts_with("PIONEER/USBANLZ")
            || relative.starts_with(".PIONEER/USBANLZ")
            || relative.starts_with("PIONEER/Artwork")
            || relative.starts_with(".PIONEER/Artwork");
        if managed
            && !protected.contains(&fs::canonicalize(path)?)
            && !replacements.contains_key(path)
        {
            deletions.insert(path.clone());
        }
    }
    Ok(deletions)
}
fn validate_device_source_replacements(
    software: DjSoftware,
    replacements: &BTreeMap<PathBuf, PathBuf>,
    protected: &std::collections::BTreeSet<PathBuf>,
) -> Result<()> {
    if !matches!(software, DjSoftware::EngineDj | DjSoftware::PioneerUsb) {
        return Ok(());
    }
    for (destination, staged) in replacements {
        let canonical = fs::canonicalize(destination).unwrap_or_else(|_| destination.clone());
        if protected.contains(destination) || protected.contains(&canonical) {
            ensure!(
                file_hash(destination)? == file_hash(staged)?,
                "Device export would replace a library source audio file; choose another destination or keep the source copy unchanged"
            );
        }
    }
    Ok(())
}

fn engine_device_library(root: &Path) -> bool {
    // Discovery checks <drive>/Engine Library on Windows. Canonical paths may
    // carry the verbatim prefix; normal Music/Engine Library folders stay local.
    let windows = root.to_string_lossy().replace('\\', "/");
    let windows = windows.strip_prefix("//?/").unwrap_or(&windows);
    let bytes = windows.as_bytes();
    let windows_drive = bytes.len() >= 3
        && bytes[0].is_ascii_alphabetic()
        && &bytes[1..3] == b":/"
        && windows[3..]
            .trim_end_matches('/')
            .eq_ignore_ascii_case("Engine Library");
    windows_drive
        || root.starts_with("/Volumes")
        || root.starts_with("/media")
        || root.starts_with("/run/media")
        || root.starts_with("/mnt")
}

/// Remove filesystem-only Windows verbatim prefixes from a serialized DJ reference.
pub fn destination_reference(path: &Path) -> PathBuf {
    let Some(text) = path.to_str() else {
        return path.to_owned();
    };
    if let Some(local) = text.strip_prefix("\\\\?\\") {
        if local
            .get(..4)
            .is_some_and(|prefix| prefix.eq_ignore_ascii_case("UNC\\"))
        {
            return PathBuf::from(format!("\\\\{}", &local[4..]));
        }
        if local.as_bytes().get(1) == Some(&b':') {
            return PathBuf::from(local);
        }
    }
    path.to_owned()
}

pub(crate) fn windows_volume_root(path: &Path) -> Option<String> {
    let reference = destination_reference(path);
    let path = reference.to_str()?.replace('\\', "/");
    let path = path.strip_prefix("//?/").unwrap_or(&path);
    if path.as_bytes().get(1) == Some(&b':') && path.as_bytes().get(2) == Some(&b'/') {
        return Some(path[..3].to_owned());
    }
    let unc = path.strip_prefix("//")?;
    let mut parts = unc.split('/');
    let server = parts.next()?;
    let share = parts.next()?;
    (!server.is_empty() && !share.is_empty()).then(|| format!("//{server}/{share}"))
}

fn compatibility_serato_library(
    environment: &cuerithm_rekordbox::TargetEnvironment,
) -> Result<PathBuf> {
    let users = environment.to_host(Path::new("C:\\users"))?;
    let mut candidates = Vec::new();
    let mut inspected = 0usize;
    for user in fs::read_dir(users)? {
        inspected += 1;
        ensure!(
            inspected <= 256,
            "Too many Windows user folders; select the Serato root library explicitly"
        );
        let user = user?;
        if user.file_type()?.is_dir() {
            let library = user.path().join("AppData/Local/Serato/Library");
            if library.join("master.sqlite").is_file() {
                candidates.push(library);
            }
        }
    }
    ensure!(
        candidates.len() == 1,
        "Select the Windows Serato root library; no unambiguous master database was found in the selected drive mapping"
    );
    Ok(candidates.remove(0))
}

fn project_windows_smartlists(
    library: &mut SyncLibrary,
    target: &SyncTarget,
    options: &SyncOptions,
) -> Vec<String> {
    use cuerithm_smartlists::{Operator, RuleValue};
    if target.environment.is_native() || options.materialize_smartlists {
        return vec![];
    }
    let mut warnings = Vec::new();
    for playlist in &mut library.playlists {
        let Some(definition) = &mut playlist.smart else {
            continue;
        };
        let mut reason = None;
        if target.software == DjSoftware::DjayPro {
            reason = Some("current djay on Windows requires ordinary playlists");
        }
        for rule in &mut definition.rules {
            if rule.field != "location" {
                continue;
            }
            let RuleValue::Text(value) = &mut rule.value else {
                reason = Some("its location rule cannot be used with these Windows paths");
                break;
            };
            let path = Path::new(value);
            if !path.is_absolute()
                || !matches!(
                    rule.operator,
                    Operator::StringEquals | Operator::StringNotEquals
                )
            {
                reason =
                    Some("its location rule could match different tracks with these Windows paths");
                break;
            }
            let Ok(native) = target.environment.to_native(path) else {
                reason = Some("its location rule has no clear Windows drive mapping");
                break;
            };
            *value = native.to_string_lossy().into_owned();
        }
        if let Some(reason) = reason {
            playlist.smart = None;
            warnings.push(format!(
                "Smart playlist '{}' exports its saved tracks because {reason}",
                playlist.name
            ));
        }
    }
    warnings
}

fn serato_group(
    library: &SyncLibrary,
    tracks: Vec<crate::SyncTrack>,
    options: &SyncOptions,
    include_native_smart: bool,
) -> SyncLibrary {
    use std::collections::BTreeSet;
    let ids: BTreeSet<_> = tracks.iter().map(|t| t.id.as_str()).collect();
    let mut keep: BTreeSet<String> = library
        .playlists
        .iter()
        .filter(|p| {
            let native = matches!(
                crate::smart_rules::compile(p, library, options, DjSoftware::Serato4),
                Ok(Some(crate::smart_rules::NativeSmart::Serato4(_)))
            );
            !p.folder
                && if native {
                    include_native_smart
                } else {
                    p.track_ids.is_empty() || p.track_ids.iter().any(|id| ids.contains(id.as_str()))
                }
        })
        .map(|p| p.id.clone())
        .collect();
    for folder in library.playlists.iter().filter(|p| {
        p.folder
            && !library
                .playlists
                .iter()
                .any(|c| c.parent_id.as_ref() == Some(&p.id))
    }) {
        keep.insert(folder.id.clone());
    }
    loop {
        let parents: Vec<_> = library
            .playlists
            .iter()
            .filter(|p| keep.contains(&p.id))
            .filter_map(|p| p.parent_id.clone())
            .filter(|id| !keep.contains(id))
            .collect();
        if parents.is_empty() {
            break;
        }
        keep.extend(parents);
    }
    let playlists = library
        .playlists
        .iter()
        .filter(|p| keep.contains(&p.id))
        .map(|p| {
            let mut p = p.clone();
            p.track_ids.retain(|id| ids.contains(id.as_str()));
            p
        })
        .collect();
    SyncLibrary { tracks, playlists }
}

/// Build actual target files on private copies. The returned preview describes
/// those exact staged bytes; applying never reruns an adapter on live files.
pub fn prepare_sync(
    journal_root: &Path,
    target: SyncTarget,
    library: &SyncLibrary,
    options: &SyncOptions,
) -> Result<SyncPreview> {
    prepare_sync_with_runtime(journal_root, target, library, options, None)
}
pub fn prepare_sync_with_runtime(
    journal_root: &Path,
    target: SyncTarget,
    library: &SyncLibrary,
    options: &SyncOptions,
    runtime: Option<&cuerithm_media::MediaRuntime>,
) -> Result<SyncPreview> {
    prepare_sync_owned_with_runtime(
        journal_root,
        target,
        library,
        options,
        runtime,
        &BTreeMap::new(),
        &Default::default(),
    )
}
pub fn prepare_sync_owned_with_runtime(
    journal_root: &Path,
    target: SyncTarget,
    library: &SyncLibrary,
    options: &SyncOptions,
    runtime: Option<&cuerithm_media::MediaRuntime>,
    contexts: &PlaylistOwnershipContexts,
    protected_sources: &std::collections::BTreeSet<PathBuf>,
) -> Result<SyncPreview> {
    ensure!(
        target.software != DjSoftware::Rekordbox,
        "Use the native rekordbox collection workflow"
    );
    let library = crate::formatting::format_library(library, options)?;
    validate_library(&library)?;
    // USB conversion happens after optional PCM transcoding, on the output's clock.
    let library = if target.software == DjSoftware::PioneerUsb {
        library
    } else {
        crate::timing::export_library(&library, target.software)?
    };
    let mut library = library;
    // Destination projections never replace the physical source evidence and are
    // always derived here, including when a caller supplied native_path.
    let environment = target.environment.freeze()?;
    ensure_closed(target.software)?;
    let selected_target = target.clone();
    let target_root = cuerithm_rekordbox::TargetPathSnapshot::capture(if target.path.is_dir() {
        &target.path
    } else {
        target.path.parent().context("DJ library parent missing")?
    })?;
    let mut target = target;
    target.path = fs::canonicalize(&target.path)
        .context("DJ target was not found; initialize it in the DJ app first")?;
    if let Some(root) = &target.source_root {
        target.source_root = Some(fs::canonicalize(root)?);
    }
    let transaction = FileTransaction::create(journal_root)?;
    let build_directory = build::BuildDirectory::new(transaction.path())?;
    let build = build_directory.path();
    let mut before = BTreeMap::new();
    let mut sources = BTreeMap::new();
    let mut input_paths = std::collections::BTreeSet::new();
    for path in protected_sources {
        input_paths.insert(fs::canonicalize(path).unwrap_or_else(|_| path.clone()));
    }
    for track in &library.tracks {
        let hash = file_hash(&track.path)?.context("Sync audio file is missing")?;
        if let Some(profile) = &track.export_timing {
            ensure!(
                profile.source_sha256 == hash,
                "Source timing profile does not belong to the selected audio file"
            );
            if let Some(runtime) = runtime {
                ensure!(
                    profile.measurement_identity == runtime.export_timing_identity(),
                    "Source timing measurement implementation changed; measure this source again"
                );
            }
        }

        sources.insert(track.path.clone(), Some(hash));
        input_paths.insert(fs::canonicalize(&track.path)?);
    }
    let copies_media = match target.software {
        DjSoftware::PioneerUsb => true,
        DjSoftware::EngineDj => {
            let database = crate::engine_dj::resolve_database_path(&target.path)?;
            let root = database
                .parent()
                .and_then(Path::parent)
                .context("Engine Library folder is missing")?;
            options.engine_copy_audio || engine_device_library(&target.environment.to_native(root)?)
        }
        _ => false,
    };
    let mut references = std::collections::BTreeSet::new();
    for track in &mut library.tracks {
        track.native_path = None;
        // Device-owned audio is named by the adapter after copying; the original
        // physical source does not need a destination-visible Windows spelling.
        if copies_media {
            continue;
        }
        let reference = destination_reference(&target.environment.to_native(&track.path)?);
        track.native_path = (reference != track.path).then_some(reference);
        if !target.environment.is_native()
            || cfg!(windows)
            || matches!(target.software, DjSoftware::VirtualDj | DjSoftware::Serato4)
        {
            let reference = track
                .reference_path()
                .to_str()
                .context("Destination requires Unicode source paths")?
                .replace('\\', "/")
                .to_lowercase();
            ensure!(
                references.insert(reference),
                "Selected source paths collide in the Windows destination"
            );
        }
    }
    let mut context_warnings = Vec::new();
    if copies_media {
        for playlist in &mut library.playlists {
            if playlist.smart.as_ref().is_some_and(|definition| {
                definition.rules.iter().any(|rule| rule.field == "location")
            }) {
                playlist.smart = None;
                if !options.materialize_smartlists {
                    context_warnings.push(format!("Smart playlist '{}' exports its saved tracks because copied audio uses new file locations",playlist.name));
                }
            }
        }
    }
    context_warnings.extend(project_windows_smartlists(&mut library, &target, options));
    let library = &library;
    let mut replacements: BTreeMap<PathBuf, PathBuf> = BTreeMap::new();
    let mut deletions = std::collections::BTreeSet::<PathBuf>::new();
    let mut directories = Vec::<crate::transaction::DirectoryChange>::new();
    let mut warnings = crate::smart_fallbacks(library, options, target.software);
    warnings.extend(context_warnings);
    let mut ownership = PlaylistOwnershipReports::new();
    let empty_ownership = crate::ownership::PlaylistOwnershipRequest::default();
    let mut related_roots = Vec::new();
    match target.software {
        DjSoftware::Rekordbox => anyhow::bail!("Use the native rekordbox collection workflow"),
        DjSoftware::VirtualDj => {
            let original = if target.path.is_dir() {
                target.path.join("database.xml")
            } else {
                target.path.clone()
            };
            let home = original.parent().context("VirtualDJ folder missing")?;
            let primary_volume = windows_volume_root(&target.environment.to_native(&original)?);
            let mut local_tracks = Vec::new();
            let mut other_drives = BTreeMap::<PathBuf, Vec<crate::SyncTrack>>::new();
            for track in &library.tracks {
                let volume = windows_volume_root(track.reference_path());
                if let Some(volume) = volume.filter(|volume| {
                    primary_volume
                        .as_ref()
                        .is_some_and(|primary| !primary.eq_ignore_ascii_case(volume))
                }) {
                    let native = PathBuf::from(format!(
                        "{}/VirtualDJ/database.xml",
                        volume.trim_end_matches('/')
                    ));
                    let database = target.environment.to_host(&native)?;
                    ensure!(
                        database.is_file(),
                        "VirtualDJ drive database is missing; initialize this drive in VirtualDJ first"
                    );
                    other_drives
                        .entry(fs::canonicalize(database)?)
                        .or_default()
                        .push(track.clone());
                } else {
                    local_tracks.push(track.clone());
                }
            }
            let stage = build.join("virtualdj");
            let database = stage.join("database.xml");
            let mut captured = DirectorySnapshots::new();
            for directory in [
                home.to_path_buf(),
                home.join("MyLists"),
                home.join("Playlists"),
                home.join("Folders"),
            ] {
                captured.insert(
                    directory.clone(),
                    crate::transaction::directory_manifest(&directory)?,
                );
            }
            copy_file(&original, &database, &mut before)?;
            let playlist_roots = crate::virtual_dj::playlist_roots(home);
            if home.join("MyLists").is_dir() {
                fs::create_dir_all(stage.join("MyLists"))?;
            }
            for root in &playlist_roots {
                copy_tree(
                    root,
                    &stage.join(root.strip_prefix(home)?),
                    &mut before,
                    &mut captured,
                )?;
            }
            let report = crate::virtual_dj::write_owned_partitioned(
                &database,
                library,
                &local_tracks,
                options,
                contexts
                    .get(&original)
                    .map_or(&empty_ownership, |(_, request)| request),
            )?;
            ownership.insert(original.clone(), (None, report));
            for path in list_files(&stage)? {
                let relative = path.strip_prefix(&stage)?;
                let destination = if relative == Path::new("database.xml") {
                    original.clone()
                } else {
                    home.join(relative)
                };
                replacements.insert(destination, path);
            }
            let staged_directories = stage_directory_plan(&stage, home, None)?;
            let original_paths: Vec<_> = before.keys().chain(captured.keys()).cloned().collect();
            let targets: Vec<_> = replacements
                .keys()
                .cloned()
                .chain(staged_directories.iter().map(|d| d.target.clone()))
                .collect();
            crate::transaction::plan_case_renames(&transaction, &original_paths, &targets)?;
            let renames = transaction.inspect()?.renames;
            let captured: DirectorySnapshots = captured
                .into_iter()
                .map(|(path, manifest)| {
                    (
                        crate::transaction::translate_case_renames(&path, &renames),
                        manifest,
                    )
                })
                .collect();
            directories.extend(stage_directory_plan(&stage, home, Some(&captured))?);
            for (path, manifest) in &captured {
                if playlist_roots.iter().any(|root| path.starts_with(root))
                    && manifest.is_some()
                    && !stage.join(path.strip_prefix(home)?).is_dir()
                {
                    directories.push(crate::transaction::DirectoryChange {
                        target: path.clone(),
                        before_manifest: manifest.clone(),
                        after_exists: false,
                    });
                }
            }
            for path in before.keys() {
                if playlist_roots.iter().any(|root| path.starts_with(root))
                    && !replacements.contains_key(path)
                {
                    deletions.insert(path.clone());
                }
            }
            for (index, (original, tracks)) in other_drives.into_iter().enumerate() {
                let stage = build
                    .join("virtualdj-drives")
                    .join(index.to_string())
                    .join("database.xml");
                related_roots.push(cuerithm_rekordbox::TargetPathSnapshot::capture(
                    original
                        .parent()
                        .context("VirtualDJ drive library parent missing")?,
                )?);
                copy_file(&original, &stage, &mut before)?;
                let group = SyncLibrary {
                    tracks,
                    playlists: vec![],
                };
                let report = crate::virtual_dj::write_owned(
                    &stage,
                    &group,
                    options,
                    contexts
                        .get(&original)
                        .map_or(&empty_ownership, |(_, request)| request),
                )?;
                ownership.insert(original.clone(), (None, report));
                replacements.insert(original, stage);
            }
        }
        DjSoftware::EngineDj => {
            let database = crate::engine_dj::resolve_database_path(&target.path)?;
            let root = database
                .parent()
                .and_then(Path::parent)
                .context("Engine Library folder is missing")?;
            related_roots.push(cuerithm_rekordbox::TargetPathSnapshot::capture(root)?);
            let stage = build.join("engine/Database2/m.db");
            sqlite_snapshot(&database, &stage, &mut before)?;
            let portable = copies_media;
            if portable {
                let (projected, media) =
                    crate::engine_dj::stage_device_media(&build.join("engine"), root, library)?;
                for (relative, file) in media {
                    let destination = root.join(relative);
                    remember(&destination, &mut before)?;
                    replacements.insert(destination, file);
                }
                let report = crate::engine_dj::write_staged_owned(
                    &stage,
                    root,
                    &projected,
                    options,
                    contexts
                        .get(&database)
                        .map_or(&empty_ownership, |(_, request)| request),
                )?;
                ownership.insert(database.clone(), (None, report));
            } else {
                let report = crate::engine_dj::write_staged_owned(
                    &stage,
                    &destination_reference(&target.environment.to_native(root)?),
                    library,
                    options,
                    contexts
                        .get(&database)
                        .map_or(&empty_ownership, |(_, request)| request),
                )?;
                ownership.insert(database.clone(), (None, report));
            }
            directories.extend(stage_directory_plan(&build.join("engine"), root, None)?);
            replacements.insert(database, stage);
        }
        DjSoftware::DjayPro => {
            let database = if target.path.is_dir() {
                target.path.join("MediaLibrary.db")
            } else {
                target.path.clone()
            };
            let stage = build.join("djay/MediaLibrary.db");
            sqlite_snapshot(&database, &stage, &mut before)?;
            let report = crate::djay_pro::write_owned_in_environment(
                &stage,
                library,
                options,
                contexts
                    .get(&database)
                    .map_or(&empty_ownership, |(_, request)| request),
                &target.environment,
            )?;
            ownership.insert(database.clone(), (None, report));
            replacements.insert(database, stage);
        }
        DjSoftware::Serato4 => {
            if target.source_root.is_some() && !options.materialize_smartlists {
                for playlist in &library.playlists {
                    if playlist.smart.is_some() {
                        warnings.push(format!("Smartlist '{}' was materialized: standalone Serato locations cannot store global root smart crates",playlist.name));
                    }
                }
            }
            crate::serato4::validate(library, options)?;
            let database = if target.path.is_dir() {
                target.path.join("root.sqlite")
            } else {
                target.path.clone()
            };
            let parent = database.parent().context("Serato Library folder missing")?;
            let local_library = if parent.join("master.sqlite").is_file() {
                parent.to_path_buf()
            } else if target.source_root.is_some() && !target.environment.is_native() {
                compatibility_serato_library(&target.environment)?
            } else if target.source_root.is_some() {
                #[cfg(windows)]
                let local = std::env::var_os("LOCALAPPDATA")
                    .map(PathBuf::from)
                    .context("Local Serato library is unavailable")?
                    .join("Serato/Library");
                #[cfg(not(windows))]
                let local = std::env::var_os("HOME")
                    .map(PathBuf::from)
                    .context("Local Serato library is unavailable")?
                    .join("Library/Application Support/Serato/Library");
                local
            } else {
                parent.to_path_buf()
            };
            let master = local_library.join("master.sqlite");
            ensure!(
                master.is_file(),
                "Open and close Serato 4 to initialize its master database"
            );
            related_roots.push(cuerithm_rekordbox::TargetPathSnapshot::capture(
                &local_library,
            )?);
            let staged_master = build.join("serato/master.sqlite");
            sqlite_snapshot(&master, &staged_master, &mut before)?;
            let mut master_request = contexts
                .get(&master)
                .map_or_else(Default::default, |(_, r)| r.clone());
            let mut master_report: Option<crate::ownership::PlaylistOwnershipReport> = None;
            let mut groups: BTreeMap<PathBuf, (Option<PathBuf>, Vec<crate::SyncTrack>)> =
                BTreeMap::new();
            if target.source_root.is_some() {
                groups.insert(
                    database.clone(),
                    (target.source_root.clone(), library.tracks.clone()),
                );
            } else {
                groups.insert(database.clone(), (None, vec![]));
                for track in &library.tracks {
                    let (path, root) = crate::serato4::location_for_track_in_environment(
                        &local_library,
                        &track.path,
                        &target.environment,
                    )?;
                    groups
                        .entry(path)
                        .or_insert_with(|| (root, vec![]))
                        .1
                        .push(track.clone());
                }
            }
            for (path, (root, _)) in contexts {
                if path != &master {
                    groups
                        .entry(path.clone())
                        .or_insert_with(|| (root.clone(), vec![]));
                }
            }
            for (index, (original, (root, tracks))) in groups.into_iter().enumerate() {
                related_roots.push(cuerithm_rekordbox::TargetPathSnapshot::capture(
                    original
                        .parent()
                        .context("Serato location parent missing")?,
                )?);
                if let Some(root) = &root {
                    related_roots.push(cuerithm_rekordbox::TargetPathSnapshot::capture(root)?);
                    let info = root.join("_Serato_/Library/location.info");
                    if info.exists() {
                        sources.insert(info.clone(), file_hash(&info)?);
                    }
                }
                let mut group_options = options.clone();
                if target.source_root.is_some() {
                    group_options.materialize_smartlists = true;
                }
                let group = serato_group(library, tracks, &group_options, root.is_none());
                let stage = build.join("serato/locations").join(index.to_string()).join(
                    original
                        .file_name()
                        .context("Serato database name missing")?,
                );
                sqlite_snapshot(&original, &stage, &mut before)?;
                let report = crate::serato4::write_database_owned(
                    &stage,
                    root.as_ref()
                        .map(|root| target.environment.to_native(root))
                        .transpose()?
                        .as_deref(),
                    &group,
                    &group_options,
                    contexts
                        .get(&original)
                        .map_or(&empty_ownership, |(_, request)| request),
                )?;
                let mut aggregate = crate::serato4::refresh_master_owned(
                    &staged_master,
                    &stage,
                    &target.environment.to_native(&original)?,
                    &master_request,
                    &report,
                )?;
                if let Some(previous) = master_report.take() {
                    aggregate.deleted.extend(previous.deleted);
                    aggregate.retired.extend(previous.retired);
                }
                master_request.previous = aggregate.current.clone();
                master_report = Some(aggregate);
                ownership.insert(original.clone(), (root, report));
                replacements.insert(original, stage);
            }
            if let Some(report) = master_report {
                ownership.insert(master.clone(), (None, report));
            }
            replacements.insert(master, staged_master);
            for (index, track) in library.tracks.iter().enumerate() {
                let extension = track
                    .path
                    .extension()
                    .and_then(|e| e.to_str())
                    .context("Serato audio format is missing")?;
                let stage = build.join("media").join(format!("{index}.{extension}"));
                copy_file(&track.path, &stage, &mut before)?;
                crate::serato4::write_media(&stage, track, options)?;
                replacements.insert(track.path.clone(), stage);
            }
        }
        DjSoftware::PioneerUsb => {
            let stage = build.join("usb");
            fs::create_dir_all(&stage)?;
            let mut device_files = vec![];
            for folder in ["PIONEER", ".PIONEER", "Contents"] {
                device_files.extend(list_files(&target.path.join(folder))?);
            }
            for path in &device_files {
                remember(path, &mut before)?;
            }
            let report = crate::pioneer_usb::write_staged_owned(
                &stage,
                &target.path,
                library,
                options,
                runtime,
                contexts
                    .get(&target.path)
                    .map_or(&empty_ownership, |(_, request)| request),
            )?;
            ownership.insert(target.path.clone(), (None, report.ownership));
            warnings.extend(report.warnings);
            let written = report.files;
            for relative in written {
                ensure!(
                    relative
                        .components()
                        .all(|c| matches!(c, std::path::Component::Normal(_))),
                    "USB export contains an invalid relative path"
                );
                replacements.insert(target.path.join(&relative), stage.join(&relative));
            }
            directories.extend(stage_directory_plan(&stage, &target.path, None)?);
            if options.usb_cleanup_unused {
                deletions.extend(usb_cleanup_candidates(
                    &target.path,
                    &device_files,
                    &replacements,
                    &input_paths,
                )?);
            }
        }
    }
    for path in replacements.keys() {
        remember(path, &mut before)?;
    }
    validate_device_source_replacements(target.software, &replacements, &input_paths)?;
    ensure_closed(target.software)?;
    // Snapshotting SQLite can change the volatile shared-memory index. It is
    // never copied into a staged database and must be removed with an old WAL.
    for (path, hash) in &mut before {
        if path.to_string_lossy().ends_with("-shm") {
            *hash = file_hash(path)?;
        } else {
            ensure!(
                file_hash(path)? == *hash,
                "DJ target changed while preparing sync"
            );
        }
    }
    for (path, hash) in &sources {
        ensure!(
            file_hash(path)? == *hash,
            "Audio changed while preparing sync"
        );
    }
    for path in before.keys() {
        if replacements.contains_key(path) {
            continue;
        }
        let name = path.to_string_lossy();
        if ["-wal", "-shm", "-journal"].iter().any(|suffix| {
            name.strip_suffix(suffix)
                .is_some_and(|base| replacements.contains_key(Path::new(base)))
        }) {
            deletions.insert(path.clone());
        }
    }
    let renames = transaction.inspect()?.renames;
    let normalize = |path: &Path| crate::transaction::translate_case_renames(path, &renames);
    let mut normalized_before = BTreeMap::new();
    for (path, hash) in before {
        let path = normalize(&path);
        if let Some(prior) = normalized_before.insert(path, hash.clone()) {
            ensure!(prior == hash, "Conflicting target alias fingerprints");
        }
    }
    let before = normalized_before;
    let replacements: BTreeMap<_, _> = replacements
        .into_iter()
        .map(|(path, staged)| (normalize(&path), staged))
        .collect();
    let deletions: std::collections::BTreeSet<_> = deletions
        .into_iter()
        .map(|p| normalize(&p))
        .filter(|p| !replacements.contains_key(p))
        .collect();
    let mut normalized_directories = BTreeMap::new();
    for mut directory in directories {
        directory.target = normalize(&directory.target);
        if let Some(prior) =
            normalized_directories.insert(directory.target.clone(), directory.clone())
        {
            ensure!(
                prior.before_manifest == directory.before_manifest
                    && prior.after_exists == directory.after_exists,
                "Conflicting target directory aliases"
            );
        }
    }
    let mut files = Vec::new();
    for path in deletions {
        files.push(crate::transaction::FilePublication {
            before_sha256: before[&path].clone(),
            target: path,
            staged: None,
        });
    }
    for (path, staged) in replacements {
        files.push(crate::transaction::FilePublication {
            before_sha256: before[&path].clone(),
            target: path,
            staged: Some(staged),
        });
    }
    transaction.add_batch(
        &files,
        &normalized_directories.into_values().collect::<Vec<_>>(),
    )?;
    // add_batch has copied and fsynced every publication into persistent staged/.
    // Release adapter working copies before returning a durable preview.
    build_directory.close()?;
    environment.verify()?;
    target_root.verify()?;
    for root in &related_roots {
        root.verify()?;
    }
    let journal = transaction.inspect()?;
    let preview = SyncPreview {
        id: journal.id,
        target: target.clone(),
        tracks: library.tracks.len(),
        playlists: library.playlists.len(),
        bytes_to_write: journal.changes.iter().map(|c| c.bytes).sum(),
        files: journal.changes,
        warnings,
        directories: journal.directories,
        deleted_playlists: ownership
            .values()
            .flat_map(|(_, r)| &r.deleted)
            .collect::<std::collections::BTreeSet<_>>()
            .len(),
        retained_external_playlists: ownership
            .values()
            .flat_map(|(_, r)| &r.retired)
            .collect::<std::collections::BTreeSet<_>>()
            .len(),
        renames: journal.renames,
    };
    crate::transaction::durable_json(
        &transaction.path().join("request.json"),
        &PreparedRequest {
            related_roots,
            target_root: Some(target_root),
            selected_target: Some(selected_target),
            environment: Some(environment),
            target,
            sources,
            preview: preview.clone(),
            preconditions: before,
            ownership,
        },
    )?;
    Ok(preview)
}
fn request(transaction: &FileTransaction) -> Result<PreparedRequest> {
    Ok(serde_json::from_reader(fs::File::open(
        transaction.path().join("request.json"),
    )?)?)
}
fn verify_environment(prepared: &PreparedRequest) -> Result<()> {
    for root in &prepared.related_roots {
        root.verify()?;
    }
    if let Some(root) = &prepared.target_root {
        root.verify()?;
    }
    if let Some(selected) = &prepared.selected_target {
        ensure!(
            fs::canonicalize(&selected.path)? == prepared.target.path
                && selected
                    .source_root
                    .as_ref()
                    .map(fs::canonicalize)
                    .transpose()?
                    == prepared.target.source_root,
            "Selected DJ library location changed after preview"
        );
    }
    if let Some(snapshot) = &prepared.environment {
        ensure!(
            snapshot.environment == prepared.target.environment,
            "Sync target environment changed after preview"
        );
        snapshot.verify()?;
    } else {
        ensure!(
            prepared.target.environment.is_native(),
            "Sync preview has no captured target environment; preview again"
        );
    }
    Ok(())
}
fn dependency_checks<'a>(
    prepared: &'a PreparedRequest,
    journal: &FileJournal,
) -> Vec<(&'a PathBuf, &'a Option<String>)> {
    let changed: std::collections::BTreeSet<_> =
        journal.changes.iter().map(|c| &c.target).collect();
    prepared
        .preconditions
        .iter()
        .filter(|(path, _)| !changed.contains(path))
        .collect()
}
pub(crate) fn is_sqlite_sidecar(path: &Path) -> bool {
    let name = path.to_string_lossy();
    ["-wal", "-shm", "-journal"]
        .iter()
        .any(|suffix| name.ends_with(suffix))
}
pub fn apply_sync(journal_root: &Path, id: &str) -> Result<FileJournal> {
    let transaction = FileTransaction::open(journal_root, id)?;
    let prepared = request(&transaction)?;
    verify_environment(&prepared)?;
    for (path, hash) in &prepared.sources {
        ensure!(
            file_hash(path)? == *hash,
            "Audio changed after sync preview; preview again"
        );
    }
    let planned = transaction.inspect()?;
    let all = dependency_checks(&prepared, &planned);
    let sidecars: Vec<_> = all
        .iter()
        .copied()
        .filter(|(path, _)| is_sqlite_sidecar(path))
        .collect();
    let first = std::cell::Cell::new(true);
    transaction.apply(|| {
        verify_environment(&prepared)?;
        ensure_closed(prepared.target.software)?;
        for (path, hash) in if first.replace(false) {
            &all
        } else {
            &sidecars
        } {
            ensure!(
                crate::transaction::observed_file_hash(&planned, path)? == **hash,
                "Sync preview dependencies changed: {}",
                path.display()
            );
        }
        Ok(())
    })?;
    verify_sync_applied(journal_root, id)
}
pub fn restore_sync(journal_root: &Path, id: &str) -> Result<FileJournal> {
    let transaction = FileTransaction::open(journal_root, id)?;
    let prepared = request(&transaction)?;
    verify_environment(&prepared)?;
    let planned = transaction.inspect()?;
    let all = dependency_checks(&prepared, &planned);
    let sidecars: Vec<_> = all
        .iter()
        .copied()
        .filter(|(path, _)| is_sqlite_sidecar(path))
        .collect();
    let first = std::cell::Cell::new(true);
    transaction.restore(|| {
        verify_environment(&prepared)?;
        ensure_closed(prepared.target.software)?;
        for (path, hash) in if first.replace(false) {
            &all
        } else {
            &sidecars
        } {
            ensure!(
                crate::transaction::observed_file_hash(&planned, path)? == **hash,
                "Sync restore dependencies changed: {}",
                path.display()
            );
        }
        Ok(())
    })?;
    verify_sync_restored(journal_root, id)
}
/// Verify captured mappings before a service changes acknowledgement metadata.
pub fn verify_sync_environment(journal_root: &Path, id: &str) -> Result<()> {
    verify_environment(&request(&FileTransaction::open(journal_root, id)?)?)
}
pub fn sync_preview(journal_root: &Path, id: &str) -> Result<SyncPreview> {
    request(&FileTransaction::open(journal_root, id)?).map(|p| p.preview)
}
pub fn sync_ownership(journal_root: &Path, id: &str) -> Result<PlaylistOwnershipReports> {
    Ok(request(&FileTransaction::open(journal_root, id)?)?.ownership)
}
pub fn verify_sync_applied(journal_root: &Path, id: &str) -> Result<FileJournal> {
    let transaction = FileTransaction::open(journal_root, id)?;
    let prepared = request(&transaction)?;
    verify_environment(&prepared)?;
    ensure_closed(prepared.target.software)?;
    let journal = transaction.verify_applied()?;
    let changes: BTreeMap<_, _> = journal.changes.iter().map(|c| (&c.target, c)).collect();
    for (path, before) in prepared.preconditions {
        if !changes.contains_key(&path) {
            ensure!(
                file_hash(&path)? == before,
                "Sync dependency changed before acknowledgement recovery"
            );
        }
    }
    for (path, before) in prepared.sources {
        let expected = changes
            .get(&path)
            .map_or(&before, |change| &change.after_sha256);
        ensure!(
            file_hash(&path)? == *expected,
            "Audio changed before acknowledgement recovery"
        );
    }
    Ok(journal)
}
pub fn verify_sync_restored(journal_root: &Path, id: &str) -> Result<FileJournal> {
    let transaction = FileTransaction::open(journal_root, id)?;
    let prepared = request(&transaction)?;
    verify_environment(&prepared)?;
    ensure_closed(prepared.target.software)?;
    let journal = transaction.verify_restored()?;
    let changes: std::collections::BTreeSet<_> =
        journal.changes.iter().map(|c| &c.target).collect();
    for (path, hash) in prepared.preconditions {
        if !changes.contains(&path) {
            ensure!(
                file_hash(&path)? == hash,
                "Sync dependency changed before restore acknowledgement recovery"
            );
        }
    }
    for (path, hash) in prepared.sources {
        ensure!(
            file_hash(&path)? == hash,
            "Audio changed before restore acknowledgement recovery"
        );
    }
    Ok(journal)
}

/// Classify exports which own an independent media copy rather than a source path.
pub fn export_media_is_independent(target: &SyncTarget, options: &SyncOptions) -> bool {
    target.software == DjSoftware::PioneerUsb
        || (target.software == DjSoftware::EngineDj
            && (options.engine_copy_audio
                || target
                    .path
                    .parent()
                    .and_then(Path::parent)
                    .and_then(|root| target.environment.to_native(root).ok())
                    .as_deref()
                    .is_some_and(engine_device_library)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::SyncTrack;
    #[test]
    fn windows_context_virtualdj_routes_external_drive_database_and_ignores_supplied_projection() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().canonicalize().unwrap();
        let local = root.join("drive_c");
        let external = root.join("drive_d");
        let home = local.join("Users/DJ/VirtualDJ");
        fs::create_dir_all(&home).unwrap();
        fs::create_dir_all(external.join("VirtualDJ")).unwrap();
        let database = home.join("database.xml");
        let external_database = external.join("VirtualDJ/database.xml");
        let initial=br#"<VirtualDJ_Database Version="8.5"><Song FilePath="C:\foreign.wav"/></VirtualDJ_Database>"#;
        let external_initial=br#"<VirtualDJ_Database Version="8.5"><Song FilePath="D:\foreign.wav"/></VirtualDJ_Database>"#;
        fs::write(&database, initial).unwrap();
        fs::write(&external_database, external_initial).unwrap();
        let source = external.join("one.wav");
        fs::write(&source, b"authored source").unwrap();
        let track = SyncTrack {
            native_path: None,
            id: "one".into(),
            path: source,
            ..Default::default()
        };
        let mut value = serde_json::to_value(&track).unwrap();
        value["native_path"] = serde_json::json!(r"C:\forged.wav");
        let input = SyncLibrary {
            tracks: vec![serde_json::from_value(value).unwrap()],
            ..Default::default()
        };
        let target:SyncTarget=serde_json::from_value(serde_json::json!({
            "software":"virtual_dj","path":database,"source_root":null,
            "environment":{"kind":"windows_compatibility","prefix":root,"installation":null,
              "mappings":[{"windows_root":"C:\\","host_root":local},{"windows_root":"D:\\","host_root":external}]}
        })).unwrap();
        let journals = root.join("journals");
        let preview = prepare_sync(&journals, target, &input, &Default::default()).unwrap();
        apply_sync(&journals, &preview.id).unwrap();
        assert_eq!(
            fs::read(&database).unwrap(),
            initial,
            "local drive remains untouched"
        );
        let xml = fs::read_to_string(&external_database).unwrap();
        assert!(xml.contains(r"D:\one.wav"));
        assert!(xml.contains(r"D:\foreign.wav"));
        assert!(!xml.contains("forged"));
        assert!(!xml.contains(root.to_str().unwrap()));
        restore_sync(&journals, &preview.id).unwrap();
        assert_eq!(fs::read(&external_database).unwrap(), external_initial);
    }
    #[test]
    fn windows_context_location_rules_map_or_materialize_captured_membership_without_host_paths() {
        use cuerithm_smartlists::{Operator, RuleValue, SmartDefinition, SmartRule};
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().canonicalize().unwrap();
        let home = root.join("VirtualDJ");
        fs::create_dir_all(home.join("MyLists")).unwrap();
        let database = home.join("database.xml");
        fs::write(&database, br#"<VirtualDJ_Database Version="8.5"/>"#).unwrap();
        let source = root.join("one.wav");
        fs::write(&source, b"authored source").unwrap();
        let playlists = [
            ("mapped", Operator::StringEquals),
            ("fallback", Operator::StringContains),
        ]
        .into_iter()
        .map(|(id, operator)| crate::SyncPlaylist {
            id: id.into(),
            name: id.into(),
            track_ids: vec!["one".into()],
            smart: Some(SmartDefinition {
                match_all: true,
                rules: vec![SmartRule {
                    id: "location".into(),
                    field: "location".into(),
                    operator,
                    value: RuleValue::Text(source.to_string_lossy().into_owned()),
                    or: false,
                }],
            }),
            ..Default::default()
        })
        .collect();
        let input = SyncLibrary {
            tracks: vec![SyncTrack {
                id: "one".into(),
                path: source,
                ..Default::default()
            }],
            playlists,
        };
        let target:SyncTarget=serde_json::from_value(serde_json::json!({"software":"virtual_dj","path":database,"source_root":null,
            "environment":{"kind":"windows_compatibility","prefix":root,"installation":null,"mappings":[{"windows_root":"C:\\","host_root":root}]}})).unwrap();
        let options = SyncOptions {
            smart_context: Some(crate::SmartExportContext {
                time_zone: "UTC".into(),
                native_time_zone: "UTC".into(),
                key_mixing_mode: cuerithm_smartlists::KeyMixingMode::Harmonic,
            }),
            ..Default::default()
        };
        let journals = root.join("journals");
        let preview = prepare_sync(&journals, target, &input, &options).unwrap();
        assert!(
            preview
                .warnings
                .iter()
                .any(|message| message.contains("fallback") && message.contains("saved tracks"))
        );
        apply_sync(&journals, &preview.id).unwrap();
        let mut filters = 0;
        let mut lists = 0;
        for file in &preview.files {
            if file
                .target
                .extension()
                .is_some_and(|extension| extension == "vdjfolder")
            {
                let xml = fs::read_to_string(&file.target).unwrap();
                assert!(!xml.contains(root.to_str().unwrap()));
                assert!(xml.contains(r"C:\one.wav"));
                if xml.contains("FilterFolder") {
                    filters += 1;
                } else {
                    assert!(xml.contains("VirtualFolder"));
                    assert!(xml.contains("<song "));
                    lists += 1;
                }
            }
        }
        assert_eq!((filters, lists), (1, 1));
        restore_sync(&journals, &preview.id).unwrap();
    }
    #[test]
    fn windows_unc_volume_roots_handle_native_verbatim_paths() {
        for value in [
            r"\\server\share\Users\DJ\VirtualDJ\database.xml",
            r"\\?\UNC\server\share\Users\DJ\VirtualDJ\database.xml",
        ] {
            assert_eq!(
                windows_volume_root(Path::new(value)).as_deref(),
                Some("//server/share")
            );
        }
    }
    #[test]
    fn windows_context_location_prefix_does_not_include_native_foreign_sibling_mapping() {
        use cuerithm_smartlists::{Operator, RuleValue, SmartDefinition, SmartRule};
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().canonicalize().unwrap();
        let selected = root.join("selected");
        let other = root.join("other");
        fs::create_dir(&selected).unwrap();
        fs::create_dir(&other).unwrap();
        let home = root.join("VirtualDJ");
        fs::create_dir_all(home.join("MyLists")).unwrap();
        let database = home.join("database.xml");
        fs::write(&database,br#"<VirtualDJ_Database Version="8.5"><Song FilePath="C:\Music2\foreign.wav"/></VirtualDJ_Database>"#).unwrap();
        let source = selected.join("one.wav");
        fs::write(&source, b"authored source").unwrap();
        let input = SyncLibrary {
            tracks: vec![SyncTrack {
                id: "one".into(),
                path: source,
                ..Default::default()
            }],
            playlists: vec![crate::SyncPlaylist {
                id: "prefix".into(),
                name: "Prefix".into(),
                track_ids: vec!["one".into()],
                smart: Some(SmartDefinition {
                    match_all: true,
                    rules: vec![SmartRule {
                        id: "location".into(),
                        field: "location".into(),
                        operator: Operator::StringStartsWith,
                        value: RuleValue::Text(selected.to_string_lossy().into_owned()),
                        or: false,
                    }],
                }),
                ..Default::default()
            }],
        };
        let target:SyncTarget=serde_json::from_value(serde_json::json!({"software":"virtual_dj","path":database,"source_root":null,"environment":{"kind":"windows_compatibility","prefix":root,"installation":null,"mappings":[{"windows_root":"C:\\","host_root":root},{"windows_root":"C:\\Music","host_root":selected},{"windows_root":"C:\\Music2","host_root":other}]}})).unwrap();
        let journals = root.join("journals");
        let preview = prepare_sync(&journals, target, &input, &SyncOptions::default()).unwrap();
        assert!(
            preview
                .warnings
                .iter()
                .any(|warning| warning.contains("Prefix") && warning.contains("saved tracks"))
        );
        apply_sync(&journals, &preview.id).unwrap();
        let list = preview
            .files
            .iter()
            .find(|file| {
                file.target
                    .extension()
                    .is_some_and(|extension| extension == "vdjfolder")
            })
            .unwrap();
        let xml = fs::read_to_string(&list.target).unwrap();
        assert!(xml.contains("VirtualFolder"));
        assert!(!xml.contains("FilterFolder"));
        assert!(xml.contains(r"C:\Music\one.wav"));
        assert!(!xml.contains("foreign.wav"));
    }
    #[test]
    fn private_request_reopens_raw_sources_dependencies_and_ownership_paths() {
        #[cfg(unix)]
        let raw = {
            use std::os::unix::ffi::OsStringExt;
            PathBuf::from(std::ffi::OsString::from_vec(
                b"/music/raw-\xff.wav".to_vec(),
            ))
        };
        #[cfg(windows)]
        let raw = {
            use std::os::windows::ffi::OsStringExt;
            PathBuf::from(std::ffi::OsString::from_wide(&[0x43, 0x3a, 0x5c, 0xd800]))
        };
        let dir = tempfile::tempdir().unwrap();
        let transaction = FileTransaction::create(dir.path()).unwrap();
        let target = SyncTarget {
            environment: Default::default(),
            software: DjSoftware::EngineDj,
            path: raw.clone(),
            source_root: Some(raw.clone()),
        };
        let file = crate::transaction::FileChange {
            target: raw.clone(),
            before_sha256: None,
            after_sha256: Some("hash".into()),
            bytes: 1,
        };
        let directory = crate::transaction::DirectoryChange {
            target: raw.clone(),
            before_manifest: None,
            after_exists: true,
        };
        let rename = crate::transaction::CaseRenameChange {
            from: raw.clone(),
            to: raw.clone(),
            temporary: raw.clone(),
            before: crate::transaction::RenameBefore::File {
                sha256: "hash".into(),
            },
        };
        let journal = FileJournal {
            id: "fixture".into(),
            state: crate::transaction::JournalState::Prepared,
            changes: vec![file.clone()],
            directories: vec![directory.clone()],
            renames: vec![rename.clone()],
        };
        crate::transaction::durable_json(&transaction.path().join("raw-journal.json"), &journal)
            .unwrap();
        let restored: FileJournal = serde_json::from_reader(
            fs::File::open(transaction.path().join("raw-journal.json")).unwrap(),
        )
        .unwrap();
        assert_eq!(restored.changes[0].target, raw);
        assert_eq!(restored.directories[0].target, raw);
        assert_eq!(restored.renames[0].temporary, raw);
        let prepared = PreparedRequest {
            related_roots: vec![],
            target_root: None,
            selected_target: None,
            environment: None,
            target: target.clone(),
            sources: BTreeMap::from([(raw.clone(), Some("audio".into()))]),
            preconditions: BTreeMap::from([(raw.clone(), None)]),
            ownership: BTreeMap::from([(
                raw.clone(),
                (
                    Some(raw.clone()),
                    crate::ownership::PlaylistOwnershipReport::default(),
                ),
            )]),
            preview: SyncPreview {
                id: transaction.inspect().unwrap().id,
                target,
                tracks: 1,
                playlists: 0,
                bytes_to_write: 1,
                files: vec![file],
                warnings: vec![],
                directories: vec![directory],
                deleted_playlists: 0,
                retained_external_playlists: 0,
                renames: vec![rename],
            },
        };
        crate::transaction::durable_json(&transaction.path().join("request.json"), &prepared)
            .unwrap();
        let restored = request(&transaction).unwrap();
        assert_eq!(restored.sources, prepared.sources);
        assert_eq!(restored.preconditions, prepared.preconditions);
        assert_eq!(restored.target.path, raw);
        assert_eq!(restored.target.source_root.as_ref(), Some(&raw));
        assert_eq!(restored.ownership[&raw].0.as_ref(), Some(&raw));
        assert_eq!(restored.preview.files[0].target, raw);
    }
    #[test]
    fn engine_device_libraries_include_windows_drive_roots_without_copying_desktop_libraries() {
        for root in [
            r"E:\Engine Library",
            r"e:\ENGINE LIBRARY\",
            "E:/Engine Library",
            r"\\?\E:\Engine Library",
            "/Volumes/DJ USB/Engine Library",
            "/media/user/DJ USB/Engine Library",
            "/run/media/user/DJ USB/Engine Library",
            "/mnt/dj/Engine Library",
        ] {
            assert!(engine_device_library(Path::new(root)), "{root}");
        }
        for root in [
            r"C:\Users\DJ\Music\Engine Library",
            r"E:\Music\Engine Library",
            r"\\?\C:\Users\DJ\Music\Engine Library",
            "/Users/dj/Music/Engine Library",
            "/mnt-backup/Music/Engine Library",
            "E:Engine Library",
        ] {
            assert!(!engine_device_library(Path::new(root)), "{root}");
        }
    }
    #[test]
    fn cleanup_cannot_remove_assets_from_a_second_pioneer_catalog() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().canonicalize().unwrap();
        for folder in ["PIONEER", ".PIONEER"] {
            fs::create_dir_all(root.join(folder).join("rekordbox")).unwrap();
            fs::write(
                root.join(folder).join("rekordbox/export.pdb"),
                b"synthetic catalog identity",
            )
            .unwrap();
        }
        let inactive = root.join(".PIONEER/USBANLZ/ANLZ0001.DAT");
        let audio = root.join("Contents/inactive.wav");
        fs::create_dir_all(inactive.parent().unwrap()).unwrap();
        fs::create_dir_all(audio.parent().unwrap()).unwrap();
        fs::write(&inactive, b"inactive analysis").unwrap();
        fs::write(&audio, b"inactive catalog audio").unwrap();
        let error = usb_cleanup_candidates(
            &root,
            &[inactive.clone(), audio.clone()],
            &Default::default(),
            &Default::default(),
        )
        .unwrap_err();
        assert!(error.to_string().contains("two Pioneer library catalogs"));
        assert_eq!(fs::read(inactive).unwrap(), b"inactive analysis");
        assert_eq!(fs::read(audio).unwrap(), b"inactive catalog audio");
    }
    #[test]
    fn device_replacements_preserve_omitted_source_audio_even_without_cleanup() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().canonicalize().unwrap();
        let source = root.join("archived-export.wav");
        let staged = root.join("new-copy.wav");
        fs::write(&source, b"protected source").unwrap();
        fs::write(&staged, b"different prepared export").unwrap();
        let replacements = BTreeMap::from([(source.clone(), staged.clone())]);
        let protected = std::collections::BTreeSet::from([source.clone()]);
        for software in [DjSoftware::EngineDj, DjSoftware::PioneerUsb] {
            assert!(
                validate_device_source_replacements(software, &replacements, &protected)
                    .unwrap_err()
                    .to_string()
                    .contains("replace a library source")
            );
        }
        assert_eq!(fs::read(&source).unwrap(), b"protected source");
        fs::write(staged, b"protected source").unwrap();
        validate_device_source_replacements(DjSoftware::PioneerUsb, &replacements, &protected)
            .unwrap();
    }
    #[test]
    fn usb_cleanup_protects_known_sources_omitted_from_the_export_scope() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().canonicalize().unwrap();
        let drive = root.join("drive");
        fs::create_dir_all(drive.join("Contents/CueRithm")).unwrap();
        let protected = drive.join("Contents/CueRithm/archived-source.wav");
        let unused = drive.join("Contents/CueRithm/unused-copy.wav");
        fs::write(&protected, b"known source outside selected scope").unwrap();
        fs::write(&unused, b"unreferenced export copy").unwrap();
        let targets = vec![protected.clone(), unused.clone()];
        let candidates = usb_cleanup_candidates(
            &drive,
            &targets,
            &BTreeMap::new(),
            &std::collections::BTreeSet::from([protected.clone()]),
        )
        .unwrap();
        assert!(!candidates.contains(&protected));
        assert!(candidates.contains(&unused));
        let transaction = FileTransaction::create(&root.join("journals")).unwrap();
        let files: Vec<_> = candidates
            .into_iter()
            .map(|path| crate::transaction::FilePublication {
                before_sha256: file_hash(&path).unwrap(),
                target: path,
                staged: None,
            })
            .collect();
        transaction.add_batch(&files, &[]).unwrap();
        transaction.apply(|| Ok(())).unwrap();
        assert!(protected.exists());
        assert!(!unused.exists());
        transaction.restore(|| Ok(())).unwrap();
        assert!(protected.exists());
        assert!(unused.exists());
    }
    #[test]
    fn real_preview_stages_xml_before_apply_and_restores_all_created_assets() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().canonicalize().unwrap();
        let home = root.join("VirtualDJ");
        fs::create_dir(&home).unwrap();
        let database = home.join("database.xml");
        let initial = b"<VirtualDJ_Database Version=\"8.5\"/>";
        fs::write(&database, initial).unwrap();
        let audio = root.join("sample.wav");
        fs::write(
            &audio,
            b"synthetic audio placeholder; XML writer does not decode",
        )
        .unwrap();
        let library = SyncLibrary {
            tracks: vec![SyncTrack {
                native_path: None,
                id: "t".into(),
                path: audio,
                duration_seconds: 60.,
                bpm: 120.,
                title: "Ready".into(),
                ..Default::default()
            }],
            ..Default::default()
        };
        let journal_root = root.join("sync");
        let preview = prepare_sync(
            &journal_root,
            SyncTarget {
                environment: Default::default(),
                software: DjSoftware::VirtualDj,
                path: database.clone(),
                source_root: None,
            },
            &library,
            &SyncOptions::default(),
        )
        .unwrap();
        assert_eq!(fs::read(&database).unwrap(), initial);
        assert_eq!(preview.tracks, 1);
        assert_eq!(preview.files.len(), 1);
        let transaction = FileTransaction::open(&journal_root, &preview.id).unwrap();
        assert!(!fs::read_dir(transaction.path()).unwrap().any(|entry| {
            entry
                .unwrap()
                .file_name()
                .to_string_lossy()
                .starts_with("build")
        }));
        assert!(transaction.path().join("request.json").is_file());
        assert!(transaction.path().join("staged/0").is_file());
        apply_sync(&journal_root, &preview.id).unwrap();
        assert!(fs::read_to_string(&database).unwrap().contains("Ready"));
        restore_sync(&journal_root, &preview.id).unwrap();
        assert_eq!(fs::read(&database).unwrap(), initial);
    }

    #[test]
    fn preparation_failure_removes_build_copies_but_keeps_journal_evidence() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().canonicalize().unwrap();
        let home = root.join("VirtualDJ");
        let resources = home.join("MyLists/CueRithm.subfolders");
        fs::create_dir_all(&resources).unwrap();
        let resource = resources.join("readonly-resource.txt");
        fs::write(&resource, b"original read-only resource").unwrap();
        let mut permissions = fs::metadata(&resource).unwrap().permissions();
        permissions.set_readonly(true);
        fs::set_permissions(&resource, permissions).unwrap();
        let database = home.join("database.xml");
        fs::write(&database, b"<VirtualDJ_Database><unclosed>").unwrap();
        let journal_root = root.join("journals");
        let result = prepare_sync(
            &journal_root,
            SyncTarget {
                environment: Default::default(),
                software: DjSoftware::VirtualDj,
                path: database.clone(),
                source_root: None,
            },
            &SyncLibrary::default(),
            &SyncOptions::default(),
        );
        let read_only = fs::metadata(&resource).unwrap().permissions().readonly();
        let mut permissions = fs::metadata(&resource).unwrap().permissions();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            permissions.set_mode(permissions.mode() | 0o200);
        }
        #[cfg(windows)]
        {
            // Windows fixture attribute; Unix uses owner-write above.
            #[allow(clippy::permissions_set_readonly_false)]
            permissions.set_readonly(false);
        }
        fs::set_permissions(&resource, permissions).unwrap();
        assert!(result.is_err());
        assert!(read_only, "Cleanup must not change source permissions");
        assert_eq!(fs::read(&resource).unwrap(), b"original read-only resource");
        let transactions = fs::read_dir(&journal_root)
            .unwrap()
            .collect::<std::io::Result<Vec<_>>>()
            .unwrap();
        assert_eq!(transactions.len(), 1);
        let transaction = &transactions[0].path();
        assert!(transaction.join("journal.json").is_file());
        assert!(transaction.join("staged").is_dir());
        assert!(transaction.join("backup").is_dir());
        assert!(!fs::read_dir(transaction).unwrap().any(|entry| {
            entry
                .unwrap()
                .file_name()
                .to_string_lossy()
                .starts_with("build")
        }));
        assert_eq!(
            fs::read(database).unwrap(),
            b"<VirtualDJ_Database><unclosed>"
        );
    }

    #[test]
    fn sqlite_wal_created_after_preview_blocks_publication_without_deleting_it() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().canonicalize().unwrap();
        let database = root.join("library.db");
        let source = rusqlite::Connection::open(&database).unwrap();
        source
            .execute_batch("CREATE TABLE example(value INTEGER); INSERT INTO example VALUES(1)")
            .unwrap();
        drop(source);
        let journal_root = root.join("journals");
        let transaction = FileTransaction::create(&journal_root).unwrap();
        let staged = transaction.path().join("snapshot.db");
        let mut before = BTreeMap::new();
        sqlite_snapshot(&database, &staged, &mut before).unwrap();
        let wal = root.join("library.db-wal");
        assert_eq!(
            before.get(&wal),
            Some(&None),
            "A missing WAL is still a read dependency"
        );
        let staged_db = rusqlite::Connection::open(&staged).unwrap();
        staged_db.execute("UPDATE example SET value=2", []).unwrap();
        drop(staged_db);
        transaction
            .add(&database, before[&database].clone(), Some(&staged))
            .unwrap();
        let journal = transaction.inspect().unwrap();
        let target = SyncTarget {
            environment: Default::default(),
            software: DjSoftware::EngineDj,
            path: database.clone(),
            source_root: None,
        };
        let preview = SyncPreview {
            id: journal.id.clone(),
            target: target.clone(),
            tracks: 0,
            playlists: 0,
            bytes_to_write: journal.changes.iter().map(|c| c.bytes).sum(),
            files: journal.changes,
            warnings: vec![],
            directories: vec![],
            deleted_playlists: 0,
            retained_external_playlists: 0,
            renames: vec![],
        };
        crate::transaction::durable_json(
            &transaction.path().join("request.json"),
            &PreparedRequest {
                related_roots: vec![],
                target_root: None,
                selected_target: None,
                environment: None,
                target,
                sources: BTreeMap::new(),
                preview,
                preconditions: before,
                ownership: BTreeMap::new(),
            },
        )
        .unwrap();
        let original = fs::read(&database).unwrap();
        fs::write(&wal, b"newly created WAL from another writer").unwrap();
        let error = apply_sync(&journal_root, &journal.id).unwrap_err();
        assert!(
            error.to_string().contains("dependencies changed"),
            "{error:#}"
        );
        assert_eq!(fs::read(&database).unwrap(), original);
        assert_eq!(
            fs::read(&wal).unwrap(),
            b"newly created WAL from another writer"
        );
        assert_eq!(
            transaction.inspect().unwrap().state,
            crate::transaction::JournalState::Prepared
        );
    }
}
