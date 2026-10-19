use camelot_core::lexicon_publication::*;
use serde_json::Value;
fn reference() -> Value {
    serde_json::from_str(include_str!(
        "../../../tests/fixtures/lexicon-publication/reference.json"
    ))
    .unwrap()
}
fn compare(a: &Value, b: &Value, path: &str) {
    match (a, b) {
        (Value::Number(a), Value::Number(b)) => assert_eq!(a.as_f64(), b.as_f64(), "{path}"),
        (Value::Array(a), Value::Array(b)) => {
            assert_eq!(a.len(), b.len(), "{path}");
            for (i, (a, b)) in a.iter().zip(b).enumerate() {
                compare(a, b, &format!("{path}/{i}"));
            }
        }
        (Value::Object(a), Value::Object(b)) => {
            assert_eq!(
                a.keys().collect::<Vec<_>>(),
                b.keys().collect::<Vec<_>>(),
                "{path}"
            );
            for (k, a) in a {
                compare(a, &b[k], &format!("{path}/{k}"));
            }
        }
        _ => assert_eq!(a, b, "{path}"),
    }
}
#[test]
fn factory_defaults_match_the_shipped_renderer_template() {
    compare(
        &serde_json::to_value(CueTemplate::default()).unwrap(),
        &reference()["defaults"],
        "default",
    );
}
#[test]
fn authored_publication_cases_match_original_renderer_records() {
    for row in reference()["cases"].as_array().unwrap() {
        let markers: Vec<StructuralMarker> =
            serde_json::from_value(row["markers"].clone()).unwrap();
        let context: PublicationContext = serde_json::from_value(row["track"].clone()).unwrap();
        let template = serde_json::from_value(row["settings"].clone()).unwrap();
        let actual =
            publish(&markers, &context, &template).unwrap_or_else(|e| panic!("{}: {e}", row["id"]));
        if let Some(bpm) = row["measuredBpm"].as_f64() {
            let saved = publish_measured(
                &markers,
                bpm,
                row["firstBeat"].as_f64().unwrap(),
                context.duration,
                &template,
                &context.cuepoints,
            )
            .unwrap();
            compare(
                &serde_json::to_value(&saved.cues).unwrap(),
                &row["expected"],
                row["id"].as_str().unwrap(),
            );
            compare(
                &serde_json::to_value(&saved.grid).unwrap(),
                &row["expectedGrid"],
                "saved-grid",
            );
            assert_eq!(saved.implementation_sha256, implementation_sha256());
            assert_eq!(saved.inputs_sha256.len(), 64);
        }
        compare(
            &serde_json::to_value(actual).unwrap(),
            &row["expected"],
            row["id"].as_str().unwrap(),
        );
    }
}
#[test]
fn saved_bpm_rounding_matches_the_actual_shipped_rounding_helper() {
    for row in reference()["rounding"].as_array().unwrap() {
        assert_eq!(
            round_bpm(row["input"].as_f64().unwrap()).unwrap(),
            row["expected"].as_f64().unwrap(),
            "{row}"
        );
    }
}

#[test]
fn invalid_timing_and_nonfinite_retained_cues_are_rejected() {
    let template = CueTemplate::default();
    let mut context = PublicationContext {
        bpm: 120.,
        duration: 160.,
        cuepoints: vec![],
    };
    let marker = StructuralMarker {
        marker_type: Marker::Start,
        start_time_seconds: Some(0.125),
    };
    for bpm in [0., -1., f64::NAN, f64::INFINITY] {
        assert!(round_bpm(bpm).is_err());
        context.bpm = bpm;
        assert!(publish(std::slice::from_ref(&marker), &context, &template).is_err());
    }
    context.bpm = f64::MIN_POSITIVE;
    assert!(publish(std::slice::from_ref(&marker), &context, &template).is_err());
    context.bpm = 120.;
    context.cuepoints = vec![PublishedCue {
        name: "Bad retained cue".into(),
        start_time: f64::NAN,
        kind: PublishedType::Cue,
        position: 0,
        color: Value::Null,
        end_time: None,
        active_loop: None,
        data: None,
    }];
    assert!(publish(&[], &context, &template).is_err());
}
#[test]
fn emergency_only_invalid_source_position_is_an_error_instead_of_null() {
    let mut value = reference()["defaults"].clone();
    value["rows"] = serde_json::json!([{"checked":true,"markerType":"emergency_loop","cueType":"loop_active","loopDuration":16,"name":"S.O.S.","color":"magenta"}]);
    let template = serde_json::from_value(value).unwrap();
    let context = PublicationContext {
        bpm: 120.,
        duration: 160.,
        cuepoints: vec![],
    };
    assert!(
        publish(
            &[StructuralMarker {
                marker_type: Marker::EmergencyLoop,
                start_time_seconds: Some(128.)
            }],
            &context,
            &template
        )
        .is_err()
    );
}

