//! A file that is moved or renamed inside a watch folder keeps its library track.
//!
//! No decoder is involved: tracks are made ready with a synthetic analysis and
//! the "audio" is distinct bytes, so only the identity (source SHA-256) and the
//! stored path matter. The service-level tests run the real watch pipeline
//! against a decoder stand-in that is never reached for a relinked file.
#[path = "support/library_fixture.rs"]
mod library_fixture;
use cuerithm_library::{PlaylistEdit, PlaylistKind};
use library_fixture::*;
use std::{collections::BTreeMap, path::PathBuf, time::Duration};

/// Every column of a track row except the ones a path change legitimately moves.
fn kept(fx: &Fixture, id: &str) -> BTreeMap<String, String> {
    let conn = rusqlite::Connection::open(&fx.db).unwrap();
    let mut stmt = conn.prepare("SELECT * FROM tracks WHERE id=?1").unwrap();
    let names: Vec<String> = stmt.column_names().iter().map(|n| n.to_string()).collect();
    let mut row = stmt
        .query_row([id], |row| {
            Ok(names
                .iter()
                .enumerate()
                .map(|(i, name)| (name.clone(), format!("{:?}", row.get_ref(i).unwrap())))
                .collect::<BTreeMap<_, _>>())
        })
        .unwrap();
    for moved in ["path", "updated", "path_revision"] {
        row.remove(moved);
    }
    row
}
fn path_revision(fx: &Fixture, id: &str) -> i64 {
    fx.sql(
        &format!("SELECT path_revision FROM tracks WHERE id='{id}'"),
        |r| r.get(0).unwrap(),
    )
}

/// 14 distinct tracks in `<watch>/[Album]/`, all ready, in one playlist, each
/// with a DJ-software sync link. Returns (ids, original paths).
fn album(fx: &Fixture) -> (Vec<String>, Vec<PathBuf>) {
    let folder = fx.watch.join("[Album]");
    let paths: Vec<PathBuf> = (0..14)
        .map(|i| fx.file(&folder, &format!("{:02} Track.flac", i + 1), i))
        .collect();
    let ids: Vec<String> = paths
        .iter()
        .enumerate()
        .map(|(i, p)| fx.ready(p, 120. + i as f64))
        .collect();
    fx.lib
        .apply_playlist_edit(
            PlaylistEdit::Create {
                id: Some("set".into()),
                parent_id: None,
                kind: PlaylistKind::Playlist,
                name: "Set".into(),
                definition: None,
            },
            fx.lib.playlist_state().unwrap().revision,
        )
        .unwrap();
    fx.lib
        .apply_playlist_edit(
            PlaylistEdit::AddMembers {
                id: "set".into(),
                track_ids: ids.iter().rev().cloned().collect(),
            },
            fx.lib.playlist_state().unwrap().revision,
        )
        .unwrap();
    for (i, id) in ids.iter().enumerate() {
        fx.exec(&format!(
            "INSERT INTO sync_links VALUES('{id}','rekordbox','ext-{i}','{{\"bpm\":{i}}}')"
        ));
    }
    (ids, paths)
}

struct Snapshot {
    rows: Vec<BTreeMap<String, String>>,
    analyses: Vec<String>,
    cue_revisions: Vec<u64>,
    playlist: Vec<String>,
    sync_links: i64,
    jobs: usize,
}
fn snapshot(fx: &Fixture, ids: &[String]) -> Snapshot {
    Snapshot {
        rows: ids.iter().map(|id| kept(fx, id)).collect(),
        analyses: ids
            .iter()
            .map(|id| serde_json::to_string(&fx.lib.analysis(id).unwrap().unwrap()).unwrap())
            .collect(),
        cue_revisions: ids
            .iter()
            .map(|id| fx.lib.cue_revision_count(id).unwrap())
            .collect(),
        playlist: fx.lib.manual_playlist_members("set").unwrap(),
        sync_links: fx.sql("SELECT COUNT(*) FROM sync_links", |r| r.get(0).unwrap()),
        jobs: fx.lib.jobs().unwrap().len(),
    }
}
fn assert_everything_kept(before: &Snapshot, after: &Snapshot) {
    assert_eq!(before.rows, after.rows, "track rows (except path)");
    assert_eq!(before.analyses, after.analyses, "analysis and cues");
    assert_eq!(before.cue_revisions, after.cue_revisions, "cue revisions");
    assert_eq!(before.playlist, after.playlist, "playlist membership/order");
    assert_eq!(before.sync_links, after.sync_links, "sync links");
    assert_eq!(before.jobs, after.jobs, "no new analysis jobs");
}
fn move_album_up(paths: &[PathBuf], fx: &Fixture) -> Vec<PathBuf> {
    paths
        .iter()
        .map(|old| {
            let new = fx.watch.join(old.file_name().unwrap());
            std::fs::rename(old, &new).unwrap();
            new
        })
        .collect()
}

