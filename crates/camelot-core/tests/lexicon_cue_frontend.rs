use camelot_core::lexicon_cues::{LexiconConfig, try_generate_renderer_cues};
use serde_json::Value;
use sha2::{Digest, Sha256};
#[path = "support/lexicon_pcm.rs"]
mod fixture;

#[test]
fn original_cue_worker_matches_authored_raw_and_renderer_filtered_inputs() {
    let reference: Value = serde_json::from_str(include_str!(
        "../../../tests/fixtures/lexicon-cue-frontend/cue-reference.json"
    ))
    .unwrap();
    let mut consumed = 0;
    let mut names = std::collections::BTreeSet::new();
    for case in reference["cases"].as_array().unwrap() {
        let rate = case["sample_rate"].as_u64().unwrap() as u32;
        let recipe: fixture::Recipe = serde_json::from_value(case["recipe"].clone()).unwrap();
        let pcm = fixture::signal(&recipe, rate);
        let mut hash = Sha256::new();
        for sample in &pcm {
            hash.update(sample.to_le_bytes());
        }
        assert_eq!(
            format!("{:x}", hash.finalize()),
            case["pcm_sha256"].as_str().unwrap(),
            "{} PCM",
            case["name"]
        );
        for row in case["rows"].as_array().unwrap() {
            let config: LexiconConfig = serde_json::from_value(row["config"].clone()).unwrap();
            let actual = try_generate_renderer_cues(
                &pcm,
                rate,
                row["timing"]["bpm"].as_f64().unwrap(),
                row["timing"]["first_beat_seconds"].as_f64().unwrap(),
                &config,
            )
            .unwrap();
            let actual = serde_json::to_value(actual).unwrap();
            compare(
                &actual,
                &row["expected"],
                &format!("{} {}", case["name"], row["config"]),
            );
            for cue in row["expected"]["cues"].as_array().unwrap() {
                names.insert(cue["name"].as_str().unwrap().to_owned());
            }
            consumed += 1;
        }
    }
    assert_eq!(consumed, 40);
    for name in [
        "Start",
        "Drop",
        "Breakdown",
        "Second Drop",
        "Second Breakdown",
        "Last beat",
    ] {
        assert!(names.contains(name), "Missing positive cue type {name}");
    }
}
fn compare(actual: &Value, expected: &Value, context: &str) {
    match (actual, expected) {
        (Value::Number(a), Value::Number(b)) => assert!(
            (a.as_f64().unwrap() - b.as_f64().unwrap()).abs() <= 1e-12,
            "{context}: {actual} vs {expected}"
        ),
        (Value::Object(a), Value::Object(b)) => {
            assert_eq!(a.len(), b.len(), "{context}");
            for (key, value) in b {
                compare(&a[key], value, &format!("{context}.{key}"));
            }
        }
        (Value::Array(a), Value::Array(b)) => {
            assert_eq!(a.len(), b.len(), "{context}");
            for (index, (a, b)) in a.iter().zip(b).enumerate() {
                compare(a, b, &format!("{context}[{index}]"));
            }
        }
        _ => assert_eq!(actual, expected, "{context}"),
    }
}
