//! Explicit physical-host and Windows destination path context.
use anyhow::{Context, Result, bail, ensure};
use serde::{Deserialize, Serialize};
use std::{
    cell::{Cell, RefCell},
    collections::{HashMap, HashSet},
    ffi::OsString,
    fs,
    path::{Component, Path, PathBuf},
    time::SystemTime,
};

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum TargetEnvironment {
    #[default]
    Native,
    WindowsCompatibility {
        prefix: PathBuf,
        installation: Option<PathBuf>,
        mappings: Vec<DriveMapping>,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DriveMapping {
    pub windows_root: String,
    pub host_root: PathBuf,
}

/// Preview evidence is distinct from the environment used for stable library identity.
/// Retain selected paths as well as physical identities to detect link retargets.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TargetEnvironmentSnapshot {
    pub environment: TargetEnvironment,
    roots: Vec<RootObservation>,
    installation: Vec<InstallationObservation>,
}

/// Selected library-directory identity, independent of database file replacement.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TargetPathSnapshot {
    root: RootObservation,
}

impl TargetPathSnapshot {
    pub fn capture(path: &Path) -> Result<Self> {
        directory_root(path)?;
        let root = observe_root(path)?;
        ensure!(
            fs::metadata(&root.canonical)?.is_dir(),
            "Select a target directory"
        );
        Ok(Self { root })
    }

    pub fn verify(&self) -> Result<()> {
        ensure!(
            Self::capture(&self.root.selected)? == *self,
            "Selected target directory changed since preview"
        );
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
struct RootObservation {
    #[serde(with = "crate::path_serde")]
    selected: PathBuf,
    #[serde(with = "crate::path_serde")]
    canonical: PathBuf,
    identity: PhysicalIdentity,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
struct PhysicalIdentity {
    volume: u64,
    file: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
struct InstallationObservation {
    root: RootObservation,
    sha256: Option<String>,
}

/// Resolve paths within one preparation. Verify before accepting the result.
/// The cache belongs to this resolver alone; create a new resolver per operation.
/// Directory changes invalidate the operation rather than refreshing earlier paths.
pub struct TargetPathResolver<'a> {
    environment: &'a TargetEnvironment,
    roots: Vec<TargetPathSnapshot>,
    directories: RefCell<HashMap<PathBuf, DirectoryCaseIndex>>,
    directory_enumerations: Cell<usize>,
    invalidated: Cell<bool>,
}

#[derive(Debug, PartialEq, Eq)]
struct DirectoryStamp {
    identity: PhysicalIdentity,
    modified: SystemTime,
    length: u64,
    #[cfg(unix)]
    changed: (i64, i64),
}

struct DirectoryCaseIndex {
    stamp: DirectoryStamp,
    names: HashSet<OsString>,
    // None records a collision. Never pick an arbitrary one of these names.
    folded: HashMap<String, Option<OsString>>,
}

impl DirectoryStamp {
    fn capture(path: &Path) -> Result<Self> {
        let metadata = fs::metadata(path).context("Inspect mapped directory identity")?;
        ensure!(
            metadata.is_dir(),
            "Mapped reference parent is not a directory"
        );
        #[cfg(unix)]
        use std::os::unix::fs::MetadataExt;
        Ok(Self {
            identity: physical_identity(path)?,
            modified: metadata
                .modified()
                .context("Inspect mapped directory modification time")?,
            length: metadata.len(),
            #[cfg(unix)]
            changed: (metadata.ctime(), metadata.ctime_nsec()),
        })
    }
}

impl DirectoryCaseIndex {
    fn capture(path: &Path, enumerations: &Cell<usize>) -> Result<Self> {
        let stamp = DirectoryStamp::capture(path)?;
        let mut names = HashSet::new();
        let mut folded = HashMap::new();
        enumerations.set(enumerations.get() + 1);
        for entry in read_mapped_directory(path).context("Inspect mapped directory")? {
            let name = entry.context("Read mapped directory entry")?.file_name();
            if let Some(text) = name.to_str() {
                match folded.entry(fold(text)) {
                    std::collections::hash_map::Entry::Vacant(entry) => {
                        entry.insert(Some(name.clone()));
                    }
                    std::collections::hash_map::Entry::Occupied(mut entry) => {
                        entry.insert(None);
                    }
                }
            }
            names.insert(name);
        }
        ensure!(
            stamp == DirectoryStamp::capture(path)?,
            "Mapped directory changed during path preparation; preview again"
        );
        Ok(Self {
            stamp,
            names,
            folded,
        })
    }
}

impl TargetPathResolver<'_> {
    pub fn is_native(&self) -> bool {
        self.environment.is_native()
    }

    /// Number of directory index scans attempted by this operation, including
    /// final verification. Contains no paths and is never serialized.
    #[doc(hidden)]
    pub fn directory_enumerations(&self) -> usize {
        self.directory_enumerations.get()
    }

    fn observation<T>(&self, observed: Result<T>) -> Result<T> {
        if observed.is_err() {
            self.invalidated.set(true);
        }
        observed
    }

    fn require_unchanged(&self, unchanged: bool) -> Result<()> {
        if !unchanged {
            self.invalidated.set(true);
            bail!("Mapped directory changed during path preparation; preview again");
        }
        Ok(())
    }

    fn verify_roots(&self) -> Result<()> {
        ensure!(
            !self.invalidated.get(),
            "Path preparation was invalidated; preview again"
        );
        for root in &self.roots {
            self.observation(root.verify())?;
        }
        Ok(())
    }

    /// Re-enumerate each indexed directory once, including raw names and coarse
    /// timestamp filesystems. Call after all projected paths have been prepared.
    pub fn verify(&self) -> Result<()> {
        self.observation(self.environment.validate())?;
        self.verify_roots()?;
        for (path, expected) in self.directories.borrow().iter() {
            let observed = self.observation(DirectoryCaseIndex::capture(
                path,
                &self.directory_enumerations,
            ))?;
            self.require_unchanged(
                expected.names == observed.names && expected.stamp == observed.stamp,
            )?;
        }
        Ok(())
    }

    fn host_component(&self, directory: &Path, component: &str) -> Result<OsString> {
        let canonical = match directory.canonicalize() {
            Ok(path) => path,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Ok(component.into());
            }
            Err(error) => return Err(error).context("Resolve mapped directory"),
        };
        let mut directories = self.directories.borrow_mut();
        let index = match directories.entry(canonical.clone()) {
            std::collections::hash_map::Entry::Occupied(entry) => entry.into_mut(),
            std::collections::hash_map::Entry::Vacant(entry) => entry.insert(self.observation(
                DirectoryCaseIndex::capture(&canonical, &self.directory_enumerations),
            )?),
        };
        let observed = self.observation(DirectoryStamp::capture(&canonical))?;
        self.require_unchanged(index.stamp == observed)?;
        match index.folded.get(&fold(component)) {
            Some(Some(name)) => Ok(name.clone()),
            Some(None) => bail!("Case-colliding host entries cannot share a Windows reference"),
            None => Ok(component.into()),
        }
    }

    fn resolve_under(&self, root: &Path, parts: &[String]) -> Result<PathBuf> {
        let canonical_root = directory_root(root)?;
        let mut result = root.to_owned();
        for part in parts {
            validate_windows_component(part)?;
            let name = self.host_component(&result, part)?;
            result.push(name);
            match fs::symlink_metadata(&result) {
                Ok(_) => ensure!(
                    result.canonicalize()?.starts_with(&canonical_root),
                    "Mapped reference escapes its physical root"
                ),
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => return Err(error).context("Inspect mapped reference"),
            }
        }
        Ok(result)
    }
}

impl TargetPathResolver<'_> {
    /// Project a physical source into the selected destination's path namespace.
    /// No source existence requirement is imposed on Native references.
    pub fn to_native(&self, path: &Path) -> Result<PathBuf> {
        let TargetEnvironment::WindowsCompatibility { mappings, .. } = self.environment else {
            return Ok(path.to_owned());
        };
        self.verify_roots()?;
        validate_host_absolute(path)?;
        path_text(path)?;
        // Select by physical ancestry, never by an alias's selected spelling:
        // both spellings of the same source must choose the same nested root.
        let physical_source = physical_path(path)?;
        let mut candidates = Vec::new();
        for mapping in mappings {
            let canonical = directory_root(&mapping.host_root)?;
            if let Ok(relative) = physical_source.strip_prefix(&canonical) {
                let parts = host_components(relative)?;
                candidates.push((canonical.components().count(), mapping, parts));
            }
        }
        candidates.sort_by_key(|(length, _, _)| std::cmp::Reverse(*length));
        let Some((length, mapping, parts)) = candidates.first() else {
            bail!("Source has no selected Windows drive mapping");
        };
        ensure!(
            candidates
                .iter()
                .filter(|(other, _, _)| other == length)
                .count()
                == 1,
            "Ambiguous reverse Windows mapping"
        );
        let source = self.resolve_under(&mapping.host_root, parts)?;
        // A spelling which resolves to a different host entry must never be published.
        ensure!(
            same_host_reference(path, &source)?,
            "Source spelling resolves to a different host file"
        );
        let mut native = WindowsPath::parse(&mapping.windows_root)?;
        // Retain the host's actual spelling, including case and Unicode.
        native
            .parts
            .extend(host_components(source.strip_prefix(&mapping.host_root)?)?);
        let projected = native.render();
        ensure!(
            self.resolve_windows(&native)? == source,
            "Mapping overlap changes the projected source"
        );
        Ok(PathBuf::from(projected))
    }