#[test]
fn live_move_up_one_level_relinks_all_fourteen_and_creates_nothing() {
    let fx = Fixture::new();
    let (ids, paths) = album(&fx);
    let before = snapshot(&fx, &ids);
    let revisions: Vec<i64> = ids.iter().map(|id| path_revision(&fx, id)).collect();
    let service = fx.start();
    std::thread::sleep(Duration::from_millis(1500));
    let events = service.subscribe();
    let moved = move_album_up(&paths, &fx);
    wait_until("all 14 tracks to follow their files", 60, || {
        ids.iter()
            .zip(&moved)
            .all(|(id, new)| fx.lib.track_path(id).unwrap() == *new)
    });
    // One more periodic scan: nothing may show up late as a new track.
    std::thread::sleep(Duration::from_secs(6));
    service.shutdown();
    assert_eq!(fx.total_tracks(), 14, "14 files are 14 tracks, not 28");
    assert_eq!(fx.lib.counts().unwrap().incoming, 0);
    assert_everything_kept(&before, &snapshot(&fx, &ids));
    for (id, old) in ids.iter().zip(&revisions) {
        assert_eq!(path_revision(&fx, id), old + 1);
    }
    let relinked: Vec<_> = events
        .try_iter()
        .filter(|e| e.stage == "relinked")
        .collect();
    assert_eq!(relinked.len(), 14, "the UI is told once per relinked track");
    assert!(relinked.iter().all(|e| e.state == "completed"));
}

#[test]
fn startup_rescan_follows_moves_made_while_the_app_was_closed() {
    let fx = Fixture::new();
    let (ids, paths) = album(&fx);
    let before = snapshot(&fx, &ids);
    let moved = move_album_up(&paths, &fx);
    let service = fx.start();
    wait_until("all 14 tracks to follow their files", 60, || {
        ids.iter()
            .zip(&moved)
            .all(|(id, new)| fx.lib.track_path(id).unwrap() == *new)
    });
    std::thread::sleep(Duration::from_secs(1));
    service.shutdown();
    assert_eq!(fx.total_tracks(), 14);
    assert_everything_kept(&before, &snapshot(&fx, &ids));
}

#[test]
fn a_file_moved_out_of_every_watch_folder_is_not_followed() {
    let fx = Fixture::new();
    let kept_file = fx.file(&fx.watch, "stays.flac", 1);
    let leaving = fx.file(&fx.watch, "leaves.flac", 2);
    let stays = fx.ready(&kept_file, 120.);
    let left = fx.ready(&leaving, 121.);
    let elsewhere = fx.root.join("elsewhere");
    std::fs::create_dir_all(&elsewhere).unwrap();
    std::fs::rename(&leaving, elsewhere.join("leaves.flac")).unwrap();
    let service = fx.start();
    // A genuinely new file proves the pipeline has handled the folder.
    fx.file(&fx.watch, "marker.flac", 99);
    wait_until("the marker file to be admitted", 60, || {
        fx.lib.counts().unwrap().incoming == 1
    });
    service.shutdown();
    assert_eq!(fx.total_tracks(), 3);
    assert_eq!(
        fx.lib.track_path(&left).unwrap(),
        fx.root.join("beatport/leaves.flac")
    );
    assert_eq!(fx.lib.track_path(&stays).unwrap(), kept_file);
}

#[test]
fn identical_files_are_never_paired_by_guesswork() {
    let fx = Fixture::new();
    // Two library tracks whose files are byte-identical, both gone.
    let a = fx.file(&fx.watch.join("a"), "x.flac", 7);
    let b = fx.file(&fx.watch.join("b"), "x.flac", 7);
    let track_a = fx.ready(&a, 120.);
    let track_b = fx.ready(&b, 121.);
    std::fs::remove_file(&a).unwrap();
    std::fs::remove_file(&b).unwrap();
    let arrived = fx.file(&fx.watch, "x.flac", 7);
    let service = fx.start();
    wait_until("the unpaired file to become a new track", 60, || {
        fx.lib.counts().unwrap().incoming == 1
    });
    service.shutdown();
    assert_eq!(fx.lib.track_path(&track_a).unwrap(), a);
    assert_eq!(fx.lib.track_path(&track_b).unwrap(), b);
    assert_eq!(fx.total_tracks(), 3);
    assert!(arrived.is_file());
}

