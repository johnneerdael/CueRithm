use cuerithm_media::MediaRuntime;
use std::path::{Path, PathBuf};

fn runtime() -> MediaRuntime {
    let root = std::env::var_os("CUERITHM_MEDIA_RUNTIME")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools/media-runtime/bin")
        });
    MediaRuntime::from_bundle(&root)
        .expect("packaged decoder must be present; do not skip codec coverage")
}

#[test]
fn all_common_formats_decode_to_a_complete_playable_clock() {
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
    let rt = runtime();
    let mut lossless = None;
    for row in cases.as_array().unwrap() {
        let path = d.path().join(row["file"].as_str().unwrap());
        let info = rt.probe(&path).unwrap();
        assert!(!info.codec.is_empty());
        let audio = rt.decode(&path, d.path()).unwrap();
        assert_eq!(
            audio.frames,
            row["frames"].as_u64().unwrap(),
            "{}",
            path.display()
        );
        assert_eq!(audio.clock.duration_seconds(), audio.frames as f64 / 44100.);
        if row["lossless"] == true {
            assert_eq!(
                audio.pcm_sha256().unwrap(),
                row["sha256"].as_str().unwrap(),
                "{}",
                path.display()
            );
        }
        for point in row["checkpoints"].as_array().unwrap() {
            let index = point[0].as_u64().unwrap();
            let actual = audio.read_frames(index / 2, 1).unwrap()[(index % 2) as usize];
            assert!(
                (f64::from(actual) - point[1].as_f64().unwrap()).abs() < 0.00005,
                "decoded sample at {index} differs"
            );
        }
        if row["lossless"] == true {
            let h = audio.pcm_sha256().unwrap();
            if let Some(ref first) = lossless {
                assert_eq!(&h, first);
            } else {
                lossless = Some(h);
            }
        }
    }
}

#[test]
fn handles_unicode_paths_wrong_extensions_and_corrupt_media() {
    let rt = runtime();
    let d = tempfile::tempdir().unwrap();
    let source = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../camelot-source-analysis/tests/fixtures/pulses64-stereo16.flac");
    let path = d.path().join("- 演奏 wrong suffix.bin");
    std::fs::copy(&source, &path).unwrap();
    assert_eq!(rt.probe(&path).unwrap().codec, "flac");
    let audio = rt.decode(&path, d.path()).unwrap();
    let hash = audio.source_sha256.clone();
    std::fs::write(&path, b"replaced").unwrap();
    assert!(!audio.source_is_unchanged().unwrap());
    assert_eq!(audio.source_sha256, hash);
    assert!(rt.decode(&path, d.path()).is_err());
    assert!(MediaRuntime::from_bundle(d.path()).is_err());
}

#[test]
fn rejects_truncated_flac_nonfinite_pcm_and_modified_runtime() {
    let rt = runtime();
    let d = tempfile::tempdir().unwrap();
    let source = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../camelot-source-analysis/tests/fixtures/pulses64-stereo16.flac");
    let mut bytes = std::fs::read(source).unwrap();
    bytes.truncate(bytes.len() / 2 + 7);
    let path = d.path().join("broken.flac");
    std::fs::write(&path, bytes).unwrap();
    assert!(rt.decode(&path, d.path()).is_err());
    let mut wav = Vec::new();
    wav.extend(b"RIFF");
    wav.extend(44_u32.to_le_bytes());
    wav.extend(b"WAVEfmt ");
    wav.extend(16_u32.to_le_bytes());
    wav.extend(3_u16.to_le_bytes());
    wav.extend(2_u16.to_le_bytes());
    wav.extend(44100_u32.to_le_bytes());
    wav.extend(352800_u32.to_le_bytes());
    wav.extend(8_u16.to_le_bytes());
    wav.extend(32_u16.to_le_bytes());
    wav.extend(b"data");
    wav.extend(8_u32.to_le_bytes());
    wav.extend(f32::NAN.to_le_bytes());
    wav.extend(0_f32.to_le_bytes());
    let path = d.path().join("nan.wav");
    std::fs::write(&path, wav).unwrap();
    assert!(rt.decode(&path, d.path()).is_err());
    std::fs::write(
        d.path().join(if cfg!(windows) {
            "ffmpeg.exe"
        } else {
            "ffmpeg"
        }),
        b"modified",
    )
    .unwrap();
    std::fs::write(
        d.path().join("runtime.json"),
        br#"{"ffmpeg_sha256":"invalid","ffprobe_sha256":"invalid","recipe":"test"}"#,
    )
    .unwrap();
    assert!(MediaRuntime::from_bundle(d.path()).is_err());
}
