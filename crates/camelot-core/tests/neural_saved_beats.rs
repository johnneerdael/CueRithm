#![cfg(feature = "neural-beats")]

use camelot_core::neural::tempo_from_beats;
use std::{collections::HashMap, path::PathBuf};

#[test]
#[ignore = "Requires explicit private tuning-pilot predictions and reference manifests"]
fn corrected_postprocessor_matches_the_saved_twelve_track_tempo_regression_corpus() {
    let predictions_path = PathBuf::from(
        std::env::var("CAMELOT_BEAT_PREDICTIONS").expect("Set tuning pilot predictions"),
    );
    let refs_path =
        PathBuf::from(std::env::var("CAMELOT_REFERENCES").expect("Set reference manifest"));
    let references: HashMap<String, serde_json::Value> = std::fs::read_to_string(refs_path)
        .unwrap()
        .lines()
        .map(|line| {
            let row: serde_json::Value = serde_json::from_str(line).unwrap();
            (row["id"].as_str().unwrap().to_string(), row)
        })
        .collect();
    let mut count = 0;
    let mut within_half = 0;
    let mut exact = 0;
    let mut output = Vec::new();
    for line in std::fs::read_to_string(predictions_path).unwrap().lines() {
        let row: serde_json::Value = serde_json::from_str(line).unwrap();
        assert_eq!(row["split"], "tuning");
        let id = row["id"].as_str().unwrap();
        let reference = &references[id];
        assert_eq!(reference["split"], "tuning");
        let beats: Vec<f64> =
            serde_json::from_value(row["result"]["analysis"]["tempo"]["beats_seconds"].clone())
                .unwrap();
        let downbeats: Vec<f64> =
            serde_json::from_value(row["result"]["analysis"]["tempo"]["downbeats_seconds"].clone())
                .unwrap();
        // The postprocessor takes timestamps only, not reference BPM.
        let estimate = tempo_from_beats(&beats, &downbeats).unwrap();
        let target = reference["bpm"].as_f64().unwrap();
        within_half += usize::from((estimate.bpm - target).abs() <= 0.5);
        exact += usize::from(format!("{:.2}", estimate.bpm) == format!("{target:.2}"));
        count += 1;
        output.push(serde_json::json!({"id": id, "bpm": estimate.bpm, "reference_bpm": target}));
    }
    assert_eq!(
        count, 12,
        "This regression test is scoped to the frozen twelve-track pilot"
    );
    eprintln!(
        "Saved-detection postprocessing: {within_half}/{count} within 0.5 BPM; {exact}/{count} exact at two decimals"
    );
    if let Ok(path) = std::env::var("CAMELOT_POSTPROCESS_REPORT") {
        std::fs::write(path, serde_json::to_vec_pretty(&serde_json::json!({"scope": "frozen neural detections; postprocessor only", "engine_source_sha256": camelot_core::analysis::engine_source_sha256(), "tracks": count, "within_half": within_half, "exact_two_decimals": exact, "rows": output})).unwrap()).unwrap();
    }
    assert_eq!(within_half, count);
}