    pub fn to_host(&self, path: &Path) -> Result<PathBuf> {
        if self.is_native() {
            return Ok(path.to_owned());
        }
        self.verify_roots()?;
        let native = WindowsPath::parse(path_text(path)?)?;
        let host = self.resolve_windows(&native)?;
        let reverse = WindowsPath::parse(path_text(&self.to_native(&host)?)?)?;
        ensure!(
            reverse.folded() == native.folded(),
            "Windows reference has an ambiguous reverse alias"
        );
        Ok(host)
    }

    fn resolve_windows(&self, path: &WindowsPath) -> Result<PathBuf> {
        let TargetEnvironment::WindowsCompatibility { mappings, .. } = self.environment else {
            unreachable!()
        };
        let mut candidates = Vec::new();
        for mapping in mappings {
            let root = WindowsPath::parse(&mapping.windows_root)?;
            if root.contains(path) {
                candidates.push((root.parts.len(), mapping));
            }
        }
        candidates.sort_by_key(|(length, _)| std::cmp::Reverse(*length));
        let Some((length, mapping)) = candidates.first() else {
            bail!("Windows reference has no selected drive mapping");
        };
        ensure!(
            candidates
                .iter()
                .filter(|(other, _)| other == length)
                .count()
                == 1,
            "Ambiguous Windows drive mapping"
        );
        self.resolve_under(&mapping.host_root, &path.parts[*length..])
    }
}

