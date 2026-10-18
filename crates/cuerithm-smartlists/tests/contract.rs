use cuerithm_smartlists::*;
use std::sync::Arc;
fn compiled(rules: Vec<SmartRule>, match_all: bool) -> CompiledSmartList {
    CompiledSmartList::new(
        SmartDefinition { match_all, rules },
        Arc::new(FieldCatalog::standard()),
    )
    .unwrap()
}
fn rule(id: &str, field: &str, operator: Operator, value: RuleValue) -> SmartRule {
    SmartRule {
        id: id.into(),
        field: field.into(),
        operator,
        value,
        or: false,
    }
}
#[test]
fn flat_any_all_or_and_archive_are_explicit() {
    let context = EvaluationContext::new(1792324800000, "UTC").unwrap();
    let mut track = SmartTrack::new("a");
    track
        .fields
        .insert("title".into(), FieldValue::Text(" YES ".into()));
    let yes = rule(
        "yes",
        "title",
        Operator::StringEquals,
        RuleValue::Text("yes".into()),
    );
    let no = rule(
        "no",
        "title",
        Operator::StringEquals,
        RuleValue::Text("no".into()),
    );
    assert!(!compiled(vec![], true).matches(&track, &context).unwrap());
    assert!(!compiled(vec![], false).matches(&track, &context).unwrap());
    assert!(
        compiled(vec![yes.clone(), no.clone()], false)
            .matches(&track, &context)
            .unwrap()
    );
    assert!(
        !compiled(vec![yes.clone(), no.clone()], true)
            .matches(&track, &context)
            .unwrap()
    );
    let mut yes_or = yes.clone();
    yes_or.or = true;
    yes_or.id = "or-yes".into();
    let mut no_or = no.clone();
    no_or.or = true;
    no_or.id = "or-no".into();
    assert!(
        compiled(vec![yes.clone(), yes_or.clone(), no_or.clone()], true)
            .matches(&track, &context)
            .unwrap()
    );
    assert!(
        !compiled(vec![yes_or, no], true)
            .matches(&track, &context)
            .unwrap()
    );
    track.archived = true;
    assert!(
        !compiled(vec![yes.clone()], false)
            .matches(&track, &context)
            .unwrap()
    );
    let archived = rule(
        "archive",
        "archived",
        Operator::BooleanTrue,
        RuleValue::None,
    );
    assert!(
        compiled(vec![yes, archived], true)
            .matches(&track, &context)
            .unwrap()
    );
}
#[test]
fn invalid_shape_unavailable_references_and_regex_errors_cannot_publish_matches() {
    let mut catalog = FieldCatalog::standard();
    let bad = rule(
        "bad",
        "title",
        Operator::NumberEquals,
        RuleValue::Number(0.),
    );
    assert!(
        CompiledSmartList::new(
            SmartDefinition {
                match_all: true,
                rules: vec![bad]
            },
            Arc::new(catalog.clone())
        )
        .is_err()
    );
    let bad = rule(
        "bad",
        "title",
        Operator::StringNotRegExpMatch,
        RuleValue::Text("[".into()),
    );
    assert!(
        CompiledSmartList::new(
            SmartDefinition {
                match_all: true,
                rules: vec![bad]
            },
            Arc::new(catalog.clone())
        )
        .is_err()
    );
    let bad = rule(
        "bad",
        "danceability",
        Operator::NumberEquals,
        RuleValue::Number(0.),
    );
    assert!(
        CompiledSmartList::new(
            SmartDefinition {
                match_all: true,
                rules: vec![bad]
            },
            Arc::new(catalog.clone())
        )
        .is_err()
    );
    let negative = rule(
        "negative",
        "title",
        Operator::StringNotRegExpMatch,
        RuleValue::Text("(a+)+$".into()),
    );
    let meter = compiled(vec![negative], true);
    let mut track = SmartTrack::new("a");
    track.fields.insert(
        "title".into(),
        FieldValue::Text(format!("{}!", "a".repeat(100))),
    );
    assert!(
        meter
            .matches(&track, &EvaluationContext::new(0, "UTC").unwrap())
            .is_err()
    );
    catalog.tags.insert("known".into());
    let bad = rule(
        "gone",
        "tags",
        Operator::TagsIncludes,
        RuleValue::Ids(vec!["deleted".into()]),
    );
    assert!(
        CompiledSmartList::new(
            SmartDefinition {
                match_all: true,
                rules: vec![bad]
            },
            Arc::new(catalog)
        )
        .is_err()
    );
}

