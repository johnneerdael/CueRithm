//! "Delete from disk" moves only verified duplicate audio files to the Trash, removes only the
//! records of files that really moved, and can be undone for the library records. The Trash here
//! is a folder in the test's temporary directory; no real Trash or music folder is involved.
#![cfg(feature = "autotag")]
#[path = "support/duplicates_fixture.rs"]
mod fixture;

use cuerithm_library::{
    DuplicateIdentity, DuplicateMergePreview, DuplicateMergeService, DuplicateReview,
    DuplicateReviewEdit, DuplicateSkipReason, DuplicateTolerance as Tolerance, GroupEdit,
    MergeDisposition,
};
use cuerithm_media::CancellationToken;
use fixture::*;
use std::sync::Arc;

fn token() -> CancellationToken {
    CancellationToken::default()
}
fn trash(f: &Fixture) -> Arc<FakeTrash> {
    Arc::new(FakeTrash::new(f.dir.path().join("fake-trash")))
}
fn service(f: &Fixture, trash: &Arc<FakeTrash>) -> DuplicateMergeService {
    DuplicateMergeService::with_trash(f.lib.clone(), f.runtime.clone(), trash.clone())
}
fn preview(
    f: &Fixture,
    review: &DuplicateReview,
    group: usize,
    choice: MergeDisposition,
) -> DuplicateMergePreview {
    f.lib
        .preview_duplicate_merge_choice(
            &review.id,
            review.revision,
            vec![review.groups[group].id.clone()],
            Some(choice),
            &token(),
        )
        .unwrap()
}
fn hashes(f: &Fixture) -> Vec<String> {
    f.paths.iter().map(|p| hash(p)).collect()
}