impl TargetEnvironment {
    pub fn resolver(&self) -> Result<TargetPathResolver<'_>> {
        self.validate()?;
        let mut roots = Vec::new();
        if let Self::WindowsCompatibility {
            prefix, mappings, ..
        } = self
        {
            roots.push(TargetPathSnapshot::capture(prefix)?);
            for mapping in mappings {
                roots.push(TargetPathSnapshot::capture(&mapping.host_root)?);
            }
        }
        Ok(TargetPathResolver {
            environment: self,
            roots,
            directories: RefCell::new(HashMap::new()),
            directory_enumerations: Cell::new(0),
            invalidated: Cell::new(false),
        })
    }
    pub fn is_native(&self) -> bool {
        matches!(self, Self::Native)
    }

    pub fn validate(&self) -> Result<()> {
        if let Self::WindowsCompatibility {
            prefix,
            installation,
            mappings,
        } = self
        {
            path_text(prefix)?;
            directory_root(prefix)?;
            ensure!(
                !mappings.is_empty(),
                "Choose explicit Windows drive mappings"
            );
            let mut windows = Vec::new();
            let mut hosts: Vec<PathBuf> = Vec::new();
            for mapping in mappings {
                let native = WindowsPath::parse(&mapping.windows_root)?;
                path_text(&mapping.host_root)?;
                let physical = directory_root(&mapping.host_root)?;
                ensure!(
                    !windows.contains(&native.folded()),
                    "Duplicate Windows mapping root"
                );
                for previous in &hosts {
                    ensure!(
                        previous != &physical
                            && fold(path_text(previous)?) != fold(path_text(&physical)?),
                        "Ambiguous physical mapping roots"
                    );
                }
                // A selected spelling can itself collide even when the physical roots differ.
                for previous in mappings.iter().take(windows.len()) {
                    ensure!(
                        fold(path_text(&previous.host_root)?)
                            != fold(path_text(&mapping.host_root)?),
                        "Ambiguous selected mapping roots"
                    );
                }
                windows.push(native.folded());
                hosts.push(physical);
            }
            if let Some(path) = installation {
                path_text(path)?;
                validate_host_absolute(path)?;
                let metadata =
                    fs::metadata(path).context("Selected installation is unavailable")?;
                ensure!(
                    metadata.is_file() || metadata.is_dir(),
                    "Select an installation file or directory"
                );
            }
        }
        Ok(())
    }

    /// Resolve one path without retaining a cache across operations.
    /// Use `resolver()` for a collection of paths within one preparation.
    pub fn to_native(&self, path: &Path) -> Result<PathBuf> {
        let resolver = self.resolver()?;
        let projected = resolver.to_native(path)?;
        resolver.verify()?;
        Ok(projected)
    }

    pub fn to_host(&self, path: &Path) -> Result<PathBuf> {
        let resolver = self.resolver()?;
        let host = resolver.to_host(path)?;
        resolver.verify()?;
        Ok(host)
    }

    pub fn freeze(&self) -> Result<TargetEnvironmentSnapshot> {
        self.validate()?;
        let mut roots = Vec::new();
        let mut installation_evidence = Vec::new();
        if let Self::WindowsCompatibility {
            prefix,
            installation,
            mappings,
        } = self
        {
            roots.push(observe_root(prefix)?);
            for mapping in mappings {
                roots.push(observe_root(&mapping.host_root)?);
            }
            if let Some(path) = installation {
                installation_evidence = installation_observations(path)?;
            }
        }
        Ok(TargetEnvironmentSnapshot {
            environment: self.clone(),
            roots,
            installation: installation_evidence,
        })
    }
}

impl TargetEnvironmentSnapshot {
    pub fn verify(&self) -> Result<()> {
        ensure!(
            self.environment.freeze()? == *self,
            "Target environment, mapped root, or installation changed since preview"
        );
        Ok(())
    }
}

#[derive(Clone, Debug)]
struct WindowsPath {
    volume: String,
    parts: Vec<String>,
}
impl WindowsPath {
    fn parse(text: &str) -> Result<Self> {
        ensure!(
            text.encode_utf16().count() <= 32767,
            "Windows reference is too long"
        );
        let text = text.replace('/', "\\");
        let (volume, tail) = if let Some(unc) = text.strip_prefix("\\\\") {
            let mut pieces = unc.splitn(3, '\\');
            let server = pieces.next().unwrap_or_default();
            let share = pieces.next().unwrap_or_default();
            validate_windows_component(server)?;
            validate_windows_component(share)?;
            (
                format!("\\\\{server}\\{share}"),
                pieces.next().unwrap_or_default(),
            )
        } else {
            let bytes = text.as_bytes();
            ensure!(
                bytes.len() >= 3
                    && bytes[0].is_ascii_alphabetic()
                    && bytes[1] == b':'
                    && bytes[2] == b'\\',
                "Choose an absolute Windows drive or UNC reference"
            );
            (
                format!("{}:", (bytes[0] as char).to_ascii_uppercase()),
                &text[3..],
            )
        };
        let mut parts = Vec::new();
        let tail = tail.strip_suffix('\\').unwrap_or(tail);
        if !tail.is_empty() {
            for part in tail.split('\\') {
                validate_windows_component(part)?;
                parts.push(part.to_owned());
            }
        }
        Ok(Self { volume, parts })
    }
    fn folded(&self) -> String {
        fold(&self.render())
    }
    fn render(&self) -> String {
        format!("{}\\{}", self.volume, self.parts.join("\\"))
    }
    fn contains(&self, other: &Self) -> bool {
        fold(&self.volume) == fold(&other.volume)
            && self.parts.len() <= other.parts.len()
            && self
                .parts
                .iter()
                .zip(&other.parts)
                .all(|(a, b)| fold(a) == fold(b))
    }
}

// Use single-character uppercase comparisons. Expanding folds (e.g. sharp-s to
// SS) would incorrectly merge distinct Windows names. Conservative collisions
// still fail closed rather than selecting an arbitrary source.
fn fold(text: &str) -> String {
    text.chars()
        .map(|character| {
            let mut upper = character.to_uppercase();
            let first = upper.next().unwrap_or(character);
            if upper.next().is_none() {
                first
            } else {
                character
            }
        })
        .collect()
}

