//! Duplicate merges that delete files and Missing & broken removals take track records out of
//! the same library, so they must save the same records, purge the same derived data and keep
//! one bounded Undo history between them. A file that moved inside a watch folder is followed
//! and so is neither a duplicate nor missing.
#![cfg(feature = "autotag")]
#[path = "support/duplicates_fixture.rs"]
mod fixture;

use cuerithm_library::{
    BrokenScanRequest, DUPLICATE_UNDO_LIMIT, DuplicateMergeService,
    DuplicateTolerance as Tolerance, MISSING_UNDO_WINDOW, MaintenanceFileState, MergeDisposition,
    REMOVAL_UNDO_LIMIT,
};
use cuerithm_media::CancellationToken;
use fixture::*;
use std::{sync::Arc, time::Duration};

fn token() -> CancellationToken {
    CancellationToken::default()
}
fn service(f: &Fixture, trash: &Arc<FakeTrash>) -> DuplicateMergeService {
    DuplicateMergeService::with_trash(f.lib.clone(), f.runtime.clone(), trash.clone())
}
fn trash(f: &Fixture) -> Arc<FakeTrash> {
    Arc::new(FakeTrash::new(f.dir.path().join("fake-trash")))
}
fn sql(f: &Fixture, query: &str) {
    rusqlite::Connection::open(&f.db)
        .unwrap()
        .execute_batch(query)
        .unwrap();
}
fn count(f: &Fixture, query: &str) -> i64 {
    rusqlite::Connection::open(&f.db)
        .unwrap()
        .query_row(query, [], |r| r.get(0))
        .unwrap()
}

/// Gives a track one of everything that can hang off it, so a removal has to deal with all of
/// it: DJ-program links, a custom tag, cached analysis, review artwork and a sync baseline.
fn give_everything(f: &Fixture, id: &str, hash: &str) {
    sql(
        f,
        &format!(
            "INSERT OR IGNORE INTO native_targets VALUES('desk','rekordbox','db','device','{{}}','fingerprint','observation','[]',1,1,1);
             INSERT OR IGNORE INTO custom_tag_categories VALUES('mood','Mood');
             INSERT OR IGNORE INTO custom_tags VALUES('warm','mood','Warm');
             INSERT INTO track_custom_tags VALUES('{id}','warm');
             INSERT INTO cue_revisions VALUES('cue-{id}','{id}',NULL,'{{}}',1);
             INSERT INTO sync_links VALUES('{id}','target','external-{id}','{{}}');
             INSERT INTO native_track_links VALUES('desk','content-1','{id}','{{}}',NULL,NULL,'{{}}','linked',1);
             INSERT INTO native_sync_track_ownership VALUES('desk','{id}','{{}}','[]','{{}}',NULL);
             INSERT OR IGNORE INTO preparation_analysis VALUES('{hash}','settings','engine','decoder','{{}}');
             INSERT INTO preparation_match_generations(track_id,job_id,generation,attempt,source_hash,settings_revision,payload_json,created)
               VALUES('{id}',NULL,'gen-{id}',1,'{hash}',(SELECT revision FROM selected_settings),'{{}}',0);
             INSERT INTO preparation_review_artwork(generation,candidate_id,url_digest,asset_json) VALUES('gen-{id}','c','d','{{}}');"
        ),
    );
}

