//! Maintenance -> Missing & broken: removing the records of unavailable tracks,
//! and finding moved files, each as preview, apply, history and undo.
#[path = "support/library_fixture.rs"]
mod library_fixture;
use cuerithm_library::{
    MISSING_REPAIR_LIMIT, MISSING_UNDO_WINDOW, MissingPreview, MissingRepairKind, MissingRowOutcome,
};
use cuerithm_media::CancellationToken;
use library_fixture::*;
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

fn cancel() -> CancellationToken {
    CancellationToken::default()
}
fn outcome(preview: &MissingPreview, id: &str) -> MissingRowOutcome {
    preview
        .rows
        .iter()
        .find(|r| r.track_id == id)
        .unwrap()
        .outcome
}
fn reason(preview: &MissingPreview, id: &str) -> String {
    preview
        .rows
        .iter()
        .find(|r| r.track_id == id)
        .unwrap()
        .reason
        .clone()
        .unwrap_or_default()
}
fn rows_of(fx: &Fixture, table: &str) -> usize {
    fx.sql(&format!("SELECT COUNT(*) FROM \"{table}\""), |r| {
        r.get::<_, i64>(0).unwrap()
    }) as usize
}

/// Three ready tracks. `a` and `b` lose their files; `c` keeps its file.
/// `a` is in two playlists and linked to DJ software; the files sit in the library
/// folder next to a cover image that must never be touched.
struct Library3 {
    fx: Fixture,
    a: String,
    b: String,
    c: String,
    files: [PathBuf; 3],
    cover: PathBuf,
}
fn library() -> Library3 {
    let fx = Fixture::new();
    let folder = fx.watch.join("Album");
    let files = [
        fx.file(&folder, "a.flac", 1),
        fx.file(&folder, "b.flac", 2),
        fx.file(&folder, "c.flac", 3),
    ];
    let cover = folder.join("cover.jpg");
    std::fs::write(&cover, b"cover art").unwrap();
    let a = fx.ready(&files[0], 120.);
    let b = fx.ready(&files[1], 121.);
    let c = fx.ready(&files[2], 122.);
    // Playlist "one": [c, a, b, c-less ...]; positions matter for Undo.
    fx.playlist("one", &[&c, &a, &b]);
    fx.playlist("two", &[&a]);
    fx.exec(&format!(
        "INSERT INTO sync_links VALUES('{a}','rekordbox','ext-a','{{}}');
         INSERT INTO native_targets VALUES('target','rekordbox','db','dev','{{}}','fp','oh','[]',1,0,0);
         INSERT INTO native_track_links(target_id,content_id,local_track_id,identity_json,observation_json,observation_hash,baseline_json,status,updated)
           VALUES('target','content-a','{a}','{{}}',NULL,NULL,'{{}}','linked',0);
         INSERT INTO native_sync_track_ownership VALUES('target','{a}','{{}}','[]','{{}}',NULL);"
    ));
    std::fs::remove_file(&files[0]).unwrap();
    std::fs::remove_file(&files[1]).unwrap();
    Library3 {
        fx,
        a,
        b,
        c,
        files,
        cover,
    }
}
fn ids(which: &[&String]) -> Vec<String> {
    which.iter().map(|s| s.to_string()).collect()
}

#[test]
fn removal_and_undo_preserve_portable_track_owned_records() {
    let l = library();
    let conn = rusqlite::Connection::open(&l.fx.db).unwrap();
    conn.execute_batch("CREATE TABLE IF NOT EXISTS portable_domain_records(kind TEXT NOT NULL,id TEXT NOT NULL,json TEXT NOT NULL,PRIMARY KEY(kind,id));").unwrap();
    for (kind, id, track) in [
        ("tracks", l.a.as_str(), l.a.as_str()),
        ("external_links", "old-link", l.a.as_str()),
        ("external_links", "kept-link", l.c.as_str()),
    ] {
        let json = serde_json::json!({"id":id,"value":{"track_id":track}}).to_string();
        conn.execute(
            "INSERT INTO portable_domain_records VALUES(?1,?2,?3)",
            rusqlite::params![kind, id, json],
        )
        .unwrap();
    }
    drop(conn);
    let report = l.fx.scan(false);
    let preview =
        l.fx.lib
            .preview_missing_removal(&report.id, &ids(&[&l.a]), &cancel())
            .unwrap();
    let entry =
        l.fx.lib
            .apply_missing_removal(&preview.id, &cancel())
            .unwrap();
    assert_eq!(rows_of(&l.fx, "portable_domain_records"), 1);
    l.fx.lib.undo_missing_repair(&entry.id, &cancel()).unwrap();
    assert_eq!(rows_of(&l.fx, "portable_domain_records"), 3);
    assert!(l.fx.lib.track(&l.a).is_ok());
}

#[test]
fn removal_preview_explains_the_impact_and_changes_nothing() {
    let l = library();
    let before = l.fx.dump();
    let report = l.fx.scan(false);
    let preview =
        l.fx.lib
            .preview_missing_removal(&report.id, &ids(&[&l.a, &l.b, &l.c]), &cancel())
            .unwrap();
    assert_eq!(preview.kind, MissingRepairKind::Remove);
    assert_eq!(outcome(&preview, &l.a), MissingRowOutcome::Ready);
    assert_eq!(outcome(&preview, &l.b), MissingRowOutcome::Ready);
    assert_eq!(outcome(&preview, &l.c), MissingRowOutcome::Blocked);
    assert_eq!(reason(&preview, &l.c), "The file is available");
    assert_eq!(preview.ready, 2);
    assert_eq!(
        preview.impact.playlist_memberships, 3,
        "a in two playlists, b in one"
    );
    assert_eq!(preview.impact.playlists.len(), 2);
    assert_eq!(
        preview.impact.sync_links, 2,
        "a DJ-sync baseline and a rekordbox cue record"
    );
    assert_eq!(preview.impact.rekordbox_links, 1);
    assert_same_dump(&l.fx.dump(), &before);
}

#[test]
fn removal_takes_only_the_records_and_undo_restores_every_one_of_them() {
    let l = library();
    let before = l.fx.dump();
    let revision = l.fx.lib.revision().unwrap();
    let report = l.fx.scan(false);
    let preview =
        l.fx.lib
            .preview_missing_removal(&report.id, &ids(&[&l.a, &l.b]), &cancel())
            .unwrap();
    let entry =
        l.fx.lib
            .apply_missing_removal(&preview.id, &cancel())
            .unwrap();
    assert_eq!(entry.kind, MissingRepairKind::Remove);
    assert_eq!(entry.tracks.len(), 2);
    assert_eq!(entry.playlist_memberships, 3);
    assert!(l.fx.lib.revision().unwrap() > revision);
    // Records are gone, with everything that hung off them.
    assert!(l.fx.lib.track(&l.a).is_err());
    assert_eq!(l.fx.total_tracks(), 1);
    assert_eq!(
        l.fx.lib.manual_playlist_members("one").unwrap(),
        vec![l.c.clone()]
    );
    assert!(l.fx.lib.manual_playlist_members("two").unwrap().is_empty());
    for table in [
        "jobs",
        "cue_revisions",
        "sync_links",
        "native_sync_track_ownership",
    ] {
        assert!(rows_of(&l.fx, table) <= 1, "{table}");
    }
    // The rekordbox link stays as a DJ-software track nobody here owns.
    let (local, status): (Option<String>, String) = l.fx.sql(
        "SELECT local_track_id,status FROM native_track_links",
        |r| (r.get(0).unwrap(), r.get(1).unwrap()),
    );
    assert_eq!((local, status.as_str()), (None, "not_in_local_library"));
    // No file was touched.
    assert!(l.files[2].is_file() && l.cover.is_file());
    assert_eq!(std::fs::read(&l.cover).unwrap(), b"cover art");
    // One history entry; the track that kept its file is untouched by the scan rules.
    let history = l.fx.lib.missing_repair_history().unwrap();
    assert_eq!(history.len(), 1);
    assert_eq!(history[0].entry.id, entry.id);
    assert!(history[0].undone_by.is_none());

    let undo = l.fx.lib.undo_missing_repair(&entry.id, &cancel()).unwrap();
    assert_eq!(undo.undo_of.as_deref(), Some(entry.id.as_str()));
    assert_eq!(
        l.fx.dump(),
        before,
        "records, positions and metadata are back exactly"
    );
    assert_eq!(
        l.fx.lib.manual_playlist_members("one").unwrap(),
        vec![l.c.clone(), l.a.clone(), l.b.clone()]
    );
    let history = l.fx.lib.missing_repair_history().unwrap();
    assert_eq!(history.len(), 2);
    assert_eq!(history[1].undone_by.as_deref(), Some(undo.id.as_str()));
    assert!(
        l.fx.lib.undo_missing_repair(&entry.id, &cancel()).is_err(),
        "only once"
    );
}

