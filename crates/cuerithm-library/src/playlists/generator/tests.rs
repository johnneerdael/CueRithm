use super::*;
fn from_source(value: &serde_json::Value) -> Template {
    let rules = value["smartlist"]["rules"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| {
            let operator: Operator = serde_json::from_value(r["operator"].clone()).unwrap();
            let values = r["values"].as_array().unwrap();
            let value = match operator {
                Operator::NumberBetween => RuleValue::NumberRange {
                    start: values[0].as_f64().unwrap(),
                    end: values[1].as_f64().unwrap(),
                },
                Operator::StringEquals => RuleValue::Text(values[0].as_str().unwrap().into()),
                Operator::TagsAtleastOne => {
                    RuleValue::Ids(serde_json::from_value(values[0].clone()).unwrap())
                }
                _ => RuleValue::Number(values[0].as_f64().unwrap()),
            };
            SmartRule {
                id: String::new(),
                field: r["field"].as_str().unwrap().into(),
                operator,
                value,
                or: false,
            }
        })
        .collect();
    Template {
        name: value["name"].as_str().unwrap().into(),
        definition: templates::definition(rules),
    }
}
#[test]
fn original_generator_static_helpers_match_all_controlled_references() {
    let reference: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../../tests/fixtures/playlist-tools/generator-reference.json"
    ))
    .unwrap();
    let labels = reference["tags"]
        .as_object()
        .unwrap()
        .iter()
        .map(|(id, v)| (id.clone(), v["label"].as_str().unwrap().to_owned()))
        .collect();
    let mut consumed = 0;
    let mut item_count = 0;
    for case in reference["cases"].as_array().unwrap() {
        let method = case["method"].as_str().unwrap();
        let numbers = |field: &str| {
            case["values"][field]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| v.as_f64().unwrap())
                .collect::<Vec<_>>()
        };
        let actual = match method {
            "buildDecadeItems" => templates::ranges("year", &numbers("year"), true, 1.),
            "buildBpmRangeItems" => templates::ranges("bpm", &numbers("bpm"), false, 1.),
            "buildBitrateBucketItems" => templates::buckets(
                "bitrate",
                &numbers("bitrate"),
                &[128., 192., 256., 320.],
                1.,
                "kbps",
            ),
            "buildSizeBucketItems" => templates::buckets(
                "sizeBytes",
                &numbers("sizeBytes"),
                &[5242880., 10485760., 26214400., 52428800.],
                1048576.,
                "MB",
            ),
            "buildDurationBucketItems" => templates::buckets(
                "duration",
                &numbers("duration"),
                &[120., 240., 360., 480., 600.],
                60.,
                "min",
            ),
            "buildTrackFieldItems" => {
                let field = case["args"][0].as_str().unwrap();
                let values = if matches!(field, "energy" | "rating") {
                    let c = &reference["catalog"][field];
                    (c["min"].as_i64().unwrap()..=c["max"].as_i64().unwrap())
                        .map(|n| FieldValue::Number(n as f64))
                        .collect()
                } else {
                    case["values"][field]
                        .as_array()
                        .unwrap()
                        .iter()
                        .map(|v| {
                            if let Some(n) = v.as_f64() {
                                FieldValue::Number(n)
                            } else {
                                FieldValue::Text(v.as_str().unwrap().into())
                            }
                        })
                        .collect()
                };
                templates::scalar(field, values).unwrap()
            }
            "buildTagItems" => templates::tags(
                &serde_json::from_value::<Vec<String>>(case["args"][0].clone()).unwrap(),
                &labels,
            ),
            "buildTagCategoryItems" => templates::tags(
                &case["args"][0]
                    .get("tags")
                    .map(|v| serde_json::from_value::<Vec<String>>(v.clone()).unwrap())
                    .unwrap_or_default(),
                &labels,
            ),
            "combineItems" => {
                let mut groups = case["args"][0]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|g| {
                        g.as_array()
                            .unwrap()
                            .iter()
                            .map(from_source)
                            .collect::<Vec<_>>()
                    })
                    .filter(|g| !g.is_empty());
                let mut items = groups.next().unwrap_or_default();
                for group in groups {
                    items = items
                        .iter()
                        .flat_map(|a| group.iter().map(move |b| templates::and(a, b)))
                        .collect();
                }
                items
            }
            _ => panic!("Unaccounted source helper {method}"),
        };
        let expected = case["expected"]
            .as_array()
            .unwrap()
            .iter()
            .map(from_source)
            .collect::<Vec<_>>();
        assert_eq!(actual, expected, "{}", case["name"]);
        consumed += 1;
        item_count += expected.len();
    }
    assert_eq!((consumed, item_count), (20, 65));
}