fn validate_windows_component(part: &str) -> Result<()> {
    ensure!(
        !part.is_empty() && part != "." && part != ".." && part.encode_utf16().count() <= 255,
        "Invalid Windows path component"
    );
    ensure!(
        !part.ends_with(['.', ' ']) && !part.chars().any(|c| c < ' ' || "<>:\"/\\|?*".contains(c)),
        "Unrepresentable Windows path component"
    );
    let stem = fold(
        part.split('.')
            .next()
            .unwrap_or_default()
            .trim_end_matches(' '),
    );
    ensure!(
        !matches!(
            stem.as_str(),
            "CON" | "PRN" | "AUX" | "NUL" | "CONIN$" | "CONOUT$"
        ) && !["COM", "LPT"]
            .iter()
            .any(
                |prefix| stem.strip_prefix(prefix).is_some_and(|suffix| matches!(
                    suffix,
                    "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9" | "¹" | "²" | "³"
                ))
            ),
        "Reserved Windows path component"
    );
    Ok(())
}
fn path_text(path: &Path) -> Result<&str> {
    path.to_str()
        .context("Windows references require UTF-8 paths")
}
fn validate_host_absolute(path: &Path) -> Result<()> {
    ensure!(
        path.is_absolute() && !path.components().any(|p| matches!(p, Component::ParentDir)),
        "Choose an absolute host path without traversal"
    );
    Ok(())
}
fn host_components(path: &Path) -> Result<Vec<String>> {
    path.components()
        .map(|part| {
            let Component::Normal(part) = part else {
                bail!("Invalid host path traversal");
            };
            let text = part
                .to_str()
                .context("Windows references require UTF-8 paths")?;
            validate_windows_component(text)?;
            Ok(text.to_owned())
        })
        .collect()
}
fn directory_root(path: &Path) -> Result<PathBuf> {
    validate_host_absolute(path)?;
    ensure!(
        fs::metadata(path)
            .context("Selected target directory is unavailable")?
            .is_dir(),
        "Select a target directory"
    );
    path.canonicalize()
        .context("Resolve selected target directory")
}
fn read_mapped_directory(path: &Path) -> std::io::Result<fs::ReadDir> {
    #[cfg(test)]
    DIRECTORY_ENUMERATIONS.with(|count| count.set(count.get() + 1));
    fs::read_dir(path)
}

#[cfg(test)]
thread_local! {
    static DIRECTORY_ENUMERATIONS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}
fn same_host_reference(requested: &Path, resolved: &Path) -> Result<bool> {
    Ok(physical_path(requested)? == physical_path(resolved)?)
}

/// Resolve existing ancestors while retaining the suffix of an absent source.
/// A dangling link is an error, rather than an apparently absent directory.
fn physical_path(path: &Path) -> Result<PathBuf> {
    let mut ancestor = path;
    let mut suffix = Vec::new();
    loop {
        match fs::symlink_metadata(ancestor) {
            Ok(_) => {
                let mut physical = ancestor
                    .canonicalize()
                    .context("Resolve physical mapped source")?;
                for component in suffix.iter().rev() {
                    physical.push(component);
                }
                return Ok(physical);
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                suffix.push(
                    ancestor
                        .file_name()
                        .context("Resolve mapped source ancestor")?,
                );
                ancestor = ancestor
                    .parent()
                    .context("Resolve mapped source ancestor")?;
            }
            Err(error) => return Err(error).context("Inspect physical mapped source"),
        }
    }
}
fn observe_root(path: &Path) -> Result<RootObservation> {
    validate_host_absolute(path)?;
    let canonical = path
        .canonicalize()
        .context("Resolve selected target identity")?;
    Ok(RootObservation {
        selected: path.to_owned(),
        identity: physical_identity(&canonical)?,
        canonical,
    })
}
#[cfg(unix)]
fn physical_identity(path: &Path) -> Result<PhysicalIdentity> {
    use std::os::unix::fs::MetadataExt;
    let metadata = fs::metadata(path)?;
    Ok(PhysicalIdentity {
        volume: metadata.dev(),
        file: metadata.ino(),
    })
}
#[cfg(windows)]
fn physical_identity(path: &Path) -> Result<PhysicalIdentity> {
    use std::os::windows::{fs::OpenOptionsExt, io::AsRawHandle};
    use windows_sys::Win32::Storage::FileSystem::{
        BY_HANDLE_FILE_INFORMATION, FILE_FLAG_BACKUP_SEMANTICS, GetFileInformationByHandle,
    };
    let file = fs::OpenOptions::new()
        .read(true)
        .custom_flags(FILE_FLAG_BACKUP_SEMANTICS)
        .open(path)?;
    let mut information: BY_HANDLE_FILE_INFORMATION = unsafe { std::mem::zeroed() };
    ensure!(
        unsafe { GetFileInformationByHandle(file.as_raw_handle().cast(), &mut information) } != 0,
        "Read selected target identity"
    );
    Ok(PhysicalIdentity {
        volume: information.dwVolumeSerialNumber as u64,
        file: ((information.nFileIndexHigh as u64) << 32) | information.nFileIndexLow as u64,
    })
}