#[test]
fn known_memberships_and_file_availability_are_never_faked_when_unavailable() {
    let mut catalog = FieldCatalog::standard();
    catalog
        .playlists
        .insert("list".into(), PlaylistKind::Playlist);
    let definition = SmartDefinition {
        match_all: true,
        rules: vec![rule(
            "member",
            "playlist",
            Operator::PlaylistNotInSpecificPlaylist,
            RuleValue::Playlist(PlaylistReference {
                id: "list".into(),
                path: vec!["Original name".into()],
                kind: PlaylistKind::Playlist,
            }),
        )],
    };
    let list = CompiledSmartList::new(definition, Arc::new(catalog)).unwrap();
    let track = SmartTrack::new("track");
    let mut context = EvaluationContext::new(0, "UTC").unwrap();
    assert!(list.matches(&track, &context).is_err());
    context
        .memberships
        .insert("list".into(), Default::default());
    assert!(list.matches(&track, &context).unwrap());
    context
        .memberships
        .get_mut("list")
        .unwrap()
        .insert("track".into());
    assert!(!list.matches(&track, &context).unwrap());
    let file = compiled(
        vec![rule(
            "file",
            "CustomFileMissing",
            Operator::BooleanFalse,
            RuleValue::None,
        )],
        true,
    );
    assert!(file.matches(&track, &context).is_err());
    let mut track = track;
    track.file = FileAvailability::Present;
    assert!(file.matches(&track, &context).unwrap());
    track.file = FileAvailability::Missing;
    assert!(!file.matches(&track, &context).unwrap());
}
#[test]
fn key_display_raw_regex_and_mixing_mode_have_separate_meanings() {
    let mut track = SmartTrack::new("key");
    track.key = Some("Am".into());
    let mut context = EvaluationContext::new(0, "UTC").unwrap();
    assert!(
        compiled(
            vec![rule(
                "display",
                "key",
                Operator::StringEquals,
                RuleValue::Text("8a".into())
            )],
            true
        )
        .matches(&track, &context)
        .unwrap()
    );
    assert!(
        compiled(
            vec![rule(
                "raw",
                "key",
                Operator::StringRegExpMatch,
                RuleValue::Text("/^Am$/".into())
            )],
            true
        )
        .matches(&track, &context)
        .unwrap()
    );
    track.key = Some("9B".into());
    let similar = compiled(
        vec![rule(
            "similar",
            "key",
            Operator::KeySimilar,
            RuleValue::Text("8A".into()),
        )],
        true,
    );
    assert!(!similar.matches(&track, &context).unwrap());
    context.key_mixing_mode = KeyMixingMode::Fuzzy;
    assert!(similar.matches(&track, &context).unwrap());
}
#[test]
fn structural_limits_reject_ambiguous_or_unrepresentable_definitions() {
    let r = rule(
        "rule",
        "bpm",
        Operator::NumberEquals,
        RuleValue::Number(120.),
    );
    for rules in [
        vec![r.clone(), r.clone()],
        (0..129)
            .map(|i| SmartRule {
                id: i.to_string(),
                ..r.clone()
            })
            .collect(),
        vec![rule(
            "nan",
            "bpm",
            Operator::NumberEquals,
            RuleValue::Number(f64::NAN),
        )],
        vec![rule(
            "overflow",
            "dateAdded",
            Operator::DateEquals,
            RuleValue::Date(i64::MAX),
        )],
    ] {
        assert!(
            SmartDefinition {
                match_all: true,
                rules
            }
            .validate_shape()
            .is_err()
        );
    }
    assert!(EvaluationContext::new(i64::MAX, "UTC").is_err());
    assert!(EvaluationContext::new(0, "not-a-zone").is_err());
}