#[test]
fn saved_records_are_kept_until_someone_explicitly_discards_them() {
    let (m, elsewhere) = moved_library();
    let report = m.fx.scan(false);
    let removal =
        m.fx.lib
            .preview_missing_removal(&report.id, &ids(&[&m.a]), &cancel())
            .unwrap();
    let removed =
        m.fx.lib
            .apply_missing_removal(&removal.id, &cancel())
            .unwrap();
    // Nothing is discarded by time or by later repairs.
    let report = m.fx.scan(false);
    let relink =
        m.fx.lib
            .preview_missing_relink(
                &report.id,
                &ids(&[&m.b]),
                std::slice::from_ref(&elsewhere),
                &cancel(),
                |_| {},
            )
            .unwrap();
    let relinked =
        m.fx.lib
            .apply_missing_relink(&relink.id, &cancel())
            .unwrap();
    assert!(
        m.fx.lib
            .missing_repair_history()
            .unwrap()
            .iter()
            .all(|h| !h.undo_discarded)
    );
    assert!(
        m.fx.lib.discard_missing_repair_undo(&relinked.id).is_err(),
        "a relink holds no saved records"
    );
    let before = m.fx.dump();
    let discarded = m.fx.lib.discard_missing_repair_undo(&removed.id).unwrap();
    assert!(discarded.undo_discarded);
    assert_same_dump(&m.fx.dump(), &before);
    let entry =
        m.fx.lib
            .missing_repair_history()
            .unwrap()
            .into_iter()
            .find(|h| h.entry.id == removed.id)
            .unwrap();
    assert!(entry.undo_discarded && entry.undone_by.is_none());
    assert!(entry.entry.tracks.is_empty() && entry.entry.track_count == 1);
    assert!(
        m.fx.lib
            .undo_missing_repair(&removed.id, &cancel())
            .unwrap_err()
            .to_string()
            .contains("discarded")
    );
    assert!(
        m.fx.lib.discard_missing_repair_undo(&removed.id).is_err(),
        "only once"
    );
    // The other repair is unaffected and still undoes.
    m.fx.lib
        .undo_missing_repair(&relinked.id, &cancel())
        .unwrap();
}

fn waveform_cache(fx: &Fixture, hash: &str, decoder: &str, bins: usize) -> PathBuf {
    let key = format!("{:x}", Sha256::digest(format!("{hash}:{decoder}:{bins}")));
    fx.lib
        .data_dir()
        .join("cache/waveforms")
        .join(format!("{key}.json"))
}
fn artwork_cache(fx: &Fixture, hash: &str) -> PathBuf {
    fx.lib.data_dir().join("cache/artwork").join(hash)
}
/// Gives a track every kind of cache the app derives from it.
fn derive_caches(fx: &Fixture, track: &str) -> String {
    let hash = fx.lib.source_hash(track).unwrap();
    for path in [
        artwork_cache(fx, &hash),
        waveform_cache(fx, &hash, "decoder-1", 240),
        waveform_cache(fx, &hash, "decoder-1", 16384),
    ] {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, "cached").unwrap();
    }
    fx.exec(&format!(
        "INSERT INTO preparation_analysis VALUES('{hash}',(SELECT revision FROM selected_settings),'engine','decoder-1','{{}}');
         INSERT INTO maintenance_fingerprints VALUES('{track}','engine','{hash}','{{}}',0);
         INSERT INTO preparation_match_generations(track_id,job_id,generation,attempt,source_hash,settings_revision,payload_json,created)
           VALUES('{track}',NULL,'gen-{track}',1,'{hash}',(SELECT revision FROM selected_settings),'{{}}',0);
         INSERT INTO preparation_review_artwork(generation,candidate_id,url_digest,asset_json) VALUES('gen-{track}','c','d','{{}}');"
    ));
    hash
}
fn derived_rows(fx: &Fixture, hash: &str, track: &str) -> [i64; 4] {
    [
        format!("SELECT COUNT(*) FROM preparation_analysis WHERE source_hash='{hash}'"),
        format!("SELECT COUNT(*) FROM maintenance_fingerprints WHERE track_id='{track}'"),
        format!("SELECT COUNT(*) FROM preparation_review_artwork WHERE generation='gen-{track}'"),
        format!("SELECT COUNT(*) FROM maintenance_track_epochs WHERE id='{track}'"),
    ]
    .map(|q| fx.sql(&q, |r| r.get(0).unwrap()))
}

#[test]
fn removal_purges_what_is_derived_from_the_tracks_but_keeps_a_twins_caches() {
    let l = library();
    // `twin` shares its bytes with `a`; `b` is alone.
    let twin_file = l.fx.file(&l.fx.watch.join("Copy"), "a.flac", 1);
    let twin = l.fx.ready(&twin_file, 130.);
    let hash_a = derive_caches(&l.fx, &l.a);
    let hash_b = derive_caches(&l.fx, &l.b);
    assert_eq!(hash_a, l.fx.lib.source_hash(&twin).unwrap());
    let report = l.fx.scan(false);
    let preview =
        l.fx.lib
            .preview_missing_removal(&report.id, &ids(&[&l.a, &l.b]), &cancel())
            .unwrap();
    let entry =
        l.fx.lib
            .apply_missing_removal(&preview.id, &cancel())
            .unwrap();
    // `b` was the only user of its bytes: what is derived from the audio is gone. Its
    // cover may be the only copy left now that the audio is missing, so it is kept
    // for Undo (see the cover tests below).
    assert!(artwork_cache(&l.fx, &hash_b).exists());
    assert!(!waveform_cache(&l.fx, &hash_b, "decoder-1", 240).exists());
    assert!(!waveform_cache(&l.fx, &hash_b, "decoder-1", 16384).exists());
    assert_eq!(derived_rows(&l.fx, &hash_b, &l.b), [0, 0, 0, 0]);
    // `a`'s bytes still belong to the twin, so only per-track rows go.
    assert!(artwork_cache(&l.fx, &hash_a).exists());
    assert!(waveform_cache(&l.fx, &hash_a, "decoder-1", 240).exists());
    assert_eq!(derived_rows(&l.fx, &hash_a, &l.a), [1, 0, 0, 0]);
    // The twin and its own records are untouched.
    assert!(l.fx.lib.track(&twin).is_ok());
    // Undo restores the records, not the caches: those are rebuilt from the file.
    l.fx.lib.undo_missing_repair(&entry.id, &cancel()).unwrap();
    assert!(l.fx.lib.track(&l.b).is_ok());
    assert_eq!(
        derived_rows(&l.fx, &hash_b, &l.b)[1],
        0,
        "fingerprints are not restored"
    );
}

