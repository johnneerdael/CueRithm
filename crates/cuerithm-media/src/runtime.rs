use crate::{CancellationToken, MediaTaskError};
use anyhow::{Context, Result, bail, ensure};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeSet,
    ffi::OsString,
    fs::File,
    io::{Read, Seek, SeekFrom},
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::{Arc, OnceLock},
    thread,
    time::{Duration, Instant},
};

struct ChildGuard(Child);
impl Drop for ChildGuard {
    fn drop(&mut self) {
        if self.0.try_wait().ok().flatten().is_none() {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }
}

#[derive(Clone)]
pub struct MediaRuntime {
    pub(crate) ffmpeg: PathBuf,
    pub(crate) ffprobe: PathBuf,
    pub identity: String,
    audio_codecs: Arc<OnceLock<BTreeSet<String>>>,
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
            audio_codecs: Arc::new(OnceLock::new()),
        })
    }
    pub(crate) fn supports_audio_codec(
        &self,
        codec: &str,
        cancel: &CancellationToken,
    ) -> Result<bool> {
        cancel.check()?;
        if self.audio_codecs.get().is_none() {
            let args: Vec<OsString> = ["-hide_banner", "-codecs"]
                .into_iter()
                .map(Into::into)
                .collect();
            let mut output = self.run_controlled(
                &self.ffmpeg,
                &args,
                tempfile::tempfile()?,
                1_048_576,
                Duration::from_secs(30),
                cancel,
            )?;
            let mut text = String::new();
            output.read_to_string(&mut text)?;
            let codecs = text
                .lines()
                .filter_map(|line| {
                    let mut words = line.split_whitespace();
                    let flags = words.next()?;
                    let name = words.next()?;
                    (flags.len() == 6 && flags.as_bytes()[0] == b'D' && flags.as_bytes()[2] == b'A')
                        .then(|| name.to_owned())
                })
                .collect::<BTreeSet<_>>();
            ensure!(
                !codecs.is_empty(),
                "Bundled decoder returned no audio codec capabilities"
            );
            cancel.check()?;
            let _ = self.audio_codecs.set(codecs);
        }
        Ok(self.audio_codecs.get().unwrap().contains(codec))
    }
    pub(crate) fn run_controlled(
        &self,
        program: &Path,
        args: &[OsString],
        mut out: File,
        max_bytes: u64,
        timeout: Duration,
        cancellation: &CancellationToken,
    ) -> Result<File> {
        cancellation.check()?;
        let mut err = tempfile::tempfile()?;
        let child = Command::new(program)
            .args(args)
            .stdin(Stdio::null())
            .stdout(Stdio::from(out.try_clone()?))
            .stderr(Stdio::from(err.try_clone()?))
            .spawn()
            .context("Start bundled decoder")?;
        let mut child = ChildGuard(child);
        let start = Instant::now();
        let status = loop {
            cancellation.check()?;
            if start.elapsed() > timeout {
                return Err(MediaTaskError::TimedOut.into());
            }
            if out.metadata()?.len() > max_bytes || err.metadata()?.len() > 1_048_576 {
                return Err(MediaTaskError::OutputLimit.into());
            }
            if let Some(status) = child.0.try_wait()? {
                break status;
            }
            thread::sleep(Duration::from_millis(20));
        };
        cancellation.check()?;
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

#[cfg(test)]
mod process_tests {
    use super::*;
    fn realtime_args(source: &Path) -> Vec<OsString> {
        ["-nostdin", "-v", "error", "-re", "-stream_loop", "-1", "-i"]
            .into_iter()
            .map(Into::into)
            .chain([source.as_os_str().to_owned()])
            .chain(["-vn", "-f", "md5", "pipe:1"].into_iter().map(Into::into))
            .collect()
    }
    #[test]
    fn cancelled_or_timed_out_native_processes_stop_promptly() {
        let root = std::env::var_os("CUERITHM_MEDIA_RUNTIME")
            .map(PathBuf::from)
            .unwrap_or_else(|| {
                Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools/media-runtime/bin")
            });
        let runtime = MediaRuntime::from_bundle(&root).unwrap();
        let d = tempfile::tempdir().unwrap();
        let source = d.path().join("loop.wav");
        let spec = hound::WavSpec {
            channels: 1,
            sample_rate: 8000,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };
        let mut out = hound::WavWriter::create(&source, spec).unwrap();
        for _ in 0..8000 {
            out.write_sample::<i16>(0).unwrap();
        }
        out.finalize().unwrap();
        let args = realtime_args(&source);
        let token = CancellationToken::default();
        let next = token.clone();
        let canceller = thread::spawn(move || {
            thread::sleep(Duration::from_millis(50));
            next.cancel();
        });
        let started = Instant::now();
        let error = runtime
            .run_controlled(
                &runtime.ffmpeg,
                &args,
                tempfile::tempfile().unwrap(),
                4096,
                Duration::from_secs(5),
                &token,
            )
            .unwrap_err();
        canceller.join().unwrap();
        assert_eq!(
            error.downcast_ref::<MediaTaskError>(),
            Some(&MediaTaskError::Cancelled)
        );
        assert!(started.elapsed() < Duration::from_secs(2));
        let started = Instant::now();
        let error = runtime
            .run_controlled(
                &runtime.ffmpeg,
                &args,
                tempfile::tempfile().unwrap(),
                4096,
                Duration::from_millis(50),
                &CancellationToken::default(),
            )
            .unwrap_err();
        assert_eq!(
            error.downcast_ref::<MediaTaskError>(),
            Some(&MediaTaskError::TimedOut)
        );
        assert!(started.elapsed() < Duration::from_secs(2));
    }
}
