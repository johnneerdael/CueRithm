//! Versioned source measurements for exporting newly prepared playable-PCM times.
//! These fields describe the encoded source; they never change the PCM clock.
use crate::{CancellationToken, MediaInfo, MediaRuntime, MediaTaskError, file_sha256_cancellable};
use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{
    ffi::OsString,
    fs::File,
    io::{Read, Seek, SeekFrom},
    path::Path,
    time::Duration,
};

pub const SOURCE_EXPORT_TIMING_PROFILE: &str = "dj-timeline-v1";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Mp3TimingCase {
    A,
    B,
    C,
    D,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum CodecTimingProfile {
    Direct,
    Mp3 {
        case: Mp3TimingCase,
    },
    IsoAudio {
        extension: String,
        audio_delay_ms: f64,
        video_delay_ms: f64,
    },
    Unmeasured {
        reason: String,
    },
}

/// Measurements bound to one encoded source, for publications on playable PCM.
/// Missing or failed measurements remain explicit rather than implying zero.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SourceExportTiming {
    pub profile: String,
    pub source_sha256: String,
    pub source_sample_rate: u32,
    pub codec_profile: CodecTimingProfile,
    pub measurement_identity: String,
    /// Native MP3 frame clock for device PVBR; distinct from gapless PCM frames.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub decoded_sample_count: Option<u64>,
}

impl MediaRuntime {
    /// Pin both the native scanner implementation and the sealed probe build.
    pub fn export_timing_identity(&self) -> String {
        let mut hash = Sha256::new();
        hash.update(SOURCE_EXPORT_TIMING_PROFILE.as_bytes());
        hash.update(include_bytes!("export_timing.rs"));
        hash.update(self.identity.as_bytes());
        format!("{:x}", hash.finalize())
    }

    pub fn export_timing(
        &self,
        path: &Path,
        source_sha256: &str,
        info: &MediaInfo,
    ) -> Result<SourceExportTiming> {
        self.export_timing_cancellable(path, source_sha256, info, &CancellationToken::default())
    }

    pub fn export_timing_cancellable(
        &self,
        path: &Path,
        source_sha256: &str,
        info: &MediaInfo,
        cancel: &CancellationToken,
    ) -> Result<SourceExportTiming> {
        let extension = path
            .extension()
            .and_then(|s| s.to_str())
            .unwrap_or_default()
            .to_ascii_lowercase();
        self.export_timing_with_extension_cancellable(path, source_sha256, info, &extension, cancel)
    }

    /// Private snapshots can be extensionless; retain the original container hint.
    pub(crate) fn export_timing_with_extension_cancellable(
        &self,
        path: &Path,
        source_sha256: &str,
        info: &MediaInfo,
        extension: &str,
        cancel: &CancellationToken,
    ) -> Result<SourceExportTiming> {
        cancel.check()?;
        let path = path.canonicalize().context("Timing source is missing")?;
        let path = path.as_path();
        ensure!(
            source_sha256.len() == 64 && source_sha256.bytes().all(|b| b.is_ascii_hexdigit()),
            "Invalid timing source hash"
        );
        ensure!(info.sample_rate > 0, "Timing source sample rate is missing");
        ensure!(
            file_sha256_cancellable(path, cancel)? == source_sha256,
            "Timing source changed before measurement"
        );
        let extension = extension.to_ascii_lowercase();
        let mut decoded_sample_count = None;
        let codec_profile = if matches!(extension.as_str(), "m4a" | "mp4") {
            let audio = self.timing_probe(
                path,
                &[
                    "-select_streams",
                    "a:0",
                    "-show_entries",
                    "stream=start_time,sample_rate",
                    "-show_entries",
                    "packet=pts_time",
                    "-read_intervals",
                    "%+0.01",
                ],
                cancel,
            )?;
            let video = self.timing_probe(
                path,
                &["-select_streams", "v", "-show_entries", "stream=codec_name"],
                cancel,
            )?;
            iso_profile(&extension, &audio, &video, info.sample_rate)?
        } else if info.codec.eq_ignore_ascii_case("mp3") || extension == "mp3" {
            if extension != "mp3" {
                unmeasured("MP3 source extension is not mp3")
            } else {
                let (case, count) = classify_mp3(path, cancel)?;
                decoded_sample_count = count;
                CodecTimingProfile::Mp3 { case }
            }
        } else if info.codec.trim().is_empty() || info.codec.eq_ignore_ascii_case("unknown") {
            unmeasured("Source audio codec is unknown")
        } else {
            CodecTimingProfile::Direct
        };
        ensure!(
            file_sha256_cancellable(path, cancel)? == source_sha256,
            "Timing source changed during measurement"
        );
        Ok(SourceExportTiming {
            profile: SOURCE_EXPORT_TIMING_PROFILE.into(),
            source_sha256: source_sha256.into(),
            source_sample_rate: info.sample_rate,
            codec_profile,
            measurement_identity: self.export_timing_identity(),
            decoded_sample_count,
        })
    }

