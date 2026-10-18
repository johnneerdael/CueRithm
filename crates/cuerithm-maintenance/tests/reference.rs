use cuerithm_maintenance::{Candidate, Matcher, Tolerance};
use serde_json::{Value, json};

fn candidate(value: &Value) -> Candidate {
    Candidate {
        id: value["id"].as_str().unwrap().into(),
        artist: value["artist"].as_str().unwrap().into(),
        title: value["title"].as_str().unwrap().into(),
        duration: value["duration"].as_f64().unwrap(),
        location: value["location"].as_str().unwrap().into(),
        comparison_path: value["locationUnique"].as_str().unwrap().into(),
        bitrate: value["bitrate"].as_f64(),
        cue_count: value["cuepoints"].as_array().unwrap().len(),
        present: value["present"].as_bool().unwrap(),
        ignored: false,
    }
}

#[test]
fn authored_cases_match_the_original_shipped_runtime() {
    let fixture: Value =
        serde_json::from_str(include_str!("fixtures/lexicon-duplicates-v1.json")).unwrap();
    let rows = fixture["cases"].as_array().unwrap();
    assert_eq!(rows.len(), 1342);
    for (index, case) in rows.iter().enumerate() {
        let matcher = Matcher::new();
        let actual = match case["kind"].as_str().unwrap() {
            "groups" => {
                let tracks: Vec<_> = case["tracks"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(candidate)
                    .collect();
                let tolerance: Tolerance =
                    serde_json::from_value(case["tolerance"].clone()).unwrap();
                let videos: Vec<String> = case["video_ids"]
                    .as_array()
                    .map(|v| v.iter().map(|s| s.as_str().unwrap().to_owned()).collect())
                    .unwrap_or_default();
                json!(
                    matcher
                        .metadata_groups(&tracks, tolerance, &[], &videos)
                        .unwrap()
                )
            }
            "normalization" => {
                let tolerance = serde_json::from_value(case["tolerance"].clone()).unwrap();
                json!({
                    "artists": matcher.artist_keys(case["artist"].as_str().unwrap(), case["broad"].as_bool().unwrap()),
                    "trigrams": matcher.title_trigrams(case["title"].as_str().unwrap(), tolerance),
                    "simple": matcher.simplified_text(case["title"].as_str().unwrap()),
                })
            }
            "selection" => {
                let tracks: Vec<_> = case["tracks"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(candidate)
                    .collect();
                let result = matcher
                    .preselect(&tracks, case["method"] == "fingerprint")
                    .unwrap();
                json!({"selected":result.keeper, "tags":result.tags_from})
            }
            other => panic!("Unexpected authored reference kind {other}"),
        };
        assert_eq!(actual, case["expected"], "reference row {index}: {case}");
    }
}