#[test]
fn delete_from_disk_moves_the_verified_duplicate_removes_its_record_and_undo_restores_the_records()
{
    let f = Fixture::new();
    f.playlist(
        "set",
        vec![f.ids[0].clone(), f.ids[2].clone(), f.ids[1].clone()],
    );
    // The duplicate also carries a custom tag, a sync link and a rekordbox link.
    let conn = rusqlite::Connection::open(&f.db).unwrap();
    conn.execute_batch("INSERT INTO custom_tag_categories VALUES('mood','Mood'); INSERT INTO custom_tags VALUES('warm','mood','Warm'); INSERT INTO native_targets VALUES('desk','rekordbox','db','device','{}','fingerprint','observation','[]',1,1,1);").unwrap();
    conn.execute(
        "INSERT INTO track_custom_tags VALUES(?1,'warm')",
        [&f.ids[1]],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO sync_links VALUES(?1,'target','external-1','{}')",
        [&f.ids[1]],
    )
    .unwrap();
    conn.execute("INSERT INTO native_track_links VALUES('desk','content-1',?1,'{}',NULL,NULL,'{}','linked',1)", [&f.ids[1]]).unwrap();
    drop(conn);
    let before = f.content();
    // The library records canonical paths.
    let canonical = |i: usize| {
        f.paths[i]
            .canonicalize()
            .unwrap()
            .to_string_lossy()
            .into_owned()
    };
    let (duplicate_path, keeper_path) = (canonical(1), canonical(0));
    let sizes = std::fs::metadata(&f.paths[1]).unwrap().len();
    let untouched = [0, 2, 3]
        .into_iter()
        .map(|i| (i, hash(&f.paths[i])))
        .collect::<Vec<_>>();
    let review = f.review(Tolerance::None);
    let preview = preview(&f, &review, 0, MergeDisposition::Delete);

    // The preview lists exactly what will happen.
    assert_eq!(preview.plan.disposition, MergeDisposition::Delete);
    let plan = preview.deletion.as_ref().unwrap();
    assert_eq!(plan.files.len(), 1);
    let file = &plan.files[0];
    assert_eq!(file.track_id, f.ids[1]);
    assert_eq!(file.keeper_id, f.ids[0]);
    assert_eq!(file.size_bytes, sizes);
    assert_eq!(file.display_path, duplicate_path);
    assert_eq!(file.keeper_display_path, keeper_path);
    assert_eq!(file.identity, DuplicateIdentity::SameAudio);
    assert!(plan.skipped.is_empty());
    assert_eq!(plan.undo_limit, 10);
    assert_eq!(plan.impact.len(), 1);
    assert_eq!(plan.impact[0].track_id, f.ids[1]);
    assert!(
        plan.impact[0].other_records >= 1,
        "the analysis job leaves with it"
    );
    assert_eq!(preview.plan.groups[0].delete_ids, vec![f.ids[1].clone()]);
    assert!(preview.plan.groups[0].archive_ids.is_empty());
    // The kept track takes the removed record's place in the playlist.
    assert_eq!(
        preview.playlists[0].after,
        vec![f.ids[2].clone(), f.ids[0].clone()]
    );

    let stand_in = trash(&f);
    let service = service(&f, &stand_in);
    // Deleting needs a separate, explicit confirmation.
    assert!(service.apply(&preview.id, &token()).is_err());
    assert_eq!(f.content(), before);
    assert!(stand_in.taken().is_empty());

    let result = service
        .apply_confirmed(&preview.id, true, &token())
        .unwrap();
    assert_eq!(result.removed_ids, vec![f.ids[1].clone()]);
    assert!(result.archived_ids.is_empty());
    assert_eq!(result.trashed, vec![file.clone()]);
    assert!(result.not_deleted.is_empty());
    assert_eq!(stand_in.taken(), vec!["track-1.wav"]);
    assert!(!f.paths[1].exists(), "the duplicate went to the Trash");
    assert!(
        !f.track_exists(&f.ids[1]),
        "its record is gone, not archived"
    );
    assert!(f.track_exists(&f.ids[0]));
    assert!(!f.archived(&f.ids[0]));
    for (i, expected) in &untouched {
        assert_eq!(&hash(&f.paths[*i]), expected, "file {i} must be untouched");
    }
    assert_eq!(
        f.lib.manual_playlist_members("set").unwrap(),
        vec![f.ids[2].clone(), f.ids[0].clone()]
    );
    let history = f.lib.duplicate_merge_history().unwrap();
    assert_eq!(history[0].result.id, result.id);
    assert_eq!(history[0].result.removed_ids, vec![f.ids[1].clone()]);
    assert!(!history[0].undo_expired);
    assert_eq!(history[0].undo_limit, 10);
    // Nothing else records the merge as a pending recovery.
    assert!(f.lib.duplicate_recovery().unwrap().is_empty());

    // Undo brings back the records, their rows in the original order and the playlist
    // position. The file is wherever the user keeps it (here: still in the stand-in Trash).
    let undone = service.undo(&result.id, &token()).unwrap();
    assert!(undone.restored_ids.contains(&f.ids[1]));
    assert!(undone.removed_ids.is_empty());
    assert_eq!(f.content(), before);
    assert_eq!(
        f.lib.manual_playlist_members("set").unwrap(),
        vec![f.ids[0].clone(), f.ids[2].clone(), f.ids[1].clone()]
    );
    assert!(
        !f.paths[1].exists(),
        "Undo does not take a file out of the Trash"
    );
    assert!(service.undo(&result.id, &token()).is_err());
    assert_eq!(
        f.lib.duplicate_merge_history().unwrap()[0]
            .undone_by
            .as_deref(),
        Some(undone.id.as_str())
    );
}

#[test]
fn nothing_can_be_previewed_without_a_choice_and_archive_only_never_touches_audio_files() {
    let f = Fixture::new();
    let review = f.review(Tolerance::None);
    let error = f
        .lib
        .preview_duplicate_merge_choice(
            &review.id,
            review.revision,
            vec![review.groups[0].id.clone()],
            None,
            &token(),
        )
        .unwrap_err()
        .to_string();
    assert!(
        error.contains("Choose Delete from disk or Archive only"),
        "{error}"
    );

    let before = f.content();
    let files = hashes(&f);
    let preview = preview(&f, &review, 0, MergeDisposition::Archive);
    assert!(preview.deletion.is_none());
    assert_eq!(preview.plan.groups[0].archive_ids, vec![f.ids[1].clone()]);
    assert!(preview.plan.groups[0].delete_ids.is_empty());
    let stand_in = trash(&f);
    let service = service(&f, &stand_in);
    let result = service.apply(&preview.id, &token()).unwrap();
    assert_eq!(result.archived_ids, vec![f.ids[1].clone()]);
    assert!(result.removed_ids.is_empty());
    assert!(f.archived(&f.ids[1]));
    assert_eq!(hashes(&f), files, "archiving never touches a file");
    assert!(stand_in.taken().is_empty());
    service.undo(&result.id, &token()).unwrap();
    assert_eq!(f.content(), before);
}

