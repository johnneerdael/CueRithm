//! `write_staged` as a whole: no-op repeats, foreign rows, readback verification.
use super::*;
use crate::{
    MixxxContainer, MixxxExistingTracks, MixxxOptions, SyncBeat, SyncCue, SyncPlaylist, SyncTrack,
    mixxx::{Host, Mp3Decoder, after_write, fixture},
};
use base64::{Engine, engine::general_purpose::STANDARD as BASE64};
use rusqlite::types::Value;
use sha2::{Digest, Sha256};

/// A Linux or Windows system (MAD decodes MP3) and a macOS one (CoreAudio).
const LINUX: Host = Host {
    case_insensitive: false,
    decoder: Mp3Decoder::Mad,
};
const MACOS: Host = Host {
    case_insensitive: true,
    decoder: Mp3Decoder::CoreAudio,
};

struct Env {
    directory: tempfile::TempDir,
    database: PathBuf,
}
impl Env {
    fn new(version: u32) -> Self {
        let directory = tempfile::tempdir().unwrap();
        let database = directory.path().join("mixxxdb.sqlite");
        drop(fixture::create(&database, version));
        Self {
            directory,
            database,
        }
    }
    fn connection(&self) -> Connection {
        fixture::open(&self.database)
    }
    fn file(&self, name: &str) -> PathBuf {
        let path = self.directory.path().join(name);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, b"not really audio").unwrap();
        path
    }
    fn track(&self, id: &str, name: &str) -> SyncTrack {
        SyncTrack {
            id: id.into(),
            path: self.file(name),
            title: format!("Title {id}"),
            artist: "Artist".into(),
            key: "8A".into(),
            bpm: 126.0,
            duration_seconds: 200.0,
            sample_rate: 44_100,
            rating: 5,
            beatgrid: vec![SyncBeat {
                seconds: 0.5,
                bpm: 126.0,
                beat: 1,
            }],
            cues: vec![SyncCue {
                index: 0,
                name: "Start".into(),
                start_seconds: 10.0,
                ..SyncCue::default()
            }],
            ..SyncTrack::default()
        }
    }
    fn hash(&self) -> String {
        format!(
            "{:x}",
            Sha256::digest(std::fs::read(&self.database).unwrap())
        )
    }
    fn run(
        &self,
        library: &SyncLibrary,
        options: &SyncOptions,
        request: &PlaylistOwnershipRequest,
    ) -> Result<StagedMixxx> {
        write_staged(
            &self.database,
            &self.database,
            library,
            options,
            request,
            LINUX,
        )
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
fn none() -> PlaylistOwnershipRequest {
    PlaylistOwnershipRequest::default()
}
fn scan_list(connection: &Connection) -> Vec<String> {
    connection
        .prepare("SELECT directory FROM directories ORDER BY directory")
        .unwrap()
        .query_map([], |row| row.get(0))
        .unwrap()
        .collect::<rusqlite::Result<_>>()
        .unwrap()
}
fn count(connection: &Connection, table: &str) -> i64 {
    connection
        .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
            row.get(0)
        })
        .unwrap()
}

#[test]
fn repeat_run_reports_no_change_and_leaves_the_file_alone() {
    for mode in [MixxxExistingTracks::Keep, MixxxExistingTracks::Replace] {
        let env = Env::new(39);
        let existing = env.file("existing.mp3");
        let key = tracks::location_key(&std::fs::canonicalize(&existing).unwrap()).unwrap();
        fixture::add_track(&env.connection(), &key, "Mixxx's own title");
        let mut known = env.track("known", "x.mp3");
        known.path = existing;
        let library = SyncLibrary {
            tracks: vec![known, env.track("new", "new.mp3")],
            playlists: vec![playlist("p", "Set", &["known", "new"])],
        };
        let first = env.run(&library, &options(mode), &none()).unwrap();
        assert!(first.changed, "{mode:?}");
        let after_first = env.hash();
        let request = PlaylistOwnershipRequest {
            previous: first.ownership.current.clone(),
            removed: Default::default(),
        };
        let second = env.run(&library, &options(mode), &request).unwrap();
        assert!(!second.changed, "{mode:?}: {:?}", second.warnings);
        assert_eq!(env.hash(), after_first, "{mode:?}: the file is untouched");
        assert_eq!(
            second.ownership.current.bindings,
            first.ownership.current.bindings
        );
    }
}

