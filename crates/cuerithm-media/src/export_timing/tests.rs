use super::*;
use std::path::PathBuf;

fn runtime() -> MediaRuntime {
    let root = std::env::var_os("CUERITHM_MEDIA_RUNTIME")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools/media-runtime/bin")
        });
    MediaRuntime::from_bundle(&root).expect("The sealed bundled media runtime is required")
}
fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}
fn source_info(codec: &str) -> MediaInfo {
    MediaInfo {
        codec: codec.into(),
        sample_rate: 44100,
        channels: 2,
        bits_per_sample: None,
        tags: Default::default(),
        properties: Default::default(),
    }
}
fn mp3_bytes(lame: bool, valid_crc: bool, same_mode: bool, marker: bool, id3: bool) -> Vec<u8> {
    let mut bytes = vec![0_u8; 2200];
    let first = 0xfffb9000_u32;
    bytes[..4].copy_from_slice(&first.to_be_bytes());
    let next = if same_mode { first } else { first | (1 << 6) };
    bytes[417..421].copy_from_slice(&next.to_be_bytes());
    if marker {
        bytes[36..40].copy_from_slice(b"Info");
    }
    bytes[156..160].copy_from_slice(if lame { b"LAME" } else { b"Lavc" });
    if valid_crc {
        let crc = crc16_arc(&bytes[..190]);
        bytes[190..192].copy_from_slice(&crc.to_be_bytes());
    }
    if id3 {
        let mut tagged = b"ID3\x04\0\0\0\0\0\x14".to_vec();
        tagged.extend([0; 20]);
        tagged.extend(bytes);
        tagged
    } else {
        bytes
    }
}

#[test]
fn mp3_headers_match_all_four_cases_crc_and_channel_mode_overrides() {
    use Mp3TimingCase::*;
    assert_eq!(
        crc16_arc(b"123456789"),
        0xbb3d,
        "CRC-16/ARC reference check value"
    );
    let cases = [
        (false, false, false, false, A),
        (false, false, false, true, B),
        (true, false, false, true, C),
        (true, true, true, true, D),
        (false, false, true, true, D),
        (true, true, false, true, C),
    ];
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("synthetic.mp3");
    for id3 in [false, true] {
        for (lame, crc, same, marker, expected) in cases {
            std::fs::write(&file, mp3_bytes(lame, crc, same, marker, id3)).unwrap();
            assert_eq!(
                classify_mp3(&file, &CancellationToken::default())
                    .unwrap()
                    .0,
                expected
            );
        }
    }
    let mut far = vec![0; 3000];
    far.extend(mp3_bytes(false, false, false, true, false));
    std::fs::write(&file, far).unwrap();
    assert_eq!(
        classify_mp3(&file, &CancellationToken::default())
            .unwrap()
            .0,
        A,
        "Only the bounded initial frame window is inspected"
    );
}

#[test]
fn decoded_sample_count_uses_xing_frames_mpeg_version_and_the_d_case_extra_frame() {
    use Mp3TimingCase::*;
    let mut bytes = mp3_bytes(false, false, false, true, false);
    bytes[40..44].copy_from_slice(&1_u32.to_be_bytes());
    bytes[44..48].copy_from_slice(&20_u32.to_be_bytes());
    assert_eq!(decoded_sample_count_from_window(&bytes, A), None);
    for case in [B, C] {
        assert_eq!(
            decoded_sample_count_from_window(&bytes, case),
            Some(20 * 1152)
        );
    }
    assert_eq!(decoded_sample_count_from_window(&bytes, D), Some(21 * 1152));
}

#[test]
fn decoded_count_handles_mpeg_versions_channel_modes_and_absent_frame_flags() {
    for version in [3_u32, 2, 0] {
        for mono in [false, true] {
            for marker in [b"Xing", b"Info"] {
                let mut bytes = vec![0_u8; 2200];
                let mode = if mono { 3 } else { 0 };
                let header = 0xffe00000_u32
                    | (version << 19)
                    | (1 << 17)
                    | (1 << 16)
                    | (8 << 12)
                    | (mode << 6);
                bytes[..4].copy_from_slice(&header.to_be_bytes());
                let side = if version == 3 {
                    if mono { 17 } else { 32 }
                } else if mono {
                    9
                } else {
                    17
                };
                let at = 4 + side;
                bytes[at..at + 4].copy_from_slice(marker);
                bytes[at + 4..at + 8].copy_from_slice(&1_u32.to_be_bytes());
                bytes[at + 8..at + 12].copy_from_slice(&20_u32.to_be_bytes());
                let per_frame = if version == 3 { 1152 } else { 576 };
                for case in [Mp3TimingCase::B, Mp3TimingCase::C] {
                    assert_eq!(
                        decoded_sample_count_from_window(&bytes, case),
                        Some(20 * per_frame)
                    );
                }
                assert_eq!(
                    decoded_sample_count_from_window(&bytes, Mp3TimingCase::D),
                    Some(21 * per_frame)
                );
                bytes[at + 4..at + 8].copy_from_slice(&2_u32.to_be_bytes());
                assert_eq!(
                    decoded_sample_count_from_window(&bytes, Mp3TimingCase::D),
                    None,
                    "A byte-count flag is not a frame-count flag"
                );
                bytes[at + 4..at + 8].copy_from_slice(&1_u32.to_be_bytes());
                bytes[at + 8..at + 12].fill(0);
                assert_eq!(
                    decoded_sample_count_from_window(&bytes, Mp3TimingCase::D),
                    None
                );
                bytes[at + 8..at + 12].copy_from_slice(&u32::MAX.to_be_bytes());
                assert_eq!(
                    decoded_sample_count_from_window(&bytes, Mp3TimingCase::D),
                    Some((u64::from(u32::MAX) + 1) * per_frame)
                );
            }
        }
    }
    assert_eq!(
        decoded_sample_count_from_window(b"short", Mp3TimingCase::D),
        None
    );
}

