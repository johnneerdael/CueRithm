use anyhow::{Context, Result, ensure};
use camelot_core::lexicon_cues::{
    DropAtStart, LexiconConfig, StructuralCueResult,
    try_generate_structural_cues as generate_structural_cues,
};
use serde::Serialize;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

const LEXICON_DECODE_RATE: u32 = 44100;

#[derive(Debug, Clone)]
pub struct ReferenceCue {
    pub name: String,
    pub time_seconds: f64,
}

#[derive(Debug, Clone)]
pub struct ReferenceTrack {
    pub track_id: u64,
    pub title: String,
    pub artist: String,
    pub bpm: f64,
    pub genre: String,
    pub location: PathBuf,
    pub first_beat: f64,
    pub cues: Vec<ReferenceCue>,
}

#[derive(Debug, Serialize)]
pub struct CueMatch {
    pub cue_name: String,
    pub reference_time: f64,
    pub predicted_time: Option<f64>,
    pub delta_seconds: Option<f64>,
    pub matched: bool,
}

#[derive(Debug, Serialize)]
pub struct TrackResult {
    pub track_id: u64,
    pub title: String,
    pub artist: String,
    pub matches: Vec<CueMatch>,
    pub matched_count: usize,
    pub total_count: usize,
    pub elapsed_seconds: f64,
    pub error: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct BenchmarkReport {
    pub tracks_analyzed: usize,
    pub tracks_failed: usize,
    pub per_cue_type: BTreeMap<String, CueTypeStats>,
    pub overall_match_rate: f64,
    pub overall_matched: usize,
    pub overall_total: usize,
    pub results: Vec<TrackResult>,
}

#[derive(Debug, Default, Serialize)]
pub struct CueTypeStats {
    pub matched: usize,
    pub missing: usize,
    pub misplaced: usize,
    pub total: usize,
    pub mean_delta_seconds: Option<f64>,
}

fn map_lexicon_name_to_ours(name: &str) -> Option<&'static str> {
    match name {
        "Start" => Some("Start"),
        "Drop" => Some("Drop"),
        "Breakdown" => Some("Breakdown"),
        "Drop #2" => Some("Second Drop"),
        "Breakdown #2" => Some("Second Breakdown"),
        "End / Fade-out" => Some("Last beat"),
        "S.O.S." => None,
        _ => None,
    }
}

pub fn compare_single_track(
    pcm: &[f32],
    sample_rate: u32,
    bpm: f64,
    first_beat: f64,
    genre: Option<String>,
    reference_cues: &[ReferenceCue],
    tolerance: f64,
) -> Result<(Vec<CueMatch>, StructuralCueResult)> {
    ensure!(
        tolerance.is_finite() && tolerance >= 0.0,
        "Invalid cue tolerance"
    );
    let config = LexiconConfig {
        genre,
        drop_at_start: DropAtStart::Never,
        min_cue_length_beats: 64,
        add_emergency_loop: false,
    };
    let result = generate_structural_cues(pcm, sample_rate, bpm, first_beat, &config)?;

    let mut matches = Vec::new();
    for ref_cue in reference_cues {
        let our_name = match map_lexicon_name_to_ours(&ref_cue.name) {
            Some(n) => n,
            None => continue,
        };

        let predicted = result.cues.iter().find(|c| c.name == our_name);
        let delta = predicted.map(|p| (p.time_seconds - ref_cue.time_seconds).abs());
        let matched = delta.is_some_and(|d| d <= tolerance);

        matches.push(CueMatch {
            cue_name: ref_cue.name.clone(),
            reference_time: ref_cue.time_seconds,
            predicted_time: predicted.map(|p| p.time_seconds),
            delta_seconds: delta,
            matched,
        });
    }
    Ok((matches, result))
}