#[test]
fn a_removed_tracks_cover_survives_undo_and_is_freed_with_its_saved_records() {
    let l = library();
    let hash_a = derive_caches(&l.fx, &l.a);
    let hash_b = derive_caches(&l.fx, &l.b);
    // A present twin shares `a`'s bytes, so its cover is not the removed track's alone.
    let twin =
        l.fx.ready(&l.fx.file(&l.fx.watch.join("Copy"), "a.flac", 1), 130.);
    assert_eq!(l.fx.lib.source_hash(&twin).unwrap(), hash_a);
    let remove = |ids_: &[&String]| {
        let report = l.fx.scan(false);
        let preview =
            l.fx.lib
                .preview_missing_removal(&report.id, &ids(ids_), &cancel())
                .unwrap();
        l.fx.lib
            .apply_missing_removal(&preview.id, &cancel())
            .unwrap()
    };
    // Removal keeps the cover; Undo brings the track back with its picture.
    let first = remove(&[&l.b]);
    assert!(artwork_cache(&l.fx, &hash_b).is_file());
    l.fx.lib.undo_missing_repair(&first.id, &cancel()).unwrap();
    assert!(
        artwork_cache(&l.fx, &hash_b).is_file(),
        "the restored track still has its cover"
    );
    // Removed again, the cover goes when someone discards the saved records...
    let second = remove(&[&l.b]);
    assert!(artwork_cache(&l.fx, &hash_b).is_file());
    l.fx.lib.discard_missing_repair_undo(&second.id).unwrap();
    assert!(
        !artwork_cache(&l.fx, &hash_b).exists(),
        "nothing can need it any more"
    );
    // ...but never while another track still has the same bytes.
    let third = remove(&[&l.a]);
    l.fx.lib.discard_missing_repair_undo(&third.id).unwrap();
    assert!(
        artwork_cache(&l.fx, &hash_a).is_file(),
        "the twin keeps its cover"
    );
}

#[test]
fn a_cover_held_by_two_removals_is_freed_only_with_the_last_of_them() {
    let l = library();
    let hash = derive_caches(&l.fx, &l.a);
    let remove = |id: &String| {
        let report = l.fx.scan(false);
        let preview =
            l.fx.lib
                .preview_missing_removal(&report.id, &ids(&[id]), &cancel())
                .unwrap();
        l.fx.lib
            .apply_missing_removal(&preview.id, &cancel())
            .unwrap()
    };
    let first = remove(&l.a);
    // The same audio comes back as another track and is removed too.
    let file = l.fx.file(&l.fx.watch.join("Again"), "a.flac", 1);
    let again = l.fx.ready(&file, 125.);
    std::fs::remove_file(&file).unwrap();
    let second = remove(&again);
    l.fx.lib.discard_missing_repair_undo(&first.id).unwrap();
    assert!(
        artwork_cache(&l.fx, &hash).is_file(),
        "the second removal can still be undone"
    );
    l.fx.lib.discard_missing_repair_undo(&second.id).unwrap();
    assert!(!artwork_cache(&l.fx, &hash).exists());
}

#[test]
fn covers_follow_the_bounded_undo_window() {
    let l = library();
    let mut covers = Vec::new();
    for n in 0..=MISSING_UNDO_WINDOW {
        let file =
            l.fx.file(&l.fx.watch.join("Old"), &format!("cover-{n}.flac"), 70 + n);
        let id = l.fx.ready(&file, 100. + n as f64);
        covers.push(derive_caches(&l.fx, &id));
        std::fs::remove_file(&file).unwrap();
        let report = l.fx.scan(false);
        let preview =
            l.fx.lib
                .preview_missing_removal(&report.id, &ids(&[&id]), &cancel())
                .unwrap();
        l.fx.lib
            .apply_missing_removal(&preview.id, &cancel())
            .unwrap();
    }
    assert!(
        !artwork_cache(&l.fx, &covers[0]).exists(),
        "the oldest removal left the window, so its cover is freed"
    );
    for hash in &covers[1..] {
        assert!(artwork_cache(&l.fx, hash).is_file());
    }
}

#[test]
fn only_the_newest_removals_keep_their_saved_records() {
    let l = library();
    let mut entries = Vec::new();
    let mut titles = Vec::new();
    for n in 0..=MISSING_UNDO_WINDOW {
        let file =
            l.fx.file(&l.fx.watch.join("Old"), &format!("window-{n}.flac"), 50 + n);
        let id = l.fx.ready(&file, 100. + n as f64);
        std::fs::remove_file(&file).unwrap();
        let report = l.fx.scan(false);
        let preview =
            l.fx.lib
                .preview_missing_removal(&report.id, &ids(&[&id]), &cancel())
                .unwrap();
        entries.push(
            l.fx.lib
                .apply_missing_removal(&preview.id, &cancel())
                .unwrap(),
        );
        titles.push(format!("window-{n}"));
    }
    let history = l.fx.lib.missing_repair_history().unwrap();
    let oldest = history
        .iter()
        .find(|h| h.entry.id == entries[0].id)
        .unwrap();
    assert!(
        oldest.undo_discarded,
        "the 11th removal pushed the first out of the window"
    );
    assert_eq!(oldest.entry.track_count, 1);
    assert!(
        oldest.entry.tracks.is_empty(),
        "no track names are kept without undo data"
    );
    assert!(
        l.fx.lib
            .undo_missing_repair(&entries[0].id, &cancel())
            .unwrap_err()
            .to_string()
            .contains("discarded")
    );
    for entry in &entries[1..] {
        let kept = history.iter().find(|h| h.entry.id == entry.id).unwrap();
        assert!(!kept.undo_discarded && kept.entry.tracks.len() == 1);
    }
    // The database no longer mentions the first removed track at all.
    let everything: String = l.fx.sql(
        "SELECT group_concat(payload_json,' ') FROM maintenance_scans",
        |r| r.get(0).unwrap(),
    );
    assert!(
        !everything.contains(&titles[0]),
        "history keeps no trace of a purged removal"
    );
    assert!(everything.contains(&titles[1]));
    // A preview is spent once applied and leaves nothing behind.
    assert_eq!(
        l.fx.sql(
            "SELECT COUNT(*) FROM maintenance_scans WHERE kind LIKE 'missing_%_preview'",
            |r| r.get::<_, i64>(0).unwrap()
        ),
        0
    );
    // The newest can still be undone, which frees its saved copy as well.
    let newest = &entries[MISSING_UNDO_WINDOW];
    l.fx.lib.undo_missing_repair(&newest.id, &cancel()).unwrap();
    let payload: String = l.fx.sql(
        &format!(
            "SELECT payload_json FROM maintenance_scans WHERE id='{}'",
            newest.id
        ),
        |r| r.get(0).unwrap(),
    );
    assert!(
        !payload.contains("\"tables\":[{"),
        "an undone removal keeps no saved records"
    );
}

