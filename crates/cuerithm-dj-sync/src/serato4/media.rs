use crate::model::{BeatgridLock, SyncOptions, SyncTrack};
use anyhow::{Context, Result, ensure};
use base64::{Engine, engine::general_purpose::STANDARD};
use lofty::{
    TextEncoding,
    config::{ParseOptions, WriteOptions},
    file::{AudioFile, FileType, TaggedFileExt},
    id3::v2::{BinaryFrame, Frame, FrameId, GeneralEncapsulatedObject, Id3v2Tag},
    mp4::{Atom, AtomData, AtomIdent, Ilst},
    prelude::TagExt,
    probe::Probe,
};
use std::{borrow::Cow, path::Path};

const MARKERS: &str = "Serato Markers2";
const GRID: &str = "Serato BeatGrid";

pub(super) fn validate(track: &SyncTrack, options: &SyncOptions) -> Result<()> {
    ensure!(
        track.path.is_file(),
        "Serato audio file is missing: {}",
        track.path.display()
    );
    let tagged = Probe::open(&track.path)?.guess_file_type()?.read()?;
    ensure!(
        matches!(
            tagged.file_type(),
            FileType::Mpeg
                | FileType::Wav
                | FileType::Aiff
                | FileType::Mp4
                | FileType::Flac
                | FileType::Vorbis
        ),
        "Serato 4 native tag sync supports MP3, MP4/M4A, FLAC, Ogg Vorbis, WAV and AIFF; unsupported audio: {}",
        track.path.display()
    );
    markers(track, options)?;
    if !options.skip_beatgrids {
        beatgrid(track)?;
        if tagged.file_type() == FileType::Vorbis && !track.beatgrid.is_empty() {
            ogg_beatgrid(track)?;
        }
    }
    Ok(())
}