#[test]
fn unrelated_tracks_playlists_cues_and_settings_rows_are_untouched() {
    for mode in [MixxxExistingTracks::Keep, MixxxExistingTracks::Replace] {
        let env = Env::new(40);
        let connection = env.connection();
        let foreign = fixture::add_track(&connection, "/elsewhere/foreign.mp3", "Foreign");
        connection
            .execute(
                "INSERT INTO cues(track_id, type, position, length, hotcue, label, color)
                 VALUES(?1, 1, 88200, 0, 0, 'mine', 255)",
                [foreign],
            )
            .unwrap();
        fixture::add_playlist(&connection, "My Set", &[foreign]);
        fixture::add_crate(&connection, "My Crate", &[foreign]);
        connection
            .execute(
                "INSERT INTO Playlists(name, position, hidden) VALUES('History 1', 9, 2)",
                [],
            )
            .unwrap();
        connection
            .execute("INSERT INTO settings(name, value) VALUES('x', 'y')", [])
            .unwrap();
        let tables = [
            "library WHERE id = 1",
            "cues WHERE track_id = 1",
            "Playlists WHERE name != 'Set'",
            "PlaylistTracks WHERE playlist_id = 1",
            "crates",
            "crate_tracks",
            "settings",
            "track_locations WHERE id = 1",
        ];
        let before: Vec<_> = tables
            .iter()
            .map(|t| fixture::dump(&connection, &format!("SELECT * FROM {t}")))
            .collect();
        drop(connection);
        let library = SyncLibrary {
            tracks: vec![env.track("a", "a.mp3"), env.track("b", "b.mp3")],
            playlists: vec![playlist("p", "Set", &["a", "b"])],
        };
        let staged = env.run(&library, &options(mode), &none()).unwrap();
        assert!(staged.changed);
        let connection = env.connection();
        let after: Vec<_> = tables
            .iter()
            .map(|t| fixture::dump(&connection, &format!("SELECT * FROM {t}")))
            .collect();
        assert_eq!(before, after, "{mode:?}");
        assert_eq!(count(&connection, "library"), 3);
    }
}

#[test]
fn verification_rejects_corrupted_writes_and_nothing_is_committed() {
    type Corruption = (&'static str, fn(&Connection), &'static str);
    let cases: [Corruption; 6] = [
        (
            "a second cue in one slot",
            |c| {
                c.execute(
                    "INSERT INTO cues(track_id, type, position, length, hotcue, label, color)
                     SELECT track_id, 1, 5, 0, hotcue, 'dup', 1 FROM cues LIMIT 1",
                    [],
                )
                .unwrap();
            },
            "two cues in one slot",
        ),
        (
            "a zero-length loop",
            |c| {
                c.execute("INSERT INTO cues(track_id, type, position, length, hotcue, label, color) VALUES((SELECT MAX(id) FROM library), 4, 5, 0, 7, 'l', 1)", []).unwrap();
            },
            "a cue Mixxx discards",
        ),
        (
            "a cue added to a foreign track",
            |c| {
                c.execute("INSERT INTO cues(track_id, type, position, length, hotcue, label, color) VALUES(1, 1, 5, 0, 3, 'x', 1)", []).unwrap();
            },
            "added to a Mixxx track outside this sync",
        ),
        (
            "a changed foreign track",
            |c| {
                c.execute("UPDATE library SET title = 'hacked' WHERE id = 1", [])
                    .unwrap();
            },
            "outside this sync",
        ),
        (
            "an undecodable grid",
            |c| {
                c.execute("UPDATE library SET beats = x'ff' WHERE id = 2", [])
                    .unwrap();
            },
            "does not decode",
        ),
        (
            "a changed setting",
            |c| {
                c.execute(
                    "UPDATE settings SET value = 'x' WHERE name = 'mixxx.schema.version'",
                    [],
                )
                .unwrap();
            },
            "settings",
        ),
    ];
    for (name, corrupt, expected) in cases {
        let env = Env::new(39);
        fixture::add_track(&env.connection(), "/elsewhere/foreign.mp3", "Foreign");
        let library = SyncLibrary {
            tracks: vec![env.track("a", "a.mp3")],
            playlists: vec![],
        };
        let before = env.hash();
        after_write(corrupt);
        let error = env
            .run(&library, &options(MixxxExistingTracks::Replace), &none())
            .unwrap_err()
            .to_string();
        assert!(error.contains(expected), "{name}: {error}");
        assert_eq!(
            count(&env.connection(), "library"),
            1,
            "{name}: nothing was committed"
        );
        assert_eq!(env.hash(), before, "{name}");
    }
}

