//! Lexical policy only. Native authority and no-follow checks belong to publication.
use serde::{Deserialize, Deserializer, Serialize, Serializer, de::Error};
use std::path::{Component, Path, PathBuf, Prefix};
use unicode_normalization::UnicodeNormalization;
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum PathRules {
    Portable,
    Unix,
}
/// Metadata separators become underscores; literals are validated, never repaired.
pub fn sanitize(value: &str) -> String {
    value
        .trim()
        .chars()
        .map(|c| {
            if c.is_control() || "<>:\"/\\|?*".contains(c) {
                '_'
            } else {
                c
            }
        })
        .collect::<String>()
        .trim_end_matches([' ', '.'])
        .into()
}
pub fn validate_relative(path: &str, rules: PathRules) -> Result<(), String> {
    if path.is_empty() || path.len() > 4096 {
        return Err("Empty or oversized relative path".into());
    }
    for part in path.split('/') {
        if part.is_empty()
            || part == "."
            || part == ".."
            || part.chars().any(|c| c.is_control() || "\\:\0".contains(c))
        {
            return Err("Unsafe path component".into());
        }
        if part.len() > 255 || part.encode_utf16().count() > 255 {
            return Err("Path component exceeds 255 units".into());
        }
        if rules == PathRules::Portable {
            if part.ends_with([' ', '.']) || part.contains(['<', '>', '"', '|', '?', '*']) {
                return Err("Nonportable path component".into());
            }
            let stem = part.split('.').next().unwrap().to_uppercase();
            if matches!(stem.as_str(), "CON" | "PRN" | "AUX" | "NUL" | "CLOCK$")
                || ((stem.starts_with("COM") || stem.starts_with("LPT"))
                    && matches!(
                        &stem[3..],
                        "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9" | "¹" | "²" | "³"
                    ))
            {
                return Err("Reserved Windows device name".into());
            }
        }
    }
    Ok(())
}
/// Win32 spells one volume several ways (`C:`, `\\?\C:`, `\\server\share`, `\\?\UNC\server\share`),
/// so the prefix is keyed by what it names rather than by its text.
fn prefix_key(prefix: Prefix<'_>) -> String {
    let lower = |name: &std::ffi::OsStr| name.to_string_lossy().to_lowercase();
    match prefix {
        Prefix::Disk(letter) | Prefix::VerbatimDisk(letter) => {
            format!("disk:{}", letter.to_ascii_lowercase() as char)
        }
        Prefix::UNC(server, share) | Prefix::VerbatimUNC(server, share) => {
            format!("unc:{}\\{}", lower(server), lower(share))
        }
        Prefix::Verbatim(name) => format!("verbatim:{}", lower(name)),
        Prefix::DeviceNS(name) => format!("device:{}", lower(name)),
    }
}
/// Conservative NFC/caseless reservations, even on case-sensitive volumes.
/// Unicode lowercase is deliberate; actual volume equivalence still requires native checks.
pub fn collision_key(path: &Path) -> Result<String, String> {
    let parts: Vec<String> = path
        .components()
        .map(|part| {
            if let Component::Prefix(prefix) = part {
                return format!("p:{}", prefix_key(prefix.kind()));
            }
            if let Some(text) = part.as_os_str().to_str() {
                return format!(
                    "u:{}",
                    text.nfc().flat_map(char::to_lowercase).collect::<String>()
                );
            }
            #[cfg(unix)]
            {
                use std::os::unix::ffi::OsStrExt;
                format!("b:{:x?}", part.as_os_str().as_bytes())
            }
            #[cfg(windows)]
            {
                use std::os::windows::ffi::OsStrExt;
                format!(
                    "w:{:x?}",
                    part.as_os_str().encode_wide().collect::<Vec<_>>()
                )
            }
        })
        .collect();
    serde_json::to_string(&parts).map_err(|e| e.to_string())
}
/// Strings stay strings; non-Unicode native paths retain originating OS units.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct NativePath(pub PathBuf);
#[derive(Serialize, Deserialize)]
#[serde(untagged)]
enum Encoded {
    Text(String),
    Unix { unix: Vec<u8> },
    Windows { windows: Vec<u16> },
}
impl Serialize for NativePath {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        if let Some(text) = self.0.to_str() {
            return Encoded::Text(text.into()).serialize(s);
        }
        #[cfg(unix)]
        {
            use std::os::unix::ffi::OsStrExt;
            Encoded::Unix {
                unix: self.0.as_os_str().as_bytes().to_vec(),
            }
            .serialize(s)
        }
        #[cfg(windows)]
        {
            use std::os::windows::ffi::OsStrExt;
            Encoded::Windows {
                windows: self.0.as_os_str().encode_wide().collect(),
            }
            .serialize(s)
        }
    }
}
impl<'de> Deserialize<'de> for NativePath {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        match Encoded::deserialize(d)? {
            Encoded::Text(text) => Ok(Self(text.into())),
            Encoded::Unix { unix } => {
                #[cfg(unix)]
                {
                    use std::os::unix::ffi::OsStringExt;
                    Ok(Self(std::ffi::OsString::from_vec(unix).into()))
                }
                #[cfg(not(unix))]
                {
                    let _ = unix;
                    Err(D::Error::custom("Unix path requires platform mapping"))
                }
            }
            Encoded::Windows { windows } => {
                #[cfg(windows)]
                {
                    use std::os::windows::ffi::OsStringExt;
                    Ok(Self(std::ffi::OsString::from_wide(&windows).into()))
                }
                #[cfg(not(windows))]
                {
                    let _ = windows;
                    Err(D::Error::custom("Windows path requires platform mapping"))
                }
            }
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::{ffi::OsStr, path::Prefix};
    #[test]
    fn win32_namespace_prefixes_of_one_volume_share_a_key() {
        let key = |prefix| prefix_key(prefix);
        assert_eq!(key(Prefix::Disk(b'C')), key(Prefix::VerbatimDisk(b'c')));
        assert_eq!(
            key(Prefix::UNC(OsStr::new("Server"), OsStr::new("Share"))),
            key(Prefix::VerbatimUNC(
                OsStr::new("server"),
                OsStr::new("SHARE")
            ))
        );
        assert_ne!(key(Prefix::Disk(b'C')), key(Prefix::Disk(b'D')));
        assert_ne!(
            key(Prefix::UNC(OsStr::new("a"), OsStr::new("b"))),
            key(Prefix::UNC(OsStr::new("a"), OsStr::new("c")))
        );
        assert_ne!(
            key(Prefix::Disk(b'C')),
            key(Prefix::DeviceNS(OsStr::new("C")))
        );
    }
    #[cfg(windows)]
    #[test]
    fn verbatim_and_plain_windows_paths_collide() {
        assert_eq!(
            collision_key(Path::new(r"C:\Music\Foo.wav")),
            collision_key(Path::new(r"\\?\C:\Music\foo.WAV"))
        );
    }
}
