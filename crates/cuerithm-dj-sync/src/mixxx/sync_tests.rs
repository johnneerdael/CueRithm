//! Mixxx through the shared prepare, apply and restore path.
use crate::{
    DjSoftware, MixxxExistingTracks, MixxxOptions, PlaylistOwnershipContexts, SyncBeat, SyncCue,
    SyncLibrary, SyncOptions, SyncPlaylist, SyncPreview, SyncTarget, SyncTrack, apply_sync,
    mixxx::fixture, ownership::PlaylistOwnershipRequest, prepare_sync_owned_with_runtime,
    restore_sync, sync_ownership, transaction::JournalState,
};
use anyhow::Result;
use rusqlite::Connection;
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeSet,
    path::{Path, PathBuf},
};

struct Env {
    root: tempfile::TempDir,
    settings: PathBuf,
    journals: PathBuf,
}
impl Env {
    fn new(version: u32) -> Self {
        // The developer may well have Mixxx open; these tests exercise the sync, and
        // `a_running_mixxx_blocks_preview_and_apply` exercises the guard itself.
        crate::discovery::pretend_target_running(false);
        let root = tempfile::tempdir().unwrap();
        let settings = root.path().join("settings");
        let journals = root.path().join("journals");
        std::fs::create_dir_all(&settings).unwrap();
        std::fs::create_dir_all(&journals).unwrap();
        std::fs::create_dir_all(root.path().join("music")).unwrap();
        drop(fixture::create(&settings.join("mixxxdb.sqlite"), version));
        Self {
            root,
            settings,
            journals,
        }
    }
    fn database(&self) -> PathBuf {
        self.settings.join("mixxxdb.sqlite")
    }
    fn connection(&self) -> Connection {
        fixture::open(&self.database())
    }
    fn hash(&self) -> String {
        format!(
            "{:x}",
            Sha256::digest(std::fs::read(self.database()).unwrap())
        )
    }
    fn track(&self, id: &str) -> SyncTrack {
        let path = self.root.path().join("music").join(format!("{id}.flac"));
        std::fs::write(&path, format!("synthetic audio {id}")).unwrap();
        SyncTrack {
            id: id.into(),
            path,
            title: format!("Title {id}"),
            artist: "Artist".into(),
            key: "8A".into(),
            bpm: 128.0,
            duration_seconds: 120.0,
            sample_rate: 44_100,
            beatgrid: vec![SyncBeat {
                seconds: 0.5,
                bpm: 128.0,
                beat: 1,
            }],
            cues: vec![SyncCue {
                index: 0,
                name: "Drop".into(),
                start_seconds: 30.0,
                ..SyncCue::default()
            }],
            ..SyncTrack::default()
        }
    }
    fn library(&self, ids: &[&str], playlists: Vec<SyncPlaylist>) -> SyncLibrary {
        SyncLibrary {
            tracks: ids.iter().map(|id| self.track(id)).collect(),
            playlists,
        }
    }
    fn target(&self) -> SyncTarget {
        SyncTarget {
            software: DjSoftware::Mixxx,
            environment: Default::default(),
            path: self.settings.clone(),
            source_root: None,
        }
    }
    fn prepare(
        &self,
        library: &SyncLibrary,
        options: &SyncOptions,
        contexts: &PlaylistOwnershipContexts,
    ) -> Result<SyncPreview> {
        prepare_sync_owned_with_runtime(
            &self.journals,
            self.target(),
            library,
            options,
            None,
            contexts,
            &Default::default(),
        )
    }
    /// What the library service would pass next time: the ownership just acknowledged.
    fn contexts(&self, id: &str, removed: &[&str]) -> PlaylistOwnershipContexts {
        sync_ownership(&self.journals, id)
            .unwrap()
            .into_iter()
            .map(|(path, (root, report))| {
                (
                    path,
                    (
                        root,
                        PlaylistOwnershipRequest {
                            previous: report.current,
                            removed: removed.iter().map(|s| (*s).to_owned()).collect(),
                        },
                    ),
                )
            })
            .collect()
    }
}
fn options(mode: MixxxExistingTracks) -> SyncOptions {
    SyncOptions {
        mixxx: MixxxOptions {
            existing_tracks: mode,
            ..MixxxOptions::default()
        },
        ..SyncOptions::default()
    }
}
fn playlist(id: &str, name: &str, tracks: &[&str]) -> SyncPlaylist {
    SyncPlaylist {
        id: id.into(),
        name: name.into(),
        track_ids: tracks.iter().map(|t| (*t).to_owned()).collect(),
        ..SyncPlaylist::default()
    }
}
fn count(connection: &Connection, table: &str) -> i64 {
    connection
        .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
            row.get(0)
        })
        .unwrap()
}
fn none() -> PlaylistOwnershipContexts {
    PlaylistOwnershipContexts::new()
}

