use camelot_core::{
    Cue, FileAnalysis,
    cues::{CueConfig, suggest},
};
use serde::Deserialize;
use std::{
    collections::{HashMap, HashSet},
    fs::File,
    io::{BufRead, BufReader},
};

#[derive(Deserialize)]
struct Row {
    id: String,
    result: Option<FileAnalysis>,
}
#[derive(Deserialize)]
struct Golden {
    id: String,
    cues: Vec<Cue>,
}

fn rows<T: serde::de::DeserializeOwned>(path: &str) -> impl Iterator<Item = T> {
    BufReader::new(File::open(path).unwrap())
        .lines()
        .map(|line| serde_json::from_str(&line.unwrap()).unwrap())
}

#[test]
#[ignore = "Requires explicit private PCM features, beat predictions and independent Python cue output"]
fn rust_cue_selection_matches_independent_feature_pipeline() {
    let expected: Vec<Golden> = rows(&std::env::var("CAMELOT_CUE_GOLDEN").unwrap()).collect();
    assert!(!expected.is_empty());
    let ids: HashSet<_> = expected.iter().map(|r| r.id.as_str()).collect();
    let structure: HashMap<_, _> = rows::<Row>(&std::env::var("CAMELOT_CUE_STRUCTURE").unwrap())
        .filter(|r| ids.contains(r.id.as_str()))
        .map(|r| (r.id, r.result.unwrap().analysis.structure.unwrap()))
        .collect();
    // A live beat run may be appending a final record. Only the prefix through
    // the final newline is a committed checkpoint and is safe to inspect.
    let bytes = std::fs::read(std::env::var("CAMELOT_CUE_BEATS").unwrap()).unwrap();
    let end = bytes.iter().rposition(|b| *b == b'\n').unwrap() + 1;
    let beats: HashMap<_, _> = std::str::from_utf8(&bytes[..end])
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str::<Row>(line).unwrap())
        .filter(|r| ids.contains(r.id.as_str()))
        .map(|r| (r.id, r.result.unwrap().analysis.tempo.unwrap()))
        .collect();
    for row in &expected {
        let cues = suggest(&structure[&row.id], &beats[&row.id], &CueConfig::default()).unwrap();
        assert_eq!(cues.len(), row.cues.len(), "{}", row.id);
        for (a, b) in cues.iter().zip(&row.cues) {
            assert!(
                (a.time_seconds - b.time_seconds).abs() < 1e-7,
                "{}: {:?} vs {:?}",
                row.id,
                cues,
                row.cues
            );
            assert_eq!(a.name, b.name);
        }
    }
    eprintln!(
        "{} private tuning tracks matched independent cue selection",
        expected.len()
    );
}
