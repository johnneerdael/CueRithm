use cuerithm_smartlists::*;
use serde_json::Value;
use std::sync::Arc;
fn input(value: &Value) -> SmartTrack {
    let mut track = SmartTrack::new("track");
    if let Some(map) = value.as_object() {
        for (field, value) in map {
            match field.as_str() {
                "tags" => {
                    track.tags = value
                        .as_array()
                        .unwrap()
                        .iter()
                        .map(|n| n.to_string().trim_matches('"').to_owned())
                        .collect();
                }
                "cuepoints" => {
                    track.cues = value
                        .as_array()
                        .unwrap()
                        .iter()
                        .map(|c| SmartCue {
                            label: c["name"].as_str().map(str::to_owned),
                            color: c["color"].as_str().map(str::to_owned),
                            kind: if c["type"] == "loop" {
                                CueKind::Loop
                            } else {
                                CueKind::Cue
                            },
                            active_loop: c["activeLoop"].as_bool().unwrap_or(false),
                            memory: c["data"]["rekordbox"]["originalNum"] == -1,
                        })
                        .collect()
                }
                "tempomarkers" => {
                    track.tempos = value
                        .as_array()
                        .unwrap()
                        .iter()
                        .map(|r| r["bpm"].as_f64().unwrap())
                        .collect()
                }
                "archived" => track.archived = value.as_bool().unwrap(),
                "key" => track.key = value.as_str().map(str::to_owned),
                _ => {
                    let v = if value.is_null() {
                        FieldValue::Null
                    } else if let Some(s) = value.as_str() {
                        FieldValue::Text(s.into())
                    } else if let Some(b) = value.as_bool() {
                        FieldValue::Boolean(b)
                    } else if field.starts_with("date") {
                        FieldValue::Date(value.as_i64().unwrap())
                    } else {
                        FieldValue::Number(value.as_f64().unwrap())
                    };
                    track.fields.insert(field.clone(), v);
                }
            }
        }
    }
    track
}
fn catalog() -> Arc<FieldCatalog> {
    let mut c = FieldCatalog::standard();
    c.tags = ["1", "2", "3"].map(String::from).into();
    c.categories
        .insert("1".into(), ["1", "2"].map(String::from).into());
    c.categories
        .insert("2".into(), ["3"].map(String::from).into());
    Arc::new(c)
}
fn single(field: &str, operator: &str, value: RuleValue) -> SmartDefinition {
    SmartDefinition {
        match_all: true,
        rules: vec![SmartRule {
            id: "test".into(),
            field: field.into(),
            operator: serde_json::from_value(Value::String(operator.into())).unwrap(),
            value,
            or: false,
        }],
    }
}
#[test]
fn replay_all_637_observed_scalar_and_combination_results() {
    let reference: Value =
        serde_json::from_str(include_str!("fixtures/lexicon-scalars-v1.json")).unwrap();
    let context = EvaluationContext::new(reference["freeze"].as_i64().unwrap(), "UTC").unwrap();
    let rows = reference["rows"].as_array().unwrap();
    assert_eq!(rows.len(), 637);
    let mut matched = 0;
    let mut invalid_regex = 0;
    for (index, row) in rows.iter().enumerate() {
        let group = row["group"].as_str().unwrap();
        let args = row["args"].as_array();
        let method = row["method"].as_str().unwrap_or("");
        let definition = match group {
            "StringFilter" => {
                let a = args.unwrap();
                let op = match method {
                    "equals" => "StringEquals",
                    "contains" => "StringContains",
                    "startsWith" => "StringStartsWith",
                    "endsWith" => "StringEndsWith",
                    "regexMatch" => "StringRegExpMatch",
                    _ => panic!("{method}"),
                };
                single(
                    a[0].as_str().unwrap(),
                    op,
                    RuleValue::Text(a[1].as_str().unwrap().into()),
                )
            }
            "NumberFilter" => {
                let a = args.unwrap();
                let op = match method {
                    "equals" => "NumberEquals",
                    "greaterThan" => "NumberGreaterThan",
                    "greaterThanEquals" => "NumberGreaterThanEquals",
                    "lessThan" => "NumberLessThan",
                    "lessThanEquals" => "NumberLessThanEquals",
                    "isExactNumber" => "BpmExactNumber",
                    "isNotExactNumber" => "BpmNotExactNumber",
                    _ => panic!("{method}"),
                };
                single(
                    a[0].as_str().unwrap(),
                    op,
                    a.get(1)
                        .map(|v| RuleValue::Number(v.as_f64().unwrap()))
                        .unwrap_or(RuleValue::None),
                )
            }
            "AnyFilter" => single(
                args.unwrap()[0].as_str().unwrap(),
                if method == "exists" {
                    "Exists"
                } else {
                    "ExistsNot"
                },
                RuleValue::None,
            ),
            "TagFilter" => {
                let a = args.unwrap();
                let op = match method {
                    "hasEvery" => "TagsIncludes",
                    "hasNone" => "TagsNotIncludes",
                    "hasAtleastOne" => "TagsAtleastOne",
                    "hasExact" => "TagsExact",
                    _ => panic!("{method}"),
                };
                single(
                    "tags",
                    op,
                    RuleValue::Ids(
                        a[0].as_array()
                            .unwrap()
                            .iter()
                            .map(|n| n.to_string())
                            .collect(),
                    ),
                )
            }
            "DateFilter" => {
                let a = args.unwrap();
                let millis = chrono::DateTime::parse_from_rfc3339(a[1].as_str().unwrap())
                    .unwrap()
                    .timestamp_millis();
                let op = match method {
                    "equals" => "DateEquals",
                    "after" => "DateAfter",
                    "before" => "DateBefore",
                    _ => panic!("{method}"),
                };
                single(a[0].as_str().unwrap(), op, RuleValue::Date(millis))
            }
            "TempomarkerFilter" => single(
                "tempomarkers",
                if method == "allSameBpm" {
                    "TempomarkerSameBpm"
                } else {
                    "TempomarkerNotSameBpm"
                },
                RuleValue::None,
            ),
            "combination" => SmartDefinition {
                match_all: row["matchAll"].as_bool().unwrap(),
                rules: row["rules"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .enumerate()
                    .map(|(i, r)| SmartRule {
                        id: i.to_string(),
                        field: r["field"].as_str().unwrap().into(),
                        operator: serde_json::from_value(r["operator"].clone()).unwrap(),
                        or: r["or"].as_bool().unwrap_or(false),
                        value: if r["values"][0].is_number() {
                            RuleValue::Number(r["values"][0].as_f64().unwrap())
                        } else {
                            RuleValue::Text(r["values"][0].as_str().unwrap().into())
                        },
                    })
                    .collect(),
            },
            _ => panic!("Unaccounted source group {group}"),
        };
        // Source catches malformed patterns as false; strict validation rejects them.
        if group == "StringFilter" && method == "regexMatch" && args.unwrap()[1] == "[" {
            assert_eq!(row["result"], false);
            assert!(CompiledSmartList::new(definition, catalog()).is_err());
            invalid_regex += 1;
            continue;
        }
        let result = CompiledSmartList::new(definition, catalog())
            .unwrap_or_else(|e| panic!("case {index}: {e:#}"));
        assert_eq!(
            result
                .matches(&input(&row["track"]), &context)
                .unwrap_or_else(|e| panic!("case {index}: {e:#}")),
            row["result"].as_bool().unwrap(),
            "case {index} {group}/{method}: {row}"
        );
        matched += 1;
    }
    assert_eq!(matched + invalid_regex, 637);
    assert_eq!(invalid_regex, 8);
}
#[test]
fn every_key_pair_both_modes_and_all_observed_aliases() {
    let reference: Value =
        serde_json::from_str(include_str!("fixtures/lexicon-keys-v1.json")).unwrap();
    let mut differences = 0;
    for row in reference["cases"].as_array().unwrap() {
        let mode = if row["mode"] == "Harmonic" {
            KeyMixingMode::Harmonic
        } else {
            KeyMixingMode::Fuzzy
        };
        let expected = row["expected_policy"].as_bool().unwrap();
        assert_eq!(
            compatible_keys(
                row["left"].as_str().unwrap(),
                row["right"].as_str().unwrap(),
                mode
            ),
            expected,
            "{row}"
        );
        if row["source_result"].as_bool().unwrap() != expected {
            differences += 1;
        }
    }
    assert_eq!(reference["cases"].as_array().unwrap().len(), 1152);
    assert_eq!(differences, 4);
    for row in reference["mapping"].as_array().unwrap() {
        assert_eq!(
            key_to_camelot(row["value"].as_str().unwrap()).as_deref(),
            row["camelot"].as_str(),
            "{row}"
        );
    }
    for bad in ["", "0A", "13A", " 8A ", "unknown"] {
        assert!(key_to_camelot(bad).is_none());
    }
}

#[test]
fn expanded_original_cases_cover_all_58_operators_and_calendar_cutoffs() {
    let fixture: Value =
        serde_json::from_str(include_str!("fixtures/lexicon-expanded-v1.json")).unwrap();
    let mut covered = std::collections::BTreeMap::<Operator, (usize, usize)>::new();
    let mut policy_rows = 0;
    for (i, row) in fixture["rows"].as_array().unwrap().iter().enumerate() {
        let op: Operator = serde_json::from_value(row["operator"].clone()).unwrap();
        let name = row["operator"].as_str().unwrap();
        let values = row["values"].as_array().unwrap();
        let value = if name.starts_with("String")
            || name == "KeySimilar"
            || name.ends_with("LabelIncludes")
        {
            RuleValue::Text(values[0].as_str().unwrap().into())
        } else if name == "NumberBetween" {
            RuleValue::NumberRange {
                start: values[0].as_f64().unwrap(),
                end: values[1].as_f64().unwrap(),
            }
        } else if name.starts_with("Number") {
            RuleValue::Number(values[0].as_f64().unwrap())
        } else if name == "DateRecent" || name == "DateNotRecent" {
            RuleValue::Relative {
                count: values[0].as_i64().unwrap(),
                unit: serde_json::from_value(values[1].clone()).unwrap(),
            }
        } else if name == "DateBetween" {
            RuleValue::DateRange {
                start: values[0].as_i64().unwrap(),
                end: values[1].as_i64().unwrap(),
            }
        } else if name.starts_with("Date") {
            RuleValue::Date(values[0].as_i64().unwrap())
        } else if name.starts_with("Color") || name.ends_with("ColorEquals") {
            RuleValue::Color(values[0].as_str().map(str::to_owned))
        } else if name.starts_with("Tags") || name.starts_with("TagCategory") {
            RuleValue::Ids(
                values[0]
                    .as_str()
                    .unwrap()
                    .split(',')
                    .filter(|s| !s.is_empty())
                    .map(str::to_owned)
                    .collect(),
            )
        } else if name.starts_with("Playlist") {
            let (path, id) = values[0].as_str().unwrap().rsplit_once("%!#!%").unwrap();
            RuleValue::Playlist(PlaylistReference {
                id: id.into(),
                path: vec![path.into()],
                kind: if name.contains("Folder") {
                    PlaylistKind::Folder
                } else if name.contains("Smartlist") {
                    PlaylistKind::Smartlist
                } else {
                    PlaylistKind::Playlist
                },
            })
        } else {
            RuleValue::None
        };
        let mut c = (*catalog()).clone();
        if let Some(fields) = row["any_text_fields"].as_array() {
            for (id, descriptor) in &mut c.fields {
                descriptor.any_text = fields.iter().any(|f| f.as_str() == Some(id));
            }
        }
        for (id, kind) in [
            ("1", PlaylistKind::Playlist),
            ("2", PlaylistKind::Folder),
            ("3", PlaylistKind::Smartlist),
        ] {
            c.playlists.insert(id.into(), kind);
        }
        let definition = single(row["field"].as_str().unwrap(), name, value);
        let compiled = CompiledSmartList::new(definition, Arc::new(c));
        if row["policy_error"] == "unavailable_ids" {
            assert!(compiled.is_err(), "case {i}: {row}");
            policy_rows += 1;
            continue;
        }
        let list = compiled.unwrap_or_else(|e| panic!("case {i}: {e:#}: {row}"));
        let mut context =
            EvaluationContext::new(row["now"].as_i64().unwrap(), row["zone"].as_str().unwrap())
                .unwrap();
        for id in ["1", "2", "3"] {
            context
                .memberships
                .insert(id.into(), ["selected".into()].into());
        }
        let mut track = input(&row["track"]);
        track.id = row["track"]["id"].as_str().unwrap().into();
        let actual = list
            .matches(&track, &context)
            .unwrap_or_else(|e| panic!("case {i}: {e:#}"));
        if row["policy_error"] == "missing_date_not_epoch" {
            assert!(!actual);
            assert_eq!(row["source_result"], true);
            policy_rows += 1;
            continue;
        }
        let expected = row["source_result"].as_bool().unwrap();
        assert_eq!(actual, expected, "case {i}: {row}");
        let coverage = covered.entry(op).or_default();
        if expected {
            coverage.0 += 1
        } else {
            coverage.1 += 1
        }
    }
    assert_eq!(covered.len(), 58);
    for op in Operator::ALL {
        let c = covered[op];
        assert!(
            c.0 > 0 && c.1 > 0,
            "operator {op:?} lacks both polarities: {c:?}"
        );
    }
    assert_eq!(policy_rows, 15);
}
