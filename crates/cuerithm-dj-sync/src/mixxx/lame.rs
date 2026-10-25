//! What the first MP3 frame says about where decoders start, read from at most a few
//! kilobytes and never modified. Mixxx's decoder drops the first frame and trims
//! nothing else; FFmpeg (CueRithm's clock) drops an Info/Xing frame and trims
//! `encoder delay + 529` samples when a `LAME`, `Lavf` or `Lavc` tag gives a delay.
//! The offset between them follows from these facts; see
//! `docs/research/mixxx-sync-contract.md`, *Timing model*.
use anyhow::{Context, Result, bail, ensure};
use std::{
    fs::File,
    io::{Read, Seek, SeekFrom},
    path::Path,
};

/// Bytes read after any ID3v2 tag; the first frame and its tag fit comfortably.
const WINDOW: usize = 2_400;
/// FFmpeg adds the 528 + 1 sample decoder delay to the encoder delay.
const DECODER_DELAY: u32 = 529;
/// Encoder delays above this are not delays but a mis-parsed field.
const MAX_ENCODER_DELAY: u32 = 4_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Mp3Start {
    /// A Xing/Info frame leads the stream (both decoders drop it).
    pub info_frame: bool,
    /// Samples in one frame: 1152 for MPEG-1, 576 for MPEG-2 and 2.5.
    pub first_frame_samples: u32,
    /// What FFmpeg trims from the start (`encoder delay + 529`), when it trusts the tag.
    pub trim: Option<u32>,
}

/// An MPEG audio layer III frame header at `at`, with its fields.
struct Header {
    mpeg1: bool,
    mono: bool,
    crc: bool,
}
fn header(bytes: &[u8], at: usize) -> Option<Header> {
    let window = bytes.get(at..at + 4)?;
    let valid = window[0] == 0xFF
        && window[1] & 0xE0 == 0xE0
        && (window[1] >> 3) & 3 != 1 // reserved version
        && (window[1] >> 1) & 3 == 1 // layer III
        && matches!(window[2] >> 4, 1..=14)
        && (window[2] >> 2) & 3 != 3;
    valid.then(|| Header {
        mpeg1: (window[1] >> 3) & 3 == 3,
        mono: window[3] >> 6 == 3,
        crc: window[1] & 1 == 0,
    })
}

fn be32(bytes: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_be_bytes(bytes.get(at..at + 4)?.try_into().ok()?))
}

/// Facts from the first frame found in `bytes` (the data after any ID3v2 tag).
pub(crate) fn parse(bytes: &[u8]) -> Result<Mp3Start> {
    let frame = (0..bytes.len().saturating_sub(3))
        .find(|at| header(bytes, *at).is_some())
        .context("MP3 start could not be determined: no audio frame found")?;
    let head = header(bytes, frame).expect("found above");
    let first_frame_samples = if head.mpeg1 { 1152 } else { 576 };
    let side_info = match (head.mpeg1, head.mono) {
        (true, true) => 17,
        (true, false) => 32,
        (false, true) => 9,
        (false, false) => 17,
    };
    let marker = frame + 4 + if head.crc { 2 } else { 0 } + side_info;
    let info_frame = matches!(bytes.get(marker..marker + 4), Some(b"Info" | b"Xing"));
    if !info_frame {
        return Ok(Mp3Start {
            info_frame: false,
            first_frame_samples,
            trim: None,
        });
    }
    // The tag follows the optional frame count, byte count, table of contents and
    // quality fields that the flags announce, as FFmpeg reads them.
    let flags = be32(bytes, marker + 4).unwrap_or(0);
    let tag = marker
        + 8
        + if flags & 1 != 0 { 4 } else { 0 }
        + if flags & 2 != 0 { 4 } else { 0 }
        + if flags & 4 != 0 { 100 } else { 0 }
        + if flags & 8 != 0 { 4 } else { 0 };
    let trusted = matches!(bytes.get(tag..tag + 4), Some(b"LAME" | b"Lavf" | b"Lavc"));
    let delay = bytes
        .get(tag + 21..tag + 23)
        .map(|field| (u32::from(field[0]) << 4) | (u32::from(field[1]) >> 4));
    let trim = match (trusted, delay) {
        (true, Some(delay)) if delay <= MAX_ENCODER_DELAY => Some(delay + DECODER_DELAY),
        _ => None,
    };
    Ok(Mp3Start {
        info_frame: true,
        first_frame_samples,
        trim,
    })
}