#[test]
fn only_audio_proven_identical_to_the_kept_track_can_be_deleted() {
    let mut f = Fixture::new();
    // Same artist and title, different audio: a candidate by name only.
    let other = f.add_track("other-mix.wav", 71);
    f.playlist("mixes", vec![f.ids[2].clone(), f.ids[other].clone()]);
    let review = f.review(Tolerance::None);
    let group = review
        .groups
        .iter()
        .position(|g| g.tracks.iter().any(|t| t.candidate.id == f.ids[other]))
        .unwrap();
    assert!(!review.groups[group].exact_audio);
    let before = f.content();
    let files = hashes(&f);
    let preview = preview(&f, &review, group, MergeDisposition::Delete);
    let plan = preview.deletion.as_ref().unwrap();
    assert!(plan.files.is_empty());
    assert_eq!(plan.skipped.len(), 1);
    assert_eq!(
        plan.skipped[0].reason,
        DuplicateSkipReason::NotVerifiedIdentical
    );
    assert!(preview.plan.groups[0].delete_ids.is_empty());
    assert!(
        preview.playlists.is_empty(),
        "a skipped record is not replaced anywhere"
    );

    let stand_in = trash(&f);
    let result = service(&f, &stand_in)
        .apply_confirmed(&preview.id, true, &token())
        .unwrap();
    assert!(result.removed_ids.is_empty());
    assert_eq!(result.not_deleted.len(), 1);
    assert!(stand_in.taken().is_empty());
    assert_eq!(hashes(&f), files);
    assert_eq!(
        f.content(),
        before,
        "a skipped file changes nothing for its record"
    );
}

#[test]
fn the_kept_tracks_own_file_is_never_deleted_even_under_another_record() {
    let mut f = Fixture::new();
    // A second record for the same physical file as track 0.
    let alias = f.dir.path().join("alias.wav");
    std::fs::hard_link(&f.paths[0], &alias).unwrap();
    let alias_index = f.import(alias.clone());
    let review = f.review(Tolerance::None);
    assert_eq!(review.groups[0].tracks.len(), 3);
    let preview = preview(&f, &review, 0, MergeDisposition::Delete);
    let plan = preview.deletion.as_ref().unwrap();
    let keeper = preview.plan.groups[0].keeper.clone();
    // Whichever record is kept, the other name for the same file is left alone.
    let same = plan
        .skipped
        .iter()
        .filter(|s| s.reason == DuplicateSkipReason::SameFileAsKeeper)
        .collect::<Vec<_>>();
    assert_eq!(same.len(), 1, "{plan:?}");
    assert!(
        [f.ids[0].clone(), f.ids[alias_index].clone()].contains(&same[0].track_id)
            && same[0].track_id != keeper
    );
    assert_eq!(plan.files.len(), 1);
    assert_eq!(plan.files[0].track_id, f.ids[1]);

    let stand_in = trash(&f);
    let result = service(&f, &stand_in)
        .apply_confirmed(&preview.id, true, &token())
        .unwrap();
    assert_eq!(result.removed_ids, vec![f.ids[1].clone()]);
    assert_eq!(stand_in.taken(), vec!["track-1.wav"]);
    assert!(f.paths[0].exists() && alias.exists());
    assert!(f.track_exists(&f.ids[0]) && f.track_exists(&f.ids[alias_index]));
}

#[test]
fn a_file_another_remaining_record_still_uses_is_not_deleted() {
    let mut f = Fixture::new();
    // The library also lists the duplicate's file under another name. That record is
    // ignored for duplicates, so it is not part of the merge and stays in the library.
    let alias = f.dir.path().join("also-track-1.wav");
    std::fs::hard_link(&f.paths[1], &alias).unwrap();
    let alias_index = f.import(alias);
    let stamp = f.lib.duplicate_ignores().unwrap().stamp;
    f.lib
        .set_duplicate_ignores(vec![f.ids[alias_index].clone()], true, stamp)
        .unwrap();
    let review = f.review(Tolerance::None);
    assert_eq!(review.groups.len(), 1);
    let before = f.content();
    let preview = preview(&f, &review, 0, MergeDisposition::Delete);
    let plan = preview.deletion.as_ref().unwrap();
    assert!(plan.files.is_empty(), "{plan:?}");
    assert_eq!(plan.skipped.len(), 1);
    assert_eq!(
        plan.skipped[0].reason,
        DuplicateSkipReason::SharedWithRemainingRecord
    );
    assert_eq!(plan.skipped[0].track_id, f.ids[1]);
    let stand_in = trash(&f);
    let result = service(&f, &stand_in)
        .apply_confirmed(&preview.id, true, &token())
        .unwrap();
    assert!(result.removed_ids.is_empty());
    assert!(f.paths[1].exists());
    assert!(stand_in.taken().is_empty());
    assert_eq!(f.content(), before);
}

