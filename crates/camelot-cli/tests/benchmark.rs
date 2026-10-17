use serde_json::json;
use std::process::Command;

fn corpus(temp: &std::path::Path) -> (std::path::PathBuf, std::path::PathBuf) {
    let audio = temp.join("silent.wav");
    let mut writer = hound::WavWriter::create(
        &audio,
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
    let input_path = temp.join("inputs.jsonl");
    let reference_path = temp.join("references.jsonl");
    let mut inputs = Vec::new();
    let mut references = Vec::new();
    for (id, group, split, path) in [
        ("a", "g1", "tuning", audio.clone()),
        ("b", "g1", "tuning", audio),
        ("c", "g2", "tuning", temp.join("missing.wav")),
        ("d", "g3", "holdout", temp.join("holdout.wav")),
    ] {
        inputs.push(json!({"id": id, "group": group, "split": split, "path": path}));
        references.push(json!({"id": id, "group": group, "split": split, "key": "8A", "bpm": 125.0, "cues": [{"time_seconds": 0.4, "name": "Energy 6"}]}));
    }
    std::fs::write(
        &input_path,
        inputs.iter().map(|v| format!("{v}\n")).collect::<String>(),
    )
    .unwrap();
    std::fs::write(
        &reference_path,
        references
            .iter()
            .map(|v| format!("{v}\n"))
            .collect::<String>(),
    )
    .unwrap();
    (input_path, reference_path)
}

#[test]
fn benchmark_deduplicates_audio_and_counts_failures_without_using_labels() {
    let temp = tempfile::tempdir().unwrap();
    let (inputs, references) = corpus(temp.path());
    let output = temp.path().join("run");
    let result = Command::new(env!("CARGO_BIN_EXE_camelot"))
        .args(["benchmark", "--inputs"])
        .arg(inputs)
        .arg("--references")
        .arg(references)
        .args(["--split", "tuning", "--output-dir"])
        .arg(&output)
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let report: serde_json::Value =
        serde_json::from_slice(&std::fs::read(output.join("report.json")).unwrap()).unwrap();
    assert_eq!(report["score"]["tracks"], 2);
    assert_eq!(report["score"]["failed_tracks"], 1);
    assert_eq!(report["score"]["key_predictions"], 0);
    assert_eq!(report["score"]["exact_key_matches"], 0);
    assert_eq!(report["score"]["cues"]["reference_count"], 2);
    let predictions = std::fs::read_to_string(output.join("predictions.jsonl")).unwrap();
    assert_eq!(predictions.lines().count(), 2);
}

#[test]
fn split_leakage_is_rejected_before_analysis() {
    let temp = tempfile::tempdir().unwrap();
    let (inputs, references) = corpus(temp.path());
    let mut rows: Vec<serde_json::Value> = std::fs::read_to_string(&inputs)
        .unwrap()
        .lines()
        .map(|s| serde_json::from_str(s).unwrap())
        .collect();
    rows[1]["split"] = json!("holdout");
    std::fs::write(
        &inputs,
        rows.iter().map(|v| format!("{v}\n")).collect::<String>(),
    )
    .unwrap();
    let result = Command::new(env!("CARGO_BIN_EXE_camelot"))
        .args(["benchmark", "--inputs"])
        .arg(inputs)
        .arg("--references")
        .arg(references)
        .args(["--split", "tuning", "--output-dir"])
        .arg(temp.path().join("run"))
        .output()
        .unwrap();
    assert!(!result.status.success());
    assert!(String::from_utf8_lossy(&result.stderr).contains("split"));
}

fn run_benchmark(
    inputs: &std::path::Path,
    references: &std::path::Path,
    output: &std::path::Path,
    extra: &[&str],
) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_camelot"))
        .args(["benchmark", "--inputs"])
        .arg(inputs)
        .arg("--references")
        .arg(references)
        .args(["--split", "tuning", "--output-dir"])
        .arg(output)
        .args(extra)
        .output()
        .unwrap()
}

#[test]
fn resume_does_not_duplicate_completed_predictions() {
    let temp = tempfile::tempdir().unwrap();
    let (inputs, references) = corpus(temp.path());
    let output = temp.path().join("run");
    assert!(
        run_benchmark(&inputs, &references, &output, &[])
            .status
            .success()
    );
    let before = std::fs::read(output.join("predictions.jsonl")).unwrap();
    let resumed = run_benchmark(&inputs, &references, &output, &["--resume"]);
    assert!(
        resumed.status.success(),
        "{}",
        String::from_utf8_lossy(&resumed.stderr)
    );
    assert_eq!(
        std::fs::read(output.join("predictions.jsonl")).unwrap(),
        before
    );
}

#[test]
fn resume_rejects_changed_analysis_configuration() {
    let temp = tempfile::tempdir().unwrap();
    let (inputs, references) = corpus(temp.path());
    let output = temp.path().join("run");
    assert!(
        run_benchmark(&inputs, &references, &output, &[])
            .status
            .success()
    );
    let result = run_benchmark(
        &inputs,
        &references,
        &output,
        &["--resume", "--max-seconds", "1"],
    );
    assert!(!result.status.success());
    assert!(String::from_utf8_lossy(&result.stderr).contains("Cannot resume"));
}