#[test]
fn captured_count_and_case_come_from_the_same_id3_skipped_frame_window() {
    let rt = runtime();
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("source.mp3");
    let mut bytes = mp3_bytes(true, false, true, true, true);
    let start = 30;
    bytes[start + 40..start + 44].copy_from_slice(&1_u32.to_be_bytes());
    bytes[start + 44..start + 48].copy_from_slice(&20_u32.to_be_bytes());
    let crc = crc16_arc(&bytes[start..start + 190]);
    bytes[start + 190..start + 192].copy_from_slice(&crc.to_be_bytes());
    std::fs::write(&file, &bytes).unwrap();
    let hash = crate::file_sha256(&file).unwrap();
    let profile = rt.export_timing(&file, &hash, &source_info("mp3")).unwrap();
    assert_eq!(
        profile.codec_profile,
        CodecTimingProfile::Mp3 {
            case: Mp3TimingCase::D
        }
    );
    assert_eq!(profile.decoded_sample_count, Some(21 * 1152));
    assert_eq!(std::fs::read(&file).unwrap(), bytes);
}

#[test]
fn iso_measurements_keep_priming_distinct_from_stream_start_and_artwork() {
    let audio = serde_json::json!({"streams":[{"sample_rate":"44100","start_time":"0"}],"packets":[{"pts_time":"-0.023220"}]});
    let cover = serde_json::json!({"streams":[{"codec_name":"mjpeg"},{"codec_name":"png"},{"codec_name":"bmp"}]});
    assert_eq!(
        iso_profile("m4a", &audio, &cover, 44100).unwrap(),
        CodecTimingProfile::IsoAudio {
            extension: "m4a".into(),
            audio_delay_ms: 23.2,
            video_delay_ms: 0.
        }
    );
    let video = serde_json::json!({"streams":[{"codec_name":"mpeg4"}]});
    assert_eq!(
        iso_profile("mp4", &audio, &video, 44100).unwrap(),
        CodecTimingProfile::IsoAudio {
            extension: "mp4".into(),
            audio_delay_ms: 23.2,
            video_delay_ms: 46.4
        }
    );
    let start = serde_json::json!({"streams":[{"sample_rate":"48000","start_time":"0.10004"}],"packets":[{"pts_time":"0.2"}]});
    assert_eq!(
        iso_profile("m4a", &start, &video, 48000).unwrap(),
        CodecTimingProfile::IsoAudio {
            extension: "m4a".into(),
            audio_delay_ms: 100.,
            video_delay_ms: 42.7
        }
    );
    assert!(iso_profile("m4a", &audio, &cover, 48000).is_err());
}

#[test]
fn missing_or_nonfinite_iso_measurements_are_explicitly_unmeasured() {
    let empty_video = serde_json::json!({"streams":[]});
    for audio in [
        serde_json::json!({"streams":[]}),
        serde_json::json!({"streams":[{"sample_rate":"44100","start_time":"0"}],"packets":[]}),
        serde_json::json!({"streams":[{"sample_rate":"44100","start_time":"0"}],"packets":[{"pts_time":"NaN"}]}),
        serde_json::json!({"streams":[{"sample_rate":"44100"}],"packets":[{"pts_time":"0"}]}),
    ] {
        assert!(matches!(
            iso_profile("m4a", &audio, &empty_video, 44100).unwrap(),
            CodecTimingProfile::Unmeasured { .. }
        ));
    }
}