#[test]
fn a_trash_failure_leaves_that_file_and_its_record_untouched_and_removes_only_what_moved() {
    let mut f = Fixture::new();
    let third = f.add_track("track-4.wav", 37);
    f.playlist("both", vec![f.ids[1].clone(), f.ids[third].clone()]);
    let review = f.review(Tolerance::None);
    assert_eq!(review.groups[0].tracks.len(), 3);
    let preview = preview(&f, &review, 0, MergeDisposition::Delete);
    assert_eq!(preview.deletion.as_ref().unwrap().files.len(), 2);
    let before = f.content();
    let stand_in = trash(&f);
    stand_in.refuse("track-1.wav");
    let result = service(&f, &stand_in)
        .apply_confirmed(&preview.id, true, &token())
        .unwrap();

    assert_eq!(result.removed_ids, vec![f.ids[third].clone()]);
    assert_eq!(stand_in.taken(), vec!["track-4.wav"]);
    assert_eq!(result.not_deleted.len(), 1);
    let failed = &result.not_deleted[0];
    assert_eq!(failed.track_id, f.ids[1]);
    assert_eq!(failed.reason, DuplicateSkipReason::TrashFailed);
    assert!(f.paths[1].exists(), "the file stays where it was");
    assert!(f.track_exists(&f.ids[1]) && !f.archived(&f.ids[1]));
    assert!(!f.track_exists(&f.ids[third]));
    let members = f.lib.manual_playlist_members("both").unwrap();
    assert!(members.contains(&f.ids[1]) && members.contains(&f.ids[0]));
    assert!(!members.contains(&f.ids[third]));

    // Undo restores exactly what was removed.
    service(&f, &stand_in).undo(&result.id, &token()).unwrap();
    assert_eq!(f.content(), before);
}

#[test]
fn a_stale_report_cannot_delete_anything() {
    let f = Fixture::new();
    let review = f.review(Tolerance::None);
    let preview = preview(&f, &review, 0, MergeDisposition::Delete);
    let before = f.content();
    let files = hashes(&f);
    let stand_in = trash(&f);

    // The duplicate changes after the preview.
    let mut bytes = std::fs::read(&f.paths[1]).unwrap();
    bytes.push(0);
    std::fs::write(&f.paths[1], &bytes).unwrap();
    let changed = hash(&f.paths[1]);
    assert!(
        service(&f, &stand_in)
            .apply_confirmed(&preview.id, true, &token())
            .is_err()
    );
    assert_eq!(hash(&f.paths[1]), changed);
    assert!(stand_in.taken().is_empty());
    assert_eq!(f.content(), before);

    // The kept track's file disappears after the preview.
    std::fs::remove_file(&f.paths[0]).unwrap();
    let error = service(&f, &stand_in)
        .apply_confirmed(&preview.id, true, &token())
        .unwrap_err()
        .to_string();
    assert!(
        error.contains("no longer has an available audio file"),
        "{error}"
    );
    assert!(stand_in.taken().is_empty());
    assert_eq!(f.content(), before);
    assert_eq!(hash(&f.paths[2]), files[2]);
}

#[test]
fn a_report_saved_before_the_file_disappeared_cannot_be_reviewed_or_deleted_from() {
    let f = Fixture::new();
    let scan = f.scan(Tolerance::None);
    std::fs::remove_file(&f.paths[1]).unwrap();
    let stand_in = trash(&f);
    assert!(f.lib.begin_duplicate_review(&scan.id, &token()).is_err());
    assert!(stand_in.taken().is_empty());
}