/// Every place in the library that still names a track, its file or its content, other than
/// the history lines that are meant to say what happened.
fn traces(f: &Fixture, needles: &[String]) -> Vec<String> {
    const HISTORY: [&str; 2] = ["maintenance_scans", "maintenance_writes"];
    let conn = rusqlite::Connection::open(&f.db).unwrap();
    let tables: Vec<String> = conn
        .prepare("SELECT name FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%' ORDER BY name")
        .unwrap()
        .query_map([], |r| r.get(0))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    let mut found = vec![];
    for table in tables.iter().filter(|t| !HISTORY.contains(&t.as_str())) {
        let columns: Vec<String> = conn
            .prepare(&format!("SELECT name FROM pragma_table_info('{table}')"))
            .unwrap()
            .query_map([], |r| r.get(0))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap();
        for column in columns {
            for needle in needles {
                let hits: i64 = conn
                    .query_row(
                        &format!(
                            "SELECT COUNT(*) FROM \"{table}\" WHERE instr(CAST(\"{column}\" AS TEXT), ?1)>0"
                        ),
                        [needle],
                        |r| r.get(0),
                    )
                    .unwrap();
                if hits > 0 {
                    found.push(format!("{table}.{column} names {needle}"));
                }
            }
        }
    }
    found
}

fn broken_report(f: &Fixture) -> cuerithm_library::BrokenScanReport {
    f.lib
        .scan_broken_tracks(BrokenScanRequest::default(), &f.runtime, &token())
        .unwrap()
}

/// A pair of identical files with a title of its own; the copy leaves through "Delete from disk".
fn merge_removal(f: &mut Fixture, service: &DuplicateMergeService, n: usize) -> String {
    let keep = f.add_track(&format!("pair-{n}-a.wav"), 301 + 2 * n as i32);
    let copy = f.add_track(&format!("pair-{n}-b.wav"), 301 + 2 * n as i32);
    let title = format!(
        "Merge pair {n} {}",
        [
            "alpha", "bravo", "delta", "echo", "golf", "kilo", "lima", "oscar", "papa", "romeo",
            "sierra", "tango", "uniform", "victor"
        ][n]
    );
    sql(
        f,
        &format!(
            "UPDATE tracks SET title='{title}' WHERE id IN ('{}','{}')",
            f.ids[keep], f.ids[copy]
        ),
    );
    let review = f.review(Tolerance::None);
    assert_eq!(review.groups.len(), 1, "round {n}");
    let preview = f
        .lib
        .preview_duplicate_merge_choice_with(
            &review.id,
            review.revision,
            vec![review.groups[0].id.clone()],
            Some(MergeDisposition::Delete),
            &Capable,
            &token(),
        )
        .unwrap();
    let result = service
        .apply_confirmed(&preview.id, true, &token())
        .unwrap();
    assert_eq!(result.removed_ids.len(), 1, "round {n}");
    result.id
}

/// A track whose file is gone is removed from the library through Missing & broken.
fn missing_removal(f: &mut Fixture, n: usize) -> String {
    let gone = f.add_track(&format!("gone-{n}.wav"), 601 + n as i32);
    std::fs::remove_file(&f.paths[gone]).unwrap();
    let report = broken_report(f);
    let preview = f
        .lib
        .preview_missing_removal(&report.id, std::slice::from_ref(&f.ids[gone]), &token())
        .unwrap();
    assert_eq!(preview.ready, 1, "round {n}");
    f.lib
        .apply_missing_removal(&preview.id, &token())
        .unwrap()
        .id
}

fn merge_is_final(f: &Fixture, id: &str) -> bool {
    f.lib
        .duplicate_merge_history()
        .unwrap()
        .into_iter()
        .find(|h| h.result.id == id)
        .unwrap()
        .undo_expired
}
fn missing_is_final(f: &Fixture, id: &str) -> bool {
    f.lib
        .missing_repair_history()
        .unwrap()
        .into_iter()
        .find(|h| h.entry.id == id)
        .unwrap()
        .undo_discarded
}

