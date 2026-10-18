use crate::{CancellationToken, MediaRuntime};
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AudioFingerprint {
    pub source_sha256: String,
    pub decoder_identity: String,
    pub algorithm: String,
    pub digest: String,
    pub sample_rate: u32,
    pub channels: u32,
    /// Zero-based ordinal amongst audio streams, matching `0:a:N`.
    pub audio_stream_index: u32,
    pub audio_stream_count: u32,
    pub pcm_format: String,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", content = "fingerprint", rename_all = "snake_case")]
pub enum FingerprintOutcome {
    Audio(AudioFingerprint),
    Video,
    NoAudio,
    Symlink,
}

use anyhow::{Context, ensure};
use sha2::{Digest, Sha256};
use std::{
    ffi::OsString,
    fs::File,
    io::{Read, Write},
    time::Duration,
};

pub(crate) fn cancellable_sha256(path: &Path, cancel: &CancellationToken) -> Result<String> {
    let mut file = File::open(path)?;
    let mut hash = Sha256::new();
    let mut bytes = [0; 65536];
    loop {
        cancel.check()?;
        let n = file.read(&mut bytes)?;
        if n == 0 {
            break;
        }
        hash.update(&bytes[..n]);
    }
    cancel.check()?;
    Ok(format!("{:x}", hash.finalize()))
}
fn copy_snapshot(source: &Path, target: &Path, cancel: &CancellationToken) -> Result<()> {
    let mut input = File::open(source)?;
    let mut output = File::create(target)?;
    let mut bytes = [0; 65536];
    loop {
        cancel.check()?;
        let n = input.read(&mut bytes)?;
        if n == 0 {
            break;
        }
        output.write_all(&bytes[..n])?;
    }
    cancel.check()?;
    Ok(())
}

impl MediaRuntime {
    /// Hash complete native-rate/channel signed-16 PCM using the reference
    /// worker's MD5 format. This does not use the stereo/resampled playback cache.
    pub fn fingerprint(
        &self,
        path: &Path,
        cache: &Path,
        cancel: &CancellationToken,
    ) -> Result<FingerprintOutcome> {
        cancel.check()?;
        let entry = std::fs::symlink_metadata(path)?;
        if entry.file_type().is_symlink() {
            return Ok(FingerprintOutcome::Symlink);
        }
        ensure!(
            entry.is_file(),
            "Fingerprint source must be a local regular file"
        );
        let source = path.canonicalize()?;
        let before = cancellable_sha256(&source, cancel)?;
        std::fs::create_dir_all(cache)?;
        let private = tempfile::Builder::new()
            .prefix("cuerithm-fingerprint-")
            .tempdir_in(cache)?;
        let mut snapshot = private.path().join("source");
        copy_snapshot(&source, &snapshot, cancel)?;
        ensure!(
            cancellable_sha256(&snapshot, cancel)? == before
                && cancellable_sha256(&source, cancel)? == before,
            "Source changed during fingerprint capture"
        );
        let mut probe_args: Vec<OsString> = [
            "-v",
            "error",
            "-protocol_whitelist",
            "file,pipe",
            "-show_entries",
            "stream=codec_type,codec_name,sample_rate,channels:stream_disposition=attached_pic,default",
            "-of",
            "json",
        ]
        .into_iter()
        .map(Into::into)
        .collect();
        probe_args.push(snapshot.as_os_str().to_owned());
        let mut output = self.run_controlled(
            &self.ffprobe,
            &probe_args,
            tempfile::tempfile()?,
            1_048_576,
            Duration::from_secs(30),
            cancel,
        )?;
        let mut bytes = Vec::new();
        output.read_to_end(&mut bytes)?;
        let value: serde_json::Value = serde_json::from_slice(&bytes)?;
        let streams = value["streams"]
            .as_array()
            .context("Probe did not return media streams")?;
        let outcome = if streams.iter().any(|s| {
            s["codec_type"] == "video"
                && s["disposition"]["attached_pic"] != 1
                && !matches!(s["codec_name"].as_str(), Some("mjpeg" | "ansi"))
        }) {
            FingerprintOutcome::Video
        } else {
            // The default disposition takes priority over channel count;
            // equal priorities keep the first audio stream.
            let audio: Vec<_> = streams
                .iter()
                .filter(|s| s["codec_type"] == "audio")
                .collect();
            let mut selected = None;
            let mut best_score = 0;
            for (i, stream) in audio.iter().enumerate() {
                let count = stream["channels"].as_u64().unwrap_or(0);
                let score = count.saturating_add(if stream["disposition"]["default"] == 1 {
                    5_000_000
                } else {
                    0
                });
                if selected.is_none() || score > best_score {
                    selected = Some((i, *stream));
                    best_score = score;
                }
            }
            if let Some((index, stream)) = selected {
                let sample_rate: u32 = stream["sample_rate"]
                    .as_str()
                    .context("Missing audio sample rate")?
                    .parse()?;
                let channels = u32::try_from(stream["channels"].as_u64().unwrap_or(0))?;
                ensure!(
                    (8000..=384000).contains(&sample_rate) && (1..=32).contains(&channels),
                    "Unsupported fingerprint sample rate/channel layout"
                );
                if stream["codec_name"] == "flac" {
                    let named = private.path().join("source.flac");
                    std::fs::rename(&snapshot, &named)?;
                    snapshot = named;
                }
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
                args.push(snapshot.as_os_str().to_owned());
                args.extend([
                    "-map".into(),
                    format!("0:a:{index}").into(),
                    "-vn".into(),
                    "-sn".into(),
                    "-dn".into(),
                    "-c:a".into(),
                    "pcm_s16le".into(),
                    "-f".into(),
                    "md5".into(),
                    "pipe:1".into(),
                ]);
                let mut result = self.run_controlled(
                    &self.ffmpeg,
                    &args,
                    tempfile::tempfile()?,
                    4096,
                    Duration::from_secs(30),
                    cancel,
                )?;
                let mut digest = String::new();
                result.read_to_string(&mut digest)?;
                let digest = digest
                    .trim()
                    .strip_prefix("MD5=")
                    .context("Decoder did not return an MD5 digest")?;
                ensure!(
                    digest.len() == 32 && digest.bytes().all(|b| b.is_ascii_hexdigit()),
                    "Invalid decoder MD5 digest"
                );
                if digest.eq_ignore_ascii_case("d41d8cd98f00b204e9800998ecf8427e") {
                    FingerprintOutcome::NoAudio
                } else {
                    FingerprintOutcome::Audio(AudioFingerprint {
                        source_sha256: before.clone(),
                        decoder_identity: self.identity.clone(),
                        algorithm: "native-s16le-md5-v1".into(),
                        digest: digest.to_lowercase(),
                        sample_rate,
                        channels,
                        audio_stream_index: u32::try_from(index)?,
                        audio_stream_count: u32::try_from(audio.len())?,
                        pcm_format: "s16le".into(),
                    })
                }
            } else {
                FingerprintOutcome::NoAudio
            }
        };
        verify_source(path, &source, &snapshot, &before, cancel)?;
        Ok(outcome)
    }
}

fn verify_source(
    original: &Path,
    canonical: &Path,
    snapshot: &Path,
    expected: &str,
    cancel: &CancellationToken,
) -> Result<()> {
    cancel.check()?;
    ensure!(
        !std::fs::symlink_metadata(original)?
            .file_type()
            .is_symlink()
            && original.canonicalize()? == canonical,
        "Source path changed during fingerprint scan"
    );
    ensure!(
        cancellable_sha256(snapshot, cancel)? == expected
            && cancellable_sha256(canonical, cancel)? == expected,
        "Source changed during fingerprint scan"
    );
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn capture_validation_rejects_source_or_snapshot_changes_even_at_the_same_size() {
        let d = tempfile::tempdir().unwrap();
        let original = d.path().join("source");
        let snapshot = d.path().join("snapshot");
        std::fs::write(&original, b"original").unwrap();
        std::fs::copy(&original, &snapshot).unwrap();
        let token = CancellationToken::default();
        let expected = cancellable_sha256(&original, &token).unwrap();
        let canonical = original.canonicalize().unwrap();
        verify_source(&original, &canonical, &snapshot, &expected, &token).unwrap();
        std::fs::write(&original, b"modified").unwrap();
        assert!(verify_source(&original, &canonical, &snapshot, &expected, &token).is_err());
        std::fs::write(&original, b"original").unwrap();
        std::fs::write(&snapshot, b"modified").unwrap();
        assert!(verify_source(&original, &canonical, &snapshot, &expected, &token).is_err());
    }
    #[cfg(unix)]
    #[test]
    fn capture_validation_rejects_a_leaf_replaced_with_a_symlink_to_identical_bytes() {
        let d = tempfile::tempdir().unwrap();
        let original = d.path().join("source");
        let snapshot = d.path().join("snapshot");
        let moved = d.path().join("moved");
        std::fs::write(&original, b"original").unwrap();
        std::fs::copy(&original, &snapshot).unwrap();
        let token = CancellationToken::default();
        let expected = cancellable_sha256(&original, &token).unwrap();
        let canonical = original.canonicalize().unwrap();
        std::fs::rename(&original, &moved).unwrap();
        std::os::unix::fs::symlink(&moved, &original).unwrap();
        assert!(verify_source(&original, &canonical, &snapshot, &expected, &token).is_err());
    }
}
