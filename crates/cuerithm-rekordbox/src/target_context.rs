//! Explicit physical-host and Windows destination path context.
use anyhow::{Context, Result, bail, ensure};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::{Component, Path, PathBuf},
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
    selected: PathBuf,
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

impl TargetEnvironment {
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
            directory_root(prefix)?;
            ensure!(
                !mappings.is_empty(),
                "Choose explicit Windows drive mappings"
            );
            let mut windows = Vec::new();
            let mut hosts: Vec<PathBuf> = Vec::new();
            for mapping in mappings {
                let native = WindowsPath::parse(&mapping.windows_root)?;
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

    /// Project a physical source into the selected destination's path namespace.
    /// No source existence requirement is imposed on Native references.
    pub fn to_native(&self, path: &Path) -> Result<PathBuf> {
        let Self::WindowsCompatibility { mappings, .. } = self else {
            return Ok(path.to_owned());
        };
        self.validate()?;
        validate_host_absolute(path)?;
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
        let source = resolve_under(&mapping.host_root, parts)?;
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
        self.validate()?;
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
        let Self::WindowsCompatibility { mappings, .. } = self else {
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
        resolve_under(&mapping.host_root, &path.parts[*length..])
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
    path_text(path)?;
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
fn resolve_under(root: &Path, parts: &[String]) -> Result<PathBuf> {
    let canonical_root = directory_root(root)?;
    let mut result = root.to_owned();
    for part in parts {
        validate_windows_component(part)?;
        let mut matches = Vec::new();
        match fs::read_dir(&result) {
            Ok(entries) => {
                for entry in entries {
                    let entry = entry.context("Read mapped directory entry")?;
                    if entry
                        .file_name()
                        .to_str()
                        .is_some_and(|name| fold(name) == fold(part))
                    {
                        matches.push(entry.file_name());
                    }
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error).context("Inspect mapped directory"),
        }
        ensure!(
            matches.len() <= 1,
            "Case-colliding host entries cannot share a Windows reference"
        );
        result.push(
            matches
                .first()
                .map_or_else(|| std::ffi::OsStr::new(part), |p| p.as_os_str()),
        );
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
}