/// Read the start facts of an MP3 file.
pub(crate) fn mp3_start(path: &Path) -> Result<Mp3Start> {
    let mut file = File::open(path).with_context(|| format!("Cannot read {}", path.display()))?;
    let mut id3 = [0u8; 10];
    let mut skip = 0u64;
    if file.read_exact(&mut id3).is_ok() && &id3[..3] == b"ID3" {
        ensure!(
            id3[6..10].iter().all(|byte| byte & 0x80 == 0),
            "MP3 start could not be determined: bad ID3 tag size"
        );
        let size = id3[6..10]
            .iter()
            .fold(0u64, |size, byte| (size << 7) | u64::from(*byte));
        skip = 10 + size + if id3[5] & 0x10 != 0 { 10 } else { 0 };
    }
    file.seek(SeekFrom::Start(skip))?;
    let mut window = Vec::with_capacity(WINDOW);
    file.take(WINDOW as u64).read_to_end(&mut window)?;
    if window.is_empty() {
        bail!("MP3 start could not be determined: the file has no audio after its tag");
    }
    parse(&window)
}

#[cfg(test)]
pub(crate) mod fixtures {
    /// A synthetic MPEG layer III first frame, built to the layout FFmpeg reads.
    pub struct Frame {
        pub version: u8, // 3 = MPEG-1, 2 = MPEG-2, 0 = MPEG-2.5
        pub mono: bool,
        pub crc: bool,
        pub marker: Option<&'static [u8; 4]>,
        pub flags: u32,
        pub identifier: [u8; 4],
        pub delay: u16,
    }
    impl Default for Frame {
        fn default() -> Self {
            Self {
                version: 3,
                mono: false,
                crc: false,
                marker: Some(b"Info"),
                flags: 0x0F,
                identifier: *b"LAME",
                delay: 576,
            }
        }
    }
    impl Frame {
        pub fn bytes(&self) -> Vec<u8> {
            let mut frame = vec![0u8; 417];
            frame[0] = 0xFF;
            frame[1] = 0xE0 | (self.version << 3) | (0b01 << 1) | u8::from(!self.crc);
            // Bitrate index 9, sample-rate index 0 (44.1 kHz), no padding.
            frame[2] = 9 << 4;
            frame[3] = (if self.mono { 0b11 } else { 0b01 }) << 6;
            let side = match (self.version == 3, self.mono) {
                (true, true) => 17,
                (true, false) => 32,
                (false, true) => 9,
                (false, false) => 17,
            };
            let marker = 4 + if self.crc { 2 } else { 0 } + side;
            if let Some(name) = self.marker {
                frame[marker..marker + 4].copy_from_slice(name);
                frame[marker + 4..marker + 8].copy_from_slice(&self.flags.to_be_bytes());
                let tag = marker
                    + 8
                    + if self.flags & 1 != 0 { 4 } else { 0 }
                    + if self.flags & 2 != 0 { 4 } else { 0 }
                    + if self.flags & 4 != 0 { 100 } else { 0 }
                    + if self.flags & 8 != 0 { 4 } else { 0 };
                frame[tag..tag + 4].copy_from_slice(&self.identifier);
                let packed = (u32::from(self.delay) << 12) & 0x00FF_F000;
                frame[tag + 21..tag + 24].copy_from_slice(&packed.to_be_bytes()[1..]);
            }
            frame
        }
        /// The whole file: an optional ID3v2 tag, optional junk, then the frame.
        pub fn file(&self, id3_payload: Option<usize>, junk: usize) -> Vec<u8> {
            let mut file = Vec::new();
            if let Some(size) = id3_payload {
                file.extend_from_slice(b"ID3\x03\x00\x00");
                file.extend_from_slice(&[
                    ((size >> 21) & 0x7F) as u8,
                    ((size >> 14) & 0x7F) as u8,
                    ((size >> 7) & 0x7F) as u8,
                    (size & 0x7F) as u8,
                ]);
                file.extend(std::iter::repeat_n(0x20u8, size));
            }
            file.extend(std::iter::repeat_n(0u8, junk));
            file.extend(self.bytes());
            file
        }
    }
}

