use cuerithm_maintenance::{Candidate, Matcher, Tolerance};

fn track(id: &str) -> Candidate {
    Candidate {
        id: id.into(),
        artist: "North".into(),
        title: "Signal".into(),
        duration: 180.,
        location: format!("/authored/{id}.flac"),
        comparison_path: format!("/authored/{id}.flac"),
        bitrate: Some(1000.),
        cue_count: 0,
        present: true,
        ignored: false,
    }
}

#[test]
fn invalid_or_duplicate_identities_and_nonfinite_measurements_are_rejected() {
    let matcher = Matcher::new();
    assert!(
        matcher
            .metadata_groups(&[track("a"), track("a")], Tolerance::None, &[], &[])
            .is_err()
    );
    let mut invalid = track("b");
    invalid.duration = f64::NAN;
    assert!(
        matcher
            .metadata_groups(&[track("a"), invalid], Tolerance::High, &[], &[])
            .is_err()
    );
    let mut invalid = track("b");
    invalid.bitrate = Some(f64::INFINITY);
    assert!(matcher.preselect(&[track("a"), invalid], true).is_err());
}

#[test]
fn missing_and_ignored_records_cannot_be_suggested_as_the_keeper() {
    let mut absent = track("absent");
    absent.present = false;
    absent.cue_count = 8;
    let mut ignored = track("ignored");
    ignored.ignored = true;
    ignored.cue_count = 8;
    let keeper = track("keeper");
    let selected = Matcher::new()
        .preselect(&[absent, ignored, keeper], true)
        .unwrap();
    assert_eq!(selected.keeper.as_deref(), Some("keeper"));
    assert!(selected.tags_from.is_none());
}

#[test]
fn metadata_candidates_do_not_automatically_transfer_cues() {
    let mut with_cues = track("cues");
    with_cues.cue_count = 8;
    with_cues.bitrate = None;
    let selected = Matcher::new()
        .preselect(&[track("keeper"), with_cues], false)
        .unwrap();
    assert_eq!(selected.keeper.as_deref(), Some("keeper"));
    assert!(selected.tags_from.is_none());
}

#[test]
fn exact_duration_tolerance_chains_adjacent_members_instead_of_first_member() {
    let mut tracks = vec![track("a"), track("b"), track("c"), track("d")];
    for (t, d) in tracks.iter_mut().zip([180., 185., 190., 196.]) {
        t.duration = d;
    }
    assert_eq!(
        Matcher::new()
            .metadata_groups(&tracks, Tolerance::None, &[], &[])
            .unwrap(),
        vec![vec!["a", "b", "c"]]
    );
    assert_eq!(
        Matcher::new()
            .metadata_groups(&tracks, Tolerance::Medium, &[], &[])
            .unwrap(),
        vec![vec!["a", "b", "c", "d"]]
    );
}
