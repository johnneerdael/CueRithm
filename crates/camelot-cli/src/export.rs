use anyhow::{Context, Result, ensure};
use camelot_core::{
    FileAnalysis,
    rekordbox::{self, ExportTrack, VerifiedBeatGrid},
};
use clap::Args;
use std::{fs::OpenOptions, io::Write, path::PathBuf};

#[derive(Args)]
pub struct ExportArgs {
    /// Audio file belonging to the saved analysis; source tags are not read.
    #[arg(long)]
    audio: PathBuf,
    /// JSON produced by `camelot analyze` for this audio file.
    #[arg(long)]
    analysis: PathBuf,
    /// New XML file. Existing files are never overwritten.
    #[arg(long)]
    output: PathBuf,
    #[arg(long)]
    title: Option<String>,
    #[arg(long, default_value = "")]
    artist: String,
    #[arg(long, default_value = "CamelotWheel")]
    playlist: String,
    /// Explicitly verified first beat of a 4/4 bar, in seconds. No grid by default.
    #[arg(long, requires = "beat_grid_bpm")]
    beat_grid_start: Option<f64>,
    /// Explicitly verified constant 4/4 beatgrid BPM; not inferred from a cue.
    #[arg(long, requires = "beat_grid_start")]
    beat_grid_bpm: Option<f64>,
}

pub fn run(args: ExportArgs) -> Result<()> {
    let saved: FileAnalysis =
        serde_json::from_slice(&std::fs::read(&args.analysis).context("Read saved analysis JSON")?)
            .context("Parse saved analysis JSON")?;
    ensure!(saved.complete, "Cannot export partial audio analysis");
    ensure!(
        saved.verified_integrity != Some(false),
        "Cannot export failed audio integrity verification"
    );
    let audio = args
        .audio
        .canonicalize()
        .context("Resolve audio file for export")?;
    ensure!(audio.is_file(), "Export audio path must be a file");
    let title = args
        .title
        .as_deref()
        .or_else(|| audio.file_stem()?.to_str())
        .unwrap_or("Untitled");
    let analysis = saved.analysis;
    let grid = args
        .beat_grid_start
        .zip(args.beat_grid_bpm)
        .map(|(start_seconds, bpm)| VerifiedBeatGrid { start_seconds, bpm });
    let track = ExportTrack {
        id: 1,
        path: &audio,
        title,
        artist: &args.artist,
        duration_seconds: analysis.duration_seconds,
        key: analysis.key.as_ref().map(|k| k.key),
        bpm: analysis.tempo.as_ref().map(|t| t.bpm),
        cues: analysis.cues.as_deref().unwrap_or(&[]),
        verified_grid: grid,
    };
    let document = rekordbox::render(&[track], &args.playlist)?;
    let mut output = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&args.output)
        .context("Create new XML output; existing files are not overwritten")?;
    output
        .write_all(document.xml.as_bytes())
        .context("Write Rekordbox XML")?;
    output.sync_all().context("Flush Rekordbox XML")?;
    println!(
        "{}",
        serde_json::json!({"output": args.output, "tracks": 1,
        "cues": analysis.cues.as_ref().map_or(0, Vec::len),
        "clamped_negative_cues": document.clamped_negative_cues, "beatgrid_exported": grid.is_some()})
    );
    Ok(())
}