#[cfg(test)]
mod tests {
    use super::fixtures::Frame;
    use super::*;
    use crate::{
        DjSoftware, SyncBeat, SyncCue, SyncLibrary, SyncTrack,
        mixxx::{Mp3Decoder, decoder::pretend_host},
        timing,
    };

    #[test]
    fn mp3_start_reads_info_frame_identifier_and_delay_for_mono_and_stereo_mpeg1_and_mpeg2() {
        for (version, mono, samples) in [
            (3u8, false, 1152),
            (3, true, 1152),
            (2, false, 576),
            (2, true, 576),
            (0, false, 576),
        ] {
            for crc in [false, true] {
                let frame = Frame {
                    version,
                    mono,
                    crc,
                    ..Frame::default()
                };
                let start = parse(&frame.bytes()).unwrap();
                assert_eq!(
                    start,
                    Mp3Start {
                        info_frame: true,
                        first_frame_samples: samples,
                        trim: Some(1105)
                    },
                    "version {version} mono {mono} crc {crc}"
                );
            }
        }
    }

    #[test]
    fn unknown_identifier_gives_no_trim_but_keeps_the_info_frame() {
        for identifier in [*b"GOGO", *b"XING", [0; 4], *b"Bolt"] {
            let start = parse(
                &Frame {
                    identifier,
                    ..Frame::default()
                }
                .bytes(),
            )
            .unwrap();
            assert!(start.info_frame && start.trim.is_none(), "{identifier:?}");
        }
    }

    #[test]
    fn no_marker_means_no_info_frame_and_first_frame_length_by_mpeg_version() {
        for (version, samples) in [(3u8, 1152), (2, 576), (0, 576)] {
            let start = parse(
                &Frame {
                    version,
                    marker: None,
                    ..Frame::default()
                }
                .bytes(),
            )
            .unwrap();
            assert_eq!(
                start,
                Mp3Start {
                    info_frame: false,
                    first_frame_samples: samples,
                    trim: None
                }
            );
        }
    }

    #[test]
    fn xing_marker_and_lavf_lavc_identifiers_trim_like_lame() {
        for identifier in [*b"LAME", *b"Lavf", *b"Lavc"] {
            for marker in [b"Info", b"Xing"] {
                let start = parse(
                    &Frame {
                        identifier,
                        marker: Some(marker),
                        ..Frame::default()
                    }
                    .bytes(),
                )
                .unwrap();
                assert_eq!(start.trim, Some(1105));
            }
        }
    }

    #[test]
    fn tag_position_follows_the_flags() {
        for flags in [0x01, 0x03, 0x07, 0x0F, 0x09, 0x00] {
            let start = parse(
                &Frame {
                    flags,
                    delay: 700,
                    ..Frame::default()
                }
                .bytes(),
            )
            .unwrap();
            assert_eq!(start.trim, Some(700 + 529), "flags {flags:#x}");
        }
    }

    #[test]
    fn non_576_delay_changes_the_trim_and_implausible_delay_is_ignored() {
        assert_eq!(
            parse(
                &Frame {
                    delay: 1000,
                    ..Frame::default()
                }
                .bytes()
            )
            .unwrap()
            .trim,
            Some(1529)
        );
        assert_eq!(
            parse(
                &Frame {
                    delay: 0,
                    ..Frame::default()
                }
                .bytes()
            )
            .unwrap()
            .trim,
            Some(529)
        );
        assert_eq!(
            parse(
                &Frame {
                    delay: 4000,
                    ..Frame::default()
                }
                .bytes()
            )
            .unwrap()
            .trim,
            Some(4529)
        );
        assert_eq!(
            parse(
                &Frame {
                    delay: 4001,
                    ..Frame::default()
                }
                .bytes()
            )
            .unwrap()
            .trim,
            None
        );
    }

    #[test]
    fn crc_fields_are_not_checked() {
        let mut bytes = Frame::default().bytes();
        let end = bytes.len();
        for byte in &mut bytes[188..192] {
            *byte ^= 0xA5; // music CRC and tag CRC
        }
        assert!(end > 192);
        assert_eq!(parse(&bytes).unwrap().trim, Some(1105));
    }

