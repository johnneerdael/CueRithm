//! Convert canonical prepared PCM positions on export copies, exactly once.
use crate::{AppliedTiming, DjSoftware, SyncLibrary, SyncTrack};
use anyhow::{Context, Result, ensure};
use cuerithm_media::{CodecTimingProfile, Mp3TimingCase, SourceExportTiming};
use sha2::{Digest, Sha256};

pub fn implementation_sha256() -> String {
    let mut hash = Sha256::new();
    hash.update(include_bytes!("timing.rs"));
    hash.update(include_bytes!("pioneer_usb/analysis.rs"));
    format!("{:x}", hash.finalize())
}
fn profile_hash(profile: Option<&SourceExportTiming>) -> Result<String> {
    Ok(format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(&profile)?)
    ))
}
/// Mixxx's decoder for this system against CueRithm's FFmpeg clock, from the file's own
/// MP3 header and, on macOS, Apple's own reading of it, not a measured profile. Only MP3
/// differs: AAC is zero where it was measured (macOS) and reported where it was not.
fn mixxx_offset(track: &SyncTrack, extension: &str) -> Result<f64> {
    mixxx_offset_with(track, extension, crate::mixxx::Mp3Decoder::host())
}
pub(crate) fn mixxx_offset_with(
    track: &SyncTrack,
    extension: &str,
    decoder: crate::mixxx::Mp3Decoder,
) -> Result<f64> {
    if matches!(extension, "m4a" | "mp4")
        && decoder == crate::mixxx::Mp3Decoder::Mad
        && crate::mixxx::aac_decoder() == crate::mixxx::AacDecoder::Faad
    {
        // FAAD does not skip the encoder's priming; FFmpeg's clock does (see `mixxx/mp4.rs`).
        ensure!(
            track.sample_rate > 0,
            "Track has no sample rate for its AAC timing"
        );
        let frames = crate::mixxx::faad_shift_frames(&track.path)
            .with_context(|| format!("Read the AAC start of track {}", track.id))?;
        return Ok(f64::from(frames) / f64::from(track.sample_rate));
    }
    if extension != "mp3" {
        return Ok(0.);
    }
    ensure!(
        track.sample_rate > 0,
        "Track has no sample rate for its MP3 timing"
    );
    let start = crate::mixxx::mp3_start(&track.path)
        .with_context(|| format!("Read MP3 start of track {}", track.id))?;
    let priming = crate::mixxx::coreaudio_priming(&track.path, decoder)
        .with_context(|| format!("Ask CoreAudio about track {}", track.id))?;
    let samples = crate::mixxx::mp3_offset_samples(decoder, &start, priming);
    Ok(samples as f64 / f64::from(track.sample_rate))
}
pub fn offset_seconds(track: &SyncTrack, target: DjSoftware) -> Result<f64> {
    let extension = track
        .path
        .extension()
        .and_then(|p| p.to_str())
        .unwrap_or("")
        .to_lowercase();
    if target == DjSoftware::Mixxx {
        return mixxx_offset(track, &extension);
    }
    let Some(profile) = &track.export_timing else {
        ensure!(
            target != DjSoftware::Rekordbox,
            "Source timing must be measured before native rekordbox export"
        );
        ensure!(
            !((extension == "mp3"
                && matches!(target, DjSoftware::Serato4 | DjSoftware::PioneerUsb))
                || (matches!(extension.as_str(), "m4a" | "mp4")
                    && matches!(target, DjSoftware::Serato4 | DjSoftware::DjayPro))),
            "Source timing must be measured before this DJ export"
        );
        return Ok(0.);
    };
    ensure!(
        profile.profile == "dj-timeline-v1",
        "Unsupported source timing profile"
    );
    ensure!(
        profile.source_sample_rate > 0
            && profile.source_sample_rate == track.sample_rate
            && profile.source_sha256.len() == 64
            && profile.source_sha256.bytes().all(|b| b.is_ascii_hexdigit())
            && !profile.measurement_identity.is_empty(),
        "Invalid timing profile identity or source rate"
    );
    ensure!(
        match &profile.codec_profile {
            CodecTimingProfile::Direct => !matches!(extension.as_str(), "mp3" | "m4a" | "mp4"),
            CodecTimingProfile::Mp3 { .. } => extension == "mp3",
            CodecTimingProfile::IsoAudio {
                extension: measured,
                ..
            } => measured == &extension,
            CodecTimingProfile::Unmeasured { .. } => true,
        },
        "Timing profile codec kind does not match the exported file"
    );

    let seconds = match &profile.codec_profile {
        CodecTimingProfile::Direct => 0.,
        CodecTimingProfile::Unmeasured { reason } => {
            ensure!(
                !matches!(
                    target,
                    DjSoftware::Serato4
                        | DjSoftware::PioneerUsb
                        | DjSoftware::DjayPro
                        | DjSoftware::Rekordbox
                ),
                "Source timing is unmeasured: {reason}"
            );
            0.
        }
        CodecTimingProfile::Mp3 { case } => match target {
            DjSoftware::Serato4 => {
                if *case == Mp3TimingCase::A {
                    0.046 - 0.051
                } else {
                    0.046
                }
            }
            DjSoftware::PioneerUsb => match case {
                Mp3TimingCase::A => 0.,
                Mp3TimingCase::B | Mp3TimingCase::C => 0.051 - 0.026,
                Mp3TimingCase::D => 0.051,
            },
            // Native desktop export is a distinct clock contract. The measured
            // canonical PCM cases map to these native offsets independently of
            // USB serialization; container priming below differs from USB.
            DjSoftware::Rekordbox => match case {
                Mp3TimingCase::A => 0.,
                Mp3TimingCase::B | Mp3TimingCase::C => 0.051 - 0.026,
                Mp3TimingCase::D => 0.051,
            },
            _ => 0.,
        },
        CodecTimingProfile::IsoAudio {
            extension,
            audio_delay_ms,
            video_delay_ms,
        } => {
            ensure!(
                matches!(extension.as_str(), "m4a" | "mp4")
                    && profile.source_sample_rate > 0
                    && audio_delay_ms.is_finite()
                    && *audio_delay_ms >= 0.
                    && video_delay_ms.is_finite()
                    && *video_delay_ms >= 0.,
                "Invalid measured container timing"
            );
            match target {
                DjSoftware::Serato4 => {
                    let audio = if *audio_delay_ms > 0. {
                        audio_delay_ms / 1000.
                            - if extension == "m4a" {
                                2048. / f64::from(profile.source_sample_rate)
                            } else {
                                0.
                            }
                    } else {
                        0.
                    };
                    audio - video_delay_ms / 1000.
                }
                DjSoftware::DjayPro => -video_delay_ms / 1000.,
                DjSoftware::Rekordbox => audio_delay_ms / 1000.,
                _ => 0.,
            }
        }
    };
    ensure!(seconds.is_finite(), "Invalid native timeline adjustment");
    Ok(seconds)
}
pub fn export_track(track: &SyncTrack, target: DjSoftware) -> Result<SyncTrack> {
    let profile_sha256 = profile_hash(track.export_timing.as_ref())?;
    if let Some(applied) = &track.native_timeline {
        ensure!(
            applied.target == target && applied.profile_sha256 == profile_sha256,
            "Export copy already belongs to a different target or timing profile"
        );
        return Ok(track.clone());
    }
    let offset = offset_seconds(track, target)
        .with_context(|| format!("Measure DJ export timing for track {}", track.id))?;
    let mut output = track.clone();
    for cue in &mut output.cues {
        cue.start_seconds += offset;
        if let Some(end) = &mut cue.end_seconds {
            *end += offset;
        }
        if target == DjSoftware::Serato4 {
            cue.start_seconds = cue.start_seconds.max(0.);
            if let Some(end) = &mut cue.end_seconds {
                *end = end.max(0.);
                ensure!(
                    *end > cue.start_seconds,
                    "Native timing adjustment collapses a saved loop"
                );
            }
        }
    }
    for beat in &mut output.beatgrid {
        beat.seconds += offset;
    }
    output.native_timeline = Some(AppliedTiming {
        target,
        offset_seconds: offset,
        profile_sha256,
    });
    Ok(output)
}
pub fn export_library(library: &SyncLibrary, target: DjSoftware) -> Result<SyncLibrary> {
    Ok(SyncLibrary {
        tracks: library
            .tracks
            .iter()
            .map(|track| export_track(track, target))
            .collect::<Result<_>>()?,
        playlists: library.playlists.clone(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{SyncBeat, SyncCue};
    fn track(profile: CodecTimingProfile) -> SyncTrack {
        let extension = match &profile {
            CodecTimingProfile::Mp3 { .. } => "mp3",
            CodecTimingProfile::IsoAudio { extension, .. } => extension.as_str(),
            _ => "wav",
        };
        SyncTrack {
            native_path: None,
            id: "synthetic".into(),
            path: format!("/synthetic.{extension}").into(),
            sample_rate: 44100,
            duration_seconds: 10.,
            export_timing: Some(SourceExportTiming {
                decoded_sample_count: None,
                profile: "dj-timeline-v1".into(),
                source_sha256: "0".repeat(64),
                source_sample_rate: 44100,
                codec_profile: profile,
                measurement_identity: "synthetic".into(),
            }),
            cues: vec![SyncCue {
                start_seconds: 1.,
                end_seconds: Some(2.),
                ..Default::default()
            }],
            beatgrid: vec![SyncBeat {
                seconds: 0.5,
                bpm: 120.,
                beat: 1,
            }],
            ..Default::default()
        }
    }
    #[test]
    fn native_rekordbox_has_its_own_measured_clock_and_converts_once() {
        for (case, expected) in [
            (Mp3TimingCase::A, 0.),
            (Mp3TimingCase::B, 0.025),
            (Mp3TimingCase::C, 0.025),
            (Mp3TimingCase::D, 0.051),
        ] {
            let original = track(CodecTimingProfile::Mp3 { case });
            let converted = export_track(&original, DjSoftware::Rekordbox).unwrap();
            assert!((converted.cues[0].start_seconds - 1. - expected).abs() < 1e-12);
            assert!((converted.beatgrid[0].seconds - 0.5 - expected).abs() < 1e-12);
            assert_eq!(
                serde_json::to_vec(&converted).unwrap(),
                serde_json::to_vec(&export_track(&converted, DjSoftware::Rekordbox).unwrap())
                    .unwrap()
            );
            assert!(export_track(&converted, DjSoftware::PioneerUsb).is_err());
        }
        for extension in ["m4a", "mp4"] {
            for delay in [0., 23.2] {
                let original = track(CodecTimingProfile::IsoAudio {
                    extension: extension.into(),
                    audio_delay_ms: delay,
                    video_delay_ms: 42.7,
                });
                assert_eq!(
                    offset_seconds(&original, DjSoftware::Rekordbox).unwrap(),
                    delay / 1000.
                );
                assert_eq!(
                    offset_seconds(&original, DjSoftware::PioneerUsb).unwrap(),
                    0.
                );
            }
        }
        let mut missing = track(CodecTimingProfile::Direct);
        missing.export_timing = None;
        assert!(export_track(&missing, DjSoftware::Rekordbox).is_err());
        let unknown = track(CodecTimingProfile::Unmeasured {
            reason: "fixture".into(),
        });
        assert!(export_track(&unknown, DjSoftware::Rekordbox).is_err());
    }
    #[test]
    fn mp3_cases_match_executed_reference_offsets_and_never_accumulate() {
        for (case, serato, usb) in [
            (Mp3TimingCase::A, -0.005, 0.),
            (Mp3TimingCase::B, 0.046, 0.025),
            (Mp3TimingCase::C, 0.046, 0.025),
            (Mp3TimingCase::D, 0.046, 0.051),
        ] {
            let original = track(CodecTimingProfile::Mp3 { case });
            let before = serde_json::to_vec(&original).unwrap();
            for (target, expected) in [
                (DjSoftware::Serato4, serato),
                (DjSoftware::PioneerUsb, usb),
                (DjSoftware::VirtualDj, 0.),
                (DjSoftware::EngineDj, 0.),
                (DjSoftware::DjayPro, 0.),
            ] {
                let converted = export_track(&original, target).unwrap();
                assert!((converted.cues[0].start_seconds - 1. - expected).abs() < 1e-12);
                assert!((converted.cues[0].end_seconds.unwrap() - 2. - expected).abs() < 1e-12);
                assert!((converted.beatgrid[0].seconds - 0.5 - expected).abs() < 1e-12);
                assert_eq!(
                    serde_json::to_vec(&converted).unwrap(),
                    serde_json::to_vec(&export_track(&converted, target).unwrap()).unwrap()
                );
            }
            assert_eq!(serde_json::to_vec(&original).unwrap(), before);
        }
    }
    #[test]
    fn priming_depends_on_source_rate_extension_video_and_known_zero() {
        for (rate, delay) in [(44100, 23.2), (48000, 21.3)] {
            for extension in ["m4a", "mp4"] {
                let mut original = track(CodecTimingProfile::IsoAudio {
                    extension: extension.into(),
                    audio_delay_ms: delay,
                    video_delay_ms: 42.7,
                });
                original.sample_rate = rate;
                original.export_timing.as_mut().unwrap().source_sample_rate = rate;
                let expected = delay / 1000.
                    - if extension == "m4a" {
                        2048. / f64::from(rate)
                    } else {
                        0.
                    }
                    - 0.0427;
                assert!(
                    (offset_seconds(&original, DjSoftware::Serato4).unwrap() - expected).abs()
                        < 1e-12
                );
                assert_eq!(
                    offset_seconds(&original, DjSoftware::PioneerUsb).unwrap(),
                    0.
                );
                assert_eq!(
                    offset_seconds(&original, DjSoftware::DjayPro).unwrap(),
                    -0.0427
                );
            }
        }
        let known = track(CodecTimingProfile::IsoAudio {
            extension: "m4a".into(),
            audio_delay_ms: 0.,
            video_delay_ms: 0.,
        });
        assert_eq!(offset_seconds(&known, DjSoftware::Serato4).unwrap(), 0.);
        let mut unknown = known.clone();
        unknown.export_timing = None;
        unknown.path = "/source.m4a".into();
        assert!(export_track(&unknown, DjSoftware::Serato4).is_err());
    }
    #[test]
    fn serato_clamps_markers_while_grid_conversion_retains_interval_math_and_djay_is_signed() {
        let mut original = track(CodecTimingProfile::Mp3 {
            case: Mp3TimingCase::A,
        });
        original.cues[0].start_seconds = 0.;
        original.beatgrid[0].seconds = 0.;
        let exported = export_track(&original, DjSoftware::Serato4).unwrap();
        assert_eq!(exported.cues[0].start_seconds, 0.);
        assert_eq!(exported.beatgrid[0].seconds, 0.046 - 0.051);
        assert!(export_track(&exported, DjSoftware::PioneerUsb).is_err());
        original.path = "/synthetic.mp4".into();
        original.export_timing.as_mut().unwrap().codec_profile = CodecTimingProfile::IsoAudio {
            extension: "mp4".into(),
            audio_delay_ms: 0.,
            video_delay_ms: 46.4,
        };
        assert_eq!(
            export_track(&original, DjSoftware::DjayPro).unwrap().cues[0].start_seconds,
            -0.0464
        );
    }
}

#[cfg(test)]
mod faad_tests {
    use super::*;
    use crate::mixxx::{AacDecoder, Mp3Decoder, pretend_aac, sample_m4a};

    fn m4a(dir: &std::path::Path, name: &str, bytes: Vec<u8>) -> SyncTrack {
        let path = dir.join(name);
        std::fs::write(&path, bytes).unwrap();
        SyncTrack {
            id: name.into(),
            path,
            sample_rate: 44_100,
            ..SyncTrack::default()
        }
    }

    #[test]
    fn faad_plays_an_m4a_late_by_its_priming_minus_one_block_and_other_decoders_do_not() {
        let dir = tempfile::tempdir().unwrap();
        // Measured in Mixxx main with FAAD: Apple's encoder (2112) is 1088 frames late, FFmpeg's (1024) is on time.
        let apple = m4a(dir.path(), "apple.m4a", sample_m4a(b"mp4a", None, true));
        let ffmpeg = m4a(
            dir.path(),
            "ffmpeg.m4a",
            sample_m4a(b"mp4a", Some(&[(1_058_400, 1024)]), false),
        );
        let lossless = m4a(
            dir.path(),
            "lossless.m4a",
            sample_m4a(b"alac", Some(&[(1_058_400, 0)]), false),
        );
        {
            let _faad = pretend_aac(AacDecoder::Faad);
            let shift = mixxx_offset_with(&apple, "m4a", Mp3Decoder::Mad).unwrap();
            assert!((shift - 1088.0 / 44_100.0).abs() < 1e-12, "{shift}");
            assert_eq!(
                mixxx_offset_with(&ffmpeg, "m4a", Mp3Decoder::Mad).unwrap(),
                0.0
            );
            assert_eq!(
                mixxx_offset_with(&lossless, "m4a", Mp3Decoder::Mad).unwrap(),
                0.0,
                "FAAD does not open Apple Lossless; Mixxx falls back to FFmpeg"
            );
        }
        // FFmpeg's decoder, or CoreAudio, trims the priming itself.
        {
            let _other = pretend_aac(AacDecoder::TrimsPriming);
            assert_eq!(
                mixxx_offset_with(&apple, "m4a", Mp3Decoder::Mad).unwrap(),
                0.0
            );
        }
        {
            let _unknown = pretend_aac(AacDecoder::Unknown);
            assert_eq!(
                mixxx_offset_with(&apple, "m4a", Mp3Decoder::Mad).unwrap(),
                0.0
            );
        }
        {
            let _faad = pretend_aac(AacDecoder::Faad);
            assert_eq!(
                mixxx_offset_with(&apple, "m4a", Mp3Decoder::CoreAudio).unwrap(),
                0.0,
                "macOS decodes AAC with CoreAudio"
            );
        }
    }
}
