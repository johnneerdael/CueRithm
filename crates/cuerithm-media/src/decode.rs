use crate::{AudioClock, MediaInfo, MediaRuntime, runtime::sha256};
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
}
impl PreparedAudio {
    pub fn source_is_unchanged(&self) -> Result<bool> {
        if !self.source_path.is_file() {
            return Ok(false);
        }
        Ok(sha256(&self.source_path)? == self.source_sha256)
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
            .chunks_exact(4)
            .map(|b| f32::from_le_bytes(b.try_into().unwrap()))
            .collect())
    }
    pub fn mono_samples(&self) -> Result<Vec<f32>> {
        let mut output = Vec::with_capacity(self.frames as usize);
        let mut pos = 0;
        while pos < self.frames {
            let samples = self.read_frames(pos, 65536)?;
            output.extend(samples.chunks_exact(2).map(|s| (s[0] + s[1]) * 0.5));
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
        std::fs::create_dir_all(cache)?;
        let private = tempfile::Builder::new()
            .prefix("cuerithm-")
            .tempdir_in(cache)?;
        let source_path = path.canonicalize().context("Audio file is missing")?;
        let before = sha256(&source_path)?;
        let mut snapshot_path = private.path().join("source");
        std::fs::copy(&source_path, &snapshot_path)?;
        ensure!(
            sha256(&snapshot_path)? == before && sha256(&source_path)? == before,
            "Source changed during capture"
        );
        let info = self.probe(&snapshot_path)?;
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
        let mut pcm = self.run(&self.ffmpeg, &args, file, 64_000_000 * 8)?;
        let bytes = pcm.metadata()?.len();
        ensure!(
            bytes > 0 && bytes.is_multiple_of(8),
            "Incomplete or empty PCM output"
        );
        let frames = bytes / 8;
        let mut chunk = [0u8; 65536];
        loop {
            let n = pcm.read(&mut chunk)?;
            if n == 0 {
                break;
            }
            ensure!(n.is_multiple_of(4), "Unaligned PCM");
            ensure!(
                chunk[..n]
                    .chunks_exact(4)
                    .all(|b| f32::from_le_bytes(b.try_into().unwrap()).is_finite()),
                "Non-finite PCM"
            );
        }
        ensure!(
            sha256(&source_path)? == before,
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
        })
    }
}