    #[test]
    fn unparsable_files_are_errors() {
        assert!(parse(&[]).is_err());
        assert!(parse(&[0u8; 500]).is_err());
        assert!(
            parse(&[0xFF, 0xFB, 0x00, 0x00]).is_err(),
            "bitrate index 0 is not a frame"
        );
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("empty.mp3");
        std::fs::write(&path, b"").unwrap();
        assert!(mp3_start(&path).is_err());
        std::fs::write(&path, b"ID3\x03\x00\x00\x00\x00\x00\x00").unwrap();
        assert!(
            mp3_start(&path)
                .unwrap_err()
                .to_string()
                .contains("no audio")
        );
        assert!(mp3_start(&directory.path().join("missing.mp3")).is_err());
    }

    #[test]
    fn id3_tags_junk_and_footers_are_skipped() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("tagged.mp3");
        let frame = Frame::default();
        // A 300-byte tag plus 40 bytes of padding before the first frame.
        std::fs::write(&path, frame.file(Some(300), 40)).unwrap();
        assert_eq!(mp3_start(&path).unwrap().trim, Some(1105));
        // Without a tag.
        std::fs::write(&path, frame.file(None, 0)).unwrap();
        assert_eq!(mp3_start(&path).unwrap().trim, Some(1105));
        // A footer flag adds ten bytes after the tag.
        let mut bytes = frame.file(Some(50), 0);
        bytes[5] |= 0x10;
        bytes.splice(60..60, [0u8; 10]);
        std::fs::write(&path, bytes).unwrap();
        assert_eq!(mp3_start(&path).unwrap().trim, Some(1105));
    }

    /// Opt-in check against files real encoders wrote. Point `CUERITHM_MP3_SAMPLES` at
    /// a folder holding `lame` and FFmpeg output (see the timing model in the research
    /// document): `A.mp3` (`lame -t`, no Info frame), `B.mp3` (identifier replaced),
    /// `C.mp3` (CRC broken), `D.mp3` (plain `lame`) and `ff.mp3` (FFmpeg's libmp3lame).
    #[test]
    #[ignore = "needs encoder-written samples; set CUERITHM_MP3_SAMPLES"]
    fn real_encoder_files_match_what_ffmpeg_trims() {
        let folder = std::path::PathBuf::from(
            std::env::var_os("CUERITHM_MP3_SAMPLES").expect("CUERITHM_MP3_SAMPLES"),
        );
        let start = |name: &str| mp3_start(&folder.join(name)).unwrap();
        let a = start("A.mp3");
        assert!(
            !a.info_frame && a.trim.is_none() && a.first_frame_samples == 1152,
            "{a:?}"
        );
        let b = start("B.mp3");
        assert!(b.info_frame && b.trim.is_none(), "{b:?}");
        for name in ["C.mp3", "D.mp3", "ff.mp3"] {
            let found = start(name);
            assert!(
                found.info_frame && found.trim == Some(1105),
                "{name}: {found:?}"
            );
        }
    }

    fn mp3_track(directory: &Path, sample_rate: u32, frame: &Frame) -> SyncTrack {
        // These tests pin the Linux and Windows decoder; macOS is covered in decoder.rs
        // and coreaudio.rs against live CoreAudio.
        pretend_host(Mp3Decoder::Mad);
        let path = directory.join("song.mp3");
        std::fs::write(&path, frame.file(Some(64), 0)).unwrap();
        SyncTrack {
            id: "t".into(),
            path,
            sample_rate,
            duration_seconds: 10.0,
            cues: vec![SyncCue {
                start_seconds: 1.0,
                end_seconds: Some(2.0),
                ..SyncCue::default()
            }],
            beatgrid: vec![SyncBeat {
                seconds: 0.5,
                bpm: 120.0,
                beat: 1,
            }],
            ..SyncTrack::default()
        }
    }

    #[test]
    fn mixxx_offsets_for_the_three_header_facts_at_44100_and_48000() {
        let directory = tempfile::tempdir().unwrap();
        for rate in [44_100u32, 48_000] {
            let rate_f = f64::from(rate);
            for (frame, expected) in [
                (
                    Frame {
                        marker: None,
                        ..Frame::default()
                    },
                    -1152.0 / rate_f,
                ),
                (
                    Frame {
                        identifier: *b"GOGO",
                        ..Frame::default()
                    },
                    0.0,
                ),
                (Frame::default(), 1105.0 / rate_f),
                (
                    Frame {
                        delay: 1000,
                        ..Frame::default()
                    },
                    1529.0 / rate_f,
                ),
            ] {
                let track = mp3_track(directory.path(), rate, &frame);
                let offset = timing::offset_seconds(&track, DjSoftware::Mixxx).unwrap();
                assert!(
                    (offset - expected).abs() < 1e-12,
                    "{rate} {expected}: {offset}"
                );
            }
        }
        // MPEG-2 sample rates use the 576-sample first frame.
        let track = mp3_track(
            directory.path(),
            22_050,
            &Frame {
                version: 2,
                marker: None,
                ..Frame::default()
            },
        );
        assert!(
            (timing::offset_seconds(&track, DjSoftware::Mixxx).unwrap() + 576.0 / 22_050.0).abs()
                < 1e-12
        );
    }

    #[test]
    fn lossless_ogg_opus_aac_and_wma_are_zero_without_reading_the_file() {
        // M4A is read only where the AAC decoder leaves priming in (FAAD, Media Foundation); a
        // decoder that drops it, like CueRithm's own clock, needs no look at the file. Pinned,
        // so the result does not depend on the Mixxx or the system the test runs on.
        let _drops_priming = crate::mixxx::pretend_aac(crate::mixxx::AacDecoder::TrimsPriming);
        for extension in [
            "flac", "wav", "aiff", "ogg", "opus", "m4a", "mp4", "aac", "wma",
        ] {
            let track = SyncTrack {
                id: "t".into(),
                path: format!("/does/not/exist.{extension}").into(),
                sample_rate: 44_100,
                ..SyncTrack::default()
            };
            assert_eq!(
                timing::offset_seconds(&track, DjSoftware::Mixxx).unwrap(),
                0.0,
                "{extension}"
            );
        }
    }

    #[test]
    fn mixxx_does_not_need_an_export_timing_profile_and_unreadable_mp3_is_an_error() {
        let directory = tempfile::tempdir().unwrap();
        let track = mp3_track(directory.path(), 44_100, &Frame::default());
        assert!(track.export_timing.is_none());
        assert!(timing::offset_seconds(&track, DjSoftware::Mixxx).is_ok());
        let missing = SyncTrack {
            path: directory.path().join("gone.mp3"),
            sample_rate: 44_100,
            ..SyncTrack::default()
        };
        assert!(timing::offset_seconds(&missing, DjSoftware::Mixxx).is_err());
        let silent = SyncTrack {
            sample_rate: 0,
            ..mp3_track(directory.path(), 44_100, &Frame::default())
        };
        assert!(timing::offset_seconds(&silent, DjSoftware::Mixxx).is_err());
        // Other targets still demand a measured profile for MP3.
        assert!(timing::offset_seconds(&track, DjSoftware::PioneerUsb).is_err());
    }

    #[test]
    fn export_is_applied_exactly_once_to_cues_and_grid() {
        let directory = tempfile::tempdir().unwrap();
        let track = mp3_track(directory.path(), 44_100, &Frame::default());
        let library = SyncLibrary {
            tracks: vec![track],
            playlists: vec![],
        };
        let once = timing::export_library(&library, DjSoftware::Mixxx).unwrap();
        let shift = 1105.0 / 44_100.0;
        assert!((once.tracks[0].cues[0].start_seconds - 1.0 - shift).abs() < 1e-12);
        assert!((once.tracks[0].cues[0].end_seconds.unwrap() - 2.0 - shift).abs() < 1e-12);
        assert!((once.tracks[0].beatgrid[0].seconds - 0.5 - shift).abs() < 1e-12);
        let twice = timing::export_library(&once, DjSoftware::Mixxx).unwrap();
        assert_eq!(
            serde_json::to_vec(&once.tracks[0]).unwrap(),
            serde_json::to_vec(&twice.tracks[0]).unwrap()
        );
    }
}
