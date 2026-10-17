use anyhow::{Context, Result, ensure};
use camelot_core::{
    Cue, MusicalKey,
    analysis::engine_source_sha256,
    evaluation::{Comparison, evaluate},
};
use camelot_core::{Engine, EngineSettings, FileAnalysis};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, HashMap, HashSet},
    fs::{self, OpenOptions},
    io::{BufWriter, Write},
    path::{Path, PathBuf},
    time::{Instant, UNIX_EPOCH},
};

#[derive(Debug, Clone, Deserialize)]
struct Input {
    id: String,
    group: String,
    split: String,
    path: PathBuf,
    file_size: Option<u64>,
    mtime_ns: Option<u64>,
}

#[derive(Debug, Clone, Deserialize)]
struct Reference {
    id: String,
    group: String,
    split: String,
    key: String,
    bpm: Option<f64>,
    cues: Vec<Cue>,
}

#[derive(Debug, Serialize, Deserialize)]
struct Prediction {
    id: String,
    group: String,
    split: String,
    result: Option<FileAnalysis>,
    error: Option<String>,
    elapsed_seconds: f64,
}

fn read_jsonl<T: serde::de::DeserializeOwned>(path: &Path) -> Result<Vec<T>> {
    let source = fs::read_to_string(path)?;
    source
        .lines()
        .enumerate()
        .filter(|(_, line)| !line.trim().is_empty())
        .map(|(i, line)| {
            serde_json::from_str(line)
                .with_context(|| format!("Parse {} line {}", path.display(), i + 1))
        })
        .collect()
}

enum CheckpointRepair {
    AddNewline,
    Truncate { length: u64, original: Vec<u8> },
}

fn read_checkpoint(path: &Path) -> Result<(Vec<Prediction>, Option<CheckpointRepair>)> {
    let bytes = fs::read(path)?;
    let committed = bytes.iter().rposition(|b| *b == b'\n').map_or(0, |i| i + 1);
    let mut predictions = Vec::new();
    for (index, line) in bytes[..committed].split(|b| *b == b'\n').enumerate() {
        if line.iter().all(u8::is_ascii_whitespace) {
            continue;
        }
        predictions.push(
            serde_json::from_slice(line).with_context(|| {
                format!("Parse {} committed line {}", path.display(), index + 1)
            })?,
        );
    }
    let tail = &bytes[committed..];
    if tail.is_empty() {
        return Ok((predictions, None));
    }
    if tail.iter().all(u8::is_ascii_whitespace) {
        return Ok((predictions, Some(CheckpointRepair::AddNewline)));
    }
    match serde_json::from_slice(tail) {
        Ok(prediction) => {
            predictions.push(prediction);
            Ok((predictions, Some(CheckpointRepair::AddNewline)))
        }
        Err(error) if error.is_eof() => Ok((
            predictions,
            Some(CheckpointRepair::Truncate {
                length: committed as u64,
                original: bytes,
            }),
        )),
        Err(error) => Err(error).context("Malformed final checkpoint record; refusing recovery"),
    }
}

fn repair_checkpoint(path: &Path, repair: Option<CheckpointRepair>) -> Result<()> {
    match repair {
        None => {}
        Some(CheckpointRepair::AddNewline) => {
            OpenOptions::new()
                .append(true)
                .open(path)?
                .write_all(b"\n")?;
        }
        Some(CheckpointRepair::Truncate { length, original }) => {
            // Preserve the interrupted checkpoint before discarding its uncommitted tail.
            let digest = format!("{:x}", Sha256::digest(&original));
            let backup = path.with_file_name(format!("predictions.incomplete-{digest}.jsonl"));
            match OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&backup)
            {
                Ok(mut file) => {
                    file.write_all(&original)?;
                    file.sync_all()?;
                }
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                    ensure!(fs::read(&backup)? == original, "Checkpoint backup differs");
                }
                Err(error) => return Err(error.into()),
            }
            OpenOptions::new().write(true).open(path)?.set_len(length)?;
        }
    }
    Ok(())
}