fn track_row(fx: &Fixture, id: &str) -> String {
    fx.sql(
        &format!(
            "SELECT json_object('id',id,'path',hex(path),'state',state,'hash',source_hash,'analysis',analysis_json,'meta',metadata_json,'bpm',bpm,'created',created,'updated',updated) FROM tracks WHERE id='{id}'"
        ),
        |r| r.get(0).unwrap(),
    )
}
fn latest_cue(fx: &Fixture, id: &str) -> String {
    fx.sql(
        &format!("SELECT id FROM cue_revisions WHERE track_id='{id}' ORDER BY created DESC,rowid DESC LIMIT 1"),
        |r| r.get(0).unwrap(),
    )
}

#[test]
fn undo_still_works_after_sqlite_hands_the_freed_rowids_to_new_records() {
    let l = library();
    // `z` is the newest track, so it holds the highest rowid in every table it touches.
    let z_file = l.fx.file(&l.fx.watch.join("Album"), "z.flac", 90);
    let z = l.fx.ready(&z_file, 140.);
    l.fx.exec(&format!(
        "INSERT INTO cue_revisions VALUES('tie-first','{z}',NULL,'{{}}',9000000000000);
         INSERT INTO cue_revisions VALUES('tie-second','{z}',NULL,'{{}}',9000000000000);"
    ));
    assert_eq!(latest_cue(&l.fx, &z), "tie-second");
    let (track_rowid, job_rowid): (i64, i64) = l.fx.sql(
        &format!("SELECT (SELECT rowid FROM tracks WHERE id='{z}'),(SELECT rowid FROM jobs WHERE track_id='{z}')"),
        |r| (r.get(0).unwrap(), r.get(1).unwrap()),
    );
    let before = track_row(&l.fx, &z);
    std::fs::remove_file(&z_file).unwrap();
    let report = l.fx.scan(false);
    let preview =
        l.fx.lib
            .preview_missing_removal(&report.id, &ids(&[&z]), &cancel())
            .unwrap();
    let entry =
        l.fx.lib
            .apply_missing_removal(&preview.id, &cancel())
            .unwrap();
    // An unrelated import is given the very rowids the removal freed.
    let newcomer = l.fx.ready(&l.fx.file(&l.fx.watch, "new.flac", 91), 141.);
    let (new_track_rowid, new_job_rowid): (i64, i64) = l.fx.sql(
        &format!("SELECT (SELECT rowid FROM tracks WHERE id='{newcomer}'),(SELECT rowid FROM jobs WHERE track_id='{newcomer}')"),
        |r| (r.get(0).unwrap(), r.get(1).unwrap()),
    );
    assert_eq!(
        (new_track_rowid, new_job_rowid),
        (track_rowid, job_rowid),
        "the premise: SQLite reuses the freed rowids"
    );
    l.fx.lib.undo_missing_repair(&entry.id, &cancel()).unwrap();
    assert_eq!(track_row(&l.fx, &z), before, "the record is back as it was");
    assert_eq!(
        latest_cue(&l.fx, &z),
        "tie-second",
        "cue order does not depend on rowids"
    );
    assert!(
        l.fx.lib.track(&newcomer).is_ok(),
        "the unrelated track is untouched"
    );
    assert_eq!(l.fx.lib.cue_revision_count(&z).unwrap(), 3);
}

#[test]
fn every_table_has_a_declared_primary_key_so_saved_rows_never_depend_on_rowids() {
    let fx = Fixture::new();
    let conn = rusqlite::Connection::open(&fx.db).unwrap();
    let without: Vec<String> = conn
        .prepare("SELECT m.name FROM sqlite_master m WHERE m.type='table' AND m.name NOT LIKE 'sqlite_%' AND NOT EXISTS(SELECT 1 FROM pragma_table_info(m.name) WHERE pk>0)")
        .unwrap()
        .query_map([], |r| r.get(0))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    assert!(
        without.is_empty(),
        "a table without a declared key cannot be restored by identity: {without:?}"
    );
}

#[test]
fn undo_leaves_a_rescanned_or_reassigned_rekordbox_link_as_it_is_now() {
    let l = library();
    l.fx.exec(&format!(
        "INSERT INTO native_track_links(target_id,content_id,local_track_id,identity_json,observation_json,observation_hash,baseline_json,status,updated)
           VALUES('target','content-b','{b}','{{}}',NULL,NULL,'{{}}','linked',0)",
        b = l.b
    ));
    let report = l.fx.scan(false);
    let preview =
        l.fx.lib
            .preview_missing_removal(&report.id, &ids(&[&l.a, &l.b]), &cancel())
            .unwrap();
    let entry =
        l.fx.lib
            .apply_missing_removal(&preview.id, &cancel())
            .unwrap();
    // After the removal rekordbox is scanned again: `a`'s link gets a new observation
    // and baseline, and `b`'s link is matched to a different library track.
    l.fx.exec(&format!(
        "UPDATE native_track_links SET observation_json='{{\"seen\":2}}',observation_hash='h2',baseline_json='{{\"bpm\":99}}',status='linked',updated=9 WHERE content_id='content-a';
         UPDATE native_track_links SET local_track_id='{c}',status='linked' WHERE content_id='content-b';",
        c = l.c
    ));
    let links = |fx: &Fixture| -> Vec<String> {
        let conn = rusqlite::Connection::open(&fx.db).unwrap();
        let mut q = conn
            .prepare("SELECT json_array(content_id,local_track_id,status,observation_json,observation_hash,baseline_json,updated) FROM native_track_links ORDER BY content_id")
            .unwrap();
        q.query_map([], |r| r.get(0))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap()
    };
    let current = links(&l.fx);
    let undo = l.fx.lib.undo_missing_repair(&entry.id, &cancel()).unwrap();
    assert_eq!(
        links(&l.fx),
        current,
        "newer observations and the other track's link survive"
    );
    assert!(
        l.fx.lib.track(&l.a).is_ok() && l.fx.lib.track(&l.b).is_ok(),
        "the records themselves are back"
    );
    let note = undo
        .skipped
        .iter()
        .find(|s| s.title == "rekordbox links")
        .unwrap();
    assert!(
        note.reason.starts_with("2 changed after the removal"),
        "{}",
        note.reason
    );
}

#[test]
fn undo_leaves_a_sync_link_that_now_belongs_to_another_track_and_restores_the_rest() {
    let l = library();
    let report = l.fx.scan(false);
    let preview =
        l.fx.lib
            .preview_missing_removal(&report.id, &ids(&[&l.a]), &cancel())
            .unwrap();
    let entry =
        l.fx.lib
            .apply_missing_removal(&preview.id, &cancel())
            .unwrap();
    // After the removal the sync linked the same external record to another track.
    l.fx.exec(&format!(
        "INSERT INTO sync_links VALUES('{c}','rekordbox','ext-a','{{}}')",
        c = l.c
    ));
    let undo = l.fx.lib.undo_missing_repair(&entry.id, &cancel()).unwrap();
    assert!(
        l.fx.lib.track(&l.a).is_ok(),
        "the record is back although one link could not be"
    );
    let owner: String = l.fx.sql(
        "SELECT track_id FROM sync_links WHERE external_id='ext-a'",
        |r| r.get(0).unwrap(),
    );
    assert_eq!(owner, l.c, "the newer link is never taken over");
    let note = undo
        .skipped
        .iter()
        .find(|s| s.title == "sync links")
        .unwrap();
    assert!(
        note.reason.starts_with("1 linked to something else"),
        "{}",
        note.reason
    );
}

