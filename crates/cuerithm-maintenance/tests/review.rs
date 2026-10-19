use cuerithm_maintenance::{
    Candidate, GroupEdit, Preference, ReviewGroup, ReviewState, ReviewTrack,
};
use serde_json::{Value, json};

#[test]
fn review_choices_and_preferences_match_shipped_runtime() {
    let fixture: Value =
        serde_json::from_str(include_str!("fixtures/lexicon-review-v1.json")).unwrap();
    let tracks: Vec<ReviewTrack> = fixture["tracks"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| ReviewTrack {
            candidate: Candidate {
                id: v["id"].as_str().unwrap().into(),
                artist: "North".into(),
                title: v["title"].as_str().unwrap().into(),
                duration: 180.,
                location: v["location"].as_str().unwrap().into(),
                comparison_path: v["locationUnique"].as_str().unwrap().into(),
                bitrate: v["bitrate"].as_f64(),
                cue_count: v["cuepoints"].as_array().unwrap().len(),
                present: v["present"].as_bool().unwrap(),
                ignored: false,
            },
            extra1: v["extra1"].as_str().unwrap().into(),
        })
        .collect();
    assert_eq!(fixture["cases"].as_array().unwrap().len(), 164);
    for (index, case) in fixture["cases"].as_array().unwrap().iter().enumerate() {
        let mut group = ReviewGroup::new("group".into(), tracks.clone(), true).unwrap();
        group
            .edit(GroupEdit::Select {
                track_id: case["initial"]["keeper"].as_str().unwrap().into(),
            })
            .unwrap();
        if let Some(id) = case["initial"]["tags"].as_str() {
            group
                .edit(GroupEdit::UseTags {
                    track_id: Some(id.into()),
                })
                .unwrap();
        }
        group.state = serde_json::from_value(case["initial"]["state"].clone()).unwrap();
        for a in case["actions"].as_array().unwrap() {
            if a["kind"] == "prefer" {
                let preference = match a["type"].as_str().unwrap() {
                    "Folder" => Preference::Folder {
                        path: a["path"].as_str().unwrap().into(),
                    },
                    "MostCuepoints" => Preference::MostCuepoints,
                    "HighestBitrate" => Preference::HighestBitrate,
                    "TitleContainsWord" => Preference::TitleContains {
                        value: a["value"].as_str().unwrap().into(),
                    },
                    "Extra1ContainsWords" => Preference::Extra1Contains {
                        value: a["value"].as_str().unwrap().into(),
                    },
                    "Extra1NotContainsWords" => Preference::Extra1NotContains {
                        value: a["value"].as_str().unwrap().into(),
                    },
                    _ => unreachable!(),
                };
                group
                    .prefer(
                        &preference,
                        a["override_locked"].as_bool().unwrap(),
                        a["use_best_cues"].as_bool().unwrap(),
                    )
                    .unwrap();
            } else {
                let edit = match a["kind"].as_str().unwrap() {
                    "select" => GroupEdit::Select {
                        track_id: a["track_id"].as_str().unwrap().into(),
                    },
                    "use_tags" => GroupEdit::UseTags {
                        track_id: Some(a["track_id"].as_str().unwrap().into()),
                    },
                    "lock" => GroupEdit::Lock,
                    "unlock" => GroupEdit::Unlock {
                        force: a["force"].as_bool().unwrap(),
                    },
                    "ignore" => GroupEdit::ToggleIgnore {
                        track_id: a["track_id"].as_str().unwrap().into(),
                    },
                    _ => unreachable!(),
                };
                group.edit(edit).unwrap();
            }
        }
        assert_eq!(
            json!({"keeper":group.keeper,"tags_from":group.tags_from,"state":group.state,"ignored":group.ignored,"changed_selection":group.changed_selection}),
            case["expected"],
            "reference {index}"
        );
    }
}

#[test]
fn unknown_or_unavailable_members_cannot_become_manual_donors_or_keepers() {
    let track = |id: &str, present| ReviewTrack {
        candidate: Candidate {
            id: id.into(),
            artist: "North".into(),
            title: "Signal".into(),
            duration: 180.,
            location: format!("/authored/{id}.flac"),
            comparison_path: format!("/authored/{id}.flac"),
            bitrate: Some(320.),
            cue_count: 0,
            present,
            ignored: false,
        },
        extra1: String::new(),
    };
    let mut group = ReviewGroup::new(
        "group".into(),
        vec![track("a", true), track("b", false)],
        false,
    )
    .unwrap();
    assert!(
        group
            .edit(GroupEdit::Select {
                track_id: "outside".into()
            })
            .is_err()
    );
    assert!(
        group
            .edit(GroupEdit::Select {
                track_id: "b".into()
            })
            .is_err()
    );
    assert!(
        group
            .edit(GroupEdit::UseTags {
                track_id: Some("b".into())
            })
            .is_err()
    );
    assert_eq!(group.keeper.as_deref(), Some("a"));
    assert_eq!(group.state, ReviewState::Unlocked);
    group
        .edit(GroupEdit::ToggleIgnore {
            track_id: "a".into(),
        })
        .unwrap();
    assert!(!group.ready());
    group
        .edit(GroupEdit::ToggleIgnore {
            track_id: "a".into(),
        })
        .unwrap();
    assert!(group.ready());
    group.edit(GroupEdit::Lock).unwrap();
    assert!(!group.ready());
}
