use super::{equal, fixture};
use crate::lexicon_timing::{anchors, intro};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

#[test]
fn original_intro_drop_dnb_and_immediate_start_results_match() {
    let source: Value = serde_json::from_str(include_str!(
        "../../../../../tests/fixtures/lexicon-timing/anchor-primitives-reference.json"
    ))
    .unwrap();
    let (mut consumed, mut dnb, mut starts, mut not_starts) = (0, 0, 0, 0);
    let mut kinds = std::collections::BTreeSet::new();
    for case in source["cases"].as_array().unwrap() {
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
            let helper = row["helper"].as_str().unwrap();
            let args = row["args"].as_array().unwrap();
            let n = |i: usize| args[i].as_f64().unwrap();
            let actual = match helper {
                "detectIntro" => {
                    kinds.insert(row["result"]["introType"].as_str().unwrap().to_owned());
                    serde_json::to_value(intro::detect(
                        &pcm,
                        rate,
                        &intro::IntroOptions {
                            max_seconds: n(0),
                            window_ms: n(1),
                            hop_ms: n(2),
                        },
                    ))
                    .unwrap()
                }
                "findDrop" => {
                    let peaks: Vec<f64> = serde_json::from_value(args[1].clone()).unwrap();
                    serde_json::to_value(anchors::find_drop(&pcm, rate, n(0), &peaks, n(2)))
                        .unwrap()
                }
                "findDnbStructuralAnchor" => {
                    let peaks: Vec<f64> = serde_json::from_value(args[1].clone()).unwrap();
                    if !row["result"].is_null() {
                        dnb += 1;
                    }
                    serde_json::to_value(anchors::dnb_anchor(
                        &pcm,
                        rate,
                        n(0),
                        &peaks,
                        args[2].as_f64(),
                    ))
                    .unwrap()
                }
                "trackStartsImmediately" => {
                    let peaks: Vec<f64> = serde_json::from_value(args[0].clone()).unwrap();
                    if row["result"].as_bool().unwrap() {
                        starts += 1;
                    } else {
                        not_starts += 1;
                    }
                    json!(anchors::starts_immediately(&peaks, n(1), n(2)))
                }
                _ => panic!("Unhandled anchor primitive {helper}"),
            };
            equal(
                &actual,
                &row["result"],
                &format!("{} {helper} {args:?}", case["name"]),
            );
            consumed += 1;
        }
    }
    assert_eq!(consumed, 216);
    assert!(dnb > 0 && starts > 0 && not_starts > 0);
    assert_eq!(
        kinds,
        ["none", "silent", "atmospheric", "buildup"]
            .into_iter()
            .map(str::to_owned)
            .collect()
    );
}

#[test]
fn anchor_primitives_keep_empty_or_invalid_inputs_unknown_without_infinite_hops() {
    let empty = [];
    assert!(!intro::detect(&empty, 44100, &Default::default()).has_intro);
    for (rate, window, hop) in [(0, 100., 50.), (44100, 100., 0.), (44100, f64::NAN, 50.)] {
        let value = intro::detect(
            &[1.; 100],
            rate,
            &intro::IntroOptions {
                max_seconds: 90.,
                window_ms: window,
                hop_ms: hop,
            },
        );
        assert!(!value.has_intro);
        assert_eq!(value.confidence, 0.);
    }
    assert!(anchors::find_drop(&empty, 44100, 0.5, &[], 180.).is_none());
    assert!(anchors::find_drop(&empty, 44100, 0., &[0.], 180.).is_none());
    assert!(anchors::dnb_anchor(&empty, 44100, 174., &[], None).is_none());
    assert!(anchors::dnb_anchor(&empty, 44100, f64::NAN, &[0.], None).is_none());
    assert!(!anchors::starts_immediately(&[], 0.5, 5.));
    assert!(!anchors::starts_immediately(&[0., 0.5, 1., 1.5], 0., 5.));
}