#[test]
fn undo_refuses_an_unrelated_file_at_the_old_location_of_a_removed_track() {
    let l = library();
    let report = l.fx.scan(false);
    let preview =
        l.fx.lib
            .preview_missing_removal(&report.id, &ids(&[&l.a]), &cancel())
            .unwrap();
    let entry =
        l.fx.lib
            .apply_missing_removal(&preview.id, &cancel())
            .unwrap();
    std::fs::write(&l.files[0], "somebody else's recording").unwrap();
    let after_removal = l.fx.dump();
    let error =
        l.fx.lib
            .undo_missing_repair(&entry.id, &cancel())
            .unwrap_err()
            .to_string();
    assert!(error.contains("a different file is now at"), "{error}");
    assert_same_dump(&l.fx.dump(), &after_removal);
    // The same bytes back at the old place are fine: the record is accurate again.
    std::fs::write(&l.files[0], "audio bytes of track 1\n".repeat(40)).unwrap();
    l.fx.lib.undo_missing_repair(&entry.id, &cancel()).unwrap();
    assert!(l.fx.lib.track(&l.a).is_ok());
}

#[test]
fn playlist_positions_come_back_even_after_the_playlist_was_edited() {
    let l = library();
    let report = l.fx.scan(false);
    let preview =
        l.fx.lib
            .preview_missing_removal(&report.id, &ids(&[&l.a]), &cancel())
            .unwrap();
    let entry =
        l.fx.lib
            .apply_missing_removal(&preview.id, &cancel())
            .unwrap();
    assert_eq!(
        l.fx.lib.manual_playlist_members("one").unwrap(),
        vec![l.c.clone(), l.b.clone()]
    );
    // The playlist changes while the removal is in history.
    let extra = l.fx.ready(&l.fx.file(&l.fx.watch, "d.flac", 4), 123.);
    l.fx.lib
        .apply_playlist_edit(
            cuerithm_library::PlaylistEdit::AddMembers {
                id: "one".into(),
                track_ids: vec![extra.clone()],
            },
            l.fx.lib.playlist_state().unwrap().revision,
        )
        .unwrap();
    l.fx.lib.undo_missing_repair(&entry.id, &cancel()).unwrap();
    assert_eq!(
        l.fx.lib.manual_playlist_members("one").unwrap(),
        vec![l.c.clone(), l.a.clone(), l.b.clone(), extra],
        "a returns to its place; the new track stays"
    );
}

#[test]
fn only_unavailable_files_can_be_removed_and_a_reappearing_file_is_kept() {
    let l = library();
    let report = l.fx.scan(false);
    let only_present =
        l.fx.lib
            .preview_missing_removal(&report.id, &ids(&[&l.c]), &cancel())
            .unwrap();
    assert_eq!(only_present.ready, 0);
    assert!(
        l.fx.lib
            .apply_missing_removal(&only_present.id, &cancel())
            .is_err()
    );
    assert_eq!(l.fx.total_tracks(), 3);

    let preview =
        l.fx.lib
            .preview_missing_removal(&report.id, &ids(&[&l.a, &l.b]), &cancel())
            .unwrap();
    // `a` comes back between the preview and the apply.
    std::fs::write(&l.files[0], "audio bytes of track 1\n".repeat(40)).unwrap();
    let entry =
        l.fx.lib
            .apply_missing_removal(&preview.id, &cancel())
            .unwrap();
    assert_eq!(entry.tracks.len(), 1);
    assert_eq!(entry.tracks[0].id, l.b);
    assert_eq!(entry.skipped.len(), 1);
    assert_eq!(entry.skipped[0].id, l.a);
    assert!(
        entry.skipped[0]
            .reason
            .contains("back at its recorded location")
    );
    assert!(l.fx.lib.track(&l.a).is_ok());
    assert!(
        l.fx.lib
            .apply_missing_removal(&preview.id, &cancel())
            .is_err(),
        "one use"
    );
}

#[test]
fn a_track_edited_after_the_preview_is_skipped() {
    let l = library();
    let report = l.fx.scan(false);
    let preview =
        l.fx.lib
            .preview_missing_removal(&report.id, &ids(&[&l.a, &l.b]), &cancel())
            .unwrap();
    l.fx.exec(&format!(
        "UPDATE tracks SET genre='Techno' WHERE id='{}'",
        l.a
    ));
    let entry =
        l.fx.lib
            .apply_missing_removal(&preview.id, &cancel())
            .unwrap();
    assert_eq!(entry.tracks.len(), 1);
    assert_eq!(entry.skipped[0].id, l.a);
    assert!(
        entry.skipped[0]
            .reason
            .contains("changed since the preview")
    );
    assert!(l.fx.lib.track(&l.a).is_ok());
}

#[test]
fn a_stale_report_is_refused() {
    let l = library();
    let report = l.fx.scan(false);
    // The library changes after the scan: the report no longer describes it.
    l.fx.lib.set_archived(&l.c, true).unwrap();
    let error =
        l.fx.lib
            .preview_missing_removal(&report.id, &ids(&[&l.b]), &cancel())
            .unwrap_err()
            .to_string();
    assert!(error.contains("scan again"), "{error}");
    let fresh = l.fx.scan(false);
    // A track the report never listed is refused, and so is an empty selection.
    assert!(
        l.fx.lib
            .preview_missing_removal(&fresh.id, &["not-a-track".into()], &cancel())
            .is_err()
    );
    assert!(
        l.fx.lib
            .preview_missing_removal(&fresh.id, &[], &cancel())
            .is_err()
    );
}

#[test]
fn a_file_that_reappears_after_the_scan_is_kept_when_previewed() {
    let l = library();
    let report = l.fx.scan(false);
    std::fs::write(&l.files[0], "audio bytes of track 1\n".repeat(40)).unwrap();
    let preview =
        l.fx.lib
            .preview_missing_removal(&report.id, &ids(&[&l.a, &l.b]), &cancel())
            .unwrap();
    assert_eq!(outcome(&preview, &l.a), MissingRowOutcome::Blocked);
    assert!(reason(&preview, &l.a).contains("at its recorded location"));
    assert_eq!(outcome(&preview, &l.b), MissingRowOutcome::Ready);
}

#[test]
fn a_selection_beyond_the_limit_is_refused_before_anything_is_read() {
    let l = library();
    let report = l.fx.scan(false);
    let many: Vec<String> = (0..=MISSING_REPAIR_LIMIT)
        .map(|i| format!("t{i}"))
        .collect();
    assert!(
        l.fx.lib
            .preview_missing_removal(&report.id, &many, &cancel())
            .is_err()
    );
}

#[test]
fn partial_selections_leave_the_rest_alone() {
    let l = library();
    let report = l.fx.scan(false);
    let preview =
        l.fx.lib
            .preview_missing_removal(&report.id, &ids(&[&l.b]), &cancel())
            .unwrap();
    l.fx.lib
        .apply_missing_removal(&preview.id, &cancel())
        .unwrap();
    assert!(l.fx.lib.track(&l.a).is_ok(), "a was not selected");
    assert!(l.fx.lib.track(&l.b).is_err());
    assert_eq!(
        l.fx.lib.manual_playlist_members("one").unwrap(),
        vec![l.c.clone(), l.a.clone()]
    );
}