pub(super) fn write(path: &Path, track: &SyncTrack, options: &SyncOptions) -> Result<()> {
    let tagged = Probe::open(path)?
        .guess_file_type()?
        .read()
        .context("Read staged Serato media tags")?;
    let mut entries = vec![(MARKERS, markers(track, options)?)];
    if !options.skip_beatgrids && !track.beatgrid.is_empty() {
        entries.push((GRID, beatgrid(track)?));
    }
    match tagged.file_type() {
        FileType::Mpeg | FileType::Wav | FileType::Aiff => {
            // Read the native tag directly: a generic Tag round trip can change
            // comment language/description fields and original text encodings.
            let mut input = std::fs::File::open(path)?;
            let mut tag = match tagged.file_type() {
                FileType::Mpeg => {
                    lofty::mpeg::MpegFile::read_from(&mut input, ParseOptions::default())?
                        .id3v2()
                        .cloned()
                }
                FileType::Wav => {
                    lofty::iff::wav::WavFile::read_from(&mut input, ParseOptions::default())?
                        .id3v2()
                        .cloned()
                }
                FileType::Aiff => {
                    lofty::iff::aiff::AiffFile::read_from(&mut input, ParseOptions::default())?
                        .id3v2()
                        .cloned()
                }
                _ => unreachable!(),
            }
            .unwrap_or_else(Id3v2Tag::default);
            tag.retain(|frame| {
                let Frame::Binary(binary) = frame else {
                    return true;
                };
                if frame.id_str() != "GEOB" {
                    return true;
                }
                let Ok(geob) = GeneralEncapsulatedObject::parse(&binary.data, frame.flags()) else {
                    return true;
                };
                !entries
                    .iter()
                    .any(|(name, _)| geob.descriptor.as_deref() == Some(name))
            });
            for (name, data) in entries {
                let geob = GeneralEncapsulatedObject::new(
                    TextEncoding::Latin1,
                    Some("application/octet-stream".into()),
                    None,
                    Some(name.into()),
                    data,
                );
                tag.insert(Frame::Binary(BinaryFrame::new(
                    FrameId::new("GEOB")?,
                    geob.as_bytes(),
                )));
            }
            tag.save_to_path(path, WriteOptions::default())?;
        }
        FileType::Flac => {
            let mut input = std::fs::File::open(path)?;
            let mut file = lofty::flac::FlacFile::read_from(&mut input, ParseOptions::default())?;
            let mut tag = file.vorbis_comments().cloned().unwrap_or_default();
            for (name, data) in entries {
                let key = if name == MARKERS {
                    "SERATO_MARKERS_V2"
                } else {
                    "SERATO_BEATGRID"
                };
                tag.insert(key.into(), STANDARD.encode(envelope(name, &data)));
            }
            file.set_vorbis_comments(tag);
            file.save_to_path(path, WriteOptions::default())?;
        }
        FileType::Vorbis => {
            let mut input = std::fs::File::open(path)?;
            let file = lofty::ogg::VorbisFile::read_from(&mut input, ParseOptions::default())?;
            let mut tag = file.vorbis_comments().clone();
            let markers = markers(track, options)?;
            let end = markers[2..]
                .iter()
                .position(|b| *b == 0)
                .map_or(markers.len(), |i| i + 2);
            tag.insert(
                "SERATO_MARKERS2".into(),
                String::from_utf8(markers[2..end].to_vec())?,
            );
            if !options.skip_beatgrids && !track.beatgrid.is_empty() {
                tag.insert("SERATO_BEATGRID".into(), ogg_beatgrid(track)?);
            }
            tag.save_to_path(path, WriteOptions::default())?;
        }
        FileType::Mp4 => {
            let mut input = std::fs::File::open(path)?;
            let file = lofty::mp4::Mp4File::read_from(&mut input, ParseOptions::default())?;
            let mut tag = file.ilst().cloned().unwrap_or_else(Ilst::default);
            for (name, data) in entries {
                let ident = AtomIdent::Freeform {
                    mean: Cow::Borrowed("com.serato.dj"),
                    name: Cow::Borrowed(if name == MARKERS {
                        "markersv2"
                    } else {
                        "beatgrid"
                    }),
                };
                tag.insert(Atom::new(
                    ident,
                    AtomData::UTF8(STANDARD.encode(envelope(name, &data))),
                ));
            }
            tag.save_to_path(path, WriteOptions::default())?;
        }
        _ => anyhow::bail!("Unsupported Serato media container"),
    }
    Ok(())
}

fn envelope(name: &str, data: &[u8]) -> Vec<u8> {
    let mut result = b"application/octet-stream\0\0".to_vec();
    result.extend(name.as_bytes());
    result.push(0);
    result.extend(data);
    result
}

fn ogg_beatgrid(track: &SyncTrack) -> Result<String> {
    let grid = beatgrid(track)?;
    let count = u32::from_be_bytes(grid[2..6].try_into()?);
    ensure!(
        count == 1,
        "Variable-tempo Serato Ogg Vorbis grid encoding has not been validated; sync would lose timing"
    );
    let first = track
        .beatgrid
        .iter()
        .min_by(|a, b| a.seconds.total_cmp(&b.seconds))
        .context("Missing Serato Ogg grid")?;
    let decimal = |n: f64| {
        if n.fract() == 0. {
            format!("{n:.1}")
        } else {
            n.to_string()
        }
    };
    Ok(format!(
        "000000010000000000000001({},{})",
        decimal(first.seconds),
        decimal(first.bpm)
    ))
}

fn entry(output: &mut Vec<u8>, name: &[u8], payload: &[u8]) -> Result<()> {
    output.extend(name);
    output.push(0);
    output.extend(u32::try_from(payload.len())?.to_be_bytes());
    output.extend(payload);
    Ok(())
}

fn millis(seconds: f64) -> Result<u32> {
    ensure!(
        seconds.is_finite() && seconds >= 0. && seconds * 1000. <= u32::MAX as f64,
        "Serato marker time is outside its unsigned millisecond range"
    );
    Ok((seconds * 1000.).trunc() as u32)
}