#[test]
fn every_kind_of_removal_shares_one_undo_limit_of_ten() {
    assert_eq!(REMOVAL_UNDO_LIMIT, 10);
    assert_eq!(DUPLICATE_UNDO_LIMIT, REMOVAL_UNDO_LIMIT);
    assert_eq!(MISSING_UNDO_WINDOW, REMOVAL_UNDO_LIMIT);

    let mut f = Fixture::empty();
    let stand_in = trash(&f);
    let service = service(&f, &stand_in);
    // Fourteen removals, alternating between the two kinds. Counted together, the newest ten
    // keep their records; the four oldest are final, whichever kind they are.
    let mut removals: Vec<(bool, String)> = vec![];
    for n in 0..14 {
        std::thread::sleep(Duration::from_millis(3));
        if n % 2 == 0 {
            removals.push((true, merge_removal(&mut f, &service, n)));
        } else {
            removals.push((false, missing_removal(&mut f, n)));
        }
    }
    let is_final = |f: &Fixture, (merge, id): &(bool, String)| {
        if *merge {
            merge_is_final(f, id)
        } else {
            missing_is_final(f, id)
        }
    };
    for (n, removal) in removals.iter().enumerate() {
        assert_eq!(
            is_final(&f, removal),
            n < 4,
            "removal {n} ({})",
            if removal.0 { "merge" } else { "missing" }
        );
    }
    // A final removal refuses Undo, in either place.
    for (merge, id) in &removals[..4] {
        let error = if *merge {
            service.undo(id, &token()).unwrap_err().to_string()
        } else {
            f.lib
                .undo_missing_repair(id, &token())
                .unwrap_err()
                .to_string()
        };
        assert!(
            error.contains("no longer available") || error.contains("discarded"),
            "{error}"
        );
    }
    // What a final removal keeps is its summary only: the records are not stored any more.
    let stored = count(
        &f,
        &format!(
            "SELECT COUNT(*) FROM maintenance_writes WHERE id='{}' AND json_extract(payload_json,'$.removed') IS NOT NULL",
            removals[0].1
        ),
    );
    assert_eq!(stored, 0);

    // Undoing one frees its place in the window: the next removal does not push another out.
    let newest_merge = removals.iter().rev().find(|r| r.0).unwrap().1.clone();
    service.undo(&newest_merge, &token()).unwrap();
    std::thread::sleep(Duration::from_millis(3));
    let extra = merge_removal(&mut f, &service, 13);
    for removal in &removals[4..] {
        if removal.1 != newest_merge {
            assert!(!is_final(&f, removal), "{}", removal.1);
        }
    }
    assert!(!merge_is_final(&f, &extra));
    // One more now exceeds the limit again, and the oldest still-undoable one becomes final.
    std::thread::sleep(Duration::from_millis(3));
    let last = missing_removal(&mut f, 99);
    assert!(!missing_is_final(&f, &last));
    assert!(
        is_final(&f, &removals[4]),
        "the oldest removal left in the window is pushed out by the eleventh"
    );
    assert!(!is_final(&f, &removals[5]));
}

