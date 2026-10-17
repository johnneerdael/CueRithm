use std::{path::PathBuf, process::Command};

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../camelot-source-analysis/tests/fixtures")
        .join(name)
}

#[test]
fn feature_command_reports_unsupported_rate() {
    let result = Command::new(env!("CARGO_BIN_EXE_camelot"))
        .arg("analyze-features")
        .arg(fixture("stereo16-48000.flac"))
        .output()
        .unwrap();
    assert!(!result.status.success());
    assert!(String::from_utf8_lossy(&result.stderr).contains("44100"));
    assert!(result.stdout.is_empty());
}

#[test]
fn feature_command_emits_computed_request_inputs() {
    let result = Command::new(env!("CARGO_BIN_EXE_camelot"))
        .arg("analyze-features")
        .arg(fixture("pulses64-stereo16.flac"))
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let output: serde_json::Value = serde_json::from_slice(&result.stdout).unwrap();
    let analysis = output["analysis"].as_object().unwrap();
    assert_eq!(analysis.len(), 10);
    assert_eq!(analysis["KeyAlgorithmVersion"], 94);
    assert_eq!(analysis["EnergyAlgorithmVersion"], "2");
    assert_eq!(analysis["CuePointAlgorithmVersion"], "3");
    assert_eq!(
        analysis["Segments"][0]["PitchProbabilities"]
            .as_array()
            .unwrap()
            .len(),
        12
    );
    assert!(
        analysis["EnergySegmentData"]
            .as_array()
            .unwrap()
            .iter()
            .all(|s| s["Features"].as_array().unwrap().len() == 183)
    );
    assert!(!analysis.contains_key("Key"));
    assert!(!analysis.contains_key("EnergyLevel"));
    assert!(!analysis.contains_key("CuePoints"));
    assert!(!analysis.contains_key("VIPCode"));
}
