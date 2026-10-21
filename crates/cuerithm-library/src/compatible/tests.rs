use super::*;
use crate::{PlaylistPreferences, model::path_bytes};
use rusqlite::params;
struct Fixture {
    _dir: tempfile::TempDir,
    lib: Library,
}
impl Fixture {
    fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let lib = Library::open(&dir.path().join("library.sqlite")).unwrap();
        Self { _dir: dir, lib }
    }
    fn add(&self, id: &str, key: Option<&str>, bpm: Option<f64>, energy: Option<u8>) {
        self.lib.conn().unwrap().execute("INSERT INTO tracks(id,path,title,artist,genre,source_hash,state,musical_key,bpm,energy,created,updated) VALUES(?1,?2,?1,'Artist','House','hash','ready',?3,?4,?5,0,0)",params![id,path_bytes(&self._dir.path().join(id)),key,bpm,energy]).unwrap();
    }
    fn request(&self) -> CompatibleTrackRequest {
        CompatibleTrackRequest {
            source_id: "source".into(),
            ..Default::default()
        }
    }
}
fn ids(r: &CompatibleTrackResult) -> Vec<&str> {
    r.matches.iter().map(|m| m.track.id.as_str()).collect()
}
#[test]
fn excludes_source_incoming_archive_and_enabled_unknown_measurements() {
    let f = Fixture::new();
    f.add("source", Some("8A"), Some(128.), Some(6));
    for id in ["valid", "incoming", "archive"] {
        f.add(id, Some("8A"), Some(128.), Some(6));
    }
    f.add("nokey", None, Some(128.), Some(6));
    f.add("nobpm", Some("8A"), None, Some(6));
    f.lib.conn().unwrap().execute_batch("UPDATE tracks SET state='incoming' WHERE id='incoming'; UPDATE tracks SET archived=1 WHERE id='archive';").unwrap();
    let r = f.lib.find_compatible_tracks(f.request()).unwrap();
    assert_eq!(ids(&r), vec!["valid"]);
    assert_eq!(r.checked, 3);
    assert_eq!(r.excluded_unknown, 2);
    let mut q = f.request();
    q.compatible_key = false;
    q.bpm_tolerance = None;
    let r = f.lib.find_compatible_tracks(q).unwrap();
    assert_eq!(r.matches.len(), 3);
    assert_eq!(r.excluded_unknown, 0);
}
#[test]
fn honors_saved_fuzzy_mode_and_ranks_aliases_before_neighbors() {
    let f = Fixture::new();
    f.add("source", Some("Am"), Some(128.), Some(6));
    f.add("z_same", Some("8A"), Some(130.), Some(6));
    f.add("a_neighbor", Some("9A"), Some(128.), Some(6));
    f.add("fuzzy", Some("9B"), Some(128.), Some(6));
    f.add("far", Some("10A"), Some(128.), Some(6));
    assert_eq!(
        ids(&f.lib.find_compatible_tracks(f.request()).unwrap()),
        vec!["z_same", "a_neighbor"]
    );
    f.lib
        .save_playlist_preferences(
            PlaylistPreferences {
                key_mixing_mode: KeyMixingMode::Fuzzy,
                ..Default::default()
            },
            0,
        )
        .unwrap();
    let r = f.lib.find_compatible_tracks(f.request()).unwrap();
    assert_eq!(ids(&r), vec!["z_same", "a_neighbor", "fuzzy"]);
    assert!(
        r.matches[0]
            .reasons
            .iter()
            .any(|r| r.contains("Same key: 8A"))
    );
    let mut q = f.request();
    q.key_mode = Some(KeyMixingMode::Harmonic);
    assert_eq!(f.lib.find_compatible_tracks(q).unwrap().matches.len(), 2);
}
#[test]
fn bpm_boundaries_and_opt_in_half_double_explain_actual_multiplier() {
    let f = Fixture::new();
    f.add("source", Some("8A"), Some(128.), Some(6));
    f.add("edge", Some("8A"), Some(132.), Some(6));
    f.add("outside", Some("8A"), Some(132.01), Some(6));
    f.add("half", Some("8A"), Some(64.), Some(6));
    f.add("double", Some("8A"), Some(256.), Some(6));
    assert_eq!(
        ids(&f.lib.find_compatible_tracks(f.request()).unwrap()),
        vec!["edge"]
    );
    let mut q = f.request();
    q.include_half_double = true;
    let r = f.lib.find_compatible_tracks(q).unwrap();
    assert_eq!(ids(&r), vec!["double", "half", "edge"]);
    assert_eq!(r.matches[0].bpm_multiplier, Some(0.5));
    assert_eq!(r.matches[1].bpm_multiplier, Some(2.));
    assert!(
        r.matches[0]
            .reasons
            .iter()
            .any(|s| s.contains("256") && s.contains("0.5"))
    );
}
#[test]
fn optional_energy_genre_and_all_custom_tags_are_anded() {
    let f = Fixture::new();
    f.add("source", Some("8A"), Some(128.), Some(6));
    f.add("match", Some("8A"), Some(128.), Some(7));
    f.add("too_high", Some("8A"), Some(128.), Some(8));
    f.add("unknown", Some("8A"), Some(128.), None);
    f.add("tagless", Some("8A"), Some(128.), Some(6));
    f.lib.conn().unwrap().execute_batch("INSERT INTO custom_tag_categories VALUES('mood','Mood'); INSERT INTO custom_tags VALUES('peak','mood','Peak'); INSERT INTO custom_tags VALUES('vocal','mood','Vocal'); INSERT INTO track_custom_tags VALUES('match','peak'); INSERT INTO track_custom_tags VALUES('match','vocal'); INSERT INTO track_custom_tags VALUES('tagless','peak');").unwrap();
    let mut q = f.request();
    q.energy_tolerance = Some(1);
    q.genre = Some("house".into());
    q.custom_tag_ids = vec!["peak".into(), "vocal".into()];
    let r = f.lib.find_compatible_tracks(q).unwrap();
    assert_eq!(ids(&r), vec!["match"]);
    assert_eq!(r.matches[0].energy_delta, Some(1));
    assert!(
        r.matches[0]
            .reasons
            .iter()
            .any(|s| s.contains("Mood: Peak"))
    );
}
#[test]
fn required_source_fields_and_invalid_options_give_actionable_errors() {
    let f = Fixture::new();
    f.add("source", None, None, None);
    for (field, q) in [
        ("key", f.request()),
        ("BPM", {
            let mut q = f.request();
            q.compatible_key = false;
            q
        }),
        ("energy", {
            let mut q = f.request();
            q.compatible_key = false;
            q.bpm_tolerance = None;
            q.energy_tolerance = Some(1);
            q
        }),
    ] {
        assert!(
            f.lib
                .find_compatible_tracks(q.clone())
                .unwrap_err()
                .to_string()
                .contains(field)
        );
    }
    let mut q = f.request();
    q.bpm_tolerance = Some(f64::NAN);
    assert!(f.lib.find_compatible_tracks(q).is_err());
    let mut q = f.request();
    q.custom_tag_ids = vec!["deleted".into()];
    assert!(f.lib.find_compatible_tracks(q).is_err());
}
#[test]
fn deterministic_order_covers_candidates_beyond_a_page() {
    let f = Fixture::new();
    f.add("source", Some("8A"), Some(128.), Some(6));
    for n in (0..520).rev() {
        f.add(
            &format!("candidate-{n:03}"),
            Some("8A"),
            Some(128.),
            Some(6),
        );
    }
    let a = f.lib.find_compatible_tracks(f.request()).unwrap();
    let b = f.lib.find_compatible_tracks(f.request()).unwrap();
    assert_eq!(a.matches.len(), 520);
    assert_eq!(ids(&a), ids(&b));
    assert_eq!(ids(&a)[0], "candidate-000");
    assert_eq!(ids(&a)[519], "candidate-519");
}

