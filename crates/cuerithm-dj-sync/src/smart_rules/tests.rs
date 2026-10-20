use super::*;
use crate::SyncTrack;
use cuerithm_smartlists::{SmartDefinition, SmartRule};
#[test]
fn date_rules_use_source_calendar_days_including_dst_and_missing_dates() {
    use cuerithm_smartlists::{
        CompiledSmartList, EvaluationContext, FieldCatalog, FieldValue, SmartTrack,
    };
    let zone = "Europe/Amsterdam";
    let options = SyncOptions {
        smart_context: Some(crate::SmartExportContext {
            time_zone: zone.into(),
            native_time_zone: zone.into(),
            key_mixing_mode: cuerithm_smartlists::KeyMixingMode::Harmonic,
        }),
        ..Default::default()
    };
    let db = rusqlite::Connection::open_in_memory().unwrap();
    for (calendar, hours) in [("2026-03-29", 23), ("2026-10-25", 25)] {
        let query = cuerithm_smartlists::calendar_day_epoch(calendar, zone).unwrap();
        let (_, end, _) = super::dates::bounds(query, zone).unwrap();
        assert_eq!(end - query, hours * 3600000);
        for operator in [Op::DateEquals, Op::DateAfter, Op::DateBefore] {
            let item = playlist(vec![rule("dateAdded", operator, RuleValue::Date(query))]);
            let compiled = CompiledSmartList::new(
                item.smart.clone().unwrap(),
                std::sync::Arc::new(FieldCatalog::standard()),
            )
            .unwrap();
            let context = EvaluationContext::new(query, zone).unwrap();
            let Some(NativeSmart::EngineDj(wire)) = compile(
                &item,
                &SyncLibrary::default(),
                &options,
                DjSoftware::EngineDj,
            )
            .unwrap() else {
                panic!()
            };
            let wire: serde_json::Value = serde_json::from_str(&wire).unwrap();
            let rule = &wire["rules"][0];
            let sql = format!(
                "SELECT COALESCE({} {} {},0) FROM (SELECT ?1 AS dateAdded)",
                rule["col"].as_str().unwrap(),
                rule["con"].as_str().unwrap(),
                rule["param"].as_str().unwrap()
            );
            for timestamp in [0, query - 1, query, query + 1, end - 1, end] {
                let mut source = SmartTrack::new("synthetic");
                source
                    .fields
                    .insert("dateAdded".into(), FieldValue::Date(timestamp));
                let expected = compiled.matches(&source, &context).unwrap();
                let actual = db
                    .query_row(&sql, [timestamp.div_euclid(1000)], |r| r.get::<_, bool>(0))
                    .unwrap();
                assert_eq!(actual, expected, "{calendar} {operator:?} {timestamp}");
            }
        }
    }
    let mut mismatched = options.clone();
    mismatched.smart_context.as_mut().unwrap().native_time_zone = "UTC".into();
    let item = playlist(vec![rule(
        "dateAdded",
        Op::DateEquals,
        RuleValue::Date(1792447200000),
    )]);
    assert!(
        compile(
            &item,
            &SyncLibrary::default(),
            &mismatched,
            DjSoftware::EngineDj
        )
        .unwrap_err()
        .contains("timezone differs")
    );
}
#[test]
fn engine_compatible_key_sql_matches_both_saved_mixing_preferences() {
    let db = rusqlite::Connection::open_in_memory().unwrap();
    for mode in [
        cuerithm_smartlists::KeyMixingMode::Harmonic,
        cuerithm_smartlists::KeyMixingMode::Fuzzy,
    ] {
        let item = playlist(vec![rule(
            "key",
            Op::KeySimilar,
            RuleValue::Text("8A".into()),
        )]);
        let options = SyncOptions {
            smart_context: Some(crate::SmartExportContext {
                time_zone: "UTC".into(),
                native_time_zone: "UTC".into(),
                key_mixing_mode: mode,
            }),
            ..Default::default()
        };
        let Some(NativeSmart::EngineDj(wire)) = compile(
            &item,
            &SyncLibrary::default(),
            &options,
            DjSoftware::EngineDj,
        )
        .unwrap() else {
            panic!()
        };
        let wire: serde_json::Value = serde_json::from_str(&wire).unwrap();
        let param = wire["rules"][0]["param"].as_str().unwrap();
        for n in 1..=12 {
            for suffix in ['A', 'B'] {
                let key = format!("{n}{suffix}");
                let open = key_query(&key, KeyNotation::OpenKey).unwrap();
                let index: u8 = open[..open.len() - 1].parse().unwrap();
                let native = (index - 1) * 2 + u8::from(open.ends_with('m'));
                let actual: bool = db
                    .query_row(&format!("SELECT ?1 IN {param}"), [native], |r| r.get(0))
                    .unwrap();
                assert_eq!(
                    actual,
                    cuerithm_smartlists::compatible_keys(&key, "8A", mode)
                );
            }
        }
    }
}
fn playlist(rules: Vec<SmartRule>) -> SyncPlaylist {
    SyncPlaylist {
        id: "smart".into(),
        name: "Synthetic smart".into(),
        smart: Some(SmartDefinition {
            match_all: true,
            rules,
        }),
        ..Default::default()
    }
}
fn rule(field: &str, operator: Op, value: RuleValue) -> SmartRule {
    SmartRule {
        id: format!("{field}-{operator:?}"),
        field: field.into(),
        operator,
        value,
        or: false,
    }
}
fn text() -> SyncPlaylist {
    playlist(vec![rule(
        "artist",
        Op::StringContains,
        RuleValue::Text("Fixture".into()),
    )])
}
#[test]
fn common_text_is_native_and_override_is_functional_for_every_target() {
    let library = SyncLibrary {
        playlists: vec![text()],
        ..Default::default()
    };
    for target in [
        DjSoftware::VirtualDj,
        DjSoftware::DjayPro,
        DjSoftware::Serato4,
    ] {
        assert!(
            compile_platform(
                &library.playlists[0],
                &library,
                &SyncOptions::default(),
                target,
                false
            )
            .unwrap()
            .is_some()
        );
        assert!(
            compile_platform(
                &library.playlists[0],
                &library,
                &SyncOptions {
                    materialize_smartlists: true,
                    ..Default::default()
                },
                target,
                false
            )
            .unwrap()
            .is_none()
        );
    }
    assert_eq!(
        smart_fallbacks(&library, &SyncOptions::default(), DjSoftware::PioneerUsb).len(),
        1
    );
    assert!(
        compile_platform(
            &library.playlists[0],
            &library,
            &SyncOptions::default(),
            DjSoftware::DjayPro,
            true
        )
        .is_err()
    );
}
#[test]
fn unsupported_tail_group_and_empty_definition_never_export_a_fragment() {
    let mut item = text();
    item.smart.as_mut().unwrap().rules.push(rule(
        "title",
        Op::StringRegExpMatch,
        RuleValue::Text("^Mix".into()),
    ));
    for target in [
        DjSoftware::VirtualDj,
        DjSoftware::EngineDj,
        DjSoftware::DjayPro,
        DjSoftware::Serato4,
    ] {
        let library = SyncLibrary {
            playlists: vec![item.clone()],
            ..Default::default()
        };
        assert!(compile(&item, &library, &SyncOptions::default(), target).is_err());
        item.smart.as_mut().unwrap().rules = vec![
            SmartRule {
                or: true,
                ..rule("artist", Op::StringContains, RuleValue::Text("A".into()))
            },
            SmartRule {
                or: true,
                ..rule("title", Op::StringContains, RuleValue::Text("B".into()))
            },
        ];
        assert!(compile(&item, &library, &SyncOptions::default(), target).is_err());
        item.smart.as_mut().unwrap().rules.clear();
        assert!(compile(&item, &library, &SyncOptions::default(), target).is_err());
        item = text();
        item.smart.as_mut().unwrap().rules.push(rule(
            "title",
            Op::StringRegExpMatch,
            RuleValue::Text("^Mix".into()),
        ));
    }
}
#[test]
fn engine_decimal_expression_matches_source_numeric_predicates_and_scales_both_rating_bounds() {
    let connection = rusqlite::Connection::open_in_memory().unwrap();
    for op in [
        Op::NumberEquals,
        Op::NumberNotEquals,
        Op::NumberGreaterThan,
        Op::NumberGreaterThanEquals,
        Op::NumberLessThan,
        Op::NumberLessThanEquals,
    ] {
        for query in [120.25, 120.26, 128.0, 138.0] {
            let item = playlist(vec![rule("bpm", op, RuleValue::Number(query))]);
            let library = SyncLibrary::default();
            let Some(NativeSmart::EngineDj(wire)) = compile(
                &item,
                &library,
                &SyncOptions::default(),
                DjSoftware::EngineDj,
            )
            .unwrap() else {
                panic!()
            };
            let wire: serde_json::Value = serde_json::from_str(&wire).unwrap();
            let col = wire["rules"][0]["col"].as_str().unwrap();
            let con = wire["rules"][0]["con"].as_str().unwrap();
            for value in [
                120.244, 120.245, 120.249, 120.25, 120.251, 120.254, 120.255, 120.256, 127.995,
                138.005,
            ] {
                let source = cuerithm_smartlists::comparison_bpm(value).unwrap();
                let expected = match op {
                    Op::NumberEquals => source == query,
                    Op::NumberNotEquals => source != query,
                    Op::NumberGreaterThan => source > query,
                    Op::NumberGreaterThanEquals => source >= query,
                    Op::NumberLessThan => source < query,
                    _ => source <= query,
                };
                let actual: bool = connection
                    .query_row(
                        &format!("SELECT {col} {con} ?1 FROM (SELECT ?2 AS bpmAnalyzed)"),
                        rusqlite::params![query, value],
                        |row| row.get(0),
                    )
                    .unwrap();
                assert_eq!(actual, expected, "{value} {op:?} {query}");
            }
        }
    }
    let item = playlist(vec![rule(
        "rating",
        Op::NumberBetween,
        RuleValue::NumberRange {
            start: 4.0,
            end: 2.0,
        },
    )]);
    let Some(NativeSmart::EngineDj(wire)) = compile(
        &item,
        &SyncLibrary::default(),
        &SyncOptions::default(),
        DjSoftware::EngineDj,
    )
    .unwrap() else {
        panic!()
    };
    let wire: serde_json::Value = serde_json::from_str(&wire).unwrap();
    assert_eq!(wire["rules"][0]["param"], "40 AND 80");
}
#[test]
fn serato_keeps_decimal_inclusivity_and_lowers_discrete_strictness_exactly() {
    let mut item = playlist(vec![rule(
        "bpm",
        Op::NumberGreaterThan,
        RuleValue::Number(120.25),
    )]);
    assert!(
        compile(
            &item,
            &SyncLibrary::default(),
            &SyncOptions::default(),
            DjSoftware::Serato4
        )
        .is_err()
    );
    item.smart.as_mut().unwrap().rules[0].operator = Op::NumberGreaterThanEquals;
    let Some(NativeSmart::Serato4(wire)) = compile(
        &item,
        &SyncLibrary::default(),
        &SyncOptions::default(),
        DjSoftware::Serato4,
    )
    .unwrap() else {
        panic!()
    };
    let wire: serde_json::Value = serde_json::from_str(&wire).unwrap();
    assert_eq!(wire["rules"][0]["value"], 120.25);
    for (field, op, value, expected_op, expected_value) in [
        (
            "rating",
            Op::NumberGreaterThan,
            2.0,
            "GREATER_THAN_OR_EQUAL_TO",
            json!(60),
        ),
        (
            "year",
            Op::NumberGreaterThanEquals,
            2020.0,
            "AFTER",
            json!("2019"),
        ),
    ] {
        let item = playlist(vec![rule(field, op, RuleValue::Number(value))]);
        let Some(NativeSmart::Serato4(wire)) = compile(
            &item,
            &SyncLibrary::default(),
            &SyncOptions::default(),
            DjSoftware::Serato4,
        )
        .unwrap() else {
            panic!()
        };
        let wire: serde_json::Value = serde_json::from_str(&wire).unwrap();
        assert_eq!(wire["rules"][0]["operation"], expected_op);
        assert_eq!(wire["rules"][0]["value"], expected_value);
    }
}
#[test]
fn key_equality_uses_each_native_representation_and_patterns_materialize() {
    let item = playlist(vec![rule(
        "key",
        Op::StringEquals,
        RuleValue::Text("8A".into()),
    )]);
    let library = SyncLibrary {
        tracks: vec![SyncTrack {
            key: "8A".into(),
            ..Default::default()
        }],
        ..Default::default()
    };
    let Some(NativeSmart::VirtualDj(filter)) = compile(
        &item,
        &library,
        &SyncOptions::default(),
        DjSoftware::VirtualDj,
    )
    .unwrap() else {
        panic!()
    };
    assert_eq!(filter, "Key is \"Am\"");
    let Some(NativeSmart::Djay(filters)) = compile_platform(
        &item,
        &library,
        &SyncOptions::default(),
        DjSoftware::DjayPro,
        false,
    )
    .unwrap() else {
        panic!()
    };
    assert!(matches!(&filters[0].value,DjayValue::Set(values)if values==&["1m"]));
    let Some(NativeSmart::EngineDj(wire)) = compile(
        &item,
        &library,
        &SyncOptions::default(),
        DjSoftware::EngineDj,
    )
    .unwrap() else {
        panic!()
    };
    let wire: serde_json::Value = serde_json::from_str(&wire).unwrap();
    assert_eq!(wire["rules"][0]["param"], "1");
    let item = playlist(vec![rule(
        "key",
        Op::StringContains,
        RuleValue::Text("8".into()),
    )]);
    for target in [
        DjSoftware::VirtualDj,
        DjSoftware::EngineDj,
        DjSoftware::DjayPro,
        DjSoftware::Serato4,
    ] {
        assert!(compile(&item, &library, &SyncOptions::default(), target).is_err());
    }
}
#[test]
fn mappings_follow_retained_source_values_and_missing_context_is_explicit() {
    let options = SyncOptions {
        field_mappings: std::collections::BTreeMap::from([
            (SyncTextField::Artist, SyncTextField::Title),
            (SyncTextField::Comment, SyncTextField::Artist),
        ]),
        ..Default::default()
    };
    let Some(NativeSmart::VirtualDj(filter)) = compile(
        &text(),
        &SyncLibrary::default(),
        &options,
        DjSoftware::VirtualDj,
    )
    .unwrap() else {
        panic!()
    };
    assert!(filter.starts_with("Comment contains"));
    let options = SyncOptions {
        field_mappings: std::collections::BTreeMap::from([(
            SyncTextField::Artist,
            SyncTextField::Title,
        )]),
        ..Default::default()
    };
    assert!(
        compile(
            &text(),
            &SyncLibrary::default(),
            &options,
            DjSoftware::VirtualDj
        )
        .is_err()
    );
    for item in [
        playlist(vec![rule(
            "dateAdded",
            Op::DateAfter,
            RuleValue::Date(1792447200000),
        )]),
        playlist(vec![rule(
            "key",
            Op::KeySimilar,
            RuleValue::Text("8A".into()),
        )]),
        playlist(vec![rule(
            "producer",
            Op::StringEquals,
            RuleValue::Text("Fixture".into()),
        )]),
    ] {
        let library = SyncLibrary {
            playlists: vec![item.clone()],
            ..Default::default()
        };
        assert!(
            !smart_fallbacks(&library, &SyncOptions::default(), DjSoftware::VirtualDj).is_empty()
        );
    }
}
#[test]
fn native_literals_and_nonflat_ranges_materialize_without_interpolation() {
    let item = playlist(vec![rule(
        "artist",
        Op::StringContains,
        RuleValue::Text("O'Fixture".into()),
    )]);
    assert!(
        compile(
            &item,
            &SyncLibrary::default(),
            &SyncOptions::default(),
            DjSoftware::EngineDj
        )
        .unwrap_err()
        .contains("trim and Unicode")
    );
    let item = playlist(vec![rule(
        "artist",
        Op::StringContains,
        RuleValue::Text("literal%wildcard".into()),
    )]);
    assert!(
        compile(
            &item,
            &SyncLibrary::default(),
            &SyncOptions::default(),
            DjSoftware::EngineDj
        )
        .is_err()
    );
    let mut item = playlist(vec![
        rule(
            "rating",
            Op::NumberBetween,
            RuleValue::NumberRange {
                start: 1.0,
                end: 4.0,
            },
        ),
        rule(
            "artist",
            Op::StringContains,
            RuleValue::Text("Fixture".into()),
        ),
    ]);
    item.smart.as_mut().unwrap().match_all = false;
    assert!(
        compile(
            &item,
            &SyncLibrary::default(),
            &SyncOptions::default(),
            DjSoftware::Serato4
        )
        .is_err()
    );
    assert!(
        compile(
            &item,
            &SyncLibrary::default(),
            &SyncOptions::default(),
            DjSoftware::VirtualDj
        )
        .is_err()
    );
}
#[test]
fn semantic_gaps_keep_whole_membership_instead_of_emitting_approximate_native_rules() {
    let mut item = playlist(vec![
        rule("artist", Op::StringEquals, RuleValue::Text("artist".into())),
        rule("bpm", Op::NumberEquals, RuleValue::Number(120.0)),
    ]);
    item.track_ids = vec!["upper".into(), "spaced".into(), "lower".into()];
    let library = SyncLibrary {
        tracks: vec![
            SyncTrack {
                id: "upper".into(),
                artist: "Artist".into(),
                bpm: 120.0049,
                ..Default::default()
            },
            SyncTrack {
                id: "spaced".into(),
                artist: " artist ".into(),
                bpm: 120.0,
                ..Default::default()
            },
            SyncTrack {
                id: "lower".into(),
                artist: "artist".into(),
                bpm: 120.0,
                ..Default::default()
            },
        ],
        playlists: vec![item.clone()],
    };
    assert!(
        compile(&item, &library, &Default::default(), DjSoftware::EngineDj)
            .unwrap_err()
            .contains("trim and Unicode")
    );
    assert_eq!(item.track_ids, library.playlists[0].track_ids);
    assert_eq!(
        smart_fallbacks(&library, &Default::default(), DjSoftware::EngineDj).len(),
        1
    );
    item.smart.as_mut().unwrap().rules.remove(0);
    assert!(
        // Exercise the native djay compiler; Windows deliberately materializes
        // before examining individual predicates (covered separately above).
        compile_platform(
            &item,
            &library,
            &Default::default(),
            DjSoftware::DjayPro,
            false
        )
        .unwrap_err()
        .contains("two-decimal")
    );
    let options = SyncOptions {
        skip_beatgrids: true,
        ..Default::default()
    };
    assert!(
        compile(&item, &library, &options, DjSoftware::VirtualDj)
            .unwrap_err()
            .contains("preserved target tempo")
    );
}
#[test]
fn serato_calendar_after_materializes_until_native_inclusivity_is_established() {
    let item = playlist(vec![rule(
        "dateAdded",
        Op::DateAfter,
        RuleValue::Date(1792447200000),
    )]);
    let options = SyncOptions {
        smart_context: Some(crate::SmartExportContext {
            time_zone: "UTC".into(),
            native_time_zone: "UTC".into(),
            key_mixing_mode: cuerithm_smartlists::KeyMixingMode::Harmonic,
        }),
        ..Default::default()
    };
    assert!(
        compile(
            &item,
            &SyncLibrary::default(),
            &options,
            DjSoftware::Serato4
        )
        .unwrap_err()
        .contains("inclusivity")
    );
}
