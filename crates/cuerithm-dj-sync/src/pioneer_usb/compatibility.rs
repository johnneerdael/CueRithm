use crate::SyncTrack;
use anyhow::{Context, Result, ensure};
use cuerithm_media::MediaRuntime;
use lofty::{file::AudioFile, probe::Probe};
use sha2::{Digest, Sha256};
use std::{
    fs::File,
    io::{BufWriter, Read, Seek, SeekFrom, Write},
    path::Path,
};
pub(super) struct Issue {
    pub message: String,
    pub fixable: bool,
}
pub(super) fn inspect(track: &SyncTrack, runtime: Option<&MediaRuntime>) -> Result<Vec<Issue>> {
    let extension = track
        .path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or_default()
        .to_lowercase();
    let (codec, rate, channels) = if let Some(runtime) = runtime {
        let info = runtime.probe(&track.path)?;
        (info.codec, info.sample_rate, info.channels)
    } else {
        let tagged = Probe::open(&track.path)?
            .read()
            .context("Inspect CDJ audio compatibility")?;
        let properties = tagged.properties();
        (
            extension.clone(),
            properties.sample_rate().unwrap_or(track.sample_rate),
            properties.channels().unwrap_or(2) as u32,
        )
    };
    let mut issues = Vec::new();
    let mut add = |message: &str, fixable: bool| {
        issues.push(Issue {
            message: message.into(),
            fixable,
        })
    };
    if rate > 48000 {
        add("sample rate exceeds 48 kHz", true);
    } else if rate < 32000 {
        add("sample rate is below 32 kHz", true);
    }
    if channels > 2 {
        add("audio has more than two channels", true);
    }
    if !matches!(
        extension.as_str(),
        "mp3" | "m4a" | "flac" | "wav" | "aif" | "aiff"
    ) {
        add(
            "audio format is unsupported by native Pioneer USB libraries",
            true,
        );
    }
    if codec == "flac" || codec == "alac" {
        add("FLAC/ALAC playback depends on the CDJ model", false);
    }
    if matches!(codec.as_str(), "pcm_f32le" | "pcm_f64le") {
        add("floating-point WAV requires PCM conversion", true);
    }
    if extension == "wav" {
        let mut file = File::open(&track.path)?;
        let mut header = [0; 12];
        file.read_exact(&mut header)?;
        if &header[..4] == b"RIFF" && &header[8..] == b"WAVE" {
            let mut offset = 12_u64;
            while offset < 1_048_576 {
                file.seek(SeekFrom::Start(offset))?;
                let mut chunk = [0; 8];
                if file.read_exact(&mut chunk).is_err() {
                    break;
                }
                let length = u32::from_le_bytes(chunk[4..].try_into()?) as u64;
                if &chunk[..4] == b"fmt " {
                    let mut fmt = [0; 2];
                    file.read_exact(&mut fmt)?;
                    match u16::from_le_bytes(fmt) {
                        0xfffe => add("WAV uses an extensible header", true),
                        3 if !issues.iter().any(|i| i.message.contains("floating-point")) => {
                            issues.push(Issue {
                                message: "floating-point WAV requires PCM conversion".into(),
                                fixable: true,
                            });
                        }
                        _ => {}
                    }
                    break;
                }
                offset = offset
                    .checked_add(8 + length + length % 2)
                    .context("Invalid WAV chunk size")?;
            }
        }
    }
    if matches!(extension.as_str(), "aif" | "aiff")
        && !matches!(
            codec.as_str(),
            "pcm_s16be" | "pcm_s24be" | "pcm_s32be" | "pcm_s8" | "aif" | "aiff"
        )
    {
        issues.push(Issue {
            message: "compressed AIFF requires PCM conversion".into(),
            fixable: true,
        });
    }
    if extension == "mp3" {
        issues.extend(mp3(&track.path)?);
    }
    Ok(issues)
}
fn mp3(path: &Path) -> Result<Vec<Issue>> {
    let mut file = File::open(path)?;
    let mut id3 = [0; 10];
    if file.read_exact(&mut id3).is_err() {
        return Ok(Vec::new());
    }
    let offset = if &id3[..3] == b"ID3" {
        10 + id3[6..10]
            .iter()
            .fold(0_u64, |n, b| (n << 7) | u64::from(*b & 0x7f))
    } else {
        0
    };
    file.seek(SeekFrom::Start(offset))?;
    let mut bytes = [0; 2200];
    let len = file.read(&mut bytes)?;
    let Some(first) = bytes[..len]
        .windows(2)
        .position(|b| b[0] == 255 && b[1] & 224 == 224)
    else {
        return Ok(Vec::new());
    };
    let Some(raw) = bytes.get(first..first + 4) else {
        return Ok(Vec::new());
    };
    let header = u32::from_be_bytes(raw.try_into()?);
    let xing = first + 4 + if header >> 6 & 3 == 3 { 17 } else { 32 };
    let marker = bytes.get(xing..xing + 4).unwrap_or_default();
    if marker == b"Xing" {
        return Ok(vec![Issue {
            message: "VBR MP3 may seek slowly on some CDJs".into(),
            fixable: false,
        }]);
    }
    if marker == b"Info" || bytes.get(first + 36..first + 40) == Some(b"VBRI") {
        return Ok(Vec::new());
    }
    const RATES: [u32; 16] = [
        0, 32, 40, 48, 56, 64, 80, 96, 112, 128, 160, 192, 224, 256, 320, 0,
    ];
    let index = (header >> 12 & 15) as usize;
    let bitrate = RATES[index];
    if bitrate == 0 {
        return Ok(Vec::new());
    }
    let sample_rates = match header >> 19 & 3 {
        3 => [44100, 48000, 32000],
        2 => [22050, 24000, 16000],
        _ => [11025, 12000, 8000],
    };
    let rate = sample_rates
        .get((header >> 10 & 3) as usize)
        .copied()
        .unwrap_or(44100);
    let mut position =
        offset + first as u64 + u64::from(144 * bitrate * 1000 / rate + (header >> 9 & 1));
    for _ in 0..10 {
        file.seek(SeekFrom::Start(position))?;
        let mut b = [0; 4];
        if file.read_exact(&mut b).is_err() || b[0] != 255 || b[1] & 224 != 224 {
            break;
        }
        let h = u32::from_be_bytes(b);
        let next = (h >> 12 & 15) as usize;
        if next != index {
            return Ok(vec![Issue {
                message: "VBR MP3 is missing a Xing/Info seek header".into(),
                fixable: true,
            }]);
        }
        if RATES[next] == 0 {
            break;
        }
        position += u64::from(144 * RATES[next] * 1000 / rate + (h >> 9 & 1));
    }
    Ok(Vec::new())
}
/// Decode the playable timeline once, retaining stereo. Never rewrite source audio.
pub(super) fn fix_copy(runtime: &MediaRuntime, track: &mut SyncTrack, target: &Path) -> Result<()> {
    let cache = tempfile::tempdir()?;
    let audio = runtime.decode(&track.path, cache.path())?;
    ensure!(
        (audio.clock.duration_seconds() - track.duration_seconds).abs() <= 0.05,
        "Compatibility conversion changes the analyzed timeline; analyze the source again before USB sync"
    );
    write_playable_pcm_wav(&audio, target, &Default::default())?;
    ensure!(
        track.native_timeline.is_none(),
        "Transcoding requires canonical cue/grid input"
    );
    track.export_timing = Some(cuerithm_media::SourceExportTiming {
        decoded_sample_count: None,
        profile: "dj-timeline-v1".into(),
        source_sha256: cuerithm_media::file_sha256(target)?,
        source_sample_rate: audio.clock.sample_rate,
        codec_profile: cuerithm_media::CodecTimingProfile::Direct,
        measurement_identity: runtime.export_timing_identity(),
    });
    track.sample_rate = audio.clock.sample_rate;
    track.duration_seconds = audio.clock.duration_seconds();
    // The staged copy is stereo PCM16; its bitrate is independent of the source.
    track.attributes.insert(
        "bitrate".into(),
        cuerithm_smartlists::FieldValue::Number(f64::from(audio.clock.sample_rate) * 32. / 1000.),
    );
    Ok(())
}

