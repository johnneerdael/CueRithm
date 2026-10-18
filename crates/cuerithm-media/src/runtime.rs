use anyhow::{Context, Result, bail, ensure};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::{
    ffi::OsString,
    fs::File,
    io::{Read, Seek, SeekFrom},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant},
};

#[derive(Clone)]
pub struct MediaRuntime {
    pub(crate) ffmpeg: PathBuf,
    pub(crate) ffprobe: PathBuf,
    pub identity: String,
}
#[derive(Deserialize)]
struct Manifest {
    ffmpeg_sha256: String,
    ffprobe_sha256: String,
    recipe: String,
}

pub fn sha256(path: &Path) -> Result<String> {
    let mut input = File::open(path)?;
    let mut hash = Sha256::new();
    let mut bytes = [0; 65536];
    loop {
        let n = input.read(&mut bytes)?;
        if n == 0 {
            break;
        }
        hash.update(&bytes[..n]);
    }
    Ok(format!("{:x}", hash.finalize()))
}

impl MediaRuntime {
    pub fn from_bundle(dir: &Path) -> Result<Self> {
        let dir = dir
            .canonicalize()
            .context("Bundled decoder folder is missing")?;
        let manifest: Manifest = serde_json::from_slice(
            &std::fs::read(dir.join("runtime.json"))
                .context("Bundled decoder manifest is missing")?,
        )?;
        let ext = if cfg!(windows) { ".exe" } else { "" };
        let ffmpeg = dir.join(format!("ffmpeg{ext}"));
        let ffprobe = dir.join(format!("ffprobe{ext}"));
        ensure!(
            sha256(&ffmpeg)? == manifest.ffmpeg_sha256,
            "Decoder hash mismatch"
        );
        ensure!(
            sha256(&ffprobe)? == manifest.ffprobe_sha256,
            "Probe hash mismatch"
        );
        let identity = format!(
            "{}:{}:{}",
            manifest.recipe, manifest.ffmpeg_sha256, manifest.ffprobe_sha256
        );
        Ok(Self {
            ffmpeg,
            ffprobe,
            identity,
        })
    }
    pub(crate) fn run(
        &self,
        program: &Path,
        args: &[OsString],
        mut out: File,
        max_bytes: u64,
    ) -> Result<File> {
        let mut err = tempfile::tempfile()?;
        let mut child = Command::new(program)
            .args(args)
            .stdin(Stdio::null())
            .stdout(Stdio::from(out.try_clone()?))
            .stderr(Stdio::from(err.try_clone()?))
            .spawn()
            .context("Start bundled decoder")?;
        let start = Instant::now();
        let status = loop {
            if start.elapsed() > Duration::from_secs(300)
                || out.metadata()?.len() > max_bytes
                || err.metadata()?.len() > 1_048_576
            {
                let _ = child.kill();
                let _ = child.wait();
                bail!("Decoder exceeded time or output limits");
            }
            if let Some(status) = child.try_wait()? {
                break status;
            }
            thread::sleep(Duration::from_millis(20));
        };
        if !status.success() {
            err.seek(SeekFrom::Start(0))?;
            let mut detail = String::new();
            err.take(16384).read_to_string(&mut detail)?;
            bail!("Audio decoder failed: {}", detail.trim());
        }
        ensure!(
            out.metadata()?.len() <= max_bytes,
            "Decoder output exceeds limit"
        );
        out.seek(SeekFrom::Start(0))?;
        Ok(out)
    }
}
