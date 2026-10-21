//! Native, data-only key derivation from the installed target executable.
use crate::*;
use serde::de::{IgnoredAny, SeqAccess, Visitor};
use std::{
    fs::{File, OpenOptions},
    io::Read,
};
const MAX_KEY: usize = 16384;
const MAX_OPTIONS: u64 = 16 * 1024 * 1024;
#[derive(Clone, Debug)]
pub struct NativeKeySource {
    pub executable: PathBuf,
}
#[derive(Clone, Copy, Debug)]
pub struct KeyProviderOptions {
    pub timeout: Duration,
}
impl Default for KeyProviderOptions {
    fn default() -> Self {
        Self {
            timeout: Duration::from_secs(20),
        }
    }
}
impl NativeKeySource {
    pub fn for_environment(
        target: &TargetInstallation,
        environment: &TargetEnvironment,
        native_platform: DesktopPlatform,
    ) -> Result<Self> {
        environment.validate()?;
        let platform = if environment.is_native() {
            native_platform
        } else {
            DesktopPlatform::Windows
        };
        Self::for_installation(target, platform)
    }
    pub fn for_installation(
        target: &TargetInstallation,
        platform: DesktopPlatform,
    ) -> Result<Self> {
        ensure!(
            platform != DesktopPlatform::Linux && matches!(target.major, 6 | 7),
            "Local target version is unsupported"
        );
        open_regular(&target.executable)?;
        Ok(Self {
            executable: target.executable.clone(),
        })
    }
}
fn open_regular(path: &Path) -> Result<File> {
    let before = std::fs::symlink_metadata(path).context("Local access file is unavailable")?;
    ensure!(
        before.is_file() && !before.file_type().is_symlink(),
        "Local access requires regular files"
    );
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NOFOLLOW);
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        options.custom_flags(windows_sys::Win32::Storage::FileSystem::FILE_FLAG_OPEN_REPARSE_POINT);
    }
    let file = options
        .open(path)
        .context("Local access file is unavailable")?;
    let meta = file.metadata()?;
    ensure!(meta.is_file(), "Local access requires regular files");
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        ensure!(
            meta.file_attributes()
                & windows_sys::Win32::Storage::FileSystem::FILE_ATTRIBUTE_REPARSE_POINT
                == 0,
            "Symbolic local access files are unsupported"
        );
    }
    Ok(file)
}
// Select dp while ignoring account/authentication fields rather than retaining a
// generic copy of every option. Parse errors are deliberately not propagated.
struct SelectedDp(Zeroizing<String>);
struct OptionEntry(Option<Zeroizing<String>>);
impl<'de> Deserialize<'de> for OptionEntry {
    fn deserialize<D: serde::Deserializer<'de>>(
        deserializer: D,
    ) -> std::result::Result<Self, D::Error> {
        struct EntryVisitor;
        impl<'de> Visitor<'de> for EntryVisitor {
            type Value = OptionEntry;
            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                f.write_str("an option pair")
            }
            fn visit_seq<A: SeqAccess<'de>>(
                self,
                mut sequence: A,
            ) -> std::result::Result<Self::Value, A::Error> {
                use serde::de::Error;
                let name = sequence
                    .next_element::<String>()?
                    .ok_or_else(|| A::Error::custom("Missing option name"))?;
                let selected = if name == "dp" {
                    let value = Zeroizing::new(
                        sequence
                            .next_element::<String>()?
                            .ok_or_else(|| A::Error::custom("Missing selected option"))?,
                    );
                    if value.is_empty() || value.len() > MAX_KEY {
                        return Err(A::Error::custom("Selected option size"));
                    }
                    Some(value)
                } else {
                    sequence
                        .next_element::<IgnoredAny>()?
                        .ok_or_else(|| A::Error::custom("Missing option value"))?;
                    None
                };
                if sequence.next_element::<IgnoredAny>()?.is_some() {
                    return Err(A::Error::custom("Invalid option pair"));
                }
                Ok(OptionEntry(selected))
            }
        }
        deserializer.deserialize_seq(EntryVisitor)
    }
}
impl<'de> Deserialize<'de> for SelectedDp {
    fn deserialize<D: serde::Deserializer<'de>>(
        deserializer: D,
    ) -> std::result::Result<Self, D::Error> {
        struct OptionsVisitor;
        impl<'de> Visitor<'de> for OptionsVisitor {
            type Value = SelectedDp;
            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                f.write_str("selected agent options")
            }
            fn visit_seq<A: SeqAccess<'de>>(
                self,
                mut sequence: A,
            ) -> std::result::Result<Self::Value, A::Error> {
                use serde::de::Error;
                let mut value = None;
                while let Some(entry) = sequence.next_element::<OptionEntry>()? {
                    if let Some(next) = entry.0 {
                        if value.is_some() {
                            return Err(A::Error::custom("Ambiguous selected option"));
                        }
                        value = Some(next);
                    }
                }
                value
                    .map(SelectedDp)
                    .ok_or_else(|| A::Error::custom("Selected option is missing"))
            }
        }
        deserializer.deserialize_seq(OptionsVisitor)
    }
}
#[derive(Deserialize)]
struct AgentOptions {
    options: SelectedDp,
}
fn selected_dp(path: &Path) -> Result<Zeroizing<String>> {
    let file = open_regular(path)?;
    let expected = file.metadata()?.len();
    ensure!(
        expected <= MAX_OPTIONS,
        "Agent options exceed inspection budget"
    );
    let mut bytes = Zeroizing::new(Vec::with_capacity(expected as usize + 2));
    file.take(expected + 1).read_to_end(&mut bytes)?;
    ensure!(
        bytes.len() as u64 == expected,
        "Agent options changed during reading"
    );
    let parsed: AgentOptions = serde_json::from_slice(&bytes)
        .map_err(|_| anyhow::anyhow!("Agent options are missing, ambiguous or unsupported"))?;
    Ok(parsed.options.0)
}