#[test]
fn delete_from_disk_with_a_donor_copies_tags_first_and_undo_restores_the_keepers_file() {
    let f = Fixture::new();
    let conn = rusqlite::Connection::open(&f.db).unwrap();
    let publication = serde_json::json!({"grid":{"bpm":128.75,"startTime":0.125},"cues":[{"position":0,"name":"Donor breakdown","startTime":8.,"type":"1","color":"blue"}],"implementation_sha256":"a".repeat(64),"inputs_sha256":"b".repeat(64)});
    conn.execute("UPDATE tracks SET title='Donor title',genre='House',bpm=136.25,musical_key='8A',energy=8,analysis_json=?1 WHERE id=?2",rusqlite::params![serde_json::json!({"raw_donor":true,"publication":publication}).to_string(),f.ids[1]]).unwrap();
    drop(conn);
    let review = f.review(Tolerance::None);
    let review = f
        .lib
        .edit_duplicate_review(
            &review.id,
            0,
            DuplicateReviewEdit::Group {
                group_id: review.groups[0].id.clone(),
                edit: GroupEdit::UseTags {
                    track_id: Some(f.ids[1].clone()),
                },
            },
            &token(),
        )
        .unwrap();
    let keeper_before = hash(&f.paths[0]);
    let before = f.content();
    let preview = preview(&f, &review, 0, MergeDisposition::Delete);
    assert_eq!(preview.plan.groups[0].copies.len(), 1);
    let stand_in = trash(&f);
    let service = service(&f, &stand_in);
    let result = service
        .apply_confirmed(&preview.id, true, &token())
        .unwrap();

    assert_eq!(result.removed_ids, vec![f.ids[1].clone()]);
    assert_eq!(result.updated_ids, vec![f.ids[0].clone()]);
    assert_eq!(f.lib.track(&f.ids[0]).unwrap().title, "Donor title");
    assert_ne!(
        hash(&f.paths[0]),
        keeper_before,
        "the keeper's tags were rewritten"
    );
    assert_eq!(stand_in.taken(), vec!["track-1.wav"]);
    assert!(!f.track_exists(&f.ids[1]));

    service.undo(&result.id, &token()).unwrap();
    assert_eq!(
        hash(&f.paths[0]),
        keeper_before,
        "the keeper's original bytes are restored"
    );
    assert_eq!(f.content(), before);
    assert!(f.track_exists(&f.ids[1]));
}

#[test]
fn undo_data_is_kept_only_for_the_most_recent_merges_that_delete_files() {
    let mut f = Fixture::empty();
    let stand_in = trash(&f);
    let service = service(&f, &stand_in);
    let mut applied = vec![];
    for round in 0..12 {
        // A new pair of identical files, unrelated to every earlier pair.
        let keep = f.add_track(&format!("pair-{round}-a.wav"), 201 + 2 * round);
        let copy = f.add_track(&format!("pair-{round}-b.wav"), 201 + 2 * round);
        let words = [
            "alpha", "bravo", "delta", "echo", "golf", "kilo", "lima", "oscar", "papa", "romeo",
            "sierra", "tango",
        ];
        let title = format!("Distinct title {round} {}", words[round as usize]);
        let conn = rusqlite::Connection::open(&f.db).unwrap();
        conn.execute(
            "UPDATE tracks SET title=?1 WHERE id IN (?2,?3)",
            rusqlite::params![title, f.ids[keep], f.ids[copy]],
        )
        .unwrap();
        drop(conn);
        let review = f.review(Tolerance::None);
        assert_eq!(review.groups.len(), 1, "round {round}");
        let preview = preview(&f, &review, 0, MergeDisposition::Delete);
        let result = service
            .apply_confirmed(&preview.id, true, &token())
            .unwrap();
        assert_eq!(result.removed_ids.len(), 1, "round {round}");
        applied.push(result.id);
    }
    let history = f.lib.duplicate_merge_history().unwrap();
    assert_eq!(history.len(), 12);
    let expired = |id: &str| {
        history
            .iter()
            .find(|h| h.result.id == id)
            .unwrap()
            .undo_expired
    };
    // The two oldest removals are final; the ten newest can still be undone.
    assert!(expired(&applied[0]) && expired(&applied[1]));
    assert!(applied[2..].iter().all(|id| !expired(id)));
    for id in &applied[..2] {
        let error = service.undo(id, &token()).unwrap_err().to_string();
        assert!(error.contains("no longer available"), "{error}");
    }
    // What a final removal keeps is only its summary.
    let conn = rusqlite::Connection::open(&f.db).unwrap();
    let stored: (Option<String>, i64) = conn
        .query_row(
            "SELECT json_extract(payload_json,'$.removed'),(SELECT COUNT(*) FROM json_each(json_extract(payload_json,'$.before'))) FROM maintenance_writes WHERE id=?1",
            [&applied[0]],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    assert_eq!(stored.0, None);
    assert_eq!(stored.1, 0);
    service.undo(&applied[2], &token()).unwrap();
}