    pub(crate) fn capture_export_timing_cancellable(
        &self,
        path: &Path,
        source_sha256: &str,
        info: &MediaInfo,
        extension: &str,
        cancel: &CancellationToken,
    ) -> Result<SourceExportTiming> {
        match self.export_timing_with_extension_cancellable(
            path,
            source_sha256,
            info,
            extension,
            cancel,
        ) {
            Ok(profile) => Ok(profile),
            Err(error)
                if error.downcast_ref::<MediaTaskError>() == Some(&MediaTaskError::Cancelled) =>
            {
                Err(error)
            }
            Err(error) => {
                cancel.check()?;
                ensure!(
                    file_sha256_cancellable(path, cancel)? == source_sha256,
                    "Timing source changed during capture"
                );
                let reason = match error.downcast_ref::<MediaTaskError>() {
                    Some(MediaTaskError::TimedOut) => "Source timing probe timed out",
                    Some(MediaTaskError::OutputLimit) => {
                        "Source timing probe exceeded its output limit"
                    }
                    _ => "Source timing measurement failed",
                };
                Ok(SourceExportTiming {
                    profile: SOURCE_EXPORT_TIMING_PROFILE.into(),
                    source_sha256: source_sha256.into(),
                    source_sample_rate: info.sample_rate,
                    codec_profile: unmeasured(reason),
                    measurement_identity: self.export_timing_identity(),
                    decoded_sample_count: None,
                })
            }
        }
    }

    fn timing_probe(
        &self,
        path: &Path,
        query: &[&str],
        cancel: &CancellationToken,
    ) -> Result<Value> {
        let mut args: Vec<OsString> = [
            "-v",
            "error",
            "-protocol_whitelist",
            "file,pipe",
            "-print_format",
            "json",
        ]
        .into_iter()
        .map(Into::into)
        .collect();
        args.extend(query.iter().map(OsString::from));
        args.push(path.as_os_str().to_owned());
        let mut output = self.run_controlled(
            &self.ffprobe,
            &args,
            tempfile::tempfile()?,
            1_048_576,
            Duration::from_secs(10),
            cancel,
        )?;
        let mut bytes = Vec::new();
        output.read_to_end(&mut bytes)?;
        cancel.check()?;
        serde_json::from_slice(&bytes).context("Read source timing probe output")
    }
}

fn unmeasured(reason: &str) -> CodecTimingProfile {
    CodecTimingProfile::Unmeasured {
        reason: reason.into(),
    }
}
fn number(value: &Value) -> Option<f64> {
    value
        .as_f64()
        .or_else(|| value.as_str()?.parse().ok())
        .filter(|n| n.is_finite())
}

fn iso_profile(
    extension: &str,
    audio: &Value,
    video: &Value,
    source_rate: u32,
) -> Result<CodecTimingProfile> {
    let Some(stream) = audio["streams"].as_array().and_then(|s| s.first()) else {
        return Ok(unmeasured("ISO audio stream timing is unavailable"));
    };
    let Some(rate) = number(&stream["sample_rate"])
        .filter(|n| *n > 0. && n.fract() == 0. && *n <= u32::MAX as f64)
    else {
        return Ok(unmeasured("ISO source sample rate is unavailable"));
    };
    ensure!(
        rate as u32 == source_rate,
        "Timing probe source sample rate differs from media metadata"
    );
    let Some(pts) = audio["packets"]
        .as_array()
        .and_then(|p| p.first())
        .and_then(|p| number(&p["pts_time"]))
    else {
        return Ok(unmeasured("ISO first audio packet timing is unavailable"));
    };
    let delay = if pts < 0. {
        pts.abs()
    } else {
        let Some(start) = number(&stream["start_time"]) else {
            return Ok(unmeasured("ISO audio stream start is unavailable"));
        };
        start
    };
    let Some(streams) = video["streams"].as_array() else {
        return Ok(unmeasured("ISO video stream timing is unavailable"));
    };
    let has_video = streams
        .iter()
        .any(|s| !matches!(s["codec_name"].as_str(), Some("mjpeg" | "png" | "bmp")));
    let audio_delay_ms = (delay * 10_000.).round() / 10.;
    ensure!(
        audio_delay_ms.is_finite(),
        "ISO audio delay exceeds measurement range"
    );
    Ok(CodecTimingProfile::IsoAudio {
        extension: extension.into(),
        audio_delay_ms,
        video_delay_ms: if has_video {
            (2048. / rate * 10_000.).round() / 10.
        } else {
            0.
        },
    })
}