#[test]
fn preview_apply_restore_round_trip_returns_the_original_file() {
    let env = Env::new(39);
    let original = env.hash();
    let library = env.library(&["a", "b"], vec![playlist("p", "Set", &["a", "b"])]);
    let preview = env
        .prepare(&library, &options(MixxxExistingTracks::Replace), &none())
        .unwrap();
    assert_eq!(preview.files.len(), 1);
    assert_eq!(
        preview.files[0].target,
        std::fs::canonicalize(env.database()).unwrap()
    );
    assert!(
        preview.warnings[0].starts_with("Mixxx: 2 tracks added; 0 replaced; 0 left unchanged"),
        "{:?}",
        preview.warnings
    );
    assert_eq!((preview.tracks, preview.playlists), (2, 1));
    assert_eq!(env.hash(), original, "preview never touches the library");

    let applied = apply_sync(&env.journals, &preview.id).unwrap();
    assert_eq!(applied.state, JournalState::Applied);
    let connection = env.connection();
    assert_eq!(
        (count(&connection, "library"), count(&connection, "cues")),
        (2, 2)
    );
    assert_eq!(count(&connection, "PlaylistTracks"), 2);
    drop(connection);

    let restored = restore_sync(&env.journals, &preview.id).unwrap();
    assert_eq!(restored.state, JournalState::Restored);
    assert_eq!(env.hash(), original);
}

#[test]
fn folder_and_file_spellings_of_the_target_both_work() {
    let env = Env::new(40);
    let library = env.library(&["a"], vec![]);
    let mut target = env.target();
    target.path = env.database();
    let preview = prepare_sync_owned_with_runtime(
        &env.journals,
        target,
        &library,
        &options(MixxxExistingTracks::Keep),
        None,
        &none(),
        &Default::default(),
    )
    .unwrap();
    assert_eq!(preview.files.len(), 1);
}

#[test]
fn second_preview_has_no_file_changes_and_its_empty_plan_applies() {
    let env = Env::new(39);
    let library = env.library(&["a", "b"], vec![playlist("p", "Set", &["b", "a"])]);
    let opts = options(MixxxExistingTracks::Replace);
    let first = env.prepare(&library, &opts, &none()).unwrap();
    apply_sync(&env.journals, &first.id).unwrap();
    let after_first = env.hash();
    let second = env
        .prepare(&library, &opts, &env.contexts(&first.id, &[]))
        .unwrap();
    assert!(second.files.is_empty(), "{:?}", second.files);
    assert_eq!(second.bytes_to_write, 0);
    assert_eq!(
        (second.deleted_playlists, second.retained_external_playlists),
        (0, 0)
    );
    let applied = apply_sync(&env.journals, &second.id).unwrap();
    assert_eq!(applied.state, JournalState::Applied);
    assert_eq!(env.hash(), after_first, "nothing was rewritten");
}

