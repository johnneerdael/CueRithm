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
    let parts = text
        .split('.')
        .map(str::parse)
        .collect::<std::result::Result<Vec<u32>, _>>()
        .ok()?;
    (!parts.is_empty() && parts.len() <= 4).then_some(parts)
}
#[cfg(windows)]
fn windows_version(path: &Path) -> Option<String> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Storage::FileSystem::{
        GetFileVersionInfoSizeW, GetFileVersionInfoW, VS_FIXEDFILEINFO, VerQueryValueW,
    };
    let name = path
        .as_os_str()
        .encode_wide()
        .chain(Some(0))
        .collect::<Vec<_>>();
    let mut unused = 0;
    let size = unsafe { GetFileVersionInfoSizeW(name.as_ptr(), &mut unused) };
    if size == 0 || size > 16 * 1024 * 1024 {
        return None;
    }
    let mut data = vec![0u8; size as usize];
    if unsafe { GetFileVersionInfoW(name.as_ptr(), 0, size, data.as_mut_ptr().cast()) } == 0 {
        return None;
    }
    let mut ptr = std::ptr::null_mut();
    let mut len = 0;
    let root = [b'\\' as u16, 0];
    if unsafe { VerQueryValueW(data.as_ptr().cast(), root.as_ptr(), &mut ptr, &mut len) } == 0
        || ptr.is_null()
        || len < std::mem::size_of::<VS_FIXEDFILEINFO>() as u32
    {
        return None;
    }
    let info = unsafe { std::ptr::read_unaligned(ptr.cast::<VS_FIXEDFILEINFO>()) };
    if info.dwSignature != 0xfeef04bd {
        return None;
    }
    let (high, low) = if info.dwProductVersionMS != 0 {
        (info.dwProductVersionMS, info.dwProductVersionLS)
    } else {
        (info.dwFileVersionMS, info.dwFileVersionLS)
    };
    Some(format!(
        "{}.{}.{}.{}",
        high >> 16,
        high & 65535,
        low >> 16,
        low & 65535
    ))
}
#[cfg(not(windows))]
fn windows_version(_path: &Path) -> Option<String> {
    None
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
