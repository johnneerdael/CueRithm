use crate::{AudioClock, MediaInfo, MediaRuntime, SourceExportTiming, runtime::sha256};
use anyhow::{Context, Result, ensure};
use std::{
    ffi::OsString,
    fs::File,
    io::{Read, Seek, SeekFrom},
    path::{Path, PathBuf},
};

pub struct PreparedAudio {
    _private: tempfile::TempDir,
    pub source_path: PathBuf,
    pub snapshot_path: PathBuf,
    pub pcm_path: PathBuf,
    pub source_sha256: String,
    pub info: MediaInfo,
    pub frames: u64,
    pub clock: AudioClock,
    pub decoder_identity: String,
    pub export_timing: Option<SourceExportTiming>,
}
impl PreparedAudio {
    pub fn source_is_unchanged(&self) -> Result<bool> {
        if !self.source_path.is_file() {
            return Ok(false);
        }
        Ok(sha256(&self.source_path)? == self.source_sha256)
    }
    pub fn source_is_unchanged_cancellable(
        &self,
        cancel: &crate::CancellationToken,
    ) -> Result<bool> {
        cancel.check()?;
        if !self.source_path.is_file() {
            return Ok(false);
        }
        Ok(crate::file_sha256_cancellable(&self.source_path, cancel)? == self.source_sha256)
    }
    pub fn pcm_sha256_cancellable(&self, cancel: &crate::CancellationToken) -> Result<String> {
        crate::file_sha256_cancellable(&self.pcm_path, cancel)
    }
    pub fn pcm_sha256(&self) -> Result<String> {
        sha256(&self.pcm_path)
    }
    pub fn read_frames(&self, start: u64, frames: usize) -> Result<Vec<f32>> {
        ensure!(
            start <= self.frames && frames <= 1_048_576,
            "Invalid PCM read range"
        );
        let count = ((self.frames - start).min(frames as u64) * 2) as usize;
        let mut input = File::open(&self.pcm_path)?;
        input.seek(SeekFrom::Start(start * 8))?;
        let mut bytes = vec![0; count * 4];
        input.read_exact(&mut bytes)?;
        Ok(bytes
            .as_chunks::<4>()
            .0
            .iter()
            .map(|b| f32::from_le_bytes(*b))
            .collect())
    }
    pub fn mono_samples(&self) -> Result<Vec<f32>> {
        let mut output = Vec::with_capacity(self.frames as usize);
        let mut pos = 0;
        while pos < self.frames {
            let samples = self.read_frames(pos, 65536)?;
            output.extend(
                samples
                    .as_chunks::<2>()
                    .0
                    .iter()
                    .map(|s| (s[0] + s[1]) * 0.5),
            );
            pos += samples.len() as u64 / 2;
        }
        Ok(output)
    }
    pub fn stereo_s16(&self) -> Result<Vec<f32>> {
        let mut output = Vec::with_capacity(self.frames as usize * 2);
        let mut pos = 0;
        while pos < self.frames {
            let samples = self.read_frames(pos, 65536)?;
            output.extend(
                samples
                    .iter()
                    .map(|x| (x.clamp(-1., 32767. / 32768.) * 32768.).round() / 32768.),
            );
            pos += samples.len() as u64 / 2;
        }
        Ok(output)
    }
}
impl MediaRuntime {
    pub fn decode(&self, path: &Path, cache: &Path) -> Result<PreparedAudio> {
        self.decode_cancellable(path, cache, &crate::CancellationToken::default())
    }
    pub fn decode_cancellable(
        &self,
        path: &Path,
        cache: &Path,
        cancel: &crate::CancellationToken,
    ) -> Result<PreparedAudio> {
        cancel.check()?;
        std::fs::create_dir_all(cache)?;
        let private = tempfile::Builder::new()
            .prefix("cuerithm-")
            .tempdir_in(cache)?;
        let source_path = path.canonicalize().context("Audio file is missing")?;
        let before = crate::file_sha256_cancellable(&source_path, cancel)?;
        let mut snapshot_path = private.path().join("source");
        let mut input = File::open(&source_path)?;
        let mut output = File::options()
            .write(true)
            .create_new(true)
            .open(&snapshot_path)?;
        let mut chunk = [0u8; 65536];
        loop {
            cancel.check()?;
            let n = input.read(&mut chunk)?;
            if n == 0 {
                break;
            }
            std::io::Write::write_all(&mut output, &chunk[..n])?;
        }
        drop(output);
        drop(input);
        ensure!(
            crate::file_sha256_cancellable(&snapshot_path, cancel)? == before
                && crate::file_sha256_cancellable(&source_path, cancel)? == before,
            "Source changed during capture"
        );
        let info = self.probe_cancellable(&snapshot_path, cancel)?;
        let extension = path
            .extension()
            .and_then(|s| s.to_str())
            .unwrap_or_default();
        let export_timing = Some(self.capture_export_timing_cancellable(
            &snapshot_path,
            &before,
            &info,
            extension,
            cancel,
        )?);
        if info.codec == "flac" {
            let named = private.path().join("source.flac");
            std::fs::rename(&snapshot_path, &named)?;
            snapshot_path = named;
        }
        let pcm_path = private.path().join("audio.f32");
        let mut args: Vec<OsString> = [
            "-nostdin",
            "-v",
            "error",
            "-xerror",
            "-err_detect",
            "explode",
            "-protocol_whitelist",
            "file,pipe",
            "-i",
        ]
        .into_iter()
        .map(Into::into)
        .collect();
        args.push(snapshot_path.as_os_str().to_owned());
        args.extend(
            [
                "-map",
                "0:a:0",
                "-vn",
                "-sn",
                "-dn",
                "-ac",
                "2",
                "-ar",
                "44100",
                "-af",
                "aresample=44100:resampler=swr:dither_method=none",
                "-c:a",
                "pcm_f32le",
                "-f",
                "f32le",
                "pipe:1",
            ]
            .into_iter()
            .map(Into::into),
        );
        let file = File::options()
            .read(true)
            .write(true)
            .create_new(true)
            .open(&pcm_path)?;
        let mut pcm = self.run_controlled(
            &self.ffmpeg,
            &args,
            file,
            64_000_000 * 8,
            std::time::Duration::from_secs(300),
            cancel,
        )?;
        let bytes = pcm.metadata()?.len();
        ensure!(
            bytes > 0 && bytes.is_multiple_of(8),
            "Incomplete or empty PCM output"
        );
        let frames = bytes / 8;
        let mut chunk = [0u8; 65536];
        loop {
            cancel.check()?;
            let n = pcm.read(&mut chunk)?;
            if n == 0 {
                break;
            }
            ensure!(n.is_multiple_of(4), "Unaligned PCM");
            ensure!(
                chunk[..n]
                    .as_chunks::<4>()
                    .0
                    .iter()
                    .all(|b| f32::from_le_bytes(*b).is_finite()),
                "Non-finite PCM"
            );
        }
        ensure!(
            crate::file_sha256_cancellable(&source_path, cancel)? == before,
            "Source changed during decode"
        );
        Ok(PreparedAudio {
            _private: private,
            source_path,
            snapshot_path,
            pcm_path,
            source_sha256: before,
            info,
            frames,
            clock: AudioClock {
                frames,
                sample_rate: 44100,
                leading_padding_frames: None,
                trailing_padding_frames: None,
            },
            decoder_identity: self.identity.clone(),
            export_timing,
        })
    }
}

#[cfg(test)]
mod cancellation_tests {
    use super::*;
    #[test]
    fn cancelled_decode_does_not_launch_or_allocate_a_snapshot() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let runtime = MediaRuntime::from_bundle(&root.join("tools/media-runtime/bin")).unwrap();
        let d = tempfile::tempdir().unwrap();
        let cache = d.path().join("cancelled-cache");
        let token = crate::CancellationToken::default();
        token.cancel();
        let result = runtime.decode_cancellable(
            &root.join("crates/cuerithm-media/tests/fixtures/covered.flac"),
            &cache,
            &token,
        );
        assert!(result.is_err());
        assert!(!cache.exists());
    }
}
