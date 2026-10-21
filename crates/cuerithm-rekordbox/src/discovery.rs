use crate::*;
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DesktopPlatform {
    MacOS,
    Windows,
    Linux,
}
#[derive(Debug, Clone)]
pub struct DiscoveryPaths {
    pub platform: DesktopPlatform,
    pub install_roots: Vec<PathBuf>,
    pub settings_file: PathBuf,
    pub default_database_dir: PathBuf,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TargetInstallation {
    pub root: PathBuf,
    pub executable: PathBuf,
    pub version: String,
    pub major: u32,
    pub version_source: String,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum DatabaseLocationState {
    Present,
    Missing,
    Unavailable,
    NotRegular,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscoveredDatabase {
    pub path: PathBuf,
    pub source: String,
    pub state: DatabaseLocationState,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscoveryIssue {
    pub path: PathBuf,
    pub code: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TargetDiscovery {
    pub platform_supported: bool,
    pub installations: Vec<TargetInstallation>,
    pub database: Option<DiscoveredDatabase>,
    pub issues: Vec<DiscoveryIssue>,
}
impl DiscoveryPaths {
    pub fn agent_options_file(&self) -> Result<PathBuf> {
        ensure!(
            self.platform != DesktopPlatform::Linux,
            "Local target agent is unavailable on this platform"
        );
        let pioneer = self
            .settings_file
            .parent()
            .and_then(Path::parent)
            .context("Agent settings location is unavailable")?;
        Ok(pioneer.join("rekordboxAgent/storage/options.json"))
    }
    pub fn current() -> Result<Self> {
        #[cfg(target_os = "macos")]
        {
            let home = PathBuf::from(std::env::var_os("HOME").context("User home is unavailable")?);
            Ok(Self {
                platform: DesktopPlatform::MacOS,
                install_roots: vec![PathBuf::from("/Applications")],
                settings_file: home
                    .join("Library/Application Support/Pioneer/rekordbox6/rekordbox3.settings"),
                default_database_dir: home.join("Library/Pioneer/rekordbox"),
            })
        }
        #[cfg(windows)]
        {
            let home = PathBuf::from(
                std::env::var_os("USERPROFILE").context("User profile is unavailable")?,
            );
            let roaming = std::env::var_os("APPDATA")
                .map(PathBuf::from)
                .unwrap_or_else(|| home.join("AppData/Roaming"));
            let install_roots = ["ProgramFiles", "ProgramFiles(x86)"]
                .into_iter()
                .filter_map(std::env::var_os)
                .map(PathBuf::from)
                .collect();
            Ok(Self {
                platform: DesktopPlatform::Windows,
                install_roots,
                settings_file: roaming.join("Pioneer/rekordbox6/rekordbox3.settings"),
                default_database_dir: roaming.join("Pioneer/rekordbox"),
            })
        }
        #[cfg(not(any(target_os = "macos", windows)))]
        {
            Ok(Self {
                platform: DesktopPlatform::Linux,
                install_roots: vec![],
                settings_file: PathBuf::new(),
                default_database_dir: PathBuf::new(),
            })
        }
    }
}
fn settings_directory(path: &Path) -> Result<Option<PathBuf>> {
    let bytes = std::fs::read(path)?;
    ensure!(bytes.len() <= 16 * 1024 * 1024, "Settings are too large");
    let text = std::str::from_utf8(&bytes)?;
    let doc = roxmltree::Document::parse_with_options(
        text,
        roxmltree::ParsingOptions {
            allow_dtd: false,
            nodes_limit: 100000,
            entity_resolver: None,
        },
    )?;
    let values = doc
        .descendants()
        .filter(|n| {
            n.is_element()
                && n.tag_name().name() == "VALUE"
                && n.attribute("name") == Some("masterDbDirectory")
        })
        .map(|n| n.attribute("val").unwrap_or(""))
        .collect::<std::collections::BTreeSet<_>>();
    ensure!(values.len() <= 1, "Conflicting database locations");
    let Some(value) = values.first().filter(|v| !v.is_empty()) else {
        return Ok(None);
    };
    let path = PathBuf::from(value);
    ensure!(
        path.is_absolute(),
        "Configured database directory is not absolute"
    );
    Ok(Some(path))
}
fn regular(path: &Path) -> bool {
    std::fs::symlink_metadata(path).is_ok_and(|m| m.is_file() && !m.file_type().is_symlink())
}
fn version_parts(text: &str) -> Option<Vec<u32>> {
    if !text
        .split('.')
        .all(|part| !part.is_empty() && part.bytes().all(|b| b.is_ascii_digit()))
    {
        return None;
    }
    let parts = text
        .split('.')
        .map(str::parse)
        .collect::<std::result::Result<Vec<u32>, _>>()
        .ok()?;
    (!parts.is_empty() && parts.len() <= 4).then_some(parts)
}
// Inspect PE resources as data on every host. Keep the file and section budgets
// bounded and reject ambiguous fixed-version records.
fn windows_version(path: &Path) -> Option<String> {
    use std::io::Read;
    let file = std::fs::File::open(path).ok()?;
    let length = file.metadata().ok()?.len();
    if !(64..=256 * 1024 * 1024).contains(&length) {
        return None;
    }
    let mut bytes = Vec::new();
    file.take(length + 1).read_to_end(&mut bytes).ok()?;
    if bytes.len() as u64 != length || bytes.get(..2)? != b"MZ" {
        return None;
    }
    let u16_at = |offset: usize| -> Option<u16> {
        Some(u16::from_le_bytes(
            bytes.get(offset..offset.checked_add(2)?)?.try_into().ok()?,
        ))
    };
    let u32_at = |offset: usize| -> Option<u32> {
        Some(u32::from_le_bytes(
            bytes.get(offset..offset.checked_add(4)?)?.try_into().ok()?,
        ))
    };
    let pe = u32_at(60)? as usize;
    if bytes.get(pe..pe.checked_add(4)?)? != b"PE\0\0" {
        return None;
    }
    let count = u16_at(pe.checked_add(6)?)? as usize;
    if count > 96 {
        return None;
    }
    let sections = pe
        .checked_add(24)?
        .checked_add(u16_at(pe.checked_add(20)?)? as usize)?;
    let mut versions = std::collections::BTreeSet::new();
    for index in 0..count {
        let section = sections.checked_add(index * 40)?;
        if bytes.get(section..section.checked_add(8)?)? != b".rsrc\0\0\0" {
            continue;
        }
        let size = u32_at(section + 16)? as usize;
        let start = u32_at(section + 20)? as usize;
        if size > 16 * 1024 * 1024 {
            return None;
        }
        let data = bytes.get(start..start.checked_add(size)?)?;
        for record in data.windows(52) {
            if record[..8] != [0xbd, 0x04, 0xef, 0xfe, 0x00, 0x00, 0x01, 0x00] {
                continue;
            }
            let word = |n| u32::from_le_bytes(record[n..n + 4].try_into().unwrap());
            let (high, low) = if word(16) != 0 {
                (word(16), word(20))
            } else {
                (word(8), word(12))
            };
            versions.insert(format!(
                "{}.{}.{}.{}",
                high >> 16,
                high & 65535,
                low >> 16,
                low & 65535
            ));
        }
    }
    if versions.len() == 1 {
        versions.into_iter().next()
    } else {
        None
    }
}

/// Inspect an explicitly selected executable without running it. Windows version
/// resources are portable; a missing resource requires a bounded version hint.
pub fn inspect_manual_installation(
    executable: &Path,
    version_hint: Option<&str>,
) -> Result<TargetInstallation> {
    ensure!(
        executable.is_absolute() && regular(executable),
        "Select a regular installed rekordbox executable"
    );
    let name = executable
        .file_name()
        .and_then(|n| n.to_str())
        .context("Executable name is not Unicode")?;
    ensure!(
        name.eq_ignore_ascii_case("rekordbox.exe") || name == "rekordbox",
        "Select the rekordbox executable"
    );
    let executable = executable.canonicalize()?;
    let root = executable
        .parent()
        .context("Installation directory missing")?
        .to_path_buf();
    let observed = if name.eq_ignore_ascii_case("rekordbox.exe") {
        windows_version(&executable).map(|v| (v, "version_resource"))
    } else {
        root.parent()
            .and_then(|p| plist::Value::from_file(p.join("Info.plist")).ok())
            .and_then(|v| {
                v.as_dictionary()?
                    .get("CFBundleShortVersionString")?
                    .as_string()
                    .map(str::to_owned)
            })
            .map(|v| (v, "bundle_plist"))
    };
    let (version, source) = observed
        .or_else(|| version_hint.map(|v| (v.to_owned(), "explicit_version_hint")))
        .context("Installed version unavailable; provide an explicit rekordbox 6 or 7 version")?;
    ensure!(version.len() <= 64, "Invalid installed version");
    let parts = version_parts(&version)
        .context("Installed version must have one to four numeric components")?;
    ensure!(
        matches!(parts[0], 6 | 7),
        "Only rekordbox 6 and 7 database protocols are supported"
    );
    Ok(TargetInstallation {
        root,
        executable,
        version,
        major: parts[0],
        version_source: source.into(),
    })
}
fn inspect_install(
    path: &Path,
    platform: DesktopPlatform,
    issues: &mut Vec<DiscoveryIssue>,
) -> Option<TargetInstallation> {
    let parsed = (|| -> Result<(PathBuf, String, String)> {
        match platform {
            DesktopPlatform::MacOS => {
                let info = plist::Value::from_file(path.join("Contents/Info.plist"))?;
                let dict = info.as_dictionary().context("Invalid bundle metadata")?;
                let exe = dict
                    .get("CFBundleExecutable")
                    .and_then(plist::Value::as_string)
                    .context("Missing executable")?;
                ensure!(
                    exe.eq_ignore_ascii_case("rekordbox"),
                    "Unrecognized executable"
                );
                let version = dict
                    .get("CFBundleShortVersionString")
                    .and_then(plist::Value::as_string)
                    .context("Missing installed version")?
                    .to_owned();
                Ok((
                    path.join("Contents/MacOS").join(exe),
                    version,
                    "bundle_plist".into(),
                ))
            }
            DesktopPlatform::Windows => {
                let exe = path.join("rekordbox.exe");
                if let Some(version) = windows_version(&exe) {
                    return Ok((exe, version, "version_resource".into()));
                }
                let name = path
                    .file_name()
                    .context("Missing installation name")?
                    .to_string_lossy();
                let hint = name
                    .to_ascii_lowercase()
                    .strip_prefix("rekordbox")
                    .context("Missing version resource")?
                    .trim()
                    .to_owned();
                ensure!(version_parts(&hint).is_some(), "Missing version resource");
                issues.push(DiscoveryIssue {
                    path: exe.clone(),
                    code: "version_from_directory_hint".into(),
                });
                Ok((exe, hint, "directory_hint".into()))
            }
            DesktopPlatform::Linux => anyhow::bail!("Unsupported platform"),
        }
    })();
    match parsed {
        Ok((executable, version, version_source)) if regular(&executable) => {
            let Some(parts) = version_parts(&version) else {
                issues.push(DiscoveryIssue {
                    path: path.into(),
                    code: "invalid_version".into(),
                });
                return None;
            };
            if !matches!(parts[0], 6 | 7) {
                issues.push(DiscoveryIssue {
                    path: path.into(),
                    code: "unsupported_major_version".into(),
                });
            }
            Some(TargetInstallation {
                root: path.into(),
                executable,
                major: parts[0],
                version,
                version_source,
            })
        }
        _ => {
            issues.push(DiscoveryIssue {
                path: path.into(),
                code: "invalid_installation".into(),
            });
            None
        }
    }
}
pub fn discover_targets(
    paths: &DiscoveryPaths,
    cancel: &CancellationToken,
) -> Result<TargetDiscovery> {
    cancel.check()?;
    let mut result = TargetDiscovery {
        platform_supported: paths.platform != DesktopPlatform::Linux,
        installations: vec![],
        database: None,
        issues: vec![],
    };
    if !result.platform_supported {
        return Ok(result);
    }
    ensure!(
        paths.install_roots.len() <= 64,
        "Too many installation roots"
    );
    let mut pending = paths
        .install_roots
        .iter()
        .map(|p| (p.clone(), 0usize))
        .collect::<Vec<_>>();
    let mut seen = std::collections::BTreeSet::new();
    let mut work = 0;
    while let Some((path, depth)) = pending.pop() {
        cancel.check()?;
        let metadata = match std::fs::symlink_metadata(&path) {
            Ok(m) => m,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => continue,
            Err(_) => {
                result.issues.push(DiscoveryIssue {
                    path,
                    code: "unavailable_install_root".into(),
                });
                continue;
            }
        };
        if !metadata.is_dir() || metadata.file_type().is_symlink() {
            continue;
        }
        let name = path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_ascii_lowercase();
        let candidate = match paths.platform {
            DesktopPlatform::MacOS => name.ends_with(".app") && name.contains("rekordbox"),
            DesktopPlatform::Windows => regular(&path.join("rekordbox.exe")),
            DesktopPlatform::Linux => false,
        };
        if candidate {
            if let Some(app) = inspect_install(&path, paths.platform, &mut result.issues) {
                let canonical = app.root.canonicalize().unwrap_or(app.root.clone());
                if seen.insert(canonical) {
                    result.installations.push(app);
                }
            }
            continue;
        }
        if depth >= 3 {
            continue;
        }
        let entries = match std::fs::read_dir(&path) {
            Ok(v) => v,
            Err(_) => {
                result.issues.push(DiscoveryIssue {
                    path,
                    code: "unavailable_install_root".into(),
                });
                continue;
            }
        };
        for entry in entries {
            cancel.check()?;
            work += 1;
            ensure!(work <= 100000, "Installation discovery budget exceeded");
            let entry = entry?;
            let name = entry.file_name().to_string_lossy().to_ascii_lowercase();
            if name.contains("rekordbox") || matches!(name.as_str(), "pioneer" | "alphatheta") {
                pending.push((entry.path(), depth + 1));
            }
        }
    }
    result.installations.sort_by(|a, b| {
        version_parts(&b.version)
            .cmp(&version_parts(&a.version))
            .then_with(|| a.root.cmp(&b.root))
    });
    let (directory, source) = match std::fs::symlink_metadata(&paths.settings_file) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            (paths.default_database_dir.clone(), "default")
        }
        Ok(m) if m.is_file() && !m.file_type().is_symlink() => {
            match settings_directory(&paths.settings_file) {
                Ok(Some(path)) => (path, "configured"),
                Ok(None) => (paths.default_database_dir.clone(), "default"),
                Err(_) => {
                    result.issues.push(DiscoveryIssue {
                        path: paths.settings_file.clone(),
                        code: "invalid_settings".into(),
                    });
                    return Ok(result);
                }
            }
        }
        _ => {
            result.issues.push(DiscoveryIssue {
                path: paths.settings_file.clone(),
                code: "unavailable_settings".into(),
            });
            return Ok(result);
        }
    };
    cancel.check()?;
    let path = directory.join("master.db");
    let state = match std::fs::symlink_metadata(&path) {
        Ok(m) if m.is_file() && !m.file_type().is_symlink() => DatabaseLocationState::Present,
        Ok(_) => DatabaseLocationState::NotRegular,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound && directory.is_dir() => {
            DatabaseLocationState::Missing
        }
        Err(_) => DatabaseLocationState::Unavailable,
    };
    result.database = Some(DiscoveredDatabase {
        path,
        source: source.into(),
        state,
    });
    Ok(result)
}

#[cfg(test)]
mod manual_tests {
    use super::*;
    #[test]
    fn manual_windows_selection_requires_a_bounded_supported_version_without_resources() {
        let dir = tempfile::tempdir().unwrap();
        let executable = dir.path().canonicalize().unwrap().join("rekordbox.exe");
        std::fs::write(&executable, b"MZ authored executable, never executed").unwrap();
        assert!(inspect_manual_installation(&executable, None).is_err());
        let installed = inspect_manual_installation(&executable, Some("7.9.42.1")).unwrap();
        assert_eq!(installed.major, 7);
        assert_eq!(installed.version_source, "explicit_version_hint");
        for invalid in ["8.1", "7.bad", "7.1.2.3.4", "7.-1", " 7.0"] {
            assert!(inspect_manual_installation(&executable, Some(invalid)).is_err());
        }
    }
    #[test]
    fn portable_pe_version_is_read_as_data() {
        let dir = tempfile::tempdir().unwrap();
        let executable = dir.path().canonicalize().unwrap().join("rekordbox.exe");
        let mut bytes = vec![0u8; 512];
        bytes[..2].copy_from_slice(b"MZ");
        bytes[60..64].copy_from_slice(&64u32.to_le_bytes());
        bytes[64..68].copy_from_slice(b"PE\0\0");
        bytes[70..72].copy_from_slice(&1u16.to_le_bytes());
        bytes[88..96].copy_from_slice(b".rsrc\0\0\0");
        bytes[104..108].copy_from_slice(&128u32.to_le_bytes());
        bytes[108..112].copy_from_slice(&256u32.to_le_bytes());
        bytes[256..264].copy_from_slice(&[0xbd, 0x04, 0xef, 0xfe, 0, 0, 1, 0]);
        bytes[272..276].copy_from_slice(&((7u32 << 16) | 3).to_le_bytes());
        bytes[276..280].copy_from_slice(&((5u32 << 16) | 42).to_le_bytes());
        std::fs::write(&executable, bytes).unwrap();
        let installed = inspect_manual_installation(&executable, Some("6.0")).unwrap();
        assert_eq!(installed.version, "7.3.5.42");
        assert_eq!(installed.version_source, "version_resource");
    }
}
