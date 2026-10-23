//! Duplicates never finds, lists, counts or merges a track whose audio file is not available.
//! Missing files belong to Missing & broken.
#[path = "support/duplicates_fixture.rs"]
mod fixture;

use cuerithm_library::{
    BrokenScanRequest, DuplicateMatchMethod, DuplicateTolerance as Tolerance, MaintenanceFileState,
};
use cuerithm_media::CancellationToken;
use fixture::*;

const TOLERANCES: [Tolerance; 5] = [
    Tolerance::None,
    Tolerance::Low,
    Tolerance::Medium,
    Tolerance::High,
    Tolerance::HighIgnoreParentheses,
];

/// The owner's real case: files moved up one folder, so the watch folder added them again.
/// The library then holds a missing record and a present twin for every file.
fn moved_library(files: usize) -> (Fixture, Vec<String>, Vec<String>) {
    let mut f = Fixture::empty();
    let sub = f.dir.path().join("album");
    std::fs::create_dir(&sub).unwrap();
    let (mut missing, mut twins) = (vec![], vec![]);
    for i in 0..files {
        let old = sub.join(format!("song-{i}.wav"));
        wav(&old, 101 + 2 * i as i32);
        let index = f.import(old.clone());
        let new = f.dir.path().join(format!("song-{i}.wav"));
        std::fs::rename(&old, &new).unwrap();
        let twin = f.import(new);
        missing.push(f.ids[index].clone());
        twins.push(f.ids[twin].clone());
    }
    // Unrelated titles, so the present tracks do not match one another loosely.
    let titles = [
        "Amber Skyline",
        "Quiet Harbour",
        "Violet Engine",
        "Hollow Garden",
    ];
    let conn = rusqlite::Connection::open(&f.db).unwrap();
    for i in 0..files {
        conn.execute(
            "UPDATE tracks SET title=?1 WHERE id IN (?2,?3)",
            rusqlite::params![titles[i], missing[i], twins[i]],
        )
        .unwrap();
    }
    (f, missing, twins)
}

#[test]
fn moved_files_leave_missing_records_that_are_never_paired_with_their_present_twins() {
    for tolerance in TOLERANCES {
        let (f, missing, _twins) = moved_library(3);
        let report = f.scan(tolerance);
        assert!(
            report.groups.is_empty(),
            "{tolerance:?} grouped {:?}",
            report.groups
        );
        let skipped = report
            .skipped_unavailable
            .iter()
            .map(|t| (t.id.clone(), t.state))
            .collect::<Vec<_>>();
        assert_eq!(
            skipped,
            missing
                .iter()
                .map(|id| (id.clone(), MaintenanceFileState::Missing))
                .collect::<Vec<_>>(),
            "{tolerance:?}"
        );
        // The missing records stay visible as file checks; they are only never compared.
        for id in &missing {
            let row = report.tracks.iter().find(|t| &t.id == id).unwrap();
            assert_eq!(row.state, MaintenanceFileState::Missing);
            assert!(row.fingerprint.is_none());
        }
        assert!(
            f.lib
                .begin_duplicate_review(&report.id, &CancellationToken::default())
                .is_ok()
        );
    }
}

#[test]
fn present_duplicates_still_group_while_the_missing_one_is_left_out() {
    let mut f = Fixture::new();
    let gone = f.add_track("gone.wav", 37);
    std::fs::remove_file(&f.paths[gone]).unwrap();
    let report = f.scan(Tolerance::None);
    assert_eq!(report.groups.len(), 1);
    let group = &report.groups[0];
    assert_eq!(group.method, DuplicateMatchMethod::Fingerprint);
    assert_eq!(group.track_ids, vec![f.ids[0].clone(), f.ids[1].clone()]);
    assert!(!group.track_ids.contains(&f.ids[gone]));
    assert_eq!(report.skipped_unavailable.len(), 1);
    assert_eq!(report.skipped_unavailable[0].id, f.ids[gone]);
    assert_ne!(
        group.suggested_keeper.as_deref(),
        Some(f.ids[gone].as_str())
    );
}