/// Write the existing stereo PCM16 compatibility format from playable PCM, and
/// return the resulting normalized float-PCM hash. Callers own the output path.
pub(crate) fn write_playable_pcm_wav(
    audio: &cuerithm_media::PreparedAudio,
    target: &Path,
    cancel: &cuerithm_media::CancellationToken,
) -> Result<String> {
    cancel.check()?;
    let payload = audio.frames.checked_mul(4).context("WAV size overflow")?;
    ensure!(
        payload + 36 <= u32::MAX as u64,
        "Converted WAV exceeds the RIFF size limit"
    );
    let mut file = BufWriter::new(File::create(target)?);
    file.write_all(b"RIFF")?;
    file.write_all(&((payload + 36) as u32).to_le_bytes())?;
    file.write_all(b"WAVEfmt ")?;
    file.write_all(&16_u32.to_le_bytes())?;
    file.write_all(&1_u16.to_le_bytes())?;
    file.write_all(&2_u16.to_le_bytes())?;
    file.write_all(&audio.clock.sample_rate.to_le_bytes())?;
    file.write_all(&(audio.clock.sample_rate * 4).to_le_bytes())?;
    file.write_all(&4_u16.to_le_bytes())?;
    file.write_all(&16_u16.to_le_bytes())?;
    file.write_all(b"data")?;
    file.write_all(&(payload as u32).to_le_bytes())?;
    let mut position = 0;
    let mut pcm_hash = Sha256::new();
    while position < audio.frames {
        cancel.check()?;
        let samples = audio.read_frames(position, 65536)?;
        for sample in &samples {
            let value = (sample.clamp(-1.0, 32767.0 / 32768.0) * 32768.0).round() as i16;
            file.write_all(&value.to_le_bytes())?;
            pcm_hash.update((f32::from(value) / 32768.).to_le_bytes());
        }
        position += samples.len() as u64 / 2;
    }
    file.flush()?;
    file.get_ref().sync_all()?;
    ensure!(
        audio.source_is_unchanged_cancellable(cancel)?,
        "Source changed during compatibility conversion"
    );
    cancel.check()?;
    Ok(format!("{:x}", pcm_hash.finalize()))
}