#[test]
fn unfinished_work_and_open_drafts_block_removal() {
    let l = library();
    l.fx.exec(&format!(
        "UPDATE jobs SET state='queued' WHERE track_id='{}'",
        l.a
    ));
    l.fx.exec(&format!(
        "INSERT INTO metadata_edit_drafts VALUES('draft',1,'editing','{{\"tracks\":[\"{}\"]}}',1,1)",
        l.b
    ));
    let report = l.fx.scan(false);
    let preview =
        l.fx.lib
            .preview_missing_removal(&report.id, &ids(&[&l.a, &l.b]), &cancel())
            .unwrap();
    assert_eq!(outcome(&preview, &l.a), MissingRowOutcome::Blocked);
    assert!(reason(&preview, &l.a).contains("queued or being prepared"));
    assert_eq!(outcome(&preview, &l.b), MissingRowOutcome::Blocked);
    assert!(reason(&preview, &l.b).contains("open metadata draft"));
    assert!(
        l.fx.lib
            .apply_missing_removal(&preview.id, &cancel())
            .is_err()
    );
    assert_eq!(l.fx.total_tracks(), 3);
}

#[test]
fn a_draft_opened_after_the_preview_keeps_its_track() {
    let l = library();
    let report = l.fx.scan(false);
    let preview =
        l.fx.lib
            .preview_missing_removal(&report.id, &ids(&[&l.a, &l.b]), &cancel())
            .unwrap();
    assert_eq!(outcome(&preview, &l.b), MissingRowOutcome::Ready);
    l.fx.exec(&format!(
        "INSERT INTO metadata_edit_drafts VALUES('draft',1,'editing','{{\"tracks\":[\"{}\"]}}',1,1)",
        l.b
    ));
    let entry =
        l.fx.lib
            .apply_missing_removal(&preview.id, &cancel())
            .unwrap();
    assert_eq!(entry.tracks.len(), 1);
    assert_eq!(entry.skipped[0].id, l.b);
    assert!(entry.skipped[0].reason.contains("open metadata draft"));
    assert!(l.fx.lib.track(&l.b).is_ok());
}

#[test]
fn archived_tracks_are_listed_on_request_and_can_be_removed_and_restored() {
    let l = library();
    l.fx.lib.set_archived(&l.b, true).unwrap();
    let hidden = l.fx.scan(false);
    assert!(
        hidden.tracks.iter().all(|t| t.id != l.b),
        "archives stay out by default"
    );
    let before = l.fx.dump();
    let report = l.fx.scan(true);
    assert!(report.tracks.iter().any(|t| t.id == l.b && t.archived));
    let preview =
        l.fx.lib
            .preview_missing_removal(&report.id, &ids(&[&l.b]), &cancel())
            .unwrap();
    assert_eq!(preview.impact.archived, 1);
    let entry =
        l.fx.lib
            .apply_missing_removal(&preview.id, &cancel())
            .unwrap();
    assert!(entry.tracks[0].archived);
    assert!(l.fx.lib.track(&l.b).is_err());
    l.fx.lib.undo_missing_repair(&entry.id, &cancel()).unwrap();
    assert!(l.fx.lib.track(&l.b).unwrap().archived);
    assert_same_dump(&l.fx.dump(), &before);
}

#[test]
fn a_present_twin_is_left_alone_not_merged() {
    let l = library();
    // `twin` has the same bytes as the missing `a`, at another place.
    let twin_file = l.fx.file(&l.fx.watch.join("Copy"), "a.flac", 1);
    let twin = l.fx.ready(&twin_file, 130.);
    let twin_row = l.fx.sql(
        &format!("SELECT json_array(title,path_revision,archived,source_hash) FROM tracks WHERE id='{twin}'"),
        |r| r.get::<_, String>(0).unwrap(),
    );
    let report = l.fx.scan(false);
    let preview =
        l.fx.lib
            .preview_missing_removal(&report.id, &ids(&[&l.a]), &cancel())
            .unwrap();
    l.fx.lib
        .apply_missing_removal(&preview.id, &cancel())
        .unwrap();
    assert!(l.fx.lib.track(&twin).is_ok());
    assert_eq!(
        l.fx.sql(
            &format!("SELECT json_array(title,path_revision,archived,source_hash) FROM tracks WHERE id='{twin}'"),
            |r| r.get::<_, String>(0).unwrap()
        ),
        twin_row
    );
    assert!(
        l.fx.lib.manual_playlist_members("two").unwrap().is_empty(),
        "twin did not inherit a's membership"
    );
}

#[test]
fn undo_refuses_when_a_new_track_has_taken_the_old_location() {
    let l = library();
    let report = l.fx.scan(false);
    let preview =
        l.fx.lib
            .preview_missing_removal(&report.id, &ids(&[&l.a]), &cancel())
            .unwrap();
    let entry =
        l.fx.lib
            .apply_missing_removal(&preview.id, &cancel())
            .unwrap();
    // The file comes back and is admitted as a brand new track at the same path.
    std::fs::write(&l.files[0], "audio bytes of track 1\n".repeat(40)).unwrap();
    let newcomer = l.fx.ready(&l.files[0], 125.);
    let after_removal = l.fx.dump();
    let error =
        l.fx.lib
            .undo_missing_repair(&entry.id, &cancel())
            .unwrap_err()
            .to_string();
    assert!(error.contains("Cannot undo"), "{error}");
    assert_same_dump(&l.fx.dump(), &after_removal);
    assert!(l.fx.lib.track(&newcomer).is_ok());
    assert!(
        l.fx.lib.missing_repair_history().unwrap()[0]
            .undone_by
            .is_none()
    );
}

#[test]
fn unreachable_folders_are_flagged_for_the_confirmation() {
    let l = library();
    let hidden = l.fx.watch.join("Hidden");
    let file = l.fx.file(&hidden, "h.flac", 9);
    let gone = l.fx.ready(&file, 128.);
    std::fs::remove_dir_all(&hidden).unwrap();
    let report = l.fx.scan(false);
    let row = report.tracks.iter().find(|t| t.id == gone).unwrap();
    assert_eq!(
        row.state,
        cuerithm_library::MaintenanceFileState::Unavailable
    );
    let preview =
        l.fx.lib
            .preview_missing_removal(&report.id, &ids(&[&gone]), &cancel())
            .unwrap();
    assert_eq!(outcome(&preview, &gone), MissingRowOutcome::Ready);
    assert_eq!(preview.impact.unreachable, 1);
}

// ----- Find moved files ---------------------------------------------------

fn moved_library() -> (Library3, PathBuf) {
    let l = library();
    // a.flac and b.flac were moved, not deleted.
    let elsewhere = l.fx.root.join("Moved");
    std::fs::create_dir_all(&elsewhere).unwrap();
    for (n, seed) in [("a.flac", 1), ("b.flac", 2)] {
        let path = l.fx.file(&elsewhere, n, seed);
        assert!(path.is_file());
    }
    (l, elsewhere)
}