/// Bounded ID3/Xing/LAME compatibility heuristic, independent of PCM decoding.
fn classify_mp3(path: &Path, cancel: &CancellationToken) -> Result<(Mp3TimingCase, Option<u64>)> {
    use Mp3TimingCase::{A, B, C, D};
    let mut file = File::open(path)?;
    let mut header = [0_u8; 10];
    let mut prefix = Vec::new();
    file.by_ref().take(10).read_to_end(&mut prefix)?;
    header[..prefix.len()].copy_from_slice(&prefix);
    let skip = if &header[..3] == b"ID3" {
        10 + ((u64::from(header[6]) << 21)
            | (u64::from(header[7]) << 14)
            | (u64::from(header[8]) << 7)
            | u64::from(header[9]))
    } else {
        0
    };
    file.seek(SeekFrom::Start(skip))?;
    cancel.check()?;
    let mut bytes = Vec::with_capacity(2200);
    file.take(2200).read_to_end(&mut bytes)?;
    bytes.resize(2200, 0);
    let Some(first) = (0..bytes.len() - 4).find(|&i| bytes[i] == 255 && bytes[i + 1] & 224 == 224)
    else {
        return Ok((A, None));
    };
    let header = be32(&bytes[first..first + 4]);
    let mode = (header >> 6) & 3;
    let marker = first + 4 + if mode == 3 { 17 } else { 32 };
    if marker + 4 >= bytes.len() || !matches!(&bytes[marker..marker + 4], b"Xing" | b"Info") {
        return Ok((A, None));
    }
    if first + 165 > bytes.len() {
        return Ok((B, decoded_sample_count_from_window(&bytes, B)));
    }
    let mut case = if &bytes[first + 156..first + 160] == b"LAME" {
        if first + 192 > bytes.len() {
            C
        } else {
            let stored = u16::from_be_bytes(bytes[first + 190..first + 192].try_into()?);
            if stored != 0 && stored == crc16_arc(&bytes[first..first + 190]) {
                D
            } else {
                C
            }
        }
    } else {
        B
    };
    let version = (header >> 19) & 3;
    let bitrate = if version == 3 {
        [
            0, 32, 40, 48, 56, 64, 80, 96, 112, 128, 160, 192, 224, 256, 320, 0,
        ][((header >> 12) & 15) as usize]
            * 1000
    } else {
        0
    };
    let rates: &[u32] = match version {
        3 => &[44100, 48000, 32000],
        2 => &[22050, 24000, 16000],
        0 => &[11025, 12000, 8000],
        _ => &[44100],
    };
    let rate = rates
        .get(((header >> 10) & 3) as usize)
        .copied()
        .unwrap_or(44100);
    if bitrate > 0 {
        let next = first + (144 * bitrate / rate) as usize + ((header >> 9) & 1) as usize;
        if next + 4 <= bytes.len() && bytes[next] == 255 && bytes[next + 1] & 224 == 224 {
            let same = mode == ((be32(&bytes[next..next + 4]) >> 6) & 3);
            if case == B && same {
                case = D;
            } else if case == D && !same {
                case = C;
            }
        }
    }
    cancel.check()?;
    Ok((case, decoded_sample_count_from_window(&bytes, case)))
}
fn be32(bytes: &[u8]) -> u32 {
    u32::from_be_bytes(bytes.try_into().expect("four bytes"))
}
fn crc16_arc(bytes: &[u8]) -> u16 {
    let mut crc = 0_u16;
    for byte in bytes {
        crc ^= u16::from(*byte);
        for _ in 0..8 {
            crc = if crc & 1 != 0 {
                (crc >> 1) ^ 0xa001
            } else {
                crc >> 1
            };
        }
    }
    crc
}

fn decoded_sample_count_from_window(bytes: &[u8], case: Mp3TimingCase) -> Option<u64> {
    if case == Mp3TimingCase::A {
        return None;
    }
    let first = (0..bytes.len().saturating_sub(4))
        .find(|&i| bytes[i] == 255 && bytes[i + 1] & 224 == 224)?;
    let header = be32(bytes.get(first..first + 4)?);
    let version = (header >> 19) & 3;
    let mode = (header >> 6) & 3;
    let samples_per_frame = if version == 3 { 1152 } else { 576 };
    let side_info = if version == 3 {
        if mode == 3 { 17 } else { 32 }
    } else if mode == 3 {
        9
    } else {
        17
    };
    let marker = first + 4 + side_info;
    if !matches!(bytes.get(marker..marker + 4), Some(b"Xing" | b"Info")) {
        return None;
    }
    let flags = be32(bytes.get(marker + 4..marker + 8)?);
    if flags & 1 == 0 {
        return None;
    }
    let frames = u64::from(be32(bytes.get(marker + 8..marker + 12)?));
    if frames == 0 {
        return None;
    }
    Some((frames + u64::from(case == Mp3TimingCase::D)) * samples_per_frame)
}

#[cfg(test)]
mod tests;