#[test]
fn conflicting_reference_labels_for_audio_aliases_are_rejected() {
    let temp = tempfile::tempdir().unwrap();
    let (inputs, references) = corpus(temp.path());
    let mut rows: Vec<serde_json::Value> = std::fs::read_to_string(&references)
        .unwrap()
        .lines()
        .map(|s| serde_json::from_str(s).unwrap())
        .collect();
    rows[1]["bpm"] = json!(130.0);
    std::fs::write(
        &references,
        rows.iter().map(|v| format!("{v}\n")).collect::<String>(),
    )
    .unwrap();
    let result = run_benchmark(&inputs, &references, &temp.path().join("run"), &[]);
    assert!(!result.status.success());
    assert!(String::from_utf8_lossy(&result.stderr).contains("Conflicting reference labels"));
}

#[test]
fn resume_rejects_a_changed_completed_source() {
    let temp = tempfile::tempdir().unwrap();
    let (inputs, references) = corpus(temp.path());
    let mut rows: Vec<serde_json::Value> = std::fs::read_to_string(&inputs)
        .unwrap()
        .lines()
        .map(|s| serde_json::from_str(s).unwrap())
        .collect();
    rows[0]["file_size"] = json!(
        std::fs::metadata(temp.path().join("silent.wav"))
            .unwrap()
            .len()
    );
    std::fs::write(
        &inputs,
        rows.iter().map(|r| format!("{r}\n")).collect::<String>(),
    )
    .unwrap();
    let output = temp.path().join("run");
    assert!(
        run_benchmark(&inputs, &references, &output, &[])
            .status
            .success()
    );
    let checkpoint = std::fs::read(output.join("predictions.jsonl")).unwrap();
    use std::io::Write;
    std::fs::OpenOptions::new()
        .append(true)
        .open(temp.path().join("silent.wav"))
        .unwrap()
        .write_all(b"changed")
        .unwrap();
    let resumed = run_benchmark(&inputs, &references, &output, &["--resume"]);
    assert!(!resumed.status.success());
    assert!(String::from_utf8_lossy(&resumed.stderr).contains("Cached source"));
    assert_eq!(
        std::fs::read(output.join("predictions.jsonl")).unwrap(),
        checkpoint
    );
}

#[test]
fn resume_recovers_only_an_incomplete_final_record_and_retries_it() {
    let temp = tempfile::tempdir().unwrap();
    let (inputs, references) = corpus(temp.path());
    let output = temp.path().join("run");
    assert!(
        run_benchmark(&inputs, &references, &output, &[])
            .status
            .success()
    );
    let path = output.join("predictions.jsonl");
    let original = std::fs::read(&path).unwrap();
    let first_end = original.iter().position(|b| *b == b'\n').unwrap() + 1;
    std::fs::write(&path, &original[..first_end + 16]).unwrap();
    let resumed = run_benchmark(&inputs, &references, &output, &["--resume"]);
    assert!(
        resumed.status.success(),
        "{}",
        String::from_utf8_lossy(&resumed.stderr)
    );
    let recovered = std::fs::read(&path).unwrap();
    assert_eq!(&recovered[..first_end], &original[..first_end]);
    let lines: Vec<serde_json::Value> = String::from_utf8(recovered)
        .unwrap()
        .lines()
        .map(|s| serde_json::from_str(s).unwrap())
        .collect();
    assert_eq!(lines.len(), 2);
    assert_eq!(lines[1]["id"], "c");
    assert!(std::fs::read_dir(&output).unwrap().any(|p| {
        p.unwrap()
            .file_name()
            .to_string_lossy()
            .starts_with("predictions.incomplete-")
    }));
}

#[test]
fn resume_finishes_a_valid_final_json_record_missing_its_newline() {
    let temp = tempfile::tempdir().unwrap();
    let (inputs, references) = corpus(temp.path());
    let output = temp.path().join("run");
    assert!(
        run_benchmark(&inputs, &references, &output, &[])
            .status
            .success()
    );
    let path = output.join("predictions.jsonl");
    let original = std::fs::read(&path).unwrap();
    std::fs::write(&path, &original[..original.len() - 1]).unwrap();
    assert!(
        run_benchmark(&inputs, &references, &output, &["--resume"])
            .status
            .success()
    );
    assert_eq!(std::fs::read(path).unwrap(), original);
}

#[test]
fn resume_does_not_repair_committed_malformed_records() {
    let temp = tempfile::tempdir().unwrap();
    let (inputs, references) = corpus(temp.path());
    let output = temp.path().join("run");
    assert!(
        run_benchmark(&inputs, &references, &output, &[])
            .status
            .success()
    );
    let path = output.join("predictions.jsonl");
    std::fs::write(&path, b"{\"id\":\"committed-but-broken\"\n").unwrap();
    let before = std::fs::read(&path).unwrap();
    assert!(
        !run_benchmark(&inputs, &references, &output, &["--resume"])
            .status
            .success()
    );
    assert_eq!(std::fs::read(path).unwrap(), before);
}