#[test]
fn moved_files_are_found_by_content_and_the_tracks_keep_everything() {
    let (l, elsewhere) = moved_library();
    let before = l.fx.dump();
    let report = l.fx.scan(false);
    let mut seen = Vec::new();
    let preview =
        l.fx.lib
            .preview_missing_relink(
                &report.id,
                &ids(&[&l.a, &l.b]),
                std::slice::from_ref(&elsewhere),
                &cancel(),
                |p| seen.push(p.completed),
            )
            .unwrap();
    assert_eq!(preview.ready, 2);
    assert_eq!(preview.files_checked, 2);
    assert!(!seen.is_empty(), "progress is reported");
    assert_same_dump(&l.fx.dump(), &before);
    let new_a = elsewhere.canonicalize().unwrap().join("a.flac");
    assert_eq!(
        preview
            .rows
            .iter()
            .find(|r| r.track_id == l.a)
            .unwrap()
            .new_path
            .as_deref(),
        Some(new_a.to_str().unwrap())
    );
    let revision_before: i64 = l.fx.sql(
        &format!("SELECT path_revision FROM tracks WHERE id='{}'", l.a),
        |r| r.get(0).unwrap(),
    );
    let entry =
        l.fx.lib
            .apply_missing_relink(&preview.id, &cancel())
            .unwrap();
    assert_eq!(entry.kind, MissingRepairKind::Relink);
    assert_eq!(entry.tracks.len(), 2);
    assert_eq!(l.fx.lib.track_path(&l.a).unwrap(), new_a);
    let revision_after: i64 = l.fx.sql(
        &format!("SELECT path_revision FROM tracks WHERE id='{}'", l.a),
        |r| r.get(0).unwrap(),
    );
    assert_eq!(revision_after, revision_before + 1);
    // Same track, same everything else.
    assert_eq!(
        l.fx.lib.manual_playlist_members("one").unwrap(),
        vec![l.c.clone(), l.a.clone(), l.b.clone()]
    );
    let mut now = l.fx.dump();
    let mut was = before.clone();
    for dump in [&mut now, &mut was] {
        dump.remove("tracks");
    }
    assert_eq!(now, was, "only the tracks' paths changed");

    let undo = l.fx.lib.undo_missing_repair(&entry.id, &cancel()).unwrap();
    assert_eq!(undo.undo_of.as_deref(), Some(entry.id.as_str()));
    assert!(
        undo.notes.is_empty(),
        "the files are outside every watch folder: {:?}",
        undo.notes
    );
    assert_eq!(
        l.fx.lib.track_path(&l.a).unwrap(),
        l.files[0].canonicalize().unwrap_or(l.files[0].clone())
    );
}

fn relinked() -> (Library3, MissingRepairEntryHandle) {
    let (l, elsewhere) = moved_library();
    let report = l.fx.scan(false);
    let preview =
        l.fx.lib
            .preview_missing_relink(
                &report.id,
                &ids(&[&l.a]),
                std::slice::from_ref(&elsewhere),
                &cancel(),
                |_| {},
            )
            .unwrap();
    let entry =
        l.fx.lib
            .apply_missing_relink(&preview.id, &cancel())
            .unwrap();
    (l, MissingRepairEntryHandle(entry.id))
}
struct MissingRepairEntryHandle(String);

/// Another connection holds the write lock; Undo must not need it to decide to refuse.
fn refused_without_waiting_for_the_write_lock(l: &Library3, entry: &str) -> String {
    let holder = rusqlite::Connection::open(&l.fx.db).unwrap();
    holder.execute_batch("BEGIN IMMEDIATE").unwrap();
    let started = std::time::Instant::now();
    let error =
        l.fx.lib
            .undo_missing_repair(entry, &cancel())
            .unwrap_err()
            .to_string();
    let waited = started.elapsed();
    holder.execute_batch("ROLLBACK").unwrap();
    assert!(
        waited < std::time::Duration::from_secs(3),
        "waited {waited:?} for the lock: {error}"
    );
    error
}

#[test]
fn a_removal_undo_checks_the_old_location_before_it_takes_the_write_lock() {
    let l = library();
    let report = l.fx.scan(false);
    let preview =
        l.fx.lib
            .preview_missing_removal(&report.id, &ids(&[&l.a]), &cancel())
            .unwrap();
    let entry =
        l.fx.lib
            .apply_missing_removal(&preview.id, &cancel())
            .unwrap();
    std::fs::write(&l.files[0], "somebody else's recording").unwrap();
    let error = refused_without_waiting_for_the_write_lock(&l, &entry.id);
    assert!(error.contains("a different file is now at"), "{error}");
}

#[test]
fn a_relink_undo_checks_the_old_location_before_it_takes_the_write_lock() {
    let (l, entry) = relinked();
    std::fs::write(&l.files[0], "an unrelated recording").unwrap();
    let error = refused_without_waiting_for_the_write_lock(&l, &entry.0);
    assert!(error.contains("a different file is now at"), "{error}");
}

#[test]
fn relink_undo_refuses_when_the_audio_changed_after_the_relink() {
    let (l, entry) = relinked();
    // The track was prepared again (tags written): its stored content identity moved on.
    l.fx.exec(&format!(
        "UPDATE tracks SET source_hash='after-preparation' WHERE id='{}'",
        l.a
    ));
    let after_relink = l.fx.dump();
    let error =
        l.fx.lib
            .undo_missing_repair(&entry.0, &cancel())
            .unwrap_err()
            .to_string();
    assert!(error.contains("changed after it was relinked"), "{error}");
    assert_same_dump(&l.fx.dump(), &after_relink);
    assert!(
        l.fx.lib
            .missing_repair_history()
            .unwrap()
            .iter()
            .all(|h| h.undone_by.is_none())
    );
}

#[test]
fn relink_undo_refuses_an_unrelated_file_at_the_old_location() {
    let (l, entry) = relinked();
    let old = l.files[0].clone();
    std::fs::write(&old, "an unrelated recording").unwrap();
    let after_relink = l.fx.dump();
    let error =
        l.fx.lib
            .undo_missing_repair(&entry.0, &cancel())
            .unwrap_err()
            .to_string();
    assert!(error.contains("a different file is now at"), "{error}");
    assert_same_dump(&l.fx.dump(), &after_relink);
    // A directory (or anything else that is not the audio) in the way is refused too.
    std::fs::remove_file(&old).unwrap();
    std::fs::create_dir(&old).unwrap();
    assert!(l.fx.lib.undo_missing_repair(&entry.0, &cancel()).is_err());
    std::fs::remove_dir(&old).unwrap();
    // With the identical bytes at the old place, Undo is accurate and goes ahead.
    std::fs::write(&old, "audio bytes of track 1\n".repeat(40)).unwrap();
    l.fx.lib.undo_missing_repair(&entry.0, &cancel()).unwrap();
    assert_eq!(l.fx.lib.track_path(&l.a).unwrap(), old);
}

#[cfg(unix)]
#[test]
fn relink_undo_puts_followed_artwork_back_too() {
    let l = library();
    let folder = l.fx.watch.join("Album");
    let cover = folder.join("cover.jpg");
    std::fs::write(&cover, b"cover art").unwrap();
    let hash = cuerithm_media::file_sha256(&cover).unwrap();
    l.fx.exec(&format!(
        "INSERT INTO organization_owned_sidecars VALUES('{a}',CAST('unix:{p}' AS BLOB),'{hash}','m',1)",
        a = l.a,
        p = cover.display()
    ));
    // The album folder was renamed: a.flac and the cover moved together.
    let renamed = l.fx.watch.join("Album (2024)");
    std::fs::rename(&folder, &renamed).unwrap();
    // (library() deleted a.flac before the rename, so put its bytes in the new folder.)
    l.fx.file(&renamed, "a.flac", 1);
    let report = l.fx.scan(false);
    let preview =
        l.fx.lib
            .preview_missing_relink(
                &report.id,
                &ids(&[&l.a]),
                std::slice::from_ref(&renamed),
                &cancel(),
                |_| {},
            )
            .unwrap();
    let entry =
        l.fx.lib
            .apply_missing_relink(&preview.id, &cancel())
            .unwrap();
    let stored = |fx: &Fixture| -> Vec<u8> {
        fx.sql("SELECT path FROM organization_owned_sidecars", |r| {
            r.get(0).unwrap()
        })
    };
    assert_eq!(
        stored(&l.fx),
        format!("unix:{}", renamed.join("cover.jpg").display()).into_bytes()
    );
    l.fx.lib.undo_missing_repair(&entry.id, &cancel()).unwrap();
    assert_eq!(
        stored(&l.fx),
        format!("unix:{}", cover.display()).into_bytes()
    );
}