pub(super) fn markers(track: &SyncTrack, options: &SyncOptions) -> Result<Vec<u8>> {
    let mut body = vec![1, 1];
    entry(
        &mut body,
        b"COLOR",
        &(track.color.unwrap_or(0xffffff) & 0xffffff).to_be_bytes(),
    )?;
    let mut cue_slots = std::collections::HashSet::new();
    let mut loop_slots = std::collections::HashSet::new();
    for cue in &track.cues {
        ensure!(
            cue.index < 8,
            "Serato supports eight hot cues and eight saved loops"
        );
        ensure!(
            !cue.name.contains('\0'),
            "Serato marker names cannot contain NUL"
        );
        let mut payload = vec![0, cue.index];
        payload.extend(millis(cue.start_seconds)?.to_be_bytes());
        if let Some(end) = cue.end_seconds {
            ensure!(
                end > cue.start_seconds,
                "Serato loop end must follow its start"
            );
            ensure!(
                loop_slots.insert(cue.index),
                "Duplicate Serato saved loop slot"
            );
            payload.extend(millis(end)?.to_be_bytes());
            payload.extend([255; 4]);
            payload.extend(0x0027aae1_u32.to_be_bytes());
            payload.extend([0, 0]); // reserved, unlocked
            payload.extend(cue.name.as_bytes());
            payload.push(0);
            entry(&mut body, b"LOOP", &payload)?;
        } else {
            ensure!(cue_slots.insert(cue.index), "Duplicate Serato hot cue slot");
            payload.extend((cue.color.unwrap_or(0) & 0xffffff).to_be_bytes());
            payload.extend([0, 0]);
            payload.extend(cue.name.as_bytes());
            payload.push(0);
            entry(&mut body, b"CUE", &payload)?;
        }
    }
    let locked = match options.beatgrid_lock {
        BeatgridLock::Lock => true,
        BeatgridLock::Unlock => false,
        BeatgridLock::Default => !track.beatgrid.is_empty(),
    };
    entry(&mut body, b"BPMLOCK", &[u8::from(locked)])?;
    let encoded = STANDARD.encode(body);
    let mut result = vec![1, 1];
    for (i, chunk) in encoded.as_bytes().chunks(72).enumerate() {
        if i > 0 {
            result.push(b'\n');
        }
        result.extend(chunk);
    }
    result.resize(result.len().max(470), 0);
    Ok(result)
}

pub(super) fn beatgrid(track: &SyncTrack) -> Result<Vec<u8>> {
    let mut beats = track.beatgrid.clone();
    beats.sort_by(|a, b| a.seconds.total_cmp(&b.seconds));
    for beat in &beats {
        ensure!(
            beat.seconds.is_finite()
                && (beat.seconds >= 0.
                    || track
                        .native_timeline
                        .as_ref()
                        .is_some_and(|t| t.target == crate::DjSoftware::Serato4))
                && beat.bpm.is_finite()
                && beat.bpm > 0.,
            "Invalid Serato beatgrid marker"
        );
    }
    // SyncBeat already contains sparse segment anchors. Preserve even equal-
    // tempo anchors because they can intentionally reset the bar phase.
    ensure!(
        beats
            .windows(2)
            .all(|pair| pair[0].seconds < pair[1].seconds),
        "Duplicate Serato beatgrid time"
    );
    let anchors = &beats;
    ensure!(
        anchors.len() <= 128,
        "Serato grids support at most 128 tempo anchors"
    );
    let mut output = vec![1, 0];
    output.extend(u32::try_from(anchors.len())?.to_be_bytes());
    for (i, anchor) in anchors.iter().enumerate() {
        output.extend((anchor.seconds.max(0.) as f32).to_be_bytes());
        if let Some(next) = anchors.get(i + 1) {
            let count = ((next.seconds - anchor.seconds) * anchor.bpm / 60.).round();
            ensure!(
                count >= 1. && count <= u32::MAX as f64,
                "Invalid Serato beat interval"
            );
            output.extend((count as u32).to_be_bytes());
        } else {
            output.extend((anchor.bpm as f32).to_be_bytes());
        }
    }
    output.push(0);
    Ok(output)
}