#[test]
fn two_new_copies_of_one_missing_track_are_not_guessed_between() {
    let fx = Fixture::new();
    let old = fx.file(&fx.watch.join("album"), "x.flac", 7);
    let track = fx.ready(&old, 120.);
    std::fs::remove_file(&old).unwrap();
    let first = fx.file(&fx.watch, "x.flac", 7);
    let second = fx.file(&fx.watch.join("copy"), "x.flac", 7);
    let out = fx.relink(&[first, second]);
    assert!(out.relinked.is_empty() && out.consumed.is_empty());
    assert_eq!(fx.lib.track_path(&track).unwrap(), old);
}

#[test]
fn a_changed_file_is_a_new_file_as_before() {
    let fx = Fixture::new();
    let old = fx.file(&fx.watch.join("album"), "x.flac", 1);
    let track = fx.ready(&old, 120.);
    std::fs::remove_file(&old).unwrap();
    let edited = fx.file(&fx.watch, "x.flac", 2);
    let out = fx.relink(std::slice::from_ref(&edited));
    assert!(out.relinked.is_empty() && out.consumed.is_empty());
    assert_eq!(fx.lib.track_path(&track).unwrap(), old);
}

#[test]
fn a_track_stored_outside_every_watch_folder_is_not_pulled_in() {
    let fx = Fixture::new();
    let library_copy = fx.file(&fx.root.join("organized"), "x.flac", 1);
    let track = fx.ready(&library_copy, 120.);
    std::fs::remove_file(&library_copy).unwrap();
    let arrived = fx.file(&fx.watch, "x.flac", 1);
    let out = fx.relink(std::slice::from_ref(&arrived));
    assert!(out.relinked.is_empty() && out.consumed.is_empty());
    assert_eq!(fx.lib.track_path(&track).unwrap(), library_copy);
}

#[test]
fn a_move_between_two_watch_folders_is_not_followed() {
    let fx = Fixture::new();
    let second = fx.root.join("traxsource");
    std::fs::create_dir_all(&second).unwrap();
    let old = fx.file(&fx.watch, "x.flac", 1);
    let track = fx.ready(&old, 120.);
    let new = second.join("x.flac");
    std::fs::rename(&old, &new).unwrap();
    let out = fx
        .lib
        .relink_moved_tracks(
            std::slice::from_ref(&new),
            &[fx.watch.clone(), second.clone()],
            |_| true,
        )
        .unwrap();
    assert!(out.relinked.is_empty());
    assert_eq!(fx.lib.track_path(&track).unwrap(), old);
}

#[test]
fn an_unreachable_watch_folder_never_counts_files_as_missing() {
    let fx = Fixture::new();
    let old = fx.file(&fx.watch.join("album"), "x.flac", 1);
    let track = fx.ready(&old, 120.);
    let new = fx.root.join("elsewhere/x.flac");
    std::fs::create_dir_all(new.parent().unwrap()).unwrap();
    std::fs::rename(&old, &new).unwrap();
    // The configured watch folder cannot be resolved (unplugged volume).
    let offline = fx.root.join("unplugged");
    let out = fx
        .lib
        .relink_moved_tracks(std::slice::from_ref(&new), &[offline], |_| true)
        .unwrap();
    assert!(out.relinked.is_empty());
    assert_eq!(fx.lib.track_path(&track).unwrap(), old);
}