pub fn load_references(cues_csv: &Path, tempomarkers_csv: &Path) -> Result<Vec<ReferenceTrack>> {
    let cue_text = std::fs::read_to_string(cues_csv)
        .with_context(|| format!("Read {}", cues_csv.display()))?;
    let tempo_text = std::fs::read_to_string(tempomarkers_csv)
        .with_context(|| format!("Read {}", tempomarkers_csv.display()))?;

    let mut first_beats: BTreeMap<u64, (f64, f64)> = BTreeMap::new();
    for line in tempo_text.lines().skip(1) {
        let fields: Vec<&str> = line.splitn(3, ',').collect();
        if fields.len() < 3 {
            continue;
        }
        let track_id: u64 = fields[0].parse().context("Parse tempomarker track_id")?;
        let first_beat: f64 = fields[1].parse().context("Parse tempomarker first_beat")?;
        let bpm: f64 = fields[2].parse().context("Parse tempomarker bpm")?;
        first_beats.insert(track_id, (first_beat, bpm));
    }

    let mut tracks: BTreeMap<u64, ReferenceTrack> = BTreeMap::new();
    for line in cue_text.lines().skip(1) {
        let fields = parse_csv_line(line);
        if fields.len() < 11 {
            continue;
        }
        let track_id: u64 = fields[0].parse().context("Parse cue track_id")?;
        let title = fields[1].to_string();
        let artist = fields[2].to_string();
        let bpm: f64 = fields[3].parse().context("Parse cue bpm")?;
        let genre = fields[5].to_string();
        let location = PathBuf::from(&fields[6]);
        let cue_name = fields[8].to_string();
        let cue_time: f64 = fields[9].parse().context("Parse cue time")?;

        let (first_beat, _tempo_bpm) = first_beats.get(&track_id).copied().unwrap_or((0.0, bpm));

        let track = tracks.entry(track_id).or_insert_with(|| ReferenceTrack {
            track_id,
            title: title.clone(),
            artist: artist.clone(),
            bpm,
            genre: genre.clone(),
            location: location.clone(),
            first_beat,
            cues: Vec::new(),
        });

        track.cues.push(ReferenceCue {
            name: cue_name,
            time_seconds: cue_time,
        });
    }

    Ok(tracks.into_values().collect())
}

fn parse_csv_line(line: &str) -> Vec<String> {
    let mut fields = Vec::new();
    let mut current = String::new();
    let mut in_quotes = false;
    let mut chars = line.chars().peekable();

    while let Some(ch) = chars.next() {
        if in_quotes {
            if ch == '"' {
                if chars.peek() == Some(&'"') {
                    current.push('"');
                    chars.next();
                } else {
                    in_quotes = false;
                }
            } else {
                current.push(ch);
            }
        } else if ch == '"' {
            in_quotes = true;
        } else if ch == ',' {
            fields.push(std::mem::take(&mut current));
        } else {
            current.push(ch);
        }
    }
    fields.push(current);
    fields
}

pub fn run_benchmark(
    cues_csv: &Path,
    tempomarkers_csv: &Path,
    tolerance: f64,
    limit: Option<usize>,
) -> Result<BenchmarkReport> {
    ensure!(
        tolerance.is_finite() && tolerance >= 0.0,
        "Invalid cue tolerance"
    );
    let references = load_references(cues_csv, tempomarkers_csv)?;
    let tracks: Vec<_> = references
        .into_iter()
        .take(limit.unwrap_or(usize::MAX))
        .collect();

    eprintln!(
        "Benchmarking {} tracks (tolerance {:.3}s)",
        tracks.len(),
        tolerance
    );

    let mut results = Vec::new();
    let mut per_cue_type: BTreeMap<String, CueTypeStats> = BTreeMap::new();
    let mut overall_matched = 0_usize;
    let mut overall_total = 0_usize;
    let mut tracks_failed = 0_usize;

    for (i, track) in tracks.iter().enumerate() {
        eprint!("\r[{}/{}] {}", i + 1, tracks.len(), track.title,);

        let start = std::time::Instant::now();
        let decode_result = camelot_core::audio::decode(&track.location, LEXICON_DECODE_RATE, None);

        let counted = track
            .cues
            .iter()
            .filter(|c| map_lexicon_name_to_ours(&c.name).is_some())
            .count();
        overall_total += counted;
        let comparison = decode_result.and_then(|audio| {
            compare_single_track(
                &audio.samples,
                audio.sample_rate,
                track.bpm,
                track.first_beat,
                (!track.genre.is_empty()).then(|| track.genre.clone()),
                &track.cues,
                tolerance,
            )
        });
        let track_result = match comparison {
            Ok((matches, _predicted)) => {
                let matched_count = matches.iter().filter(|m| m.matched).count();
                let total_count = matches.len();
                overall_matched += matched_count;

                for m in &matches {
                    let stats = per_cue_type.entry(m.cue_name.clone()).or_default();
                    stats.total += 1;
                    if m.matched {
                        stats.matched += 1;
                    } else if m.predicted_time.is_some() {
                        stats.misplaced += 1;
                    } else {
                        stats.missing += 1;
                    }
                }

                TrackResult {
                    track_id: track.track_id,
                    title: track.title.clone(),
                    artist: track.artist.clone(),
                    matches,
                    matched_count,
                    total_count,
                    elapsed_seconds: start.elapsed().as_secs_f64(),
                    error: None,
                }
            }
            Err(e) => {
                tracks_failed += 1;
                for cue in &track.cues {
                    if map_lexicon_name_to_ours(&cue.name).is_some() {
                        let stats = per_cue_type.entry(cue.name.clone()).or_default();
                        stats.total += 1;
                        stats.missing += 1;
                    }
                }
                TrackResult {
                    track_id: track.track_id,
                    title: track.title.clone(),
                    artist: track.artist.clone(),
                    matches: Vec::new(),
                    matched_count: 0,
                    total_count: counted,
                    elapsed_seconds: start.elapsed().as_secs_f64(),
                    error: Some(format!("{:#}", e)),
                }
            }
        };

        results.push(track_result);
    }

    eprintln!();

    for (name, stats) in per_cue_type.iter_mut() {
        let deltas: Vec<f64> = results
            .iter()
            .flat_map(|r| &r.matches)
            .filter(|m| m.cue_name == *name)
            .filter_map(|m| m.delta_seconds)
            .collect();
        if !deltas.is_empty() {
            stats.mean_delta_seconds = Some(deltas.iter().sum::<f64>() / deltas.len() as f64);
        }
    }

    let overall_match_rate = if overall_total > 0 {
        overall_matched as f64 / overall_total as f64
    } else {
        0.0
    };

    Ok(BenchmarkReport {
        tracks_analyzed: results.len(),
        tracks_failed,
        per_cue_type,
        overall_match_rate,
        overall_matched,
        overall_total,
        results,
    })
}

