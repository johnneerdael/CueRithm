use cuerithm_maintenance::{
    Candidate, MergePlan, ReviewGroup, ReviewState, ReviewTrack, replace_members,
};
use serde_json::{Value, json};
use std::collections::BTreeSet;
fn group(id: &str) -> ReviewGroup {
    ReviewGroup::new(
        id.into(),
        ["a", "b", "c"]
            .iter()
            .map(|id| ReviewTrack {
                candidate: Candidate {
                    id: (*id).into(),
                    title: "Signal".into(),
                    artist: "North".into(),
                    duration: 180.,
                    location: format!("/authored/{id}.flac"),
                    comparison_path: format!("/authored/{id}.flac"),
                    bitrate: Some(320.),
                    cue_count: 1,
                    present: true,
                    ignored: false,
                },
                extra1: String::new(),
            })
            .collect(),
        true,
    )
    .unwrap()
}
#[test]
fn merge_intents_match_shipped_execute_actions() {
    let fixture: Value =
        serde_json::from_str(include_str!("fixtures/lexicon-merge-v1.json")).unwrap();
    assert_eq!(fixture["cases"].as_array().unwrap().len(), 40);
    for (index, case) in fixture["cases"].as_array().unwrap().iter().enumerate() {
        let mut g = group("g");
        g.keeper = Some(case["keeper"].as_str().unwrap().into());
        g.tags_from = case["donor"].as_str().map(str::to_owned);
        g.ignored = serde_json::from_value(case["ignored"].clone()).unwrap();
        g.state = serde_json::from_value(case["state"].clone()).unwrap();
        let archived = if case["keeper_archived"] == true {
            BTreeSet::from([g.keeper.clone().unwrap()])
        } else {
            BTreeSet::new()
        };
        let plan = MergePlan::build(&[g], &["g".into()], &archived).unwrap();
        assert_eq!(plan.groups.len(), 1);
        let row = &plan.groups[0];
        let replacements = if row.archive_ids.is_empty() {
            vec![]
        } else {
            vec![json!({"removed":row.archive_ids,"keeper":row.keeper})]
        };
        let unarchives = if row.unarchive_keeper {
            vec![row.keeper.clone()]
        } else {
            vec![]
        };
        assert_eq!(
            json!({"copies":row.copies,"archives":row.archive_ids,"unarchives":unarchives,"replacements":replacements}),
            case["expected"],
            "original execute case {index}"
        );
    }
}
#[test]
fn merge_rejects_locked_invalid_and_overlapping_selections() {
    let mut locked = group("g");
    locked.state = ReviewState::Locked;
    assert!(MergePlan::build(&[locked], &["g".into()], &BTreeSet::new()).is_err());
    assert!(MergePlan::build(&[group("g")], &["outside".into()], &BTreeSet::new()).is_err());
    assert!(
        MergePlan::build(
            &[group("g"), group("h")],
            &["g".into(), "h".into()],
            &BTreeSet::new()
        )
        .is_err()
    );
}
#[test]
fn replacement_moves_existing_keeper_to_the_first_removed_slot_only_in_affected_lists() {
    let list = |ids: &[&str]| ids.iter().map(|id| (*id).to_owned()).collect::<Vec<_>>();
    let removed = list(&["a", "b"]);
    assert_eq!(
        replace_members(&list(&["k", "x", "a", "y", "b"]), &removed, "k").unwrap(),
        list(&["x", "k", "y"])
    );
    assert_eq!(
        replace_members(&list(&["x", "a", "k", "b", "y"]), &removed, "k").unwrap(),
        list(&["x", "k", "y"])
    );
    assert_eq!(
        replace_members(&list(&["x", "k", "y"]), &removed, "k").unwrap(),
        list(&["x", "k", "y"])
    );
    assert!(replace_members(&list(&["k", "a"]), &list(&["k"]), "k").is_err());
}

#[test]
fn replacements_match_original_method_with_ordered_query_adapter() {
    let fixture: Value =
        serde_json::from_str(include_str!("fixtures/lexicon-replacement-v1.json")).unwrap();
    assert_eq!(fixture["cases"].as_array().unwrap().len(), 485);
    for (index, case) in fixture["cases"].as_array().unwrap().iter().enumerate() {
        let ids = |key: &str| {
            case[key]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| v.as_i64().unwrap().to_string())
                .collect::<Vec<_>>()
        };
        assert_eq!(
            replace_members(
                &ids("members"),
                &ids("removed"),
                &case["keeper"].as_i64().unwrap().to_string()
            )
            .unwrap(),
            ids("expected"),
            "replacement case {index}"
        );
    }
}
