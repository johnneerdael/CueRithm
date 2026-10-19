mod benchmark;
mod export;
mod lexicon_benchmark;

use anyhow::{Context, Result};
use camelot_core::{AnalysisConfig, Engine, EngineSettings};
use clap::{Args, Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(
    name = "camelot",
    version,
    about = "Offline musical-key, tempo and cue analysis"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Export saved complete analysis as a new Rekordbox XML playlist.
    ExportRekordbox(export::ExportArgs),
    /// Analyze audio without consulting existing analysis tags or reference labels.
    Analyze {
        path: PathBuf,
        #[command(flatten)]
        engine: EngineArgs,
    },
    /// Compare Lexicon-compatible cue output against Lexicon's stored reference cues.
    LexiconBenchmark(lexicon_benchmark::LexiconBenchmarkArgs),
    /// Compare one prediction per audio group with external reference labels.
    Benchmark {
        #[arg(long)]
        inputs: PathBuf,
        #[arg(long)]
        references: PathBuf,
        #[arg(long, default_value = "tuning", value_parser = ["tuning", "holdout"])]
        split: String,
        #[arg(long)]
        output_dir: PathBuf,
        #[arg(long)]
        limit: Option<usize>,
        #[arg(long)]
        resume: bool,
        #[arg(long, default_value_t = 0.05)]
        cue_tolerance: f64,
        #[command(flatten)]
        engine: EngineArgs,
    },
}

#[derive(Args)]
struct EngineArgs {
    #[arg(long, default_value_t = 11025)]
    sample_rate: u32,
    /// Analyze only this many seconds; the result is explicitly marked partial.
    #[arg(long)]
    max_seconds: Option<f64>,
    /// JSON AnalysisConfig. Defaults are recorded in every benchmark run.
    #[arg(long)]
    config: Option<PathBuf>,
    #[arg(long, requires = "mel_model")]
    beat_model: Option<PathBuf>,
    #[arg(long, requires = "beat_model")]
    mel_model: Option<PathBuf>,
    /// Experimental learned key-profile JSON. No reference labels are read.
    #[arg(long)]
    key_model: Option<PathBuf>,
    /// Experimental pinned S-KEY ONNX model, evaluated on direct 22050 Hz PCM.
    #[arg(long, conflicts_with = "key_model")]
    key_neural_model: Option<PathBuf>,
    /// Pinned OpenKeyScan3 ONNX graph, evaluated on direct 44100 Hz PCM.
    #[arg(long, conflicts_with_all = ["key_model", "key_neural_model"])]
    key_openkeyscan_model: Option<PathBuf>,
    /// Include experimental PCM loudness/band/change features for cue development.
    #[arg(long)]
    structure_features: bool,
    /// Exact source-profile timing; requires complete 44100 Hz stereo 16-bit FLAC.
    #[arg(long, conflicts_with_all = ["max_seconds", "beat_model", "mel_model"])]
    source_profile_timing: bool,
    /// Complete local first-channel timing, using the fixed 98–195 BPM range.
    #[arg(long, conflicts_with_all = ["source_profile_timing", "max_seconds", "beat_model", "mel_model"])]
    lexicon_timing: bool,
    /// Generate structural cues using the Lexicon-compatible energy envelope algorithm.
    #[arg(long, alias = "cue-suggestions")]
    lexicon_cues: bool,
    /// Genre hint for Lexicon cue detection (e.g. "techno", "house").
    #[arg(long, requires = "lexicon_cues")]
    lexicon_genre: Option<String>,
    /// Local RBF SVC JSON for a single whole-track energy rating (complete profile FLAC).
    #[arg(long, conflicts_with = "max_seconds")]
    energy_model: Option<PathBuf>,
    /// Minimum breakdown length, in beats.
    #[arg(long, default_value_t = 64)]
    breakdown_min_beats: u32,
    /// Add an optional 16-beat emergency loop.
    #[arg(long, requires = "lexicon_cues")]
    emergency_loop: bool,
}

impl EngineArgs {
    fn settings(self) -> Result<EngineSettings> {
        let config = if let Some(path) = self.config {
            serde_json::from_slice(&std::fs::read(path)?).context("Read analysis configuration")?
        } else {
            AnalysisConfig::default()
        };
        config.validate()?;
        anyhow::ensure!(
            (8000..=192000).contains(&self.sample_rate),
            "Invalid analysis sample rate"
        );
        anyhow::ensure!(
            self.max_seconds.is_none_or(|x| x.is_finite() && x > 0.0),
            "Invalid analysis duration"
        );
        Ok(EngineSettings {
            config,
            sample_rate: self.sample_rate,
            max_seconds: self.max_seconds,
            beat_model: self.beat_model,
            mel_model: self.mel_model,
            key_model: self.key_model,
            key_neural_model: self.key_neural_model,
            key_openkeyscan_model: self.key_openkeyscan_model,
            structure_features: self.structure_features,
            source_profile_timing: self.source_profile_timing,
            lexicon_timing: self.lexicon_timing,
            cue_config: None,
            energy_model: self.energy_model,
            lexicon_energy: false,
            lexicon_tempo: None,
            cue_template: None,
            lexicon_config: self
                .lexicon_cues
                .then(|| camelot_core::lexicon_cues::LexiconConfig {
                    genre: self.lexicon_genre.clone(),
                    drop_at_start: camelot_core::lexicon_cues::DropAtStart::Never,
                    add_emergency_loop: self.emergency_loop,
                    min_cue_length_beats: self.breakdown_min_beats,
                }),
        })
    }
}

fn main() -> Result<()> {
    match Cli::parse().command {
        Command::ExportRekordbox(args) => export::run(args)?,
        Command::Analyze { path, engine } => {
            let result = Engine::new(&engine.settings()?)?.analyze_file(&path)?;
            println!("{}", serde_json::to_string_pretty(&result)?);
        }
        Command::LexiconBenchmark(args) => lexicon_benchmark::run(args)?,
        Command::Benchmark {
            inputs,
            references,
            split,
            output_dir,
            limit,
            resume,
            cue_tolerance,
            engine,
        } => {
            let settings = engine.settings()?;
            let report = benchmark::run(
                &inputs,
                &references,
                &split,
                &output_dir,
                limit,
                resume,
                cue_tolerance,
                &settings,
            )?;
            println!("{}", serde_json::to_string_pretty(&report)?);
        }
    }
    Ok(())
}

#[cfg(test)]
mod timing_args_tests {
    use super::*;

    #[test]
    fn lexicon_timing_is_explicit_and_rejects_other_timing_modes_before_analysis() {
        let cli = Cli::try_parse_from([
            "camelot",
            "analyze",
            "track.wav",
            "--lexicon-timing",
            "--lexicon-cues",
        ])
        .unwrap();
        let Command::Analyze { engine, .. } = cli.command else {
            panic!("Analyze command");
        };
        let settings = engine.settings().unwrap();
        assert!(settings.lexicon_timing && !settings.source_profile_timing);
        assert!(settings.validate().is_ok());
        for conflict in [
            vec!["--source-profile-timing"],
            vec!["--max-seconds", "64"],
            vec!["--beat-model", "beat.onnx", "--mel-model", "mel.onnx"],
        ] {
            let mut args = vec!["camelot", "analyze", "track.wav", "--lexicon-timing"];
            args.extend(conflict);
            assert!(Cli::try_parse_from(args).is_err());
        }
        let cli = Cli::try_parse_from(["camelot", "analyze", "track.flac"]).unwrap();
        let Command::Analyze { engine, .. } = cli.command else {
            panic!("Analyze command");
        };
        assert!(!engine.settings().unwrap().lexicon_timing);
    }
}