#[test]
fn unsupported_schema_is_refused_before_anything_is_written() {
    // 38 predates the formats written; 41 is refused only when Mixxx marks it as needing
    // newer software than the one this was checked against.
    for (version, needs, expected) in [(38, "3", "older"), (41, "41", "update CueRithm")] {
        let env = Env::new(version);
        env.connection()
            .execute(
                "UPDATE settings SET value = ?1 WHERE name = 'mixxx.schema.min_compatible_version'",
                [needs],
            )
            .unwrap();
        let before = env.hash();
        let library = SyncLibrary {
            tracks: vec![env.track("a", "a.mp3")],
            playlists: vec![],
        };
        let error = env
            .run(&library, &options(MixxxExistingTracks::Keep), &none())
            .unwrap_err()
            .to_string();
        assert!(error.contains(expected), "{error}");
        assert_eq!(env.hash(), before, "{version}");
    }
}

#[test]
fn a_newer_mixxx_release_that_keeps_the_format_is_written_and_reported() {
    // A schema Mixxx has not released yet, marked usable by older software, as every
    // schema so far has been.
    let env = Env::new(52);
    let library = SyncLibrary {
        tracks: vec![env.track("a", "a.mp3")],
        playlists: vec![],
    };
    let staged = env
        .run(&library, &options(MixxxExistingTracks::Keep), &none())
        .unwrap();
    assert!(staged.changed);
    assert!(
        staged
            .warnings
            .iter()
            .any(|w| w.contains("schema 52") && w.contains("newer than the newest CueRithm")),
        "{:?}",
        staged.warnings
    );
    assert_eq!(count(&env.connection(), "library"), 1);
    // The schema version Mixxx wrote is left exactly as it was.
    let version: String = env
        .connection()
        .query_row(
            "SELECT value FROM settings WHERE name = 'mixxx.schema.version'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(version, "52");
}

#[test]
fn aac_and_wma_warnings_depend_on_which_decoder_was_measured() {
    let env = Env::new(39);
    let library = SyncLibrary {
        tracks: vec![
            env.track("a", "a.m4a"),
            env.track("b", "b.WMA"),
            env.track("c", "c.aac"),
            env.track("d", "d.flac"),
        ],
        playlists: vec![],
    };
    let run = |host: Host, library: &SyncLibrary| {
        write_staged(
            &env.database,
            &env.database,
            library,
            &options(MixxxExistingTracks::Keep),
            &none(),
            host,
        )
        .unwrap()
        .warnings
    };
    // Linux and Windows decode AAC with FAAD or Media Foundation: unverified.
    let linux = run(LINUX, &library);
    let aac: Vec<_> = linux.iter().filter(|w| w.contains("AAC track")).collect();
    assert_eq!(aac.len(), 1, "{linux:?}");
    assert!(aac[0].starts_with("2 AAC track(s)") && aac[0].contains("not verified"));
    assert_eq!(linux.iter().filter(|w| w.contains("WMA track")).count(), 1);
    // macOS decodes AAC with CoreAudio, measured to match: no AAC warning there.
    let env_two = Env::new(39);
    let library = SyncLibrary {
        tracks: vec![
            env_two.track("a", "a.m4a"),
            env_two.track("b", "b.WMA"),
            env_two.track("c", "c.aac"),
        ],
        playlists: vec![],
    };
    let mac = write_staged(
        &env_two.database,
        &env_two.database,
        &library,
        &options(MixxxExistingTracks::Keep),
        &none(),
        MACOS,
    )
    .unwrap()
    .warnings;
    assert!(mac.iter().all(|w| !w.contains("AAC track")), "{mac:?}");
    assert_eq!(
        mac.iter().filter(|w| w.contains("WMA track")).count(),
        1,
        "WMA is never verified"
    );
    // Lossless sources warn about nothing on either system.
    let env_three = Env::new(39);
    let lossless = SyncLibrary {
        tracks: vec![env_three.track("d", "d.flac")],
        playlists: vec![],
    };
    let quiet = write_staged(
        &env_three.database,
        &env_three.database,
        &lossless,
        &options(MixxxExistingTracks::Keep),
        &none(),
        LINUX,
    )
    .unwrap()
    .warnings;
    assert!(
        quiet
            .iter()
            .all(|w| !w.contains("AAC") && !w.contains("WMA")),
        "{quiet:?}"
    );
}

#[test]
fn folders_mixxx_does_not_scan_are_reported_only_when_it_scans_some() {
    let env = Env::new(39);
    let library = SyncLibrary {
        tracks: vec![env.track("a", "a.mp3")],
        playlists: vec![],
    };
    let quiet = env
        .run(&library, &options(MixxxExistingTracks::Keep), &none())
        .unwrap();
    assert!(
        quiet
            .warnings
            .iter()
            .all(|w| !w.contains("outside the folders Mixxx scans")),
        "an empty list is seeded, so nothing is outside it"
    );
    let connection = env.connection();
    connection.execute("DELETE FROM directories", []).unwrap();
    connection
        .execute(
            "INSERT INTO directories(directory) VALUES('/somewhere/else')",
            [],
        )
        .unwrap();
    drop(connection);
    let outside = env
        .run(&library, &options(MixxxExistingTracks::Keep), &none())
        .unwrap();
    assert!(
        outside
            .warnings
            .iter()
            .any(|w| w.contains("1 track(s) are outside the folders Mixxx scans"))
    );
    let root = std::fs::canonicalize(env.directory.path()).unwrap();
    env.connection()
        .execute(
            "INSERT INTO directories(directory) VALUES(?1)",
            [root.to_str().unwrap()],
        )
        .unwrap();
    let inside = env
        .run(&library, &options(MixxxExistingTracks::Keep), &none())
        .unwrap();
    assert!(
        inside
            .warnings
            .iter()
            .all(|w| !w.contains("outside the folders Mixxx scans"))
    );
}

#[test]
fn flatpak_library_warns_about_tracks_outside_visible_folders() {
    let env = Env::new(39);
    let library = SyncLibrary {
        tracks: vec![env.track("a", "a.mp3")],
        playlists: vec![],
    };
    let flatpak = Path::new("/home/dj/.var/app/org.mixxx.Mixxx/.mixxx/mixxxdb.sqlite");
    let staged = write_staged(
        &env.database,
        flatpak,
        &library,
        &options(MixxxExistingTracks::Keep),
        &none(),
        LINUX,
    )
    .unwrap();
    assert!(
        staged
            .warnings
            .iter()
            .any(|w| w.contains("1 track(s)") && w.contains("Flatpak")),
        "{:?}",
        staged.warnings
    );
    let env = Env::new(39);
    let library = SyncLibrary {
        tracks: vec![env.track("a", "a.mp3")],
        playlists: vec![],
    };
    let native = Path::new("/home/dj/.mixxx/mixxxdb.sqlite");
    let staged = write_staged(
        &env.database,
        native,
        &library,
        &options(MixxxExistingTracks::Keep),
        &none(),
        LINUX,
    )
    .unwrap();
    assert!(staged.warnings.iter().all(|w| !w.contains("Flatpak")));
}

#[test]
fn macos_sandbox_warns_about_tracks_outside_the_folders_mixxx_may_read() {
    let env = Env::new(39);
    let library = SyncLibrary {
        tracks: vec![env.track("a", "a.mp3")],
        playlists: vec![],
    };
    // A library in a container-shaped path, so `sandbox.cfg` can sit beside it.
    let container = env
        .directory
        .path()
        .join("home/Library/Containers/org.mixxx.mixxx/Data/Library/Application Support/Mixxx");
    std::fs::create_dir_all(&container).unwrap();
    let container_db = container.join("mixxxdb.sqlite");
    let run = |database: &Path| {
        write_staged(
            &env.database,
            database,
            &library,
            &options(MixxxExistingTracks::Keep),
            &none(),
            MACOS,
        )
        .unwrap()
        .warnings
    };
    let flagged = |warnings: &[String]| {
        warnings.iter().any(|w| {
            w.contains("1 track(s)") && w.contains("sandbox") && w.contains("Music and Downloads")
        })
    };
    // First sync: the scan list is empty, so CueRithm seeds it. That folder has no
    // bookmark, so Mixxx may not read it.
    let first = run(&container_db);
    assert!(flagged(&first), "{first:?}");
    assert_eq!(scan_list(&env.connection()).len(), 1);
    // Later syncs see a folder in the list and still know nobody granted it.
    let second = run(&container_db);
    assert!(flagged(&second), "{second:?}");
    // A library that is not the sandboxed build's has no such warning.
    let plain = run(Path::new(
        "/Users/dj/Library/Application Support/Mixxx/mixxxdb.sqlite",
    ));
    assert!(plain.iter().all(|w| !w.contains("sandbox")), "{plain:?}");
    // Once the user has chosen that folder in Mixxx, its bookmark is in sandbox.cfg
    // (the key is the base64 of the path) and the warning goes.
    let root = std::fs::canonicalize(env.directory.path()).unwrap();
    std::fs::write(
        container.join("sandbox.cfg"),
        format!(
            "<MixxxMinimalSettings><group name=\"[OSXBookmark]\"><item name=\"{}\">AAAA</item></group></MixxxMinimalSettings>",
            BASE64.encode(root.to_str().unwrap())
        ),
    )
    .unwrap();
    let granted = run(&container_db);
    assert!(
        granted.iter().all(|w| !w.contains("sandbox")),
        "{granted:?}"
    );

    // Music inside Mixxx's own container needs no grant.
    let inside = Env::new(39);
    // The library's path is canonical in production, and so are the tracks' locations.
    let own = std::fs::canonicalize(inside.directory.path())
        .unwrap()
        .join("home/Library/Containers/org.mixxx.mixxx/Data/Library/Application Support/Mixxx");
    std::fs::create_dir_all(&own).unwrap();
    let library = SyncLibrary {
        tracks: vec![inside.track(
            "a",
            "home/Library/Containers/org.mixxx.mixxx/Data/Library/Application Support/Mixxx/a.mp3",
        )],
        playlists: vec![],
    };
    let warnings = write_staged(
        &inside.database,
        &own.join("mixxxdb.sqlite"),
        &library,
        &options(MixxxExistingTracks::Keep),
        &none(),
        MACOS,
    )
    .unwrap()
    .warnings;
    assert!(
        warnings.iter().all(|w| !w.contains("sandbox")),
        "{warnings:?}"
    );
}

#[test]
fn an_empty_scan_list_is_seeded_once_and_a_users_list_is_left_alone() {
    let env = Env::new(39);
    let library = SyncLibrary {
        tracks: vec![
            env.track("a", "set/a.mp3"),
            env.track("b", "set/deep/b.mp3"),
            env.track("c", "other/c.mp3"),
        ],
        playlists: vec![],
    };
    let first = env
        .run(&library, &options(MixxxExistingTracks::Keep), &none())
        .unwrap();
    let root = std::fs::canonicalize(env.directory.path()).unwrap();
    let root = root.to_str().unwrap();
    assert!(first.changed);
    assert_eq!(
        scan_list(&env.connection()),
        vec![format!("{root}/other"), format!("{root}/set")],
        "outermost folders only, none above the tracks"
    );
    assert!(
        first
            .warnings
            .iter()
            .any(|w| w.contains("Mixxx had no music folder")
                && w.contains("the 2 folders that hold the synced tracks")),
        "{:?}",
        first.warnings
    );
    let before = env.hash();
    let again = env
        .run(&library, &options(MixxxExistingTracks::Keep), &none())
        .unwrap();
    assert!(!again.changed && env.hash() == before);
    assert!(
        again
            .warnings
            .iter()
            .all(|w| !w.contains("no music folder"))
    );

    // A list the user built is not edited, however far the tracks are from it.
    let env = Env::new(39);
    env.connection()
        .execute(
            "INSERT INTO directories(directory) VALUES('/somewhere/else')",
            [],
        )
        .unwrap();
    let library = SyncLibrary {
        tracks: vec![env.track("a", "a.mp3")],
        playlists: vec![],
    };
    let staged = env
        .run(&library, &options(MixxxExistingTracks::Keep), &none())
        .unwrap();
    assert_eq!(
        scan_list(&env.connection()),
        vec!["/somewhere/else".to_owned()]
    );
    assert!(
        staged
            .warnings
            .iter()
            .all(|w| !w.contains("no music folder"))
    );
}

#[test]
fn verification_rejects_an_edit_to_a_scan_list_the_user_built() {
    let env = Env::new(39);
    env.connection()
        .execute(
            "INSERT INTO directories(directory) VALUES('/somewhere/else')",
            [],
        )
        .unwrap();
    let library = SyncLibrary {
        tracks: vec![env.track("a", "a.mp3")],
        playlists: vec![],
    };
    after_write(|connection| {
        connection.execute("DELETE FROM directories", []).unwrap();
    });
    let error = env
        .run(&library, &options(MixxxExistingTracks::Keep), &none())
        .unwrap_err()
        .to_string();
    assert!(error.contains("folders Mixxx already scans"), "{error}");
}

#[test]
fn summary_line_comes_first_and_counts_the_work() {
    let env = Env::new(39);
    let existing = env.file("existing.mp3");
    let key = tracks::location_key(&std::fs::canonicalize(&existing).unwrap()).unwrap();
    let connection = env.connection();
    let id = fixture::add_track(&connection, &key, "old");
    connection
        .execute("UPDATE library SET mixxx_deleted = 1", [])
        .unwrap();
    drop(connection);
    let mut known = env.track("known", "unused.mp3");
    known.path = existing;
    let library = SyncLibrary {
        tracks: vec![known, env.track("new", "new.mp3")],
        playlists: vec![playlist("p", "Set", &["known", "new"])],
    };
    let staged = env
        .run(&library, &options(MixxxExistingTracks::Replace), &none())
        .unwrap();
    assert_eq!(
        staged.warnings[0],
        "Mixxx: 1 track added; 1 replaced; 0 left unchanged; 1 made visible again; \
         1 hot cue or loop replaced; 1 playlist or crate created, 0 updated and 0 removed."
    );
    assert_eq!(
        env.connection()
            .query_row(
                "SELECT mixxx_deleted FROM library WHERE id = ?1",
                [id],
                |r| r.get::<_, i64>(0)
            )
            .unwrap(),
        0
    );
}

#[test]
fn full_run_on_schema_39_and_40_in_both_containers() {
    for version in [39, 40] {
        for container in [MixxxContainer::Playlists, MixxxContainer::Crates] {
            let env = Env::new(version);
            let library = SyncLibrary {
                tracks: vec![env.track("a", "a.mp3"), env.track("b", "b.mp3")],
                playlists: vec![playlist("p", "Set", &["b", "a"])],
            };
            let mut opts = options(MixxxExistingTracks::Replace);
            opts.mixxx.container = container;
            let staged = env.run(&library, &opts, &none()).unwrap();
            assert!(staged.changed);
            let connection = env.connection();
            assert_eq!(count(&connection, "library"), 2);
            assert_eq!(count(&connection, "cues"), 2);
            let table = if container == MixxxContainer::Playlists {
                "PlaylistTracks"
            } else {
                "crate_tracks"
            };
            assert_eq!(count(&connection, table), 2, "v{version} {container:?}");
            let integrity: String = connection
                .query_row("PRAGMA integrity_check", [], |r| r.get(0))
                .unwrap();
            assert_eq!(integrity, "ok");
            let row = fixture::dump(
                &connection,
                "SELECT header_parsed, source_synchronized_ms FROM library LIMIT 1",
            );
            assert_eq!(row, vec![vec![Value::Integer(1), Value::Null]]);
        }
    }
}