#[test]
fn relinking_changes_only_the_path_and_reports_what_it_did() {
    let fx = Fixture::new();
    let (ids, paths) = album(&fx);
    let before = snapshot(&fx, &ids[..1]);
    let revision = fx.lib.revision().unwrap();
    let smart: i64 = fx.sql(
        "SELECT value FROM meta WHERE key='smart_input_revision'",
        |r| r.get(0).unwrap(),
    );
    let moved = move_album_up(&paths[..1], &fx);
    let out = fx.relink(&moved);
    assert_eq!(out.relinked.len(), 1);
    assert_eq!(out.relinked[0].track_id, ids[0]);
    assert_eq!(out.relinked[0].from, paths[0]);
    assert_eq!(out.relinked[0].to, moved[0]);
    assert_eq!(out.consumed, moved);
    assert_eq!(fx.lib.track_path(&ids[0]).unwrap(), moved[0]);
    assert!(
        fx.lib.revision().unwrap() > revision,
        "library change counter"
    );
    let after_smart: i64 = fx.sql(
        "SELECT value FROM meta WHERE key='smart_input_revision'",
        |r| r.get(0).unwrap(),
    );
    assert!(after_smart > smart, "smart playlists re-evaluate");
    let after = snapshot(&fx, &ids[..1]);
    assert_eq!(before.rows, after.rows);
    assert_eq!(before.analyses, after.analyses);
    assert_eq!(before.cue_revisions, after.cue_revisions);
    // A second pass finds the file already owned and does nothing.
    let again = fx.relink(&moved);
    assert!(again.relinked.is_empty() && again.consumed.is_empty());
}

#[test]
fn a_healthy_library_pays_nothing_for_move_detection() {
    let fx = Fixture::new();
    let (_ids, paths) = album(&fx);
    let fresh = fx.file(&fx.watch, "new.flac", 500);
    let mut asked = Vec::new();
    let out = fx
        .lib
        .relink_moved_tracks(
            std::slice::from_ref(&fresh),
            std::slice::from_ref(&fx.watch),
            |p| {
                asked.push(p.to_path_buf());
                true
            },
        )
        .unwrap();
    assert!(out.relinked.is_empty() && out.deferred.is_empty());
    assert!(
        asked.is_empty(),
        "nothing is missing, so nothing is hashed or waited on"
    );
    assert!(paths.iter().all(|p| p.is_file()));
}

#[test]
fn a_file_that_is_still_being_written_is_retried_not_hashed() {
    let fx = Fixture::new();
    let old = fx.file(&fx.watch.join("album"), "x.flac", 1);
    let track = fx.ready(&old, 120.);
    let new = fx.watch.join("x.flac");
    std::fs::rename(&old, &new).unwrap();
    let out = fx
        .lib
        .relink_moved_tracks(
            std::slice::from_ref(&new),
            std::slice::from_ref(&fx.watch),
            |_| false,
        )
        .unwrap();
    assert!(out.relinked.is_empty() && out.consumed.is_empty());
    assert_eq!(out.deferred, vec![new.clone()]);
    assert_eq!(fx.lib.track_path(&track).unwrap(), old);
    let out = fx.relink(std::slice::from_ref(&new));
    assert_eq!(out.relinked.len(), 1);
}

#[test]
fn archived_tracks_are_left_alone() {
    let fx = Fixture::new();
    let old = fx.file(&fx.watch.join("album"), "x.flac", 1);
    let track = fx.ready(&old, 120.);
    fx.lib.set_archived(&track, true).unwrap();
    let new = fx.watch.join("x.flac");
    std::fs::rename(&old, &new).unwrap();
    let out = fx.relink(&[new]);
    assert!(out.relinked.is_empty() && out.consumed.is_empty());
    assert_eq!(fx.lib.track_path(&track).unwrap(), old);
}

#[test]
fn a_path_already_owned_by_another_track_is_never_taken_over() {
    let fx = Fixture::new();
    let gone = fx.file(&fx.watch.join("album"), "gone.flac", 1);
    let missing = fx.ready(&gone, 120.);
    let other = fx.file(&fx.watch, "other.flac", 2);
    let owner = fx.ready(&other, 121.);
    std::fs::remove_file(&gone).unwrap();
    // The other track's file is replaced by bytes identical to the missing one.
    std::fs::write(&other, "audio bytes of track 1\n".repeat(40)).unwrap();
    age(&other);
    let out = fx.relink(std::slice::from_ref(&other));
    assert!(out.relinked.is_empty() && out.consumed.is_empty());
    assert_eq!(fx.lib.track_path(&missing).unwrap(), gone);
    assert_eq!(fx.lib.track_path(&owner).unwrap(), other);
}