#[test]
fn a_removal_leaves_nothing_derived_from_the_track_behind_whichever_way_it_happens() {
    // Through "Delete from disk": the copy has the same audio but not the same bytes.
    let f = Fixture::new();
    let (copy, hash_copy) = (f.ids[1].clone(), hash(&f.paths[1]));
    let copy_path = f.paths[1].to_string_lossy().into_owned();
    assert_ne!(hash_copy, hash(&f.paths[0]));
    give_everything(&f, &copy, &hash_copy);
    let art = f.lib.data_dir().join("cache/artwork");
    std::fs::create_dir_all(&art).unwrap();
    std::fs::write(art.join(&hash_copy), b"cover").unwrap();
    let review = f.review(Tolerance::None);
    let preview = f
        .lib
        .preview_duplicate_merge_choice_with(
            &review.id,
            review.revision,
            vec![review.groups[0].id.clone()],
            Some(MergeDisposition::Delete),
            &Capable,
            &token(),
        )
        .unwrap();
    let stand_in = trash(&f);
    let result = service(&f, &stand_in)
        .apply_confirmed(&preview.id, true, &token())
        .unwrap();
    assert_eq!(result.removed_ids, vec![copy.clone()]);
    let left = traces(&f, &[copy.clone(), hash_copy.clone(), copy_path.clone()]);
    assert!(left.is_empty(), "left behind by the merge: {left:#?}");
    assert!(
        !art.join(&hash_copy).exists(),
        "its cover art cache went too"
    );

    // Through Missing & broken: the same call to the same engine, so the same result.
    let mut f = Fixture::new();
    let gone = f.add_track("gone.wav", 211);
    let (hash_gone, gone_path) = (
        hash(&f.paths[gone]),
        f.paths[gone].to_string_lossy().into_owned(),
    );
    let id = f.ids[gone].clone();
    give_everything(&f, &id, &hash_gone);
    let art = f.lib.data_dir().join("cache/artwork");
    std::fs::create_dir_all(&art).unwrap();
    std::fs::write(art.join(&hash_gone), b"cover").unwrap();
    std::fs::remove_file(&f.paths[gone]).unwrap();
    let audio_before = audio_files(&f);
    assert!(!audio_before.is_empty());
    let report = broken_report(&f);
    let preview = f
        .lib
        .preview_missing_removal(&report.id, std::slice::from_ref(&id), &token())
        .unwrap();
    f.lib.apply_missing_removal(&preview.id, &token()).unwrap();
    let left = traces(&f, &[id.clone(), hash_gone.clone(), gone_path]);
    assert!(left.is_empty(), "left behind by the removal: {left:#?}");
    assert!(!art.join(&hash_gone).exists());
    assert_eq!(
        audio_files(&f),
        audio_before,
        "removing records never touches an audio file"
    );
}

/// Every audio file next to the library, by name and content.
fn audio_files(f: &Fixture) -> Vec<(String, String)> {
    let mut files: Vec<(String, String)> = std::fs::read_dir(f.dir.path())
        .unwrap()
        .filter_map(|entry| entry.ok())
        .filter(|entry| entry.path().is_file())
        .map(|entry| {
            (
                entry.file_name().to_string_lossy().into_owned(),
                hash(&entry.path()),
            )
        })
        .collect();
    files.sort();
    files
}

#[test]
fn both_kinds_of_removal_save_the_same_rows_and_unlink_the_rekordbox_track() {
    // The same rows are saved whichever way a track record leaves: compare what each stored.
    let f = Fixture::new();
    let copy = f.ids[1].clone();
    give_everything(&f, &copy, &hash(&f.paths[1]));
    let review = f.review(Tolerance::None);
    let preview = f
        .lib
        .preview_duplicate_merge_choice_with(
            &review.id,
            review.revision,
            vec![review.groups[0].id.clone()],
            Some(MergeDisposition::Delete),
            &Capable,
            &token(),
        )
        .unwrap();
    let stand_in = trash(&f);
    let merged = service(&f, &stand_in)
        .apply_confirmed(&preview.id, true, &token())
        .unwrap();
    let merge_tables = saved_tables(
        &f,
        &format!(
            "SELECT json_extract(payload_json,'$.removed.tables') FROM maintenance_writes WHERE id='{}'",
            merged.id
        ),
    );

    let mut g = Fixture::new();
    let gone = g.add_track("gone.wav", 211);
    let id = g.ids[gone].clone();
    give_everything(&g, &id, &hash(&g.paths[gone]));
    std::fs::remove_file(&g.paths[gone]).unwrap();
    let report = broken_report(&g);
    let preview = g
        .lib
        .preview_missing_removal(&report.id, std::slice::from_ref(&id), &token())
        .unwrap();
    let removed = g.lib.apply_missing_removal(&preview.id, &token()).unwrap();
    let missing_tables = saved_tables(
        &g,
        &format!(
            "SELECT json_extract(payload_json,'$.snapshot.image.tables') FROM maintenance_scans WHERE id='{}'",
            removed.id
        ),
    );
    assert_eq!(
        merge_tables, missing_tables,
        "one engine saves the same rows for the same kind of track"
    );
    // The rekordbox link stays in place, unlinked; every other row leaves and comes back; the
    // caches that are rebuilt on demand are not saved.
    for table in [
        "sync_links",
        "native_sync_track_ownership",
        "track_custom_tags",
        "cue_revisions",
        "jobs",
        "tracks",
    ] {
        assert!(
            merge_tables.contains(&(table.to_owned(), false)),
            "{table} is saved: {merge_tables:?}"
        );
    }
    assert!(merge_tables.contains(&("native_track_links".to_owned(), true)));
    for cache in ["maintenance_fingerprints", "smart_playlist_members"] {
        assert!(!merge_tables.iter().any(|(t, _)| t == cache), "{cache}");
    }
    for fx in [&f, &g] {
        assert_eq!(
            count(
                fx,
                "SELECT COUNT(*) FROM native_track_links WHERE status='not_in_local_library' AND local_track_id IS NULL"
            ),
            1
        );
    }
}