#[derive(clap::Args)]
pub struct LexiconBenchmarkArgs {
    /// Path to a single audio file for quick comparison.
    #[arg(long)]
    pub file: Option<PathBuf>,
    /// BPM for single-file mode.
    #[arg(long, requires = "file")]
    pub bpm: Option<f64>,
    /// First beat time in seconds for single-file mode.
    #[arg(long, requires = "file")]
    pub first_beat: Option<f64>,
    /// Genre string for single-file mode.
    #[arg(long)]
    pub genre: Option<String>,
    /// Reference cue times (name:seconds pairs) for single-file comparison.
    #[arg(long, value_delimiter = ',')]
    pub reference: Vec<String>,
    /// Path to reference cues CSV (batch mode).
    #[arg(long, conflicts_with = "file")]
    pub cues_csv: Option<PathBuf>,
    /// Path to reference tempomarkers CSV (batch mode).
    #[arg(long, conflicts_with = "file")]
    pub tempomarkers_csv: Option<PathBuf>,
    /// Tolerance in seconds for cue matching.
    #[arg(long, default_value_t = 0.5)]
    pub tolerance: f64,
    /// Maximum number of tracks to process.
    #[arg(long)]
    pub limit: Option<usize>,
    /// Write detailed per-track results to this directory.
    #[arg(long)]
    pub output_dir: Option<PathBuf>,
}

pub fn run(args: LexiconBenchmarkArgs) -> Result<()> {
    if let Some(file) = &args.file {
        run_single(file, &args)
    } else if let (Some(cues_csv), Some(tempomarkers_csv)) =
        (&args.cues_csv, &args.tempomarkers_csv)
    {
        let report = run_benchmark(cues_csv, tempomarkers_csv, args.tolerance, args.limit)?;

        if let Some(output_dir) = &args.output_dir {
            std::fs::create_dir_all(output_dir)?;
            let path = output_dir.join("lexicon_benchmark_report.json");
            std::fs::write(&path, serde_json::to_string_pretty(&report)?)?;
            eprintln!("Detailed report: {}", path.display());
        }

        println!("\n=== Lexicon Cue Benchmark ===");
        println!(
            "Tracks: {} analyzed, {} failed",
            report.tracks_analyzed, report.tracks_failed
        );
        println!(
            "Overall: {}/{} cues matched ({:.1}%)",
            report.overall_matched,
            report.overall_total,
            report.overall_match_rate * 100.0
        );
        println!("\nPer cue type:");
        for (name, stats) in &report.per_cue_type {
            println!(
                "  {:<20} {}/{} matched, {} missing, {} misplaced",
                name, stats.matched, stats.total, stats.missing, stats.misplaced
            );
        }

        let mut mismatched: Vec<_> = report
            .results
            .iter()
            .filter(|r| r.matched_count < r.total_count && r.error.is_none())
            .collect();
        mismatched.sort_by(|a, b| {
            (a.matched_count as f64 / a.total_count.max(1) as f64)
                .partial_cmp(&(b.matched_count as f64 / b.total_count.max(1) as f64))
                .unwrap()
        });
        if !mismatched.is_empty() {
            println!("\nWorst mismatches (showing up to 10):");
            for r in mismatched.iter().take(10) {
                println!(
                    "  {} - {} ({}/{})",
                    r.artist, r.title, r.matched_count, r.total_count
                );
                for m in &r.matches {
                    if !m.matched {
                        println!(
                            "    {} ref={:.3}s pred={} delta={}",
                            m.cue_name,
                            m.reference_time,
                            m.predicted_time
                                .map(|t| format!("{:.3}s", t))
                                .unwrap_or_else(|| "MISSING".into()),
                            m.delta_seconds
                                .map(|d| format!("{:.3}s", d))
                                .unwrap_or_else(|| "-".into()),
                        );
                    }
                }
            }
        }

        Ok(())
    } else {
        anyhow::bail!(
            "Provide either --file for single-track mode or --cues-csv and --tempomarkers-csv for batch mode"
        );
    }
}

