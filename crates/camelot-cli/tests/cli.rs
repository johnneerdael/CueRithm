use std::process::Command;

#[test]
fn analyze_returns_real_json_for_a_generated_silent_wav() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("fixture.wav");
    let mut writer = hound::WavWriter::create(
        &path,
        hound::WavSpec {
            channels: 1,
            sample_rate: 11025,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        },
    )
    .unwrap();
    for _ in 0..11025 * 2 {
        writer.write_sample(0_i16).unwrap();
    }
    writer.finalize().unwrap();
    let result = Command::new(env!("CARGO_BIN_EXE_camelot"))
        .args(["analyze"])
        .arg(&path)
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let data: serde_json::Value =
        serde_json::from_slice(&result.stdout).expect("CLI must return JSON");
    assert_eq!(data["analysis"]["duration_seconds"], 2.0);
    assert!(data["analysis"]["key"].is_null());
    assert!(data["analysis"]["tempo"].is_null());
    assert_eq!(data["complete"], true);
    assert!(data["analysis"].get("structure").is_none());
    let with_features = Command::new(env!("CARGO_BIN_EXE_camelot"))
        .arg("analyze")
        .arg(path)
        .arg("--structure-features")
        .output()
        .unwrap();
    assert!(with_features.status.success());
    let enriched: serde_json::Value = serde_json::from_slice(&with_features.stdout).unwrap();
    assert_eq!(
        enriched["analysis"]["structure"]["schema"],
        "centered-rms-six-band-flux-v1"
    );
    assert!(
        !enriched["analysis"]["structure"]["frames"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    assert!(enriched["analysis"]["cues"].is_null());
}

#[test]
fn missing_source_is_an_error_exit() {
    let result = Command::new(env!("CARGO_BIN_EXE_camelot"))
        .args(["analyze", "/no-such-audio-file.wav"])
        .output()
        .unwrap();
    assert!(!result.status.success());
}

fn source_fixture(name: &str) -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../camelot-source-analysis/tests/fixtures")
        .join(name)
}

#[test]
fn source_timing_flag_returns_complete_timing_without_confidence_or_cues() {
    let result = Command::new(env!("CARGO_BIN_EXE_camelot"))
        .arg("analyze")
        .arg(source_fixture("pulses64-stereo16.flac"))
        .arg("--source-profile-timing")
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let data: serde_json::Value = serde_json::from_slice(&result.stdout).unwrap();
    assert_eq!(data["complete"], true);
    assert_eq!(data["verified_integrity"], true);
    assert_eq!(data["source_timing"]["full_source_frames"], 64 * 44100);
    assert_eq!(
        data["analysis"]["tempo"]["method"],
        "source-profile-timing-v1"
    );
    assert!(data["analysis"]["tempo"]["bpm"].as_f64().unwrap() > 0.0);
    assert!(data["analysis"]["tempo"].get("strength").is_none());
    assert!(data["analysis"]["tempo"]["downbeat_time"].is_number());
    assert!(data["analysis"]["cues"].is_null());
}

#[test]
fn source_timing_flag_rejects_unsupported_audio_and_conflicting_flags() {
    let unsupported = Command::new(env!("CARGO_BIN_EXE_camelot"))
        .arg("analyze")
        .arg(source_fixture("stereo16-48000.flac"))
        .arg("--source-profile-timing")
        .output()
        .unwrap();
    assert!(!unsupported.status.success());
    assert!(String::from_utf8_lossy(&unsupported.stderr).contains("44100"));
    for extra in [
        vec!["--max-seconds", "64"],
        vec!["--beat-model", "beat.onnx", "--mel-model", "mel.onnx"],
        vec!["--cue-suggestions"],
    ] {
        let result = Command::new(env!("CARGO_BIN_EXE_camelot"))
            .args(["analyze", "missing.flac", "--source-profile-timing"])
            .args(extra)
            .output()
            .unwrap();
        assert_eq!(result.status.code(), Some(2));
        assert!(String::from_utf8_lossy(&result.stderr).contains("cannot be used with"));
    }
    let help = Command::new(env!("CARGO_BIN_EXE_camelot"))
        .args(["benchmark", "--help"])
        .output()
        .unwrap();
    assert!(help.status.success());
    assert!(String::from_utf8_lossy(&help.stdout).contains("--source-profile-timing"));
}

#[test]
fn openkeyscan_option_is_exposed_and_conflicts_with_other_key_backends() {
    let help = Command::new(env!("CARGO_BIN_EXE_camelot"))
        .args(["analyze", "--help"])
        .output()
        .unwrap();
    assert!(help.status.success());
    assert!(String::from_utf8_lossy(&help.stdout).contains("--key-openkeyscan-model"));
    for other in ["--key-model", "--key-neural-model"] {
        let result = Command::new(env!("CARGO_BIN_EXE_camelot"))
            .args([
                "analyze",
                "missing.flac",
                "--key-openkeyscan-model",
                "missing.onnx",
                other,
                "other.onnx",
            ])
            .output()
            .unwrap();
        assert_eq!(result.status.code(), Some(2));
        assert!(String::from_utf8_lossy(&result.stderr).contains("cannot be used with"));
    }
}