#[test]
fn sealed_runtime_measures_aac_rates_alac_and_artwork_without_source_changes() {
    let rt = runtime();
    for (name, rate, delay) in [
        ("timing-aac-44100.m4a", 44100, 23.2),
        ("timing-aac-48000.m4a", 48000, 21.3),
        ("timing-alac.m4a", 44100, 0.),
        ("timing-artwork.m4a", 44100, 23.2),
    ] {
        let path = fixture(name);
        let hash = crate::file_sha256(&path).unwrap();
        let info = rt.probe(&path).unwrap();
        let profile = rt.export_timing(&path, &hash, &info).unwrap();
        assert_eq!(profile.source_sample_rate, rate);
        assert_eq!(profile.decoded_sample_count, None);
        assert_eq!(
            profile.codec_profile,
            CodecTimingProfile::IsoAudio {
                extension: "m4a".into(),
                audio_delay_ms: delay,
                video_delay_ms: 0.
            }
        );
        assert_eq!(profile.measurement_identity, rt.export_timing_identity());
        assert_eq!(profile.source_sha256, hash);
        assert_eq!(crate::file_sha256(&path).unwrap(), hash);
    }
    let path = fixture("authored-video.mp4");
    let hash = crate::file_sha256(&path).unwrap();
    let info = rt.probe(&path).unwrap();
    let CodecTimingProfile::IsoAudio {
        extension,
        video_delay_ms,
        ..
    } = rt.export_timing(&path, &hash, &info).unwrap().codec_profile
    else {
        panic!("Expected a measured video container");
    };
    assert_eq!(extension, "mp4");
    assert_eq!(video_delay_ms, 46.4);
}

#[test]
fn extensionless_snapshot_capture_preserves_original_container_and_playable_clock() {
    let rt = runtime();
    let dir = tempfile::tempdir().unwrap();
    let path = fixture("timing-aac-48000.m4a");
    let audio = rt.decode(&path, dir.path()).unwrap();
    assert!(audio.snapshot_path.extension().is_none());
    assert_eq!(audio.clock.sample_rate, 44100);
    let profile = audio.export_timing.unwrap();
    assert_eq!(profile.source_sample_rate, 48000);
    assert_eq!(profile.source_sha256, audio.source_sha256);
    assert_eq!(
        profile.codec_profile,
        CodecTimingProfile::IsoAudio {
            extension: "m4a".into(),
            audio_delay_ms: 21.3,
            video_delay_ms: 0.
        }
    );
}

#[test]
fn profiles_reject_changed_sources_and_propagate_cancellation() {
    let rt = runtime();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("source.mp3");
    std::fs::write(&path, mp3_bytes(false, false, false, true, false)).unwrap();
    let hash = crate::file_sha256(&path).unwrap();
    let info = source_info("mp3");
    let profile = rt.export_timing(&path, &hash, &info).unwrap();
    assert_eq!(
        profile.codec_profile,
        CodecTimingProfile::Mp3 {
            case: Mp3TimingCase::B
        }
    );
    std::fs::write(&path, b"changed").unwrap();
    assert!(rt.export_timing(&path, &hash, &info).is_err());
    let cancelled = CancellationToken::default();
    cancelled.cancel();
    let error = rt
        .export_timing_cancellable(&path, &hash, &info, &cancelled)
        .unwrap_err();
    assert_eq!(
        error.downcast_ref::<MediaTaskError>(),
        Some(&MediaTaskError::Cancelled)
    );
}

#[test]
fn failed_capture_stays_unmeasured_and_never_accepts_a_wrong_source_hash() {
    let mut rt = runtime();
    let path = fixture("timing-aac-44100.m4a");
    let info = rt.probe(&path).unwrap();
    let hash = crate::file_sha256(&path).unwrap();
    rt.ffprobe = PathBuf::from("/nonexistent/cuerithm-timing-probe");
    let profile = rt
        .capture_export_timing_cancellable(
            &path,
            &hash,
            &info,
            "m4a",
            &CancellationToken::default(),
        )
        .unwrap();
    assert!(matches!(
        profile.codec_profile,
        CodecTimingProfile::Unmeasured { .. }
    ));
    assert_eq!(profile.source_sha256, hash);
    assert!(
        rt.capture_export_timing_cancellable(
            &path,
            &"0".repeat(64),
            &info,
            "m4a",
            &CancellationToken::default()
        )
        .is_err()
    );
}

#[test]
fn unknown_codec_is_not_a_zero_delay_measurement_and_direct_sources_are_bound() {
    let rt = runtime();
    let path = fixture("metadata.flac");
    let hash = crate::file_sha256(&path).unwrap();
    let direct = rt
        .export_timing(&path, &hash, &rt.probe(&path).unwrap())
        .unwrap();
    assert_eq!(direct.codec_profile, CodecTimingProfile::Direct);
    assert_eq!(direct.decoded_sample_count, None);
    let unknown = rt
        .export_timing(&path, &hash, &source_info("unknown"))
        .unwrap();
    assert!(matches!(
        unknown.codec_profile,
        CodecTimingProfile::Unmeasured { .. }
    ));
    let mut other = rt.clone();
    other.identity.push_str("-changed");
    assert_ne!(other.export_timing_identity(), rt.export_timing_identity());
    assert_eq!(
        serde_json::from_str::<SourceExportTiming>(&serde_json::to_string(&direct).unwrap())
            .unwrap(),
        direct
    );
}
