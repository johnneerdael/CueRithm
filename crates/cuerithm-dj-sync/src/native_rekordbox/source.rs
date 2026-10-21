//! Measured native playback compatibility and explicitly requested owned copies.
use crate::{DjSoftware, SyncTrack, pioneer_usb::compatibility::write_playable_pcm_wav, timing};
use anyhow::{Context, Result, bail, ensure};
use cuerithm_media::{
    CancellationToken, CodecTimingProfile, MediaInfo, MediaRuntime, PreparedAudio,
    SourceExportTiming, file_sha256_cancellable,
};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NativeOriginalSourceProof {
    pub path: PathBuf,
    pub sha256: String,
    pub size_bytes: u64,
    pub pcm_sha256: String,
    pub playable_frames: u64,
    pub playable_sample_rate: u32,
    pub export_timing: SourceExportTiming,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NativeSourceCompatibility {
    pub native_file_type: Option<i64>,
    pub reason: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NativeSourceInspection {
    pub compatibility: NativeSourceCompatibility,
    pub original: NativeOriginalSourceProof,
    pub duration_seconds: f64,
    pub estimated_copy_bytes: u64,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NativeSourceDescriptor {
    pub native_path: PathBuf,
    pub native_file_type: i64,
    pub sample_rate: u32,
    pub bits_per_sample: u32,
    pub bitrate_kbps: Option<f64>,
    pub duration_seconds: f64,
    pub size_bytes: u64,
    pub sha256: String,
    /// Float32 stereo playable PCM, after any explicit PCM16 copy quantization.
    pub pcm_sha256: String,
    pub export_timing: SourceExportTiming,
    pub converted: bool,
    pub original: NativeOriginalSourceProof,
}
pub struct PreparedNativeSource {
    pub descriptor: NativeSourceDescriptor,
    _owned: tempfile::TempDir,
}
/// Native numeric IDs come from the observed direct writer. Playback eligibility
/// additionally follows the official rekordbox 7 manual, pages 253–254:
/// https://cdn.rekordbox.com/files/20260409151936/rekordbox7.214_manual_EN.pdf
/// A known numeric ID alone does not make raw ADTS or other unsupported audio
/// playable. Actual codec and container observations take precedence over names.
pub fn native_rekordbox_file_type(info: &MediaInfo, path: &Path) -> Result<i64> {
    ensure!(
        (1..=2).contains(&info.channels),
        "Rekordbox needs mono or stereo audio; enable compatible audio copies"
    );
    let extension = path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    let codec = info.codec.to_ascii_lowercase();
    let container = info
        .properties
        .get("format.format_name")
        .and_then(serde_json::Value::as_str);
    let container_is = |expected: &[&str]| {
        container.is_none_or(|actual| actual.split(',').any(|name| expected.contains(&name)))
    };
    let pcm_rates = [44100, 48000, 88200, 96000, 192000];
    match codec.as_str() {
        "mp3" if extension == "mp3" && container_is(&["mp3"]) => {
            ensure!(
                [16000, 22050, 24000, 32000, 44100, 48000].contains(&info.sample_rate),
                "MP3 sample rate is unsupported by rekordbox; enable compatible audio copies"
            );
            Ok(1)
        }
        "aac"
            if matches!(extension.as_str(), "m4a" | "mp4")
                && container_is(&["mov", "mp4", "m4a"]) =>
        {
            ensure!(
                info.properties
                    .get("audio.profile")
                    .and_then(serde_json::Value::as_str)
                    == Some("LC"),
                "Rekordbox needs AAC-LC; enable compatible audio copies"
            );
            ensure!(
                [16000, 22050, 24000, 32000, 44100, 48000].contains(&info.sample_rate),
                "AAC sample rate is unsupported by rekordbox; enable compatible audio copies"
            );
            Ok(if extension == "mp4" { 3 } else { 4 })
        }
        "alac" if extension == "m4a" && container_is(&["mov", "mp4", "m4a"]) => {
            supported_pcm(info, &pcm_rates)?;
            Ok(6)
        }
        "flac" if matches!(extension.as_str(), "flac" | "fla") && container_is(&["flac"]) => {
            supported_pcm(info, &pcm_rates)?;
            Ok(5)
        }
        "pcm_s16le" | "pcm_s24le" if extension == "wav" && container_is(&["wav"]) => {
            supported_pcm(info, &pcm_rates)?;
            Ok(11)
        }
        "pcm_s16be" | "pcm_s24be"
            if matches!(extension.as_str(), "aif" | "aiff") && container_is(&["aiff"]) =>
        {
            supported_pcm(info, &pcm_rates)?;
            Ok(12)
        }
        _ => bail!(
            "This codec/container layout cannot be played by native rekordbox; enable compatible audio copies to create a separate PCM WAV"
        ),
    }
}
fn value_number(value: &serde_json::Value) -> Option<f64> {
    value
        .as_f64()
        .or_else(|| value.as_str()?.parse().ok())
        .filter(|value| value.is_finite() && *value >= 0.)
}
fn bits(info: &MediaInfo) -> Option<u32> {
    info.bits_per_sample
        .filter(|bits| *bits > 0)
        .or_else(|| {
            info.properties
                .get("audio.bits_per_sample")
                .and_then(value_number)
                .filter(|bits| *bits > 0.)
                .map(|bits| bits as u32)
        })
        .or(match info.codec.as_str() {
            "pcm_s16le" | "pcm_s16be" => Some(16),
            "pcm_s24le" | "pcm_s24be" => Some(24),
            "mp3" | "aac" => Some(16),
            _ => None,
        })
}
fn supported_pcm(info: &MediaInfo, rates: &[u32]) -> Result<()> {
    ensure!(
        rates.contains(&info.sample_rate) && matches!(bits(info), Some(16 | 24)),
        "Rekordbox needs supported 16/24-bit PCM sample rates; enable compatible audio copies"
    );
    Ok(())
}
fn original_proof(
    audio: &PreparedAudio,
    cancel: &CancellationToken,
) -> Result<NativeOriginalSourceProof> {
    Ok(NativeOriginalSourceProof {
        path: audio.source_path.clone(),
        sha256: audio.source_sha256.clone(),
        size_bytes: std::fs::metadata(&audio.source_path)?.len(),
        pcm_sha256: audio.pcm_sha256_cancellable(cancel)?,
        playable_frames: audio.frames,
        playable_sample_rate: audio.clock.sample_rate,
        export_timing: audio
            .export_timing
            .clone()
            .context("Measured source timing is missing")?,
    })
}
fn inspect_audio(
    audio: &PreparedAudio,
    cancel: &CancellationToken,
) -> Result<NativeSourceInspection> {
    cancel.check()?;
    let eligibility = native_rekordbox_file_type(&audio.info, &audio.source_path);
    let (native_file_type, reason) = match eligibility {
        Ok(kind) => (Some(kind), None),
        Err(error) => (None, Some(error.to_string())),
    };
    let estimated_copy_bytes = if native_file_type.is_some() {
        0
    } else {
        audio
            .frames
            .checked_mul(4)
            .and_then(|bytes| bytes.checked_add(44))
            .context("Compatible WAV size overflow")?
    };
    let original = original_proof(audio, cancel)?;
    ensure!(
        audio.source_is_unchanged_cancellable(cancel)?,
        "Audio changed during native compatibility inspection"
    );
    Ok(NativeSourceInspection {
        compatibility: NativeSourceCompatibility {
            native_file_type,
            reason,
        },
        original,
        duration_seconds: audio.clock.duration_seconds(),
        estimated_copy_bytes,
    })
}
pub fn inspect_native_rekordbox_source(
    path: &Path,
    runtime: &MediaRuntime,
    cancel: &CancellationToken,
) -> Result<NativeSourceInspection> {
    cancel.check()?;
    let workspace = tempfile::tempdir()?;
    let audio = runtime.decode_cancellable(path, workspace.path(), cancel)?;
    inspect_audio(&audio, cancel)
}
/// Prepare a supported original or an explicitly requested temporary compatible
/// copy. The caller keeps this owner alive until a coordinated file transaction
/// stages immutable native-owned audio and commits its database reference last.
pub fn prepare_native_rekordbox_source(
    path: &Path,
    workspace: &Path,
    convert_incompatible_audio: bool,
    runtime: &MediaRuntime,
    cancel: &CancellationToken,
) -> Result<PreparedNativeSource> {
    cancel.check()?;
    std::fs::create_dir_all(workspace)?;
    let metadata = std::fs::symlink_metadata(workspace)?;
    ensure!(
        metadata.is_dir() && !metadata.file_type().is_symlink(),
        "Native compatibility workspace must be a regular directory"
    );
    let owned = tempfile::Builder::new()
        .prefix("cuerithm-native-audio-")
        .tempdir_in(workspace)?;
    let audio = runtime.decode_cancellable(path, owned.path(), cancel)?;
    let inspection = inspect_audio(&audio, cancel)?;
    let converted = inspection.compatibility.native_file_type.is_none();
    ensure!(
        !converted || convert_incompatible_audio,
        "{}",
        inspection
            .compatibility
            .reason
            .as_deref()
            .unwrap_or("Enable compatible audio copies for this source")
    );
    let original = inspection.original;
    let (
        native_path,
        native_file_type,
        sample_rate,
        bits_per_sample,
        bitrate_kbps,
        sha256,
        pcm_sha256,
        export_timing,
    ) = if let Some(kind) = inspection.compatibility.native_file_type {
        let bitrate = audio
            .info
            .properties
            .get("audio.bit_rate")
            .and_then(value_number)
            .or_else(|| {
                audio
                    .info
                    .properties
                    .get("format.bit_rate")
                    .and_then(value_number)
            })
            .map(|rate| rate / 1000.);
        (
            audio.source_path.clone(),
            kind,
            audio.info.sample_rate,
            bits(&audio.info).context("Native source bit depth is not known")?,
            bitrate,
            audio.source_sha256.clone(),
            original.pcm_sha256.clone(),
            original.export_timing.clone(),
        )
    } else {
        let target = owned.path().join("playable.wav");
        let pcm_hash = write_playable_pcm_wav(&audio, &target, cancel)?;
        let hash = file_sha256_cancellable(&target, cancel)?;
        let profile = SourceExportTiming {
            profile: "dj-timeline-v1".into(),
            source_sha256: hash.clone(),
            source_sample_rate: audio.clock.sample_rate,
            codec_profile: CodecTimingProfile::Direct,
            measurement_identity: runtime.export_timing_identity(),
            decoded_sample_count: None,
        };
        (
            target,
            11,
            audio.clock.sample_rate,
            16,
            Some(f64::from(audio.clock.sample_rate) * 32. / 1000.),
            hash,
            pcm_hash,
            profile,
        )
    };
    // Validate timing under the native target before returning a playable source.
    timing::offset_seconds(
        &SyncTrack {
            native_path: None,
            path: native_path.clone(),
            sample_rate,
            export_timing: Some(export_timing.clone()),
            ..Default::default()
        },
        DjSoftware::Rekordbox,
    )?;
    ensure!(
        audio.source_is_unchanged_cancellable(cancel)?,
        "Audio changed while creating its native compatible copy"
    );
    let descriptor = NativeSourceDescriptor {
        size_bytes: std::fs::metadata(&native_path)?.len(),
        native_path,
        native_file_type,
        sample_rate,
        bits_per_sample,
        bitrate_kbps,
        duration_seconds: inspection.duration_seconds,
        sha256,
        pcm_sha256,
        export_timing,
        converted,
        original,
    };
    cancel.check()?;
    Ok(PreparedNativeSource {
        descriptor,
        _owned: owned,
    })
}