fn run_single(file: &Path, args: &LexiconBenchmarkArgs) -> Result<()> {
    let bpm = args.bpm.context("--bpm is required for single-file mode")?;
    let first_beat = args.first_beat.unwrap_or(0.0);

    ensure!(file.exists(), "File not found: {}", file.display());

    eprintln!(
        "Decoding {} at {} Hz...",
        file.display(),
        LEXICON_DECODE_RATE
    );
    let audio = camelot_core::audio::decode(file, LEXICON_DECODE_RATE, None)?;
    eprintln!(
        "Decoded {:.1}s @ {} Hz ({} samples)",
        audio.duration_seconds(),
        audio.sample_rate,
        audio.samples.len()
    );

    let config = LexiconConfig {
        genre: args.genre.clone(),
        drop_at_start: DropAtStart::Never,
        min_cue_length_beats: 64,
        add_emergency_loop: false,
    };

    let start = std::time::Instant::now();
    let result =
        generate_structural_cues(&audio.samples, audio.sample_rate, bpm, first_beat, &config)?;
    let elapsed = start.elapsed();

    println!("\n=== Predicted Cues ({:.3}s) ===", elapsed.as_secs_f64());
    for cue in &result.cues {
        println!(
            "  {:<20} {:.3}s  ({})",
            cue.name, cue.time_seconds, cue.color
        );
    }
    if let Some(el) = &result.emergency_loop {
        println!(
            "  {:<20} {:.3}s  ({} beats)",
            "Emergency Loop", el.time_seconds, el.loop_beats
        );
    }

    if !args.reference.is_empty() {
        let mut reference_cues = Vec::new();
        for r in &args.reference {
            let (name, time_str) = r
                .split_once(':')
                .context("Reference format: Name:seconds")?;
            reference_cues.push(ReferenceCue {
                name: name.to_string(),
                time_seconds: time_str.parse()?,
            });
        }

        let (matches, _) = compare_single_track(
            &audio.samples,
            audio.sample_rate,
            bpm,
            first_beat,
            args.genre.clone(),
            &reference_cues,
            args.tolerance,
        )?;

        println!("\n=== Comparison ===");
        for m in &matches {
            let status = if m.matched { "OK" } else { "MISS" };
            println!(
                "  [{:<4}] {:<20} ref={:.3}s pred={} delta={}",
                status,
                m.cue_name,
                m.reference_time,
                m.predicted_time
                    .map(|t| format!("{:.3}s", t))
                    .unwrap_or_else(|| "MISSING".into()),
                m.delta_seconds
                    .map(|d| format!("{:.3}s", d))
                    .unwrap_or_else(|| "-".into()),
            );
        }
        let matched = matches.iter().filter(|m| m.matched).count();
        println!(
            "\n{}/{} matched ({:.1}%)",
            matched,
            matches.len(),
            matched as f64 / matches.len().max(1) as f64 * 100.0
        );
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn missing_audio_is_a_failed_row_in_the_denominator() {
        let dir = tempfile::tempdir().unwrap();
        let cues = dir.path().join("cues.csv");
        let timing = dir.path().join("timing.csv");
        let missing = dir.path().join("missing.flac");
        std::fs::write(&cues,format!("id,title,artist,bpm,key,genre,location,cue_id,cue_name,time,color\n1,Fixture,Artist,120,8A,,{},0,Start,0,Yellow\n",missing.display())).unwrap();
        std::fs::write(&timing, "id,first_beat,bpm\n1,0,120\n").unwrap();
        let result = run_benchmark(&cues, &timing, 0.05, None).unwrap();
        assert_eq!(result.tracks_analyzed, 1);
        assert_eq!(result.tracks_failed, 1);
        assert_eq!(result.overall_total, 1);
        assert_eq!(result.overall_matched, 0);
        assert_eq!(result.per_cue_type["Start"].missing, 1);
    }
    #[test]
    fn invalid_tolerance_is_rejected_before_io() {
        assert!(
            run_benchmark(
                Path::new("absent"),
                Path::new("absent"),
                f64::INFINITY,
                None
            )
            .unwrap_err()
            .to_string()
            .contains("tolerance")
        );
    }
}