/// The tables a removal saved for Undo, and whether each row stays in place with its
/// reference cleared (detached) or leaves and comes back.
fn saved_tables(f: &Fixture, query: &str) -> Vec<(String, bool)> {
    let text: String = rusqlite::Connection::open(&f.db)
        .unwrap()
        .query_row(query, [], |r| r.get(0))
        .unwrap();
    let mut tables: Vec<(String, bool)> = serde_json::from_str::<Vec<serde_json::Value>>(&text)
        .unwrap()
        .iter()
        .map(|t| {
            (
                t["table"].as_str().unwrap().to_owned(),
                t["detached"].as_bool().unwrap(),
            )
        })
        .collect();
    tables.sort();
    tables
}

#[test]
fn a_file_moved_inside_a_watch_folder_is_followed_so_it_is_neither_missing_nor_a_duplicate() {
    let mut f = Fixture::empty();
    let album = f.dir.path().join("album");
    std::fs::create_dir(&album).unwrap();
    let old = album.join("song.wav");
    wav(&old, 101);
    let moved = f.import(old.clone());
    let other = f.add_track("other.wav", 43);
    // Distinct titles: only the audio could make these two a pair.
    sql(
        &f,
        &format!(
            "UPDATE tracks SET title='Amber Skyline' WHERE id='{}'; UPDATE tracks SET title='Quiet Harbour' WHERE id='{}'",
            f.ids[moved], f.ids[other]
        ),
    );
    // The file is moved up one folder, outside CueRithm.
    let new = f.dir.path().join("song.wav");
    std::fs::rename(&old, &new).unwrap();
    let before = f.scan(Tolerance::None);
    assert_eq!(
        before
            .skipped_unavailable
            .iter()
            .map(|t| (t.id.clone(), t.state))
            .collect::<Vec<_>>(),
        vec![(f.ids[moved].clone(), MaintenanceFileState::Missing)],
        "until it is followed, the track is missing and Duplicates leaves it out"
    );
    // The watch folder sees the file and follows it instead of admitting a second track.
    let outcome = f
        .lib
        .relink_moved_tracks(
            std::slice::from_ref(&new),
            &[f.dir.path().to_path_buf()],
            |_| true,
        )
        .unwrap();
    assert_eq!(outcome.relinked.len(), 1);
    assert_eq!(outcome.consumed, vec![new.clone()], "not admitted as new");
    // Afterwards the track is neither a duplicate nor missing, and nothing was added.
    let report = f.scan(Tolerance::None);
    assert!(report.groups.is_empty(), "{:?}", report.groups);
    assert!(report.skipped_unavailable.is_empty());
    let files = broken_report(&f);
    assert!(
        files
            .tracks
            .iter()
            .all(|t| t.state == MaintenanceFileState::Present),
        "{:?}",
        files.tracks
    );
    assert_eq!(count(&f, "SELECT COUNT(*) FROM tracks"), 2);
    assert_eq!(
        f.lib.track_path(&f.ids[moved]).unwrap(),
        new.canonicalize().unwrap()
    );
}