#[test]
fn a_library_edited_after_preview_makes_the_preview_stale() {
    let env = Env::new(39);
    let library = env.library(&["a"], vec![]);
    let preview = env
        .prepare(&library, &options(MixxxExistingTracks::Keep), &none())
        .unwrap();
    env.connection()
        .execute(
            "INSERT INTO settings(name, value) VALUES('edited', 'later')",
            [],
        )
        .unwrap();
    let edited = env.hash();
    assert!(apply_sync(&env.journals, &preview.id).is_err());
    assert_eq!(env.hash(), edited, "the newer library is left as it was");
    assert_eq!(count(&env.connection(), "library"), 0);
}

#[test]
fn removal_of_an_owned_playlist_is_previewed_and_applied() {
    let env = Env::new(39);
    let opts = options(MixxxExistingTracks::Keep);
    let first_library = env.library(
        &["a", "b"],
        vec![
            playlist("p", "Keep me", &["a"]),
            playlist("q", "Remove me", &["b"]),
        ],
    );
    let first = env.prepare(&first_library, &opts, &none()).unwrap();
    apply_sync(&env.journals, &first.id).unwrap();
    let second_library = env.library(&["a", "b"], vec![playlist("p", "Keep me", &["a"])]);
    let second = env
        .prepare(&second_library, &opts, &env.contexts(&first.id, &["q"]))
        .unwrap();
    assert_eq!(second.deleted_playlists, 1);
    apply_sync(&env.journals, &second.id).unwrap();
    let names: BTreeSet<String> = fixture::dump(&env.connection(), "SELECT name FROM Playlists")
        .into_iter()
        .map(|row| match &row[0] {
            rusqlite::types::Value::Text(name) => name.clone(),
            other => panic!("{other:?}"),
        })
        .collect();
    assert_eq!(names, BTreeSet::from(["Keep me".to_owned()]));
}

#[test]
fn smart_playlists_warn_and_export_members() {
    let env = Env::new(39);
    let mut smart = playlist("s", "Smart", &["a"]);
    smart.smart = Some(cuerithm_smartlists::SmartDefinition {
        match_all: true,
        rules: vec![],
    });
    let library = env.library(&["a"], vec![smart]);
    let preview = env
        .prepare(&library, &options(MixxxExistingTracks::Keep), &none())
        .unwrap();
    assert!(
        preview.warnings.iter().any(|w| w.contains(
            "Smartlist 'Smart' was materialized: Mixxx has no smart playlists or crates"
        )),
        "{:?}",
        preview.warnings
    );
    apply_sync(&env.journals, &preview.id).unwrap();
    assert_eq!(count(&env.connection(), "PlaylistTracks"), 1);
}

