//! Automatic local access using the installed target's isolated agent runtime.
//! Key material is carried through private pipes and never persisted or logged.
use crate::*;
use serde::de::{IgnoredAny, SeqAccess, Visitor};
use std::{
    fs::{File, OpenOptions},
    io::{Read, Write},
    process::{Child, Command, Stdio},
};
const MAX_KEY: usize = 16384;
const MAX_OPTIONS: u64 = 16 * 1024 * 1024;
const BRIDGE: &str = include_str!("key_bridge.cjs");
const FUSE: &[u8] = b"dL7pKGdnNz796PbbjQWNKmHXBZaB9tsX";

#[derive(Clone, Debug)]
pub struct AgentKeySource {
    pub executable: PathBuf,
    pub archive: PathBuf,
    pub fuse_image: PathBuf,
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
impl AgentKeySource {
    /// Locate only the target's installed agent. No reference application is used.
    /// Source discovery never launches a process or opens a music database.
    pub fn for_installation(
        target: &TargetInstallation,
        platform: DesktopPlatform,
    ) -> Result<Self> {
        ensure!(matches!(target.major, 6 | 7), "Unsupported target version");
        let source = match platform {
            DesktopPlatform::MacOS => {
                let root = target
                    .root
                    .join("Contents/MacOS/rekordboxAgent.app/Contents");
                Self {
                    executable: root.join("MacOS/rekordboxAgent"),
                    archive: root.join("Resources/app.asar"),
                    fuse_image: root.join(
                        "Frameworks/Electron Framework.framework/Versions/A/Electron Framework",
                    ),
                }
            }
            DesktopPlatform::Windows => {
                let mut roots = Vec::new();
                for (index, entry) in std::fs::read_dir(&target.root)?.take(4097).enumerate() {
                    ensure!(index < 4096, "Target agent discovery budget exceeded");
                    let entry = entry?;
                    let name = entry.file_name().to_string_lossy().to_ascii_lowercase();
                    if name == "rekordboxagent" || name.starts_with("rekordboxagent-win32-") {
                        let meta = std::fs::symlink_metadata(entry.path())?;
                        if meta.is_dir() && !meta.file_type().is_symlink() {
                            roots.push(entry.path());
                        }
                    }
                }
                let mut found = roots
                    .into_iter()
                    .filter_map(|root| {
                        let executable = root.join("rekordboxAgent.exe");
                        let archive = root.join("resources/app.asar");
                        (open_regular(&executable).is_ok() && open_regular(&archive).is_ok()).then(
                            || Self {
                                fuse_image: executable.clone(),
                                executable,
                                archive,
                            },
                        )
                    })
                    .collect::<Vec<_>>();
                ensure!(found.len() == 1, "Target agent is unavailable or ambiguous");
                found.remove(0)
            }
            DesktopPlatform::Linux => {
                anyhow::bail!("Local target agent is unavailable on this platform")
            }
        };
        for path in [&source.executable, &source.archive, &source.fuse_image] {
            open_regular(path)?;
        }
        Ok(source)
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
fn require_node_fuse(path: &Path, cancel: &CancellationToken) -> Result<()> {
    let mut file = open_regular(path)?;
    ensure!(
        file.metadata()?.len() <= 1024 * 1024 * 1024,
        "Agent runtime exceeds inspection budget"
    );
    let mut tail: Vec<u8> = Vec::new();
    let mut found = false;
    let mut inspected = 0u64;
    loop {
        cancel.check()?;
        let mut buffer = [0u8; 65536];
        let n = file.read(&mut buffer)?;
        if n == 0 {
            break;
        }
        inspected += n as u64;
        ensure!(
            inspected <= 1024 * 1024 * 1024,
            "Agent runtime exceeds inspection budget"
        );
        tail.extend_from_slice(&buffer[..n]);
        for at in 0..tail.len().saturating_sub(FUSE.len() + 2) {
            if &tail[at..at + FUSE.len()] == FUSE {
                let flags = &tail[at + FUSE.len()..];
                ensure!(
                    flags[0] == 1 && flags[1] > 0 && flags[2] == b'1',
                    "Target runtime does not allow isolated Node execution"
                );
                found = true;
            }
        }
        if tail.len() > FUSE.len() + 2 {
            tail.drain(..tail.len() - FUSE.len() - 2);
        }
    }
    ensure!(found, "Target runtime has no supported Node execution fuse");
    Ok(())
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
fn selected_input(path: &Path) -> Result<Zeroizing<Vec<u8>>> {
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
    #[derive(Serialize)]
    struct Input<'a> {
        dp: &'a str,
    }
    Ok(Zeroizing::new(serde_json::to_vec(&Input {
        dp: &parsed.options.0,
    })?))
}
struct OwnedChild(Child);
impl Drop for OwnedChild {
    fn drop(&mut self) {
        if self.0.try_wait().ok().flatten().is_none() {
            let _ = self.0.kill();
        }
        let _ = self.0.wait();
    }
}
fn run_bridge(
    source: &AgentKeySource,
    input: Zeroizing<Vec<u8>>,
    options: KeyProviderOptions,
    cancel: &CancellationToken,
) -> Result<Zeroizing<Vec<u8>>> {
    let mut command = Command::new(&source.executable);
    command
        .args(["-e", BRIDGE, "--"])
        .arg(&source.archive)
        .env("ELECTRON_RUN_AS_NODE", "1")
        .env_remove("NODE_OPTIONS")
        .env_remove("NODE_PATH")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000); // CREATE_NO_WINDOW
    }
    let mut child = OwnedChild(
        command
            .spawn()
            .context("Could not start isolated target access")?,
    );
    let mut stdin = child
        .0
        .stdin
        .take()
        .context("Local access input is unavailable")?;
    let mut stdout = child
        .0
        .stdout
        .take()
        .context("Local access output is unavailable")?;
    let writer = std::thread::spawn(move || stdin.write_all(&input));
    let reader = std::thread::spawn(move || -> Result<Zeroizing<Vec<u8>>> {
        // Leave spare capacity beyond the bounded read so plaintext key bytes
        // never pass through reallocations of the response buffer.
        let mut data = Zeroizing::new(Vec::with_capacity(MAX_KEY + 10));
        (&mut stdout)
            .take((MAX_KEY + 9) as u64)
            .read_to_end(&mut data)
            .context("Local access output failed")?;
        ensure!(
            data.len() <= MAX_KEY + 8,
            "Local access output exceeds budget"
        );
        Ok(data)
    });
    let started = Instant::now();
    let outcome = (|| -> Result<()> {
        loop {
            cancel.check()?;
            ensure!(
                started.elapsed() < options.timeout,
                "Local access helper timed out"
            );
            if let Some(status) = child.0.try_wait()? {
                ensure!(
                    status.success(),
                    "Installed target does not support automatic local access"
                );
                return Ok(());
            }
            std::thread::sleep(Duration::from_millis(10));
        }
    })();
    drop(child); // Reap before joining either pipe worker, including cancellation.
    let wrote = writer.join();
    let read = reader.join();
    let wrote = wrote.map_err(|_| anyhow::anyhow!("Local access input worker failed"))?;
    let read = read.map_err(|_| anyhow::anyhow!("Local access output worker failed"))?;
    outcome?;
    wrote.context("Local access input failed")?;
    read
}
/// Resolve a runtime-only key from this machine's selected target installation.
/// No music database is opened here; callers validate access against their copy
/// or use the separately preflighted direct adapter. The helper never starts the
/// ordinary agent lifecycle; its filesystem, account and network modules are stubbed.
pub fn resolve_database_key(
    source: &AgentKeySource,
    options_file: &Path,
    options: KeyProviderOptions,
    cancel: &CancellationToken,
) -> Result<DatabaseKey> {
    cancel.check()?;
    ensure!(
        !options.timeout.is_zero() && options.timeout <= Duration::from_secs(120),
        "Invalid local access deadline"
    );
    open_regular(&source.executable)?;
    open_regular(&source.archive)?;
    let input = selected_input(options_file)?;
    require_node_fuse(&source.fuse_image, cancel)?;
    cancel.check()?;
    let output = run_bridge(source, input, options, cancel)?;
    ensure!(
        output.len() >= 8 && &output[..4] == b"CRK1",
        "Invalid local access response"
    );
    let length = u32::from_be_bytes(output[4..8].try_into()?) as usize;
    ensure!(
        length > 0 && length <= MAX_KEY && output.len() == 8 + length,
        "Invalid local access response length"
    );
    DatabaseKey::new(&output[8..])
}