#[cfg(unix)]
#[test]
fn a_path_reserved_by_organization_is_never_taken_over() {
    let fx = Fixture::new();
    let old = fx.file(&fx.watch.join("album"), "x.flac", 1);
    let track = fx.ready(&old, 120.);
    let busy = fx.ready(&fx.file(&fx.watch, "busy.flac", 2), 121.);
    let new = fx.watch.join("x.flac");
    std::fs::rename(&old, &new).unwrap();
    // Another track's publication is about to put its own file at `new`.
    fx.exec(&format!(
        "INSERT INTO organization_operations VALUES('op','{busy}','publishing','{{}}',NULL,0,0);\
         INSERT INTO organization_reservations VALUES(CAST('unix:{path}' AS BLOB),NULL,NULL,'op');",
        path = new.display(),
    ));
    let out = fx.relink(std::slice::from_ref(&new));
    assert!(out.relinked.is_empty() && out.consumed.is_empty());
    assert_eq!(fx.lib.track_path(&track).unwrap(), old);
}

#[test]
fn a_track_with_file_work_in_flight_is_left_to_that_work() {
    for blocker in [
        "INSERT INTO organization_operations VALUES('op','TRACK','publishing','{}',NULL,0,0)",
        "UPDATE jobs SET state='queued' WHERE track_id='TRACK'",
    ] {
        let fx = Fixture::new();
        let old = fx.file(&fx.watch.join("album"), "x.flac", 1);
        let track = fx.ready(&old, 120.);
        fx.exec(&blocker.replace("TRACK", &track));
        let new = fx.watch.join("x.flac");
        std::fs::rename(&old, &new).unwrap();
        let out = fx.relink(std::slice::from_ref(&new));
        assert!(out.relinked.is_empty(), "{blocker}");
        assert_eq!(fx.lib.track_path(&track).unwrap(), old, "{blocker}");
    }
}

#[cfg(unix)]
#[test]
fn owned_artwork_follows_a_renamed_album_folder() {
    let fx = Fixture::new();
    let old = fx.file(&fx.watch.join("Album"), "x.flac", 1);
    let cover = old.with_file_name("cover.jpg");
    std::fs::write(&cover, b"cover art").unwrap();
    let track = fx.ready(&old, 120.);
    let hash = cuerithm_media::file_sha256(&cover).unwrap();
    fx.exec(&format!(
        "INSERT INTO organization_owned_sidecars VALUES('{track}',CAST('unix:{}' AS BLOB),'{hash}','m',1)",
        cover.display()
    ));
    let renamed = fx.watch.join("Album (2024)");
    std::fs::rename(old.parent().unwrap(), &renamed).unwrap();
    let out = fx.relink(&[renamed.join("x.flac")]);
    assert_eq!(out.relinked.len(), 1);
    let stored: Vec<u8> = fx.sql("SELECT path FROM organization_owned_sidecars", |r| {
        r.get(0).unwrap()
    });
    assert_eq!(
        stored,
        format!("unix:{}", renamed.join("cover.jpg").display()).into_bytes()
    );
}

#[cfg(unix)]
#[test]
fn artwork_that_stayed_behind_keeps_its_recorded_location() {
    let fx = Fixture::new();
    let old = fx.file(&fx.watch.join("Album"), "x.flac", 1);
    let cover = old.with_file_name("cover.jpg");
    std::fs::write(&cover, b"cover art").unwrap();
    let track = fx.ready(&old, 120.);
    let hash = cuerithm_media::file_sha256(&cover).unwrap();
    fx.exec(&format!(
        "INSERT INTO organization_owned_sidecars VALUES('{track}',CAST('unix:{}' AS BLOB),'{hash}','m',1)",
        cover.display()
    ));
    let new = fx.watch.join("x.flac");
    std::fs::rename(&old, &new).unwrap();
    assert_eq!(fx.relink(&[new]).relinked.len(), 1);
    let stored: Vec<u8> = fx.sql("SELECT path FROM organization_owned_sidecars", |r| {
        r.get(0).unwrap()
    });
    assert_eq!(stored, format!("unix:{}", cover.display()).into_bytes());
}

#[test]
fn a_file_is_reported_missing_only_when_it_is_definitely_gone() {
    let fx = Fixture::new();
    let path = fx.file(&fx.watch, "x.flac", 1);
    let track = fx.ready(&path, 120.);
    assert!(!fx.lib.track_file_missing(&track).unwrap());
    std::fs::remove_file(&path).unwrap();
    assert!(fx.lib.track_file_missing(&track).unwrap());
    // Only a definite "not found" counts; anything occupying the path does not.
    std::fs::create_dir_all(&path).unwrap();
    assert!(
        !fx.lib.track_file_missing(&track).unwrap(),
        "something is there"
    );
}
