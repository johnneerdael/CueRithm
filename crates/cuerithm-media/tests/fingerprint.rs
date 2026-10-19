use cuerithm_media::{CancellationToken, FingerprintOutcome, MediaRuntime};
use std::path::{Path, PathBuf};

fn runtime() -> MediaRuntime {
    let root = std::env::var_os("CUERITHM_MEDIA_RUNTIME")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools/media-runtime/bin")
        });
    MediaRuntime::from_bundle(&root).unwrap()
}
fn wav(path: &Path, rate: u32, channels: u16) {
    let spec = hound::WavSpec {
        channels,
        sample_rate: rate,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut out = hound::WavWriter::create(path, spec).unwrap();
    for i in 0..rate {
        for c in 0..channels {
            out.write_sample::<i16>((((i as i32 * 37) + (c as i32 * 13)) % 20001 - 10000) as i16)
                .unwrap();
        }
    }
    out.finalize().unwrap();
}

#[test]
fn native_fingerprint_matches_the_source_md5_clock_without_playback_resampling() {
    let fixture: serde_json::Value =
        serde_json::from_str(include_str!("fingerprint-reference.json")).unwrap();
    let rt = runtime();
    for case in fixture["cases"].as_array().unwrap() {
        let d = tempfile::tempdir().unwrap();
        let source = d.path().join("- 演奏.wav");
        let rate = case["sample_rate"].as_u64().unwrap() as u32;
        let channels = case["channels"].as_u64().unwrap() as u16;
        wav(&source, rate, channels);
        let before = cuerithm_media::file_sha256(&source).unwrap();
        let FingerprintOutcome::Audio(actual) = rt
            .fingerprint(&source, d.path(), &CancellationToken::default())
            .unwrap()
        else {
            panic!("Expected audio fingerprint")
        };
        assert_eq!(actual.sample_rate, rate);
        assert_eq!(actual.channels, u32::from(channels));
        assert_eq!(actual.digest, case["digest"].as_str().unwrap());
        assert_eq!(actual.source_sha256, before);
        assert_eq!(cuerithm_media::file_sha256(&source).unwrap(), before);
        assert_eq!(actual.pcm_format, "s16le");
    }
}

#[test]
fn audio_digest_ignores_metadata_bytes_but_pins_each_source_identity() {
    let rt = runtime();
    let d = tempfile::tempdir().unwrap();
    let a = d.path().join("a.wav");
    let b = d.path().join("b.wav");
    wav(&a, 44100, 2);
    let mut bytes = std::fs::read(&a).unwrap();
    bytes.extend_from_slice(b"LIST\x10\0\0\0INFOICMT\x04\0\0\0one\0");
    let len = (bytes.len() - 8) as u32;
    bytes[4..8].copy_from_slice(&len.to_le_bytes());
    std::fs::write(&b, bytes).unwrap();
    let FingerprintOutcome::Audio(first) = rt
        .fingerprint(&a, d.path(), &CancellationToken::default())
        .unwrap()
    else {
        panic!("Expected audio")
    };
    let FingerprintOutcome::Audio(second) = rt
        .fingerprint(&b, d.path(), &CancellationToken::default())
        .unwrap()
    else {
        panic!("Expected audio")
    };
    assert_ne!(first.source_sha256, second.source_sha256);
    assert_eq!(first.digest, second.digest);
    assert_eq!(first.decoder_identity, second.decoder_identity);
}

#[test]
fn every_common_format_has_a_native_digest_and_lossless_copies_group_together() {
    let rt = runtime();
    let d = tempfile::tempdir().unwrap();
    let archive = std::fs::File::open(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools/fixtures/media.tar.gz"),
    )
    .unwrap();
    tar::Archive::new(flate2::read::GzDecoder::new(archive))
        .unpack(d.path())
        .unwrap();
    let cases: serde_json::Value =
        serde_json::from_slice(&std::fs::read(d.path().join("cases.json")).unwrap()).unwrap();
    assert_eq!(cases.as_array().unwrap().len(), 14);
    let mut lossless = None;
    for case in cases.as_array().unwrap() {
        let source = d.path().join(case["file"].as_str().unwrap());
        let before = cuerithm_media::file_sha256(&source).unwrap();
        let FingerprintOutcome::Audio(actual) = rt
            .fingerprint(&source, d.path(), &CancellationToken::default())
            .unwrap()
        else {
            panic!("Expected audio: {case}")
        };
        assert_eq!(actual.source_sha256, before);
        assert_eq!(cuerithm_media::file_sha256(&source).unwrap(), before);
        assert_eq!(actual.digest.len(), 32);
        assert_eq!(actual.decoder_identity, rt.identity);
        if case["lossless"] == true {
            if let Some(previous) = &lossless {
                assert_eq!(&actual.digest, previous);
            } else {
                lossless = Some(actual.digest);
            }
        }
    }
}

#[test]
fn cancellation_and_symlink_inputs_do_not_start_a_capture() {
    let d = tempfile::tempdir().unwrap();
    let source = d.path().join("source.wav");
    wav(&source, 44100, 2);
    let token = CancellationToken::default();
    token.cancel();
    assert!(
        runtime()
            .fingerprint(&source, d.path(), &token)
            .unwrap_err()
            .to_string()
            .contains("cancel")
    );
    #[cfg(unix)]
    {
        let alias = d.path().join("alias.wav");
        std::os::unix::fs::symlink(&source, &alias).unwrap();
        assert!(matches!(
            runtime()
                .fingerprint(&alias, d.path(), &CancellationToken::default())
                .unwrap(),
            FingerprintOutcome::Symlink
        ));
    }
    assert_eq!(
        std::fs::read_dir(d.path())
            .unwrap()
            .filter_map(Result::ok)
            .filter(|e| e.file_name().to_string_lossy().starts_with("cuerithm-"))
            .count(),
        0
    );
}

#[test]
fn malformed_and_truncated_files_never_get_a_successful_digest() {
    let d = tempfile::tempdir().unwrap();
    let source = d.path().join("invalid.bin");
    std::fs::write(&source, b"not audio").unwrap();
    assert!(
        runtime()
            .fingerprint(&source, d.path(), &CancellationToken::default())
            .is_err()
    );
    let flac = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../camelot-source-analysis/tests/fixtures/pulses64-stereo16.flac");
    let mut bytes = std::fs::read(flac).unwrap();
    bytes.truncate(bytes.len() / 2 + 7);
    std::fs::write(&source, bytes).unwrap();
    assert!(
        runtime()
            .fingerprint(&source, d.path(), &CancellationToken::default())
            .is_err()
    );
}

#[test]
fn real_video_is_separate_from_embedded_poster_art_even_with_a_wrong_suffix() {
    let rt = runtime();
    let d = tempfile::tempdir().unwrap();
    let video = d.path().join("looks-like-audio.flac");
    std::fs::copy(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/authored-video.mp4"),
        &video,
    )
    .unwrap();
    let before = cuerithm_media::file_sha256(&video).unwrap();
    assert_eq!(
        rt.fingerprint(&video, d.path(), &CancellationToken::default())
            .unwrap(),
        FingerprintOutcome::Video
    );
    assert_eq!(cuerithm_media::file_sha256(&video).unwrap(), before);
    let art = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/covered.flac");
    assert!(matches!(
        rt.fingerprint(&art, d.path(), &CancellationToken::default())
            .unwrap(),
        FingerprintOutcome::Audio(_)
    ));
}

#[test]
fn default_audio_disposition_takes_priority_over_a_larger_channel_layout() {
    let reference: serde_json::Value =
        serde_json::from_str(include_str!("fingerprint-reference.json")).unwrap();
    let case = &reference["multi_stream"];
    let d = tempfile::tempdir().unwrap();
    let source = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(case["fixture"].as_str().unwrap());
    let FingerprintOutcome::Audio(actual) = runtime()
        .fingerprint(&source, d.path(), &CancellationToken::default())
        .unwrap()
    else {
        panic!("Expected audio")
    };
    assert_eq!(
        actual.channels,
        case["expected_channels"].as_u64().unwrap() as u32
    );
    assert_eq!(
        actual.sample_rate,
        case["expected_rate"].as_u64().unwrap() as u32
    );
    assert_eq!(actual.digest, case["expected_digest"].as_str().unwrap());
    assert_eq!(actual.audio_stream_index, 0);
    assert_eq!(actual.audio_stream_count, 2);
}

#[test]
fn a_well_formed_file_without_samples_cannot_be_an_exact_audio_candidate() {
    let d = tempfile::tempdir().unwrap();
    let source = d.path().join("empty.wav");
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: 8000,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    hound::WavWriter::create(&source, spec)
        .unwrap()
        .finalize()
        .unwrap();
    assert_eq!(
        runtime()
            .fingerprint(&source, d.path(), &CancellationToken::default())
            .unwrap(),
        FingerprintOutcome::NoAudio
    );
}