fn installation_observations(path: &Path) -> Result<Vec<InstallationObservation>> {
    let root = observe_root(path)?;
    if fs::metadata(path)?.is_file() {
        return Ok(vec![InstallationObservation {
            root,
            sha256: Some(cuerithm_media::file_sha256(path)?),
        }]);
    }
    ensure!(
        fs::metadata(path)?.is_dir(),
        "Select an installation file or directory"
    );
    let mut evidence = vec![InstallationObservation { root, sha256: None }];
    let canonical_root = path.canonicalize()?;
    let mut pending = vec![(path.to_owned(), 0)];
    let mut count = 0;
    while let Some((directory, depth)) = pending.pop() {
        ensure!(
            depth <= 16,
            "Installation directory exceeds inspection depth"
        );
        for entry in fs::read_dir(directory)? {
            let entry = entry?;
            count += 1;
            ensure!(
                count <= 100_000,
                "Installation directory exceeds inspection limit"
            );
            let kind = entry.file_type()?;
            ensure!(
                !(kind.is_symlink() && fs::metadata(entry.path())?.is_dir()),
                "Installation contains a symbolic subdirectory; select its executable file directly"
            );
            // Inspect only executable resources, never account/configuration files.
            if kind.is_dir() {
                pending.push((entry.path(), depth + 1));
            } else if entry
                .path()
                .extension()
                .is_some_and(|extension| extension.eq_ignore_ascii_case("exe"))
            {
                let observed = observe_root(&entry.path())?;
                ensure!(
                    observed.canonical.starts_with(&canonical_root),
                    "Installation executable escapes the selected directory"
                );
                evidence.push(InstallationObservation {
                    root: observed,
                    sha256: Some(cuerithm_media::file_sha256(&entry.path())?),
                });
            }
        }
    }
    evidence[1..].sort_by(|a, b| a.root.selected.cmp(&b.root.selected));
    Ok(evidence)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn operation_resolver_enumerates_flat_library_directories_a_bounded_number_of_times() {
        let root = tempfile::tempdir().unwrap();
        let music = root.path().join("Music");
        fs::create_dir(&music).unwrap();
        for index in 0..128 {
            fs::write(music.join(format!("track-{index:03}.wav")), b"authored").unwrap();
        }
        let env = environment(root.path(), &[("C:\\", root.path())]);
        DIRECTORY_ENUMERATIONS.with(|count| count.set(0));
        let resolver = env.resolver().unwrap();
        for index in 0..128 {
            let source = music.join(format!("track-{index:03}.wav"));
            let native = PathBuf::from(format!("C:\\Music\\track-{index:03}.wav"));
            assert_eq!(resolver.to_native(&source).unwrap(), native);
            assert_eq!(resolver.to_host(&native).unwrap(), source);
        }
        resolver.verify().unwrap();
        let enumerations = DIRECTORY_ENUMERATIONS.with(|count| count.get());
        assert_eq!(resolver.directory_enumerations(), enumerations);
        assert!(
            enumerations <= 4,
            "enumerated two directories {enumerations} times for 128 tracks"
        );
    }

    #[test]
    fn operation_resolver_refuses_directory_entry_changes_after_paths_were_resolved() {
        let root = tempfile::tempdir().unwrap();
        fs::write(root.path().join("Song.wav"), b"authored").unwrap();
        let env = environment(root.path(), &[("C:\\", root.path())]);
        let resolver = env.resolver().unwrap();
        resolver.to_host(Path::new("C:\\song.wav")).unwrap();
        fs::write(root.path().join("another.wav"), b"new entry").unwrap();
        assert!(resolver.verify().is_err());
        assert!(resolver.to_host(Path::new("C:\\song.wav")).is_err());
        let next_environment = env.clone();
        let fresh = next_environment.resolver().unwrap();
        fresh.to_host(Path::new("C:\\song.wav")).unwrap();
        fresh.verify().unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn operation_resolver_rechecks_directory_entries_when_modification_time_is_restored() {
        let root = tempfile::tempdir().unwrap();
        fs::write(root.path().join("Song.wav"), b"authored").unwrap();
        let modified = fs::metadata(root.path()).unwrap().modified().unwrap();
        let env = environment(root.path(), &[("C:\\", root.path())]);
        let resolver = env.resolver().unwrap();
        resolver.to_host(Path::new("C:\\Song.wav")).unwrap();
        fs::write(root.path().join("new.wav"), b"new entry").unwrap();
        fs::File::open(root.path())
            .unwrap()
            .set_times(fs::FileTimes::new().set_modified(modified))
            .unwrap();
        assert_eq!(
            fs::metadata(root.path()).unwrap().modified().unwrap(),
            modified
        );
        assert!(resolver.verify().is_err());
    }

    #[cfg(unix)]
    #[test]
    fn operation_resolver_rejects_selected_mapping_symlink_retargeting() {
        use std::os::unix::fs::symlink;
        let root = tempfile::tempdir().unwrap();
        let first = root.path().join("first");
        let second = root.path().join("second");
        fs::create_dir(&first).unwrap();
        fs::create_dir(&second).unwrap();
        fs::write(first.join("song.wav"), b"first").unwrap();
        fs::write(second.join("song.wav"), b"second").unwrap();
        let selected = root.path().join("selected");
        symlink(&first, &selected).unwrap();
        let env = environment(root.path(), &[("C:\\", &selected)]);
        let resolver = env.resolver().unwrap();
        resolver.to_host(Path::new("C:\\song.wav")).unwrap();
        fs::remove_file(&selected).unwrap();
        symlink(&second, &selected).unwrap();
        assert!(resolver.verify().is_err());
        assert!(resolver.to_host(Path::new("C:\\song.wav")).is_err());
        // Restoring the original physical root cannot revive an operation whose
        // observations already failed. A new operation captures fresh evidence.
        fs::remove_file(&selected).unwrap();
        symlink(&first, &selected).unwrap();
        assert!(resolver.verify().is_err());
        assert!(resolver.to_host(Path::new("C:\\song.wav")).is_err());
        let fresh = env.resolver().unwrap();
        fresh.to_host(Path::new("C:\\song.wav")).unwrap();
        fresh.verify().unwrap();
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn operation_resolver_refuses_new_case_collisions_and_fresh_resolvers_also_reject_them() {
        let root = tempfile::tempdir().unwrap();
        fs::write(root.path().join("Song.wav"), b"authored").unwrap();
        let env = environment(root.path(), &[("C:\\", root.path())]);
        let resolver = env.resolver().unwrap();
        resolver.to_native(&root.path().join("Song.wav")).unwrap();
        fs::write(root.path().join("song.wav"), b"different source").unwrap();
        assert!(resolver.verify().is_err());
        assert!(resolver.to_host(Path::new("C:\\Song.wav")).is_err());
        assert!(
            env.resolver()
                .unwrap()
                .to_host(Path::new("C:\\Song.wav"))
                .is_err()
        );
    }

    fn environment(root: &Path, mappings: &[(&str, &Path)]) -> TargetEnvironment {
        TargetEnvironment::WindowsCompatibility {
            prefix: root.to_owned(),
            installation: None,
            mappings: mappings
                .iter()
                .map(|(windows_root, host_root)| DriveMapping {
                    windows_root: (*windows_root).to_owned(),
                    host_root: (*host_root).to_owned(),
                })
                .collect(),
        }
    }

    #[test]
    fn native_identity_accepts_absent_paths_and_roundtrips_serialization() {
        let env = TargetEnvironment::default();
        let path = Path::new("absent/../native reference");
        assert!(env.is_native());
        env.validate().unwrap();
        assert_eq!(env.to_native(path).unwrap(), path);
        assert_eq!(env.to_host(path).unwrap(), path);
        assert_eq!(
            serde_json::to_value(&env).unwrap(),
            serde_json::json!({"kind":"native"})
        );
        let frozen: TargetEnvironmentSnapshot =
            serde_json::from_slice(&serde_json::to_vec(&env.freeze().unwrap()).unwrap()).unwrap();
        frozen.verify().unwrap();
    }

    #[test]
    fn drive_unc_unicode_and_case_references_resolve_to_physical_spelling() {
        let root = tempfile::tempdir().unwrap();
        let music = root.path().join("Music");
        fs::create_dir(&music).unwrap();
        let source = music.join("Été #100%.flac");
        fs::write(&source, b"authored").unwrap();
        for native in ["D:\\Music", "\\\\server\\share\\Music"] {
            let env = environment(root.path(), &[(native, &music)]);
            env.validate().unwrap();
            assert_eq!(
                env.to_native(&source).unwrap(),
                PathBuf::from(format!("{native}\\Été #100%.flac"))
            );
            assert_eq!(
                env.to_host(Path::new(&format!("{native}/été #100%.FLAC")))
                    .unwrap(),
                source
            );
        }
    }

    #[test]
    fn longest_component_root_wins_without_matching_sibling_names() {
        let root = tempfile::tempdir().unwrap();
        let broad = root.path().join("broad");
        let specific = root.path().join("specific");
        fs::create_dir(&broad).unwrap();
        fs::create_dir(&specific).unwrap();
        let env = environment(root.path(), &[("C:\\", &broad), ("C:\\Music", &specific)]);
        assert_eq!(
            env.to_host(Path::new("C:\\Music\\track.wav")).unwrap(),
            specific.join("track.wav")
        );
        assert_eq!(
            env.to_host(Path::new("C:\\Musical\\track.wav")).unwrap(),
            broad.join("Musical/track.wav")
        );
        assert_eq!(
            env.to_native(&specific.join("track.wav")).unwrap(),
            PathBuf::from("C:\\Music\\track.wav")
        );
        assert!(env.to_native(&broad.join("Music/track.wav")).is_err());
    }

    #[test]
    fn rejects_unresolved_drives_relative_traversal_devices_and_reserved_components() {
        let root = tempfile::tempdir().unwrap();
        let env = environment(root.path(), &[("C:\\", root.path())]);
        for path in [
            "D:\\song.wav",
            "C:song.wav",
            "song.wav",
            "C:\\..\\song",
            "C:\\.\\song",
            "C:\\A\\..\\song",
            "C:\\CON.wav",
            "C:\\aux",
            "C:\\LPT1.txt",
            "C:\\name.",
            "C:\\name ",
            "C:\\a:b",
            "C:\\a?b",
            "\\\\?\\C:\\song",
        ] {
            assert!(env.to_host(Path::new(path)).is_err(), "accepted {path}");
        }
        assert!(env.to_native(Path::new("relative.wav")).is_err());
        assert!(env.to_native(&root.path().join("../escape.wav")).is_err());
        assert!(env.to_native(&root.path().join("NUL.flac")).is_err());
    }

    #[test]
    fn rejects_duplicate_windows_roots_and_ambiguous_host_aliases() {
        let root = tempfile::tempdir().unwrap();
        let other = root.path().join("other");
        fs::create_dir(&other).unwrap();
        assert!(
            environment(
                root.path(),
                &[("C:\\Music", root.path()), ("c:/music", &other)]
            )
            .validate()
            .is_err()
        );
        assert!(
            environment(root.path(), &[("C:\\", root.path()), ("D:\\", root.path())])
                .validate()
                .is_err()
        );
        assert!(
            environment(root.path(), &[("C:\\", &root.path().join("missing"))])
                .validate()
                .is_err()
        );
        assert!(
            environment(Path::new("relative"), &[("C:\\", root.path())])
                .validate()
                .is_err()
        );
    }

    #[cfg(unix)]
    #[test]
    fn rejects_case_colliding_files_and_non_utf8_references() {
        use std::os::unix::ffi::OsStringExt;
        let root = tempfile::tempdir().unwrap();
        fs::write(root.path().join("Song.wav"), b"one").unwrap();
        fs::write(root.path().join("song.wav"), b"two").unwrap();
        let env = environment(root.path(), &[("C:\\", root.path())]);
        // The same fixture runs on case-sensitive Linux and case-insensitive
        // macOS/Windows volumes. Only distinct entries constitute a collision.
        if fs::read_dir(root.path()).unwrap().count() == 2 {
            assert!(env.to_native(&root.path().join("Song.wav")).is_err());
            assert!(env.to_host(Path::new("C:\\Song.wav")).is_err());
        } else {
            assert_eq!(
                fs::read(env.to_host(Path::new("C:\\SONG.wav")).unwrap()).unwrap(),
                b"two"
            );
        }
        assert!(
            env.to_native(&root.path().join(std::ffi::OsString::from_vec(vec![0xff])))
                .is_err()
        );
    }

    #[cfg(unix)]
    #[test]
    fn rejects_canonical_aliases_and_symlink_escape() {
        use std::os::unix::fs::symlink;
        let root = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        let alias = root.path().join("alias");
        symlink(outside.path(), &alias).unwrap();
        assert!(
            environment(root.path(), &[("C:\\", &alias), ("D:\\", outside.path())])
                .validate()
                .is_err()
        );
        let env = environment(root.path(), &[("C:\\", root.path())]);
        assert!(env.to_host(Path::new("C:\\alias\\file.wav")).is_err());
        assert!(env.to_native(&alias.join("file.wav")).is_err());
    }

    #[cfg(unix)]
    #[test]
    fn frozen_roots_reject_symlink_retarget_and_directory_replacement() {
        use std::os::unix::fs::symlink;
        let root = tempfile::tempdir().unwrap();
        let first = root.path().join("first");
        let second = root.path().join("second");
        fs::create_dir(&first).unwrap();
        fs::create_dir(&second).unwrap();
        let alias = root.path().join("selected");
        symlink(&first, &alias).unwrap();
        let env = environment(&alias, &[("C:\\", &alias)]);
        let frozen = env.freeze().unwrap();
        frozen.verify().unwrap();
        fs::remove_file(&alias).unwrap();
        symlink(&second, &alias).unwrap();
        assert!(frozen.verify().is_err());
        let frozen = environment(root.path(), &[("C:\\", &first)])
            .freeze()
            .unwrap();
        fs::rename(&first, root.path().join("retired")).unwrap();
        fs::create_dir(&first).unwrap();
        assert!(frozen.verify().is_err());
    }

    #[test]
    fn snapshot_ignores_music_changes_but_rejects_installation_content_updates() {
        let root = tempfile::tempdir().unwrap();
        let install = root.path().join("installation");
        fs::create_dir(&install).unwrap();
        let binary = install.join("rekordbox.exe");
        fs::write(&binary, b"first executable").unwrap();
        let env = TargetEnvironment::WindowsCompatibility {
            prefix: root.path().to_owned(),
            installation: Some(install),
            mappings: vec![DriveMapping {
                windows_root: "C:\\".into(),
                host_root: root.path().to_owned(),
            }],
        };
        let frozen = env.freeze().unwrap();
        fs::write(root.path().join("new music.wav"), b"authored").unwrap();
        frozen.verify().unwrap();
        fs::write(binary, b"other executable").unwrap();
        assert!(frozen.verify().is_err());
    }

    #[test]
    fn rejects_reverse_alias_from_nested_host_mapping() {
        let root = tempfile::tempdir().unwrap();
        let nested = root.path().join("nested");
        fs::create_dir(&nested).unwrap();
        let env = environment(root.path(), &[("C:\\", root.path()), ("D:\\", &nested)]);
        assert_eq!(
            env.to_host(Path::new("D:\\track.wav")).unwrap(),
            nested.join("track.wav")
        );
        assert!(env.to_host(Path::new("C:\\nested\\track.wav")).is_err());
    }

    #[test]
    fn rejects_reserved_stem_with_spaces_before_extension() {
        let root = tempfile::tempdir().unwrap();
        let env = environment(root.path(), &[("C:\\", root.path())]);
        assert!(env.to_host(Path::new("C:\\CON .wav")).is_err());
    }

    #[cfg(unix)]
    #[test]
    fn physical_longest_root_wins_for_selected_aliases_and_absent_descendants() {
        use std::os::unix::fs::symlink;
        let root = tempfile::tempdir().unwrap();
        let physical = root.path().join("physical");
        let nested = physical.join("nested");
        fs::create_dir_all(&nested).unwrap();
        fs::write(nested.join("song.wav"), b"authored").unwrap();
        let selected = root.path().join("selected");
        symlink(&physical, &selected).unwrap();
        let env = environment(root.path(), &[("C:\\", &selected), ("D:\\", &nested)]);
        for relative in ["nested/song.wav", "nested/absent/deeper/song.wav"] {
            let expected = if relative == "nested/song.wav" {
                "D:\\song.wav"
            } else {
                "D:\\absent\\deeper\\song.wav"
            };
            assert_eq!(
                env.to_native(&physical.join(relative)).unwrap(),
                PathBuf::from(expected)
            );
            assert_eq!(
                env.to_native(&selected.join(relative)).unwrap(),
                PathBuf::from(expected)
            );
        }
        assert!(env.to_host(Path::new("C:\\nested\\song.wav")).is_err());
        assert_eq!(
            env.to_host(Path::new("D:\\song.wav")).unwrap(),
            nested.join("song.wav")
        );
    }

    #[cfg(unix)]
    #[test]
    fn installation_subdirectory_symlinks_are_refused_instead_of_omitting_executables() {
        use std::os::unix::fs::symlink;
        let root = tempfile::tempdir().unwrap();
        let installation = root.path().join("installation");
        let actual = installation.join("actual-bin");
        fs::create_dir_all(&actual).unwrap();
        fs::write(actual.join("app.exe"), b"authored executable").unwrap();
        symlink(&actual, installation.join("bin")).unwrap();
        let mut env = environment(root.path(), &[("C:\\", root.path())]);
        if let TargetEnvironment::WindowsCompatibility {
            installation: selected,
            ..
        } = &mut env
        {
            *selected = Some(installation);
        }
        assert!(env.freeze().is_err());
    }

    #[test]
    fn target_directory_snapshot_allows_database_replacement_and_survives_serialization() {
        let root = tempfile::tempdir().unwrap();
        let database = root.path().join("database.sqlite");
        fs::write(&database, b"first database").unwrap();
        let snapshot = TargetPathSnapshot::capture(root.path()).unwrap();
        let json = serde_json::to_value(&snapshot).unwrap();
        assert_eq!(json["root"]["selected"], root.path().to_str().unwrap());
        assert_eq!(
            json["root"]["canonical"],
            root.path().canonicalize().unwrap().to_str().unwrap()
        );
        let snapshot: TargetPathSnapshot =
            serde_json::from_slice(&serde_json::to_vec(&snapshot).unwrap()).unwrap();
        fs::rename(&database, root.path().join("retired.sqlite")).unwrap();
        fs::write(&database, b"replacement database").unwrap();
        snapshot.verify().unwrap();
        assert!(TargetPathSnapshot::capture(&database).is_err());
        assert!(TargetPathSnapshot::capture(Path::new("relative")).is_err());
    }

    #[cfg(unix)]
    #[test]
    fn target_directory_snapshot_rejects_selected_symlink_retarget_and_same_path_replacement() {
        use std::os::unix::fs::symlink;
        let root = tempfile::tempdir().unwrap();
        let first = root.path().join("first");
        let second = root.path().join("second");
        fs::create_dir(&first).unwrap();
        fs::create_dir(&second).unwrap();
        let selected = root.path().join("selected");
        symlink(&first, &selected).unwrap();
        let snapshot = TargetPathSnapshot::capture(&selected).unwrap();
        snapshot.verify().unwrap();
        fs::remove_file(&selected).unwrap();
        symlink(&second, &selected).unwrap();
        assert!(snapshot.verify().is_err());
        let snapshot = TargetPathSnapshot::capture(&first).unwrap();
        fs::rename(&first, root.path().join("retired")).unwrap();
        fs::create_dir(&first).unwrap();
        assert!(snapshot.verify().is_err());
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn native_raw_directory_snapshot_roundtrips_and_allows_database_replacement() {
        use std::os::unix::ffi::OsStringExt;
        let root = tempfile::tempdir().unwrap();
        let raw = root
            .path()
            .join(std::ffi::OsString::from_vec(b"native-\xff".to_vec()));
        fs::create_dir(&raw).unwrap();
        let database = raw.join("database.sqlite");
        fs::write(&database, b"first database").unwrap();
        let snapshot = TargetPathSnapshot::capture(&raw).unwrap();
        let bytes = serde_json::to_vec(&snapshot).unwrap();
        let reopened: TargetPathSnapshot = serde_json::from_slice(&bytes).unwrap();
        fs::rename(&database, raw.join("retired.sqlite")).unwrap();
        fs::write(&database, b"replacement database").unwrap();
        reopened.verify().unwrap();
        assert_eq!(
            TargetEnvironment::Native.to_native(&database).unwrap(),
            database
        );
        assert!(
            environment(root.path(), &[("C:\\", root.path())])
                .to_native(&database)
                .is_err()
        );
    }

    #[cfg(unix)]
    #[test]
    fn raw_selected_and_canonical_paths_serialize_losslessly_in_directory_evidence() {
        use std::os::unix::ffi::OsStringExt;
        let root = tempfile::tempdir().unwrap();
        let raw = root
            .path()
            .join(std::ffi::OsString::from_vec(b"evidence-\xfe".to_vec()));
        // macOS volumes reject raw directory names. Exercise the lossless DTO
        // independently here; Linux tests cover capture/reopen/verification.
        let snapshot = TargetPathSnapshot {
            root: RootObservation {
                selected: raw.clone(),
                canonical: raw.clone(),
                identity: physical_identity(root.path()).unwrap(),
            },
        };
        let bytes = serde_json::to_vec(&snapshot).unwrap();
        let reopened: TargetPathSnapshot = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(reopened.root.selected, raw);
        assert_eq!(reopened.root.canonical, raw);
    }

    #[cfg(unix)]
    #[test]
    fn native_absolute_path_validation_accepts_raw_bytes_without_a_windows_projection() {
        use std::os::unix::ffi::OsStringExt;
        let root = tempfile::tempdir().unwrap();
        let raw = root
            .path()
            .join(std::ffi::OsString::from_vec(b"native-\xff".to_vec()));
        validate_host_absolute(&raw).unwrap();
        assert_eq!(TargetEnvironment::Native.to_native(&raw).unwrap(), raw);
        assert!(
            environment(root.path(), &[("C:\\", root.path())])
                .to_native(&raw)
                .is_err()
        );
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn native_raw_directory_symlink_proof_reopens_and_rejects_retargeting() {
        use std::os::unix::{ffi::OsStringExt, fs::symlink};
        let root = tempfile::tempdir().unwrap();
        let raw = root
            .path()
            .join(std::ffi::OsString::from_vec(b"physical-\xff".to_vec()));
        let other = root.path().join("other");
        fs::create_dir(&raw).unwrap();
        fs::create_dir(&other).unwrap();
        let selected = root
            .path()
            .join(std::ffi::OsString::from_vec(b"selected-\xfe".to_vec()));
        symlink(&raw, &selected).unwrap();
        let snapshot = TargetPathSnapshot::capture(&selected).unwrap();
        let reopened: TargetPathSnapshot =
            serde_json::from_slice(&serde_json::to_vec(&snapshot).unwrap()).unwrap();
        reopened.verify().unwrap();
        fs::remove_file(&selected).unwrap();
        symlink(&other, &selected).unwrap();
        assert!(reopened.verify().is_err());
    }
}