#[cfg(test)]
mod export_timing_tests {
    use super::*;
    use crate::{SyncBeat, SyncCue};
    #[test]
    fn native_marker_loop_and_clamped_grid_fields_use_the_export_copy_only() {
        let canonical = SyncTrack {
            native_path: None,
            path: "/synthetic.mp3".into(),
            sample_rate: 44100,
            cues: vec![
                SyncCue {
                    index: 0,
                    name: "Cue".into(),
                    start_seconds: 0.001,
                    ..Default::default()
                },
                SyncCue {
                    index: 1,
                    name: "Loop".into(),
                    start_seconds: 0.25,
                    end_seconds: Some(0.5),
                    ..Default::default()
                },
            ],
            beatgrid: vec![SyncBeat {
                seconds: 0.001,
                bpm: 120.,
                beat: 1,
            }],
            export_timing: Some(cuerithm_media::SourceExportTiming {
                decoded_sample_count: None,
                profile: "dj-timeline-v1".into(),
                source_sha256: "0".repeat(64),
                source_sample_rate: 44100,
                codec_profile: cuerithm_media::CodecTimingProfile::Mp3 {
                    case: cuerithm_media::Mp3TimingCase::A,
                },
                measurement_identity: "synthetic".into(),
            }),
            ..Default::default()
        };
        let exported = crate::timing::export_track(&canonical, crate::DjSoftware::Serato4).unwrap();
        let bytes = markers(&exported, &SyncOptions::default()).unwrap();
        let text: Vec<_> = bytes[2..]
            .iter()
            .copied()
            .filter(|b| !matches!(*b, 0 | b'\n' | b'\r'))
            .collect();
        let body = STANDARD.decode(text).unwrap();
        let mut pos = 2;
        let mut cue = None;
        let mut saved_loop = None;
        while pos < body.len() {
            let end = pos + body[pos..].iter().position(|b| *b == 0).unwrap();
            let name = &body[pos..end];
            let len = u32::from_be_bytes(body[end + 1..end + 5].try_into().unwrap()) as usize;
            let payload = &body[end + 5..end + 5 + len];
            if name == b"CUE" {
                cue = Some(u32::from_be_bytes(payload[2..6].try_into().unwrap()));
            }
            if name == b"LOOP" {
                saved_loop = Some((
                    u32::from_be_bytes(payload[2..6].try_into().unwrap()),
                    u32::from_be_bytes(payload[6..10].try_into().unwrap()),
                ));
            }
            pos = end + 5 + len;
        }
        assert_eq!(cue, Some(0));
        assert_eq!(saved_loop, Some((245, 495)));
        let grid = beatgrid(&exported).unwrap();
        let start = f32::from_be_bytes(grid[6..10].try_into().unwrap());
        assert_eq!(start, 0.);
        assert_eq!(canonical.cues[0].start_seconds, 0.001);
        assert_eq!(canonical.beatgrid[0].seconds, 0.001);
        for (seconds, expected) in [(0.009, 4_u32), (0.019, 14)] {
            let mut near = canonical.clone();
            near.cues.truncate(1);
            near.cues[0].start_seconds = seconds;
            let converted = crate::timing::export_track(&near, crate::DjSoftware::Serato4).unwrap();
            let encoded = markers(&converted, &SyncOptions::default()).unwrap();
            let text: Vec<_> = encoded[2..]
                .iter()
                .copied()
                .filter(|b| !matches!(*b, 0 | b'\n' | b'\r'))
                .collect();
            let body = STANDARD.decode(text).unwrap();
            let start = body.windows(4).position(|p| p == b"CUE\0").unwrap();
            assert_eq!(
                u32::from_be_bytes(body[start + 10..start + 14].try_into().unwrap()),
                expected,
                "Reference subtraction order matters at millisecond truncation"
            );
        }
    }
}