#[test]
fn refusals_name_their_reason_and_change_nothing() {
    // A Wine/Proton environment: Mixxx is native everywhere.
    let env = Env::new(39);
    let before = env.hash();
    let prefix = env.root.path().join("prefix");
    std::fs::create_dir_all(prefix.join("drive_c")).unwrap();
    let mut target = env.target();
    target.environment = cuerithm_rekordbox::TargetEnvironment::WindowsCompatibility {
        prefix: prefix.clone(),
        installation: None,
        mappings: vec![cuerithm_rekordbox::DriveMapping {
            windows_root: "C:\\".into(),
            host_root: prefix.join("drive_c"),
        }],
    };
    let library = env.library(&["a"], vec![]);
    let error = prepare_sync_owned_with_runtime(
        &env.journals,
        target,
        &library,
        &options(MixxxExistingTracks::Keep),
        None,
        &none(),
        &Default::default(),
    )
    .unwrap_err()
    .to_string();
    assert!(error.contains("native environment"), "{error}");

    // A crashed Mixxx's leftover journal.
    std::fs::write(env.settings.join("mixxxdb.sqlite-journal"), b"pending").unwrap();
    let error = env
        .prepare(&library, &options(MixxxExistingTracks::Keep), &none())
        .unwrap_err()
        .to_string();
    assert!(error.contains("did not close cleanly"), "{error}");
    std::fs::remove_file(env.settings.join("mixxxdb.sqlite-journal")).unwrap();

    // A newer schema that Mixxx itself marks as needing newer software.
    let newer = Env::new(41);
    newer
        .connection()
        .execute(
            "UPDATE settings SET value = '41' WHERE name = 'mixxx.schema.min_compatible_version'",
            [],
        )
        .unwrap();
    let error = newer
        .prepare(
            &newer.library(&["a"], vec![]),
            &options(MixxxExistingTracks::Keep),
            &none(),
        )
        .unwrap_err()
        .to_string();
    assert!(
        error.contains("needs software that knows schema 41") && error.contains("update CueRithm"),
        "{error}"
    );
    assert_eq!(env.hash(), before);

    // A file that is not a Mixxx library, and a place where neither the folder nor the
    // folder it would be in exists. (A folder with no library is not a refusal: the sync
    // creates one.)
    let empty = tempfile::tempdir().unwrap();
    let stray = empty.path().join("notes.txt");
    std::fs::write(&stray, b"not a library").unwrap();
    for (path, reason) in [
        (stray, "Choose the Mixxx settings folder"),
        (
            empty.path().join("a/b/mixxx"),
            "Neither the Mixxx settings folder",
        ),
    ] {
        let mut target = env.target();
        target.path = path;
        let error = prepare_sync_owned_with_runtime(
            &env.journals,
            target,
            &library,
            &options(MixxxExistingTracks::Keep),
            None,
            &none(),
            &Default::default(),
        )
        .unwrap_err();
        assert!(format!("{error:#}").contains(reason), "{error:#}");
    }
    assert!(!empty.path().join("a").exists(), "nothing was created");
}

#[test]
fn a_running_mixxx_blocks_preview_and_apply() {
    let env = Env::new(39);
    let library = env.library(&["a"], vec![]);
    let opts = options(MixxxExistingTracks::Keep);
    let preview = env.prepare(&library, &opts, &none()).unwrap();
    let before = env.hash();
    crate::discovery::pretend_target_running(true);
    let error = env
        .prepare(&library, &opts, &none())
        .unwrap_err()
        .to_string();
    assert!(error.contains("Close the target DJ software"), "{error}");
    let error = apply_sync(&env.journals, &preview.id)
        .unwrap_err()
        .to_string();
    assert!(error.contains("Close the target DJ software"), "{error}");
    assert_eq!(env.hash(), before);
    // Closing Mixxx lets the same preview apply.
    crate::discovery::pretend_target_running(false);
    assert_eq!(
        apply_sync(&env.journals, &preview.id).unwrap().state,
        JournalState::Applied
    );
}

#[test]
fn preview_counts_added_replaced_and_kept_tracks() {
    let env = Env::new(39);
    // Two tracks Mixxx already knows by their canonical location.
    for id in ["known", "other"] {
        let path = env.track(id).path;
        let key =
            crate::mixxx::tracks::location_key(&std::fs::canonicalize(path).unwrap()).unwrap();
        fixture::add_track(&env.connection(), &key, &format!("Mixxx {id}"));
    }
    let library = env.library(&["known", "other", "fresh"], vec![]);
    let keep = env
        .prepare(&library, &options(MixxxExistingTracks::Keep), &none())
        .unwrap();
    assert!(
        keep.warnings[0].starts_with("Mixxx: 1 track added; 0 replaced; 2 left unchanged"),
        "{:?}",
        keep.warnings
    );
    let replace = env
        .prepare(&library, &options(MixxxExistingTracks::Replace), &none())
        .unwrap();
    assert!(
        replace.warnings[0].starts_with("Mixxx: 1 track added; 2 replaced; 0 left unchanged"),
        "{:?}",
        replace.warnings
    );
    let _ = Path::new("");
}