#[test]
fn saving_rejects_library_change_after_preview_without_creating_playlist() {
    let f = Fixture::new();
    f.add("source", Some("8A"), Some(128.), Some(6));
    f.add("candidate", Some("8A"), Some(128.), Some(6));
    let preview = f.lib.find_compatible_tracks(f.request()).unwrap();
    let playlist_revision = f.lib.playlist_state().unwrap().revision;
    // A separate library write lands after a client precheck and before the save transaction.
    {
        let conn = f.lib.conn().unwrap();
        conn.execute("UPDATE tracks SET bpm=150 WHERE id='candidate'", [])
            .unwrap();
        Library::bump(&conn).unwrap();
    }
    let saved = f
        .lib
        .save_prepared_playlist(crate::PreparedPlaylistSaveRequest {
            name: "Practice".into(),
            track_ids: vec!["candidate".into()],
            expected_library_revision: preview.library_revision,
            expected_playlist_revision: playlist_revision,
        });
    assert!(
        saved.is_err(),
        "a stale measurement snapshot must never authorize saving"
    );
    assert!(f.lib.playlist_state().unwrap().nodes.is_empty());
}
#[test]
fn saving_checks_readiness_inside_transaction_and_keeps_order_and_undo() {
    let f = Fixture::new();
    f.add("source", Some("8A"), Some(128.), Some(6));
    f.add("a", Some("8A"), Some(128.), Some(6));
    f.add("b", Some("8A"), Some(129.), Some(6));
    let preview = f.lib.find_compatible_tracks(f.request()).unwrap();
    let revision = f.lib.playlist_state().unwrap().revision;
    let request = crate::PreparedPlaylistSaveRequest {
        name: "Practice".into(),
        track_ids: vec!["b".into(), "a".into(), "b".into()],
        expected_library_revision: preview.library_revision,
        expected_playlist_revision: revision,
    };
    f.lib
        .conn()
        .unwrap()
        .execute("UPDATE tracks SET state='incoming' WHERE id='a'", [])
        .unwrap();
    assert!(f.lib.save_prepared_playlist(request.clone()).is_err());
    assert!(f.lib.playlist_state().unwrap().nodes.is_empty());
    f.lib
        .conn()
        .unwrap()
        .execute("UPDATE tracks SET state='ready' WHERE id='a'", [])
        .unwrap();
    let change = f.lib.save_prepared_playlist(request).unwrap();
    assert_eq!(
        f.lib
            .manual_playlist_members(&change.created_ids[0])
            .unwrap(),
        ["b", "a"]
    );
    f.lib
        .undo_playlist_edit(change.edit_id.as_deref().unwrap(), change.revision)
        .unwrap();
    assert!(f.lib.playlist_state().unwrap().nodes.is_empty());
}