#[test]
fn artist_title_tolerances_never_pull_an_unavailable_track_into_a_group() {
    for tolerance in [
        Tolerance::None,
        Tolerance::Low,
        Tolerance::Medium,
        Tolerance::High,
        Tolerance::HighIgnoreParentheses,
    ] {
        let mut f = Fixture::new();
        // Tracks 2 and the new one are different audio with the same artist and title.
        let other = f.add_track("other.wav", 71);
        let gone = f.add_track("gone.wav", 79);
        std::fs::remove_file(&f.paths[gone]).unwrap();
        let report = f.scan(tolerance);
        for group in &report.groups {
            assert!(
                !group.track_ids.contains(&f.ids[gone]),
                "{tolerance:?}: {group:?}"
            );
            assert_ne!(
                group.suggested_keeper.as_deref(),
                Some(f.ids[gone].as_str())
            );
            assert_ne!(
                group.suggested_tags_from.as_deref(),
                Some(f.ids[gone].as_str())
            );
        }
        assert!(
            report
                .groups
                .iter()
                .any(|g| g.method == DuplicateMatchMethod::ArtistTitle
                    && g.track_ids.contains(&f.ids[other])),
            "{tolerance:?} should still group the available tracks"
        );
        assert_eq!(report.skipped_unavailable.len(), 1);
    }
}

#[test]
fn a_scan_of_available_files_reports_nothing_skipped_and_groups_as_before() {
    let f = Fixture::new();
    let report = f.scan(Tolerance::None);
    assert!(report.skipped_unavailable.is_empty());
    assert_eq!(report.groups.len(), 1);
    assert_eq!(
        report.groups[0].track_ids,
        vec![f.ids[0].clone(), f.ids[1].clone()]
    );
    assert_eq!(report.groups[0].method, DuplicateMatchMethod::Fingerprint);
}

#[test]
fn duplicates_leaves_out_exactly_the_tracks_missing_and_broken_calls_unavailable() {
    let (f, missing, _twins) = moved_library(2);
    // An offline folder is unavailable rather than missing.
    let offline = f.dir.path().join("offline");
    std::fs::create_dir(&offline).unwrap();
    let mut f = f;
    let path = offline.join("away.wav");
    wav(&path, 131);
    let away = f.import(path.clone());
    std::fs::remove_file(&path).unwrap();
    std::fs::remove_dir(&offline).unwrap();

    let token = CancellationToken::default();
    let broken = f
        .lib
        .scan_broken_tracks(BrokenScanRequest::default(), &f.runtime, &token)
        .unwrap();
    let broken_states = broken
        .tracks
        .iter()
        .filter(|t| t.state != MaintenanceFileState::Present)
        .map(|t| (t.id.clone(), t.state))
        .collect::<Vec<_>>();
    let duplicates = f.scan(Tolerance::None);
    let skipped = duplicates
        .skipped_unavailable
        .iter()
        .map(|t| (t.id.clone(), t.state))
        .collect::<Vec<_>>();
    assert_eq!(skipped, broken_states);
    assert!(skipped.contains(&(f.ids[away].clone(), MaintenanceFileState::Unavailable)));
    assert!(skipped.contains(&(missing[0].clone(), MaintenanceFileState::Missing)));
}

fn pair() -> (Fixture, cuerithm_library::DuplicateReview) {
    let f = Fixture::new();
    let review = f.review(Tolerance::None);
    assert_eq!(review.groups.len(), 1);
    (f, review)
}

#[test]
fn a_saved_report_cannot_be_reloaded_into_a_review_after_a_file_disappears() {
    let f = Fixture::new();
    let scan = f.scan(Tolerance::None);
    std::fs::remove_file(&f.paths[1]).unwrap();
    let before = f.content();
    let reloaded = f.lib.duplicate_scan(&scan.id).unwrap();
    assert!(reloaded.stale);
    assert!(
        f.lib
            .begin_duplicate_review(&scan.id, &CancellationToken::default())
            .is_err()
    );
    assert_eq!(f.content(), before);
}

