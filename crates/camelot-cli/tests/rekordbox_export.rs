use camelot_core::{AnalysisConfig, Cue, FileAnalysis, analyze_pcm};
use std::{fs, process::Command};

fn fixture(dir: &std::path::Path, complete: bool) -> (std::path::PathBuf, std::path::PathBuf) {
    let audio = dir.join("test & café.wav");
    let mut wav = hound::WavWriter::create(
        &audio,
        hound::WavSpec {
            channels: 1,
            sample_rate: 11025,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        },
    )
    .unwrap();
    for _ in 0..11025 {
        wav.write_sample(0_i16).unwrap();
    }
    wav.finalize().unwrap();
    let mut analysis = analyze_pcm(&vec![0.0; 11025], 11025, &AnalysisConfig::default()).unwrap();
    analysis.cues = Some(vec![
        Cue {
            time_seconds: -0.005,
            name: "Start & cue".into(),
        },
        Cue {
            time_seconds: 0.75,
            name: "End".into(),
        },
    ]);
    let file = FileAnalysis {
        selection: None,
        selected_beatgrid: None,
        publication: None,
        analysis,
        complete,
        verified_integrity: None,
        source_sample_rate: 11025,
        beat_sample_rate: None,
        neural_key_sample_rate: None,
        openkeyscan_sample_rate: None,
        source_timing: None,
        elapsed_seconds: 0.0,
        media: None,
    };
    let saved = dir.join("analysis.json");
    fs::write(&saved, serde_json::to_vec(&file).unwrap()).unwrap();
    (audio, saved)
}

#[test]
fn exports_saved_analysis_without_modifying_audio_or_overwriting_output() {
    let dir = tempfile::tempdir().unwrap();
    let (audio, saved) = fixture(dir.path(), true);
    let before = fs::read(&audio).unwrap();
    let output = dir.path().join("practice.xml");
    let command = || {
        let mut c = Command::new(env!("CARGO_BIN_EXE_camelot"));
        c.arg("export-rekordbox")
            .arg("--audio")
            .arg(&audio)
            .arg("--analysis")
            .arg(&saved)
            .arg("--output")
            .arg(&output);
        c
    };
    let first = command().output().unwrap();
    assert!(
        first.status.success(),
        "{}",
        String::from_utf8_lossy(&first.stderr)
    );
    let report: serde_json::Value = serde_json::from_slice(&first.stdout).unwrap();
    assert_eq!(report["clamped_negative_cues"], 1);
    let xml = fs::read(&output).unwrap();
    assert!(String::from_utf8_lossy(&xml).contains("POSITION_MARK"));
    assert!(!command().output().unwrap().status.success());
    assert_eq!(fs::read(output).unwrap(), xml);
    assert_eq!(fs::read(audio).unwrap(), before);
}

#[test]
fn refuses_partial_analysis_and_unpaired_grid_flags() {
    let dir = tempfile::tempdir().unwrap();
    let (audio, saved) = fixture(dir.path(), false);
    let output = dir.path().join("partial.xml");
    let result = Command::new(env!("CARGO_BIN_EXE_camelot"))
        .arg("export-rekordbox")
        .arg("--audio")
        .arg(audio)
        .arg("--analysis")
        .arg(saved)
        .arg("--output")
        .arg(&output)
        .output()
        .unwrap();
    assert!(!result.status.success());
    assert!(String::from_utf8_lossy(&result.stderr).contains("partial"));
    assert!(!output.exists());
    let result = Command::new(env!("CARGO_BIN_EXE_camelot"))
        .args([
            "export-rekordbox",
            "--audio",
            "a.wav",
            "--analysis",
            "a.json",
            "--output",
            "a.xml",
            "--beat-grid-start",
            "0",
        ])
        .output()
        .unwrap();
    assert!(!result.status.success());
    assert!(String::from_utf8_lossy(&result.stderr).contains("--beat-grid-bpm"));
}