fn row(marker: Marker, cue_type: CueType) -> TemplateRow {
    TemplateRow {
        checked: true,
        marker_type: marker,
        name: "Cue".into(),
        cue_type,
        color: "green".into(),
        label: None,
        anchor: None,
        position: None,
        beats_offset: None,
        loop_duration: None,
    }
}
fn offset(anchor: Marker, position: OffsetPosition, beats: f64) -> TemplateRow {
    TemplateRow {
        anchor: Some(anchor),
        position: Some(position),
        beats_offset: Some(beats),
        ..row(Marker::Normal, CueType::HotCue)
    }
}
fn with_rows(rows: Vec<TemplateRow>) -> CueTemplate {
    CueTemplate {
        rows,
        ..CueTemplate::default()
    }
}

#[test]
fn factory_and_edited_templates_pass_user_validation() {
    CueTemplate::default().validate_user_template().unwrap();
    let mut rows = CueTemplate::default().rows;
    rows.push(offset(Marker::Drop, OffsetPosition::Before, 16.));
    rows.push(TemplateRow {
        loop_duration: Some(8.),
        cue_type: CueType::HotLoop,
        ..offset(Marker::Drop, OffsetPosition::After, 32.)
    });
    // The same position as a hot cue and as a memory cue are distinct cues.
    rows.push(row(Marker::Drop, CueType::MemoryCue));
    with_rows(rows).validate_user_template().unwrap();
}

#[test]
fn user_validation_rejects_rows_the_publisher_would_misread() {
    let bad = |rows: Vec<TemplateRow>| {
        with_rows(rows)
            .validate_user_template()
            .unwrap_err()
            .to_string()
    };
    let mut unnamed = row(Marker::Start, CueType::HotCue);
    unnamed.name = "  ".into();
    assert!(bad(vec![unnamed]).contains("needs a name"));
    let mut no_anchor = offset(Marker::Drop, OffsetPosition::Before, 4.);
    no_anchor.anchor = None;
    assert!(bad(vec![no_anchor]).contains("anchor"));
    let mut negative = offset(Marker::Drop, OffsetPosition::Before, -4.);
    assert!(bad(vec![negative.clone()]).contains("offset"));
    negative.beats_offset = Some(1.5);
    assert!(bad(vec![negative]).contains("whole"));
    let loop_without_length = TemplateRow {
        cue_type: CueType::ActiveLoop,
        ..offset(Marker::Drop, OffsetPosition::After, 8.)
    };
    assert!(bad(vec![loop_without_length]).contains("loop length"));
    let structural_with_offset = TemplateRow {
        beats_offset: Some(4.),
        ..row(Marker::Drop, CueType::HotCue)
    };
    assert!(bad(vec![structural_with_offset]).contains("structural"));
    let twice = vec![
        offset(Marker::Drop, OffsetPosition::Before, 16.),
        offset(Marker::Drop, OffsetPosition::Before, 16.),
    ];
    assert!(bad(twice.clone()).contains("same cue"));
    // An unchecked duplicate is harmless.
    let mut disabled = twice;
    disabled[1].checked = false;
    with_rows(disabled).validate_user_template().unwrap();
}

#[test]
fn custom_anchor_settings_are_validated_when_enabled() {
    let mut template = CueTemplate {
        enable_custom_anchors: true,
        custom_anchors: serde_json::json!({"drop": {"name": "My Drop"}}),
        ..CueTemplate::default()
    };
    template.validate_user_template().unwrap();
    template.custom_anchors = serde_json::json!({"chorus": {"name": "x"}});
    assert!(template.validate_user_template().is_err());
    template.custom_anchors = serde_json::json!({"drop": {"name": " "}});
    assert!(template.validate_user_template().is_err());
    template.custom_anchors = serde_json::json!({"drop": {}});
    assert!(template.validate_user_template().is_err());
}

/// The desktop editor shows the factory template from this shared fixture while no custom
/// template is saved, so the two must not drift.
#[test]
fn desktop_factory_template_fixture_matches_the_engine_default() {
    let fixture: Value = serde_json::from_str(include_str!(
        "../../../apps/desktop/src/factory-cue-template.json"
    ))
    .unwrap();
    assert_eq!(
        fixture,
        serde_json::to_value(CueTemplate::default()).unwrap()
    );
}