#[test]
fn preview_and_apply_refuse_when_a_participant_file_disappears_after_review() {
    let token = CancellationToken::default();
    let (f, review) = pair();
    let group = review.groups[0].id.clone();
    // One preview made while everything was available, a second review state to preview later.
    let early = f
        .lib
        .preview_duplicate_merge(&review.id, review.revision, vec![group.clone()], &token)
        .unwrap();
    std::fs::remove_file(&f.paths[1]).unwrap();
    let before = f.content();

    let preview_error = f
        .lib
        .preview_duplicate_merge(&review.id, review.revision, vec![group.clone()], &token)
        .unwrap_err()
        .to_string();
    assert!(
        preview_error.contains("no longer has an available audio file"),
        "{preview_error}"
    );
    assert!(
        preview_error.contains("Missing & broken"),
        "{preview_error}"
    );
    assert!(
        preview_error.contains("Nothing was changed"),
        "{preview_error}"
    );
    let apply_error = f
        .lib
        .apply_duplicate_merge(&early.id, &token)
        .unwrap_err()
        .to_string();
    assert!(
        apply_error.contains("no longer has an available audio file"),
        "{apply_error}"
    );
    assert_eq!(f.content(), before);
    // The reloaded review is stale too, and the stale rules still hold.
    assert!(f.lib.duplicate_review(&review.id, &token).unwrap().stale);
}

#[test]
fn the_keeper_disappearing_is_refused_as_well() {
    let token = CancellationToken::default();
    let (f, review) = pair();
    let group = review.groups[0].id.clone();
    let preview = f
        .lib
        .preview_duplicate_merge(&review.id, review.revision, vec![group], &token)
        .unwrap();
    let keeper = preview.plan.groups[0].keeper.clone();
    let index = f.ids.iter().position(|id| *id == keeper).unwrap();
    std::fs::remove_file(&f.paths[index]).unwrap();
    let before = f.content();
    assert!(f.lib.apply_duplicate_merge(&preview.id, &token).is_err());
    assert_eq!(f.content(), before);
}

#[test]
fn apply_checks_the_files_itself_even_when_the_saved_proofs_do_not_notice() {
    let token = CancellationToken::default();
    let (f, review) = pair();
    let group = review.groups[0].id.clone();
    let preview = f
        .lib
        .preview_duplicate_merge(&review.id, review.revision, vec![group], &token)
        .unwrap();
    std::fs::remove_file(&f.paths[1]).unwrap();
    // A report saved by older logic, or damaged, can record the vanished file as already
    // missing. The staleness rules then see nothing wrong; the merge must still refuse.
    let conn = rusqlite::Connection::open(&f.db).unwrap();
    conn.execute(
        "UPDATE maintenance_scans SET payload_json=json_set(payload_json,'$.review.scan.files[1].state','missing','$.review.scan.files[1].hash',json('null')) WHERE id=?1",
        [&preview.id],
    )
    .unwrap();
    drop(conn);
    let before = f.content();
    let error = f
        .lib
        .apply_duplicate_merge(&preview.id, &token)
        .unwrap_err()
        .to_string();
    assert!(
        error.contains("no longer has an available audio file"),
        "{error}"
    );
    assert_eq!(f.content(), before);
    assert!(!f.archived(&f.ids[1]));
}

#[test]
fn artist_title_groups_are_refused_the_same_way() {
    let token = CancellationToken::default();
    let mut f = Fixture::new();
    let other = f.add_track("other.wav", 71);
    let review = f.review(Tolerance::None);
    let group = review
        .groups
        .iter()
        .find(|g| g.tracks.iter().any(|t| t.candidate.id == f.ids[other]))
        .unwrap();
    assert!(!group.exact_audio);
    let id = group.id.clone();
    let early = f
        .lib
        .preview_duplicate_merge(&review.id, review.revision, vec![id.clone()], &token)
        .unwrap();
    std::fs::remove_file(&f.paths[other]).unwrap();
    let before = f.content();
    assert!(
        f.lib
            .preview_duplicate_merge(&review.id, review.revision, vec![id], &token)
            .is_err()
    );
    assert!(f.lib.apply_duplicate_merge(&early.id, &token).is_err());
    assert_eq!(f.content(), before);
}