/// Read-only native artifact profile. The public string marker precedes a
/// sixteen-byte C string in the inspected x86_64/arm64 images. No seed/key value
/// is compiled in, and ambiguous or unknown layouts are explicitly rejected.
fn native_seed(
    source: &NativeKeySource,
    options: KeyProviderOptions,
    cancel: &CancellationToken,
    started: Instant,
) -> Result<Zeroizing<Vec<u8>>> {
    use zeroize::Zeroize;
    const MARKER: &[u8] = b"rekordbox_client\0";
    const RECORD: usize = MARKER.len() + 16 + 1;
    const LIMIT: u64 = 1024 * 1024 * 1024;
    let mut file = open_regular(&source.executable)?;
    let expected = file.metadata()?.len();
    ensure!(
        (4..=LIMIT).contains(&expected),
        "Native target exceeds inspection budget"
    );
    let mut signature = [0u8; 4];
    file.read_exact(&mut signature)?;
    ensure!(
        matches!(
            signature,
            [0xca, 0xfe, 0xba, 0xbe]
                | [0xbe, 0xba, 0xfe, 0xca]
                | [0xca, 0xfe, 0xba, 0xbf]
                | [0xbf, 0xba, 0xfe, 0xca]
                | [0xfe, 0xed, 0xfa, 0xcf]
                | [0xcf, 0xfa, 0xed, 0xfe]
        ) || &signature[..2] == b"MZ",
        "Unsupported native target format"
    );
    let mut inspected = 4u64;
    let mut seeds: Vec<Zeroizing<Vec<u8>>> = Vec::new();
    let mut buffer = Zeroizing::new([0u8; 65536]);
    let mut data = Zeroizing::new(Vec::with_capacity(65536 + RECORD));
    data.extend(signature);
    loop {
        cancel.check()?;
        ensure!(
            started.elapsed() < options.timeout,
            "Native target inspection timed out"
        );
        let n = file.read(&mut *buffer)?;
        if n == 0 {
            break;
        }
        inspected += n as u64;
        ensure!(
            inspected <= expected && inspected <= LIMIT,
            "Native target changed during inspection"
        );
        data.extend_from_slice(&buffer[..n]);
        for record in data.windows(RECORD) {
            if record.starts_with(MARKER) && record[RECORD - 1] == 0 {
                let seed = &record[MARKER.len()..RECORD - 1];
                if seed.iter().all(|b| b.is_ascii_graphic() || *b == b' ')
                    && !seeds.iter().any(|s| s.as_slice() == seed)
                {
                    ensure!(
                        seeds.len() < 64,
                        "Native target profile exceeds candidate budget"
                    );
                    seeds.push(Zeroizing::new(seed.to_vec()));
                }
            }
        }
        let keep = data.len().min(RECORD - 1);
        let mut carry = Zeroizing::new([0u8; RECORD - 1]);
        carry[..keep].copy_from_slice(&data[data.len() - keep..]);
        data.zeroize();
        data.extend_from_slice(&carry[..keep]);
    }
    ensure!(
        inspected == expected,
        "Native target changed during inspection"
    );
    ensure!(
        seeds.len() == 1,
        "Native target key profile is unavailable or ambiguous"
    );
    Ok(seeds.remove(0))
}
/// Derive a runtime-only database key in Rust. This reads local target bytes and
/// the selected encrypted option; it launches no process and opens no database.
pub fn resolve_database_key(
    source: &NativeKeySource,
    options_file: &Path,
    options: KeyProviderOptions,
    cancel: &CancellationToken,
) -> Result<DatabaseKey> {
    use base64::{Engine as _, engine::general_purpose::STANDARD};
    use blowfish::{
        Blowfish,
        cipher::{BlockDecrypt, KeyInit},
    };
    cancel.check()?;
    ensure!(
        !options.timeout.is_zero() && options.timeout <= Duration::from_secs(120),
        "Invalid native inspection deadline"
    );
    let started = Instant::now();
    let dp = selected_dp(options_file)?;
    let seed = native_seed(source, options, cancel, started)?;
    let mut decoded = Zeroizing::new(
        STANDARD
            .decode(dp.as_bytes())
            .map_err(|_| anyhow::anyhow!("Invalid encrypted database option"))?,
    );
    ensure!(
        !decoded.is_empty() && decoded.len().is_multiple_of(8) && decoded.len() <= MAX_KEY,
        "Invalid encrypted database option size"
    );
    let cipher: Blowfish = Blowfish::new_from_slice(&seed)
        .map_err(|_| anyhow::anyhow!("Unsupported native key profile"))?;
    for block in decoded.as_chunks_mut::<8>().0 {
        cipher.decrypt_block(block.into());
    }
    let padding = *decoded
        .last()
        .context("Invalid encrypted database option")? as usize;
    // The real option can be block-aligned text without padding. The target's
    // decoder retains that variant; remove PKCS5 only when it is present.
    if (1..=8).contains(&padding) {
        ensure!(
            padding <= decoded.len()
                && decoded[decoded.len() - padding..]
                    .iter()
                    .all(|b| *b as usize == padding),
            "Invalid encrypted database option padding"
        );
        let length = decoded.len() - padding;
        decoded.truncate(length);
    }
    ensure!(
        !decoded.is_empty()
            && std::str::from_utf8(&decoded).is_ok_and(|s| !s.chars().any(char::is_control)),
        "Unsupported decrypted database key format"
    );
    cancel.check()?;
    DatabaseKey::new(decoded.as_slice())
}
