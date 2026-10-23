//! A track whose audio file is not available is never grouped, offered or merged.
use cuerithm_maintenance::{
    Candidate, GroupMerge, Matcher, MergeDisposition, MergePlan, ReviewGroup, ReviewTrack,
    Tolerance,
};
use std::collections::BTreeSet;

const TOLERANCES: [Tolerance; 5] = [
    Tolerance::None,
    Tolerance::Low,
    Tolerance::Medium,
    Tolerance::High,
    Tolerance::HighIgnoreParentheses,
];

fn track(id: &str, present: bool) -> Candidate {
    Candidate {
        id: id.into(),
        artist: "North".into(),
        title: "Signal".into(),
        duration: 180.,
        location: format!("/library/{id}.flac"),
        comparison_path: format!("/library/{id}.flac"),
        bitrate: Some(1000.),
        cue_count: 0,
        present,
        ignored: false,
    }
}
fn review_group(tracks: Vec<Candidate>) -> ReviewGroup {
    ReviewGroup::new(
        "group".into(),
        tracks
            .into_iter()
            .map(|candidate| ReviewTrack {
                candidate,
                extra1: String::new(),
            })
            .collect(),
        false,
    )
    .unwrap()
}

#[test]
fn a_missing_track_and_its_present_twin_form_no_group_under_any_match_method() {
    for tolerance in TOLERANCES {
        let found = Matcher::new()
            .available_metadata_groups(
                &[track("missing", false), track("present", true)],
                tolerance,
                &[],
                &[],
            )
            .unwrap();
        assert!(found.groups.is_empty(), "{tolerance:?}: {:?}", found.groups);
        assert_eq!(found.skipped_unavailable, vec!["missing"], "{tolerance:?}");
    }
}

#[test]
fn unavailable_tracks_are_left_out_of_a_group_of_available_tracks() {
    for tolerance in TOLERANCES {
        let found = Matcher::new()
            .available_metadata_groups(
                &[
                    track("a", true),
                    track("gone", false),
                    track("b", true),
                    track("also-gone", false),
                ],
                tolerance,
                &[],
                &[],
            )
            .unwrap();
        assert_eq!(found.groups, vec![vec!["a", "b"]], "{tolerance:?}");
        assert_eq!(
            found.skipped_unavailable,
            vec!["gone", "also-gone"],
            "{tolerance:?}"
        );
    }
}

#[test]
fn available_libraries_group_exactly_as_the_original_matcher_does() {
    let mut tracks: Vec<_> = ["a", "b", "c", "d"].map(|id| track(id, true)).into();
    tracks[1].duration = 183.;
    tracks[2].title = "Signal (Extended Mix)".into();
    tracks[3].artist = "South".into();
    tracks[3].title = "Other".into();
    for tolerance in TOLERANCES {
        let matcher = Matcher::new();
        let available = matcher
            .available_metadata_groups(&tracks, tolerance, &[], &[])
            .unwrap();
        assert!(available.skipped_unavailable.is_empty());
        assert_eq!(
            available.groups,
            matcher
                .metadata_groups(&tracks, tolerance, &[], &[])
                .unwrap(),
            "{tolerance:?}"
        );
    }
}

#[test]
fn ignored_tracks_are_not_grouped_and_are_not_reported_as_unavailable() {
    let mut ignored = track("ignored", true);
    ignored.ignored = true;
    let found = Matcher::new()
        .available_metadata_groups(
            &[track("a", true), ignored, track("b", true)],
            Tolerance::None,
            &[],
            &[],
        )
        .unwrap();
    assert_eq!(found.groups, vec![vec!["a", "b"]]);
    assert!(found.skipped_unavailable.is_empty());
}

#[test]
fn a_merge_plan_refuses_a_group_that_still_holds_an_unavailable_track() {
    let group = review_group(vec![track("keeper", true), track("gone", false)]);
    assert_eq!(group.keeper.as_deref(), Some("keeper"));
    for disposition in [MergeDisposition::Archive, MergeDisposition::Delete] {
        let error = MergePlan::build_with(
            std::slice::from_ref(&group),
            &["group".into()],
            &BTreeSet::new(),
            disposition,
        )
        .unwrap_err()
        .to_string();
        assert!(error.contains("not available"), "{error}");
    }
    // The user may leave an unavailable record out of the merge by ignoring it.
    let mut ignored = group.clone();
    ignored.ignored.insert("gone".into());
    let plan = MergePlan::build(&[ignored], &["group".into()], &BTreeSet::new()).unwrap();
    assert!(plan.groups[0].archive_ids.is_empty());
}

#[test]
fn deletion_replaces_records_without_archiving_them_and_plain_builds_archive() {
    let group = review_group(vec![track("a", true), track("b", true), track("c", true)]);
    let archived = BTreeSet::from(["a".to_owned()]);
    let default =
        MergePlan::build(std::slice::from_ref(&group), &["group".into()], &archived).unwrap();
    assert_eq!(default.disposition, MergeDisposition::Archive);
    assert_eq!(default.groups[0].archive_ids, vec!["b", "c"]);
    assert!(default.groups[0].delete_ids.is_empty());

    let deletion = MergePlan::build_with(
        std::slice::from_ref(&group),
        &["group".into()],
        &archived,
        MergeDisposition::Delete,
    )
    .unwrap();
    let row = &deletion.groups[0];
    assert_eq!(deletion.disposition, MergeDisposition::Delete);
    assert!(row.archive_ids.is_empty());
    assert_eq!(row.delete_ids, vec!["b", "c"]);
    assert_eq!(row.replaced_ids().collect::<Vec<_>>(), vec!["b", "c"]);
    // A kept record that sits in the archive is restored whichever way the others go.
    assert!(row.unarchive_keeper);
    assert_eq!(
        deletion.participants(),
        BTreeSet::from(["a".to_owned(), "b".to_owned(), "c".to_owned()])
    );
}

#[test]
fn plans_saved_before_deletion_existed_still_read_as_archive_plans() {
    let old: MergePlan = serde_json::from_str(
        r#"{"groups":[{"group_id":"g","mode":"unlocked","exact_audio":false,"keeper":"a","archive_ids":["b"],"unarchive_keeper":false,"copies":[]}]}"#,
    )
    .unwrap();
    assert_eq!(old.disposition, MergeDisposition::Archive);
    let row: &GroupMerge = &old.groups[0];
    assert!(row.delete_ids.is_empty());
    assert_eq!(row.replaced_ids().collect::<Vec<_>>(), vec!["b"]);
}