#[test]
fn a_file_with_changed_content_is_never_relinked() {
    let l = library();
    let elsewhere = l.fx.root.join("Moved");
    // Same name, edited bytes.
    let edited = l.fx.file(&elsewhere, "a.flac", 100);
    assert!(edited.is_file());
    let report = l.fx.scan(false);
    let preview =
        l.fx.lib
            .preview_missing_relink(&report.id, &ids(&[&l.a]), &[elsewhere], &cancel(), |_| {})
            .unwrap();
    assert_eq!(outcome(&preview, &l.a), MissingRowOutcome::NotFound);
    assert!(reason(&preview, &l.a).contains("content is different"));
    assert!(
        l.fx.lib
            .apply_missing_relink(&preview.id, &cancel())
            .is_err()
    );
}

#[test]
fn identical_files_are_ambiguous_and_a_tracked_twin_is_not_taken_over() {
    let l = library();
    let first = l.fx.root.join("One");
    let second = l.fx.root.join("Two");
    l.fx.file(&first, "a.flac", 1);
    l.fx.file(&second, "a.flac", 1);
    let report = l.fx.scan(false);
    let preview =
        l.fx.lib
            .preview_missing_relink(
                &report.id,
                &ids(&[&l.a]),
                &[first.clone(), second],
                &cancel(),
                |_| {},
            )
            .unwrap();
    assert_eq!(outcome(&preview, &l.a), MissingRowOutcome::Ambiguous);
    // Another library track already owns the only matching file.
    let twin_file = l.fx.file(&l.fx.watch.join("Copy"), "x.flac", 2);
    let twin = l.fx.ready(&twin_file, 130.);
    let report = l.fx.scan(false);
    let preview =
        l.fx.lib
            .preview_missing_relink(
                &report.id,
                &ids(&[&l.b]),
                std::slice::from_ref(&l.fx.watch.join("Copy")),
                &cancel(),
                |_| {},
            )
            .unwrap();
    assert_eq!(outcome(&preview, &l.b), MissingRowOutcome::AlreadyTracked);
    assert!(
        l.fx.lib
            .apply_missing_relink(&preview.id, &cancel())
            .is_err()
    );
    assert!(l.fx.lib.track(&twin).is_ok());
}

/// SQL blob literal for a path in the library's own stored encoding (see `model::path_bytes`).
fn stored_path(path: &Path) -> String {
    #[cfg(unix)]
    let bytes = {
        use std::os::unix::ffi::OsStrExt;
        [b"unix:".as_slice(), path.as_os_str().as_bytes()].concat()
    };
    #[cfg(windows)]
    let bytes = {
        use std::os::windows::ffi::OsStrExt;
        let mut out = b"windows:".to_vec();
        for unit in path.as_os_str().encode_wide() {
            out.extend(unit.to_le_bytes());
        }
        out
    };
    format!(
        "X'{}'",
        bytes.iter().map(|b| format!("{b:02x}")).collect::<String>()
    )
}

#[test]
fn a_file_that_changes_or_is_claimed_before_apply_is_skipped() {
    let (l, elsewhere) = moved_library();
    let report = l.fx.scan(false);
    let preview =
        l.fx.lib
            .preview_missing_relink(
                &report.id,
                &ids(&[&l.a, &l.b]),
                std::slice::from_ref(&elsewhere),
                &cancel(),
                |_| {},
            )
            .unwrap();
    // `a`'s file is edited; another track is admitted at `b`'s new path.
    std::fs::write(elsewhere.join("a.flac"), "edited since the preview").unwrap();
    let b_new = elsewhere.canonicalize().unwrap().join("b.flac");
    l.fx.exec(&format!(
        "UPDATE tracks SET path={} WHERE id='{}'",
        stored_path(&b_new),
        l.c
    ));
    let error =
        l.fx.lib
            .apply_missing_relink(&preview.id, &cancel())
            .unwrap_err()
            .to_string();
    assert!(error.contains("No files were relinked"), "{error}");
    assert_eq!(
        l.fx.lib.track_path(&l.a).unwrap(),
        l.files[0].canonicalize().unwrap_or(l.files[0].clone())
    );
}

#[test]
fn archived_tracks_and_single_file_locations_work_too() {
    let (l, elsewhere) = moved_library();
    l.fx.lib.set_archived(&l.b, true).unwrap();
    let report = l.fx.scan(true);
    let file = elsewhere.join("b.flac");
    let preview =
        l.fx.lib
            .preview_missing_relink(
                &report.id,
                &ids(&[&l.b]),
                std::slice::from_ref(&file),
                &cancel(),
                |_| {},
            )
            .unwrap();
    assert_eq!(preview.ready, 1);
    let entry =
        l.fx.lib
            .apply_missing_relink(&preview.id, &cancel())
            .unwrap();
    assert!(entry.tracks[0].archived);
    assert_eq!(
        l.fx.lib.track_path(&l.b).unwrap(),
        file.canonicalize().unwrap()
    );
    assert!(l.fx.lib.track(&l.b).unwrap().archived);
}

#[test]
fn a_relink_of_a_stale_report_or_available_file_is_refused() {
    let (l, elsewhere) = moved_library();
    let report = l.fx.scan(false);
    let preview =
        l.fx.lib
            .preview_missing_relink(
                &report.id,
                &ids(&[&l.c]),
                std::slice::from_ref(&elsewhere),
                &cancel(),
                |_| {},
            )
            .unwrap();
    assert_eq!(outcome(&preview, &l.c), MissingRowOutcome::Blocked);
    l.fx.lib.set_archived(&l.c, true).unwrap();
    assert!(
        l.fx.lib
            .preview_missing_relink(
                &report.id,
                &ids(&[&l.b]),
                std::slice::from_ref(&elsewhere),
                &cancel(),
                |_| {}
            )
            .is_err()
    );
    let fresh = l.fx.scan(false);
    assert!(
        l.fx.lib
            .preview_missing_relink(&fresh.id, &ids(&[&l.b]), &[], &cancel(), |_| {})
            .is_err()
    );
}

#[test]
fn nothing_outside_the_chosen_location_is_read() {
    let (l, elsewhere) = moved_library();
    let report = l.fx.scan(false);
    let empty = l.fx.root.join("Empty");
    std::fs::create_dir_all(&empty).unwrap();
    let preview =
        l.fx.lib
            .preview_missing_relink(&report.id, &ids(&[&l.a]), &[empty], &cancel(), |_| {})
            .unwrap();
    assert_eq!(outcome(&preview, &l.a), MissingRowOutcome::NotFound);
    assert_eq!(preview.files_checked, 0);
    assert!(Path::new(&elsewhere).is_dir());
}