fn hash_file(path: &Path) -> Result<String> {
    use std::io::Read;
    let mut hash = Sha256::new();
    let mut file = fs::File::open(path)?;
    let mut buffer = [0_u8; 65536];
    loop {
        let count = file.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        hash.update(&buffer[..count]);
    }
    Ok(format!("{:x}", hash.finalize()))
}

fn source_unchanged(input: &Input) -> Result<()> {
    let stat = input.path.metadata()?;
    if let Some(size) = input.file_size {
        ensure!(
            stat.len() == size,
            "Source size differs from corpus manifest"
        );
    }
    if let Some(time) = input.mtime_ns {
        let actual = stat.modified()?.duration_since(UNIX_EPOCH)?.as_nanos();
        ensure!(
            actual == time as u128,
            "Source modification time differs from corpus manifest"
        );
    }
    Ok(())
}

fn select(
    inputs: Vec<Input>,
    references: Vec<Reference>,
    split: &str,
    limit: Option<usize>,
) -> Result<Vec<(Input, Reference)>> {
    let mut ids = HashSet::new();
    let mut groups = BTreeMap::<String, Vec<Input>>::new();
    for input in inputs {
        ensure!(ids.insert(input.id.clone()), "Duplicate corpus input ID");
        ensure!(
            matches!(input.split.as_str(), "tuning" | "holdout"),
            "Unknown corpus split"
        );
        groups.entry(input.group.clone()).or_default().push(input);
    }
    let mut refs = HashMap::new();
    for reference in references {
        ensure!(
            refs.insert(reference.id.clone(), reference).is_none(),
            "Duplicate reference ID"
        );
    }
    let mut selected = Vec::new();
    for aliases in groups.values_mut() {
        aliases.sort_by(|a, b| a.id.cmp(&b.id));
        let first = &aliases[0];
        ensure!(
            aliases.iter().all(|a| a.split == first.split),
            "Audio group crosses tuning/holdout split"
        );
        if first.split != split {
            continue;
        }
        let first_ref = refs
            .get(&first.id)
            .context("Missing reference for selected input")?;
        MusicalKey::from_camelot(&first_ref.key)?;
        for alias in aliases.iter() {
            let reference = refs.get(&alias.id).context("Missing reference alias")?;
            ensure!(
                reference.group == alias.group && reference.split == alias.split,
                "Input/reference group or split mismatch"
            );
            ensure!(
                reference.key == first_ref.key
                    && reference.bpm == first_ref.bpm
                    && reference.cues == first_ref.cues,
                "Conflicting reference labels within one audio group"
            );
        }
        selected.push((first.clone(), first_ref.clone()));
    }
    if let Some(count) = limit {
        ensure!(count > 0, "Limit must be positive");
        selected.truncate(count);
    }
    ensure!(!selected.is_empty(), "No selected audio groups");
    Ok(selected)
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn run(
    inputs_path: &Path,
    refs_path: &Path,
    split: &str,
    output: &Path,
    limit: Option<usize>,
    resume: bool,
    cue_tolerance: f64,
    settings: &EngineSettings,
) -> Result<Value> {
    ensure!(
        cue_tolerance.is_finite() && cue_tolerance >= 0.0,
        "Invalid cue tolerance"
    );
    let selected = select(
        read_jsonl(inputs_path)?,
        read_jsonl(refs_path)?,
        split,
        limit,
    )?;
    let mut engine = Engine::new(settings)?;
    let metadata = json!({
        "schema_version": 1, "engine_source_sha256": engine_source_sha256(),
        "binary_sha256": hash_file(&std::env::current_exe()?)?,
        "inputs_sha256": hash_file(inputs_path)?, "references_sha256": hash_file(refs_path)?,
        "split": split, "limit": limit, "settings": settings,
        "selected_groups": selected.iter().map(|(i, _)| &i.group).collect::<Vec<_>>(),
        "cue_tolerance_seconds": cue_tolerance,
        "models": engine.model_identity(),
    });
    fs::create_dir_all(output)?;
    let metadata_path = output.join("run.json");
    let predictions_path = output.join("predictions.jsonl");
    if metadata_path.exists() {
        ensure!(
            resume,
            "Output already contains a run; use a new directory or --resume"
        );
        let previous: Value = serde_json::from_slice(&fs::read(&metadata_path)?)?;
        ensure!(
            previous == metadata,
            "Cannot resume: engine, configuration, corpus or selection changed"
        );
    } else {
        ensure!(
            !predictions_path.exists(),
            "Predictions exist without run metadata"
        );
        fs::write(&metadata_path, serde_json::to_vec_pretty(&metadata)?)?;
    }
    let (mut predictions, repair) = if predictions_path.exists() {
        read_checkpoint(&predictions_path)?
    } else {
        (Vec::new(), None)
    };
    let selected_by_id: HashMap<_, _> = selected.iter().map(|(i, _)| (i.id.as_str(), i)).collect();
    let mut completed = HashSet::new();
    for prediction in &predictions {
        let input = selected_by_id
            .get(prediction.id.as_str())
            .context("Invalid resumed prediction ID")?;
        ensure!(
            prediction.group == input.group
                && prediction.split == input.split
                && completed.insert(prediction.id.clone()),
            "Invalid or duplicate resumed prediction"
        );
        if prediction.result.is_some() {
            source_unchanged(input)
                .context("Cached source changed or disappeared; start a new run")?;
        }
    }
    repair_checkpoint(&predictions_path, repair)?;
    let mut writer = BufWriter::new(
        OpenOptions::new()
            .create(true)
            .append(true)
            .open(&predictions_path)?,
    );
    let start = Instant::now();
    for (index, (input, _)) in selected.iter().enumerate() {
        if completed.contains(&input.id) {
            continue;
        }
        let item_start = Instant::now();
        // Reference is deliberately absent from this invocation.
        let result = source_unchanged(input).and_then(|()| engine.analyze_file(&input.path));
        let (result, error) = match result {
            Ok(result) => (Some(result), None),
            Err(error) => (None, Some(format!("{error:#}"))),
        };
        if (index + 1) % 10 == 0 || index == 0 || index + 1 == selected.len() {
            eprintln!(
                "[{}/{}] {} elapsed={:.1}s",
                index + 1,
                selected.len(),
                if error.is_none() {
                    "analyzed"
                } else {
                    "failed"
                },
                start.elapsed().as_secs_f64()
            );
        }
        let prediction = Prediction {
            id: input.id.clone(),
            group: input.group.clone(),
            split: input.split.clone(),
            result,
            error,
            elapsed_seconds: item_start.elapsed().as_secs_f64(),
        };
        serde_json::to_writer(&mut writer, &prediction)?;
        writer.write_all(b"\n")?;
        writer.flush()?;
        predictions.push(prediction);
    }
    let by_id: HashMap<_, _> = predictions.iter().map(|p| (p.id.as_str(), p)).collect();
    let mut comparisons = Vec::new();
    for (input, reference) in &selected {
        let prediction = by_id
            .get(input.id.as_str())
            .context("Selected track has no prediction")?;
        ensure!(
            prediction.group == input.group && prediction.split == split,
            "Prediction identity mismatch"
        );
        let analysis = prediction.result.as_ref().map(|r| &r.analysis);
        comparisons.push(Comparison {
            reference_key: MusicalKey::from_camelot(&reference.key)?,
            reference_bpm: reference.bpm,
            reference_cues: reference.cues.clone(),
            predicted_key: analysis.and_then(|a| a.key.as_ref()).map(|k| k.key),
            predicted_bpm: analysis.and_then(|a| a.tempo.as_ref()).map(|t| t.bpm),
            predicted_cues: analysis.and_then(|a| a.cues.clone()),
            failed: prediction.result.is_none(),
        });
    }
    let report = json!({"run": metadata, "score": evaluate(&comparisons, cue_tolerance)?,
        "analysis_elapsed_seconds": predictions.iter().map(|p| p.elapsed_seconds).sum::<f64>(),
        "complete_audio_tracks": predictions.iter().filter(|p| p.result.as_ref().is_some_and(|r| r.complete)).count(),
        "verified_integrity_tracks": predictions.iter().filter(|p| p.result.as_ref().is_some_and(|r| r.verified_integrity == Some(true))).count(),
    });
    fs::write(
        output.join("report.json"),
        serde_json::to_vec_pretty(&report)?,
    )?;
    Ok(report)
}
