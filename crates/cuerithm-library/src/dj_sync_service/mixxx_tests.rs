//! Mixxx through the library service: target identity, the two modes, scopes and
//! the acknowledged baseline.
use super::*;
use cuerithm_dj_sync::{MixxxExistingTracks, mixxx::fixture};
use cuerithm_media::MediaInfo;
use rusqlite::Connection;
use serde_json::json;

struct Setup {
    _dir: tempfile::TempDir,
    root: PathBuf,
    library: Library,
    settings: PathBuf,
    target: SyncTarget,
}

fn setup() -> Setup {
    // The developer may have Mixxx open; the guard itself is covered below.
    cuerithm_dj_sync::discovery::pretend_target_running(false);
    let (dir, library, _virtualdj) = super::tests::fixture();
    let root = dir.path().canonicalize().unwrap();
    let settings = root.join("mixxx-settings");
    fs::create_dir(&settings).unwrap();
    drop(fixture::create(
        &settings.join(cuerithm_dj_sync::mixxx::DATABASE_FILE),
        39,
    ));
    let target = SyncTarget {
        software: DjSoftware::Mixxx,
        environment: Default::default(),
        // The folder spelling; the service normalizes it to the database file.
        path: settings.clone(),
        source_root: None,
    };
    Setup {
        _dir: dir,
        root,
        library,
        settings,
        target,
    }
}

impl Setup {
    fn mixxx(&self) -> Connection {
        fixture::open(&self.settings.join(cuerithm_dj_sync::mixxx::DATABASE_FILE))
    }
    /// Another prepared track with its own audio file.
    fn add_track(&self, id: &str) {
        let source = self.root.join(format!("{id}.wav"));
        fs::write(&source, format!("Synthetic source bytes for {id}")).unwrap();
        let hash = file_hash(&source).unwrap().unwrap();
        let media = MediaInfo {
            codec: "pcm_s16le".into(),
            sample_rate: 48000,
            channels: 2,
            bits_per_sample: Some(16),
            tags: BTreeMap::new(),
            properties: BTreeMap::new(),
        };
        let analysis = json!({"source_sample_rate":48000,"publication":{
            "grid":{"bpm":124.0,"startTime":0.25},"cues":[],
            "implementation_sha256":"synthetic","inputs_sha256":"synthetic"}});
        let connection = self.library.conn().unwrap();
        connection
            .execute(
                "INSERT INTO tracks(id,path,title,artist,album,genre,source_hash,state,analysis_json,metadata_json,musical_key,bpm,duration,created,updated,sort_title)
                 VALUES(?1,?2,?3,'Artist','Album','House',?4,'ready',?5,?6,'8A',124,120,1,1,?3)",
                params![
                    id,
                    crate::model::path_bytes(&source),
                    format!("Title {id}"),
                    hash,
                    analysis.to_string(),
                    serde_json::to_string(&media).unwrap()
                ],
            )
            .unwrap();
        Library::bump(&connection).unwrap();
    }
    fn add_playlist(&self, id: &str, name: &str, members: &[&str]) {
        let connection = self.library.conn().unwrap();
        connection
            .execute(
                "INSERT INTO playlists(id,kind,name,position,last_edit_id) VALUES(?1,'playlist',?2,0,'fixture')",
                params![id, name],
            )
            .unwrap();
        for (position, track) in members.iter().enumerate() {
            connection
                .execute(
                    "INSERT INTO playlist_members VALUES(?1,?2,?3)",
                    params![id, track, position as i64],
                )
                .unwrap();
        }
        Library::bump(&connection).unwrap();
    }
    /// A Mixxx track the user already has for one of our source files.
    fn mixxx_knows(&self, id: &str, title: &str) -> i64 {
        let path = self.library.track_path(id).unwrap();
        let location = cuerithm_dj_sync::destination_reference(&fs::canonicalize(path).unwrap())
            .to_string_lossy()
            .replace('\\', "/");
        fixture::add_track(&self.mixxx(), &location, title)
    }
    fn request(&self, scope: DjSyncScope, mode: MixxxExistingTracks) -> DjSyncRequest {
        let mut request = super::tests::request(&self.target, scope);
        request.options.mixxx.existing_tracks = mode;
        request
    }
    fn titles(&self) -> Vec<String> {
        let connection = self.mixxx();
        let mut statement = connection
            .prepare("SELECT title FROM library ORDER BY id")
            .unwrap();
        statement
            .query_map([], |row| row.get(0))
            .unwrap()
            .map(Result::unwrap)
            .collect()
    }
    fn database_bytes(&self) -> Vec<u8> {
        fs::read(self.settings.join(cuerithm_dj_sync::mixxx::DATABASE_FILE)).unwrap()
    }
}

#[test]
fn mixxx_folder_and_file_targets_share_one_baseline() {
    let setup = setup();
    let folder = canonical_target(setup.target.clone()).unwrap();
    let mut by_file = setup.target.clone();
    by_file.path = setup.settings.join(cuerithm_dj_sync::mixxx::DATABASE_FILE);
    let file = canonical_target(by_file).unwrap();
    assert_eq!(folder.path, file.path);
    assert!(
        folder
            .path
            .ends_with(cuerithm_dj_sync::mixxx::DATABASE_FILE)
    );
    assert_eq!(
        digest(&folder.identity_target()).unwrap(),
        digest(&file.identity_target()).unwrap()
    );
    // A folder with no library yet is where a sync would create one, and keeps one
    // identity before and after.
    let empty = setup.root.join("no-mixxx-here");
    fs::create_dir(&empty).unwrap();
    let mut future = setup.target.clone();
    future.path = empty.clone();
    let before = canonical_target(future.clone()).unwrap();
    assert_eq!(
        before.path,
        empty.join(cuerithm_dj_sync::mixxx::DATABASE_FILE)
    );
    drop(fixture::create(&before.path, 39));
    assert_eq!(canonical_target(future).unwrap().path, before.path);
    // A file that is not a Mixxx library is still refused.
    let stray = setup.root.join("notes.txt");
    fs::write(&stray, b"x").unwrap();
    let mut other = setup.target.clone();
    other.path = stray;
    assert!(canonical_target(other).is_err());
}

#[test]
fn a_library_is_created_where_mixxx_has_none_and_restore_takes_it_away_again() {
    let setup = setup();
    setup.add_track("b");
    setup.add_playlist("pl", "Warmup", &["a", "b"]);
    // Mixxx was never started: no folder at all, in a place whose parent exists.
    let folder = setup.root.join("never-started/mixxx");
    fs::create_dir(setup.root.join("never-started")).unwrap();
    let mut request = setup.request(DjSyncScope::Full, MixxxExistingTracks::Keep);
    request.target.path = folder.clone();
    let preview = setup.library.preview_dj_sync(request.clone()).unwrap();
    assert!(
        preview
            .warnings
            .iter()
            .any(|w| w.contains("Mixxx has no library here yet")),
        "{:?}",
        preview.warnings
    );
    assert!(!folder.exists(), "a preview creates nothing");
    setup.library.apply_dj_sync(&preview.id).unwrap();

    let database = folder.join(cuerithm_dj_sync::mixxx::DATABASE_FILE);
    let connection = fixture::open(&database);
    let (version, tracks, playlists, scanned): (String, i64, i64, i64) = (
        connection
            .query_row(
                "SELECT value FROM settings WHERE name = 'mixxx.schema.version'",
                [],
                |row| row.get(0),
            )
            .unwrap(),
        connection
            .query_row("SELECT COUNT(*) FROM library", [], |row| row.get(0))
            .unwrap(),
        connection
            .query_row(
                "SELECT COUNT(*) FROM Playlists WHERE hidden = 0",
                [],
                |row| row.get(0),
            )
            .unwrap(),
        connection
            .query_row("SELECT COUNT(*) FROM directories", [], |row| row.get(0))
            .unwrap(),
    );
    assert_eq!(
        (version.as_str(), tracks, playlists),
        ("39", 2, 1),
        "Mixxx's own default playlists stay hidden and are not counted"
    );
    assert!(scanned >= 1, "Mixxx would otherwise ask for a music folder");
    drop(connection);

    // The same sync again has nothing to do.
    let again = setup.library.preview_dj_sync(request).unwrap();
    assert!(again.files.is_empty(), "{:?}", again.files);

    setup.library.restore_dj_sync(&preview.id).unwrap();
    assert!(!database.exists(), "restore removes the library it created");
}

#[test]
fn replacing_the_whole_library_builds_a_new_one_and_restore_brings_the_old_one_back() {
    let setup = setup();
    setup.add_track("b");
    setup.mixxx_knows("a", "Mixxx title for a");
    let foreign = fixture::add_track(&setup.mixxx(), "/elsewhere/other.mp3", "Not in CueRithm");
    fixture::add_playlist(&setup.mixxx(), "Mine in Mixxx", &[foreign]);
    let original = setup.database_bytes();

    let mut request = setup.request(DjSyncScope::Full, MixxxExistingTracks::Keep);
    request.options.mixxx.library = cuerithm_dj_sync::MixxxLibrary::Overwrite;
    let preview = setup.library.preview_dj_sync(request.clone()).unwrap();
    assert!(
        preview
            .warnings
            .iter()
            .any(|w| w.contains("Replacing the whole Mixxx library")
                && w.contains("2 track(s)")
                && w.contains("1 playlist(s)")),
        "{:?}",
        preview.warnings
    );
    setup.library.apply_dj_sync(&preview.id).unwrap();
    let mut titles = setup.titles();
    titles.sort();
    assert_eq!(titles, ["Authored", "Title b"], "only CueRithm's tracks");
    let names: Vec<String> = setup
        .mixxx()
        .prepare("SELECT name FROM Playlists WHERE hidden = 0")
        .unwrap()
        .query_map([], |row| row.get(0))
        .unwrap()
        .map(Result::unwrap)
        .collect();
    assert!(names.is_empty(), "Mixxx's own playlist is gone: {names:?}");

    setup.library.restore_dj_sync(&preview.id).unwrap();
    assert_eq!(setup.database_bytes(), original);

    // A partial scope would leave most of the library out, so it is refused.
    let mut partial = setup.request(DjSyncScope::Changed, MixxxExistingTracks::Keep);
    partial.options.mixxx.library = cuerithm_dj_sync::MixxxLibrary::Overwrite;
    let error = setup
        .library
        .preview_dj_sync(partial)
        .unwrap_err()
        .to_string();
    assert!(error.contains("needs the full library"), "{error}");
}

#[test]
fn add_only_keeps_existing_tracks_while_manage_library_replaces_them() {
    let setup = setup();
    setup.add_track("b");
    setup.mixxx_knows("a", "Mixxx title for a");
    let original = setup.database_bytes();

    // Add only: "b" is new and added; "a" is already in Mixxx and left alone.
    let add = setup
        .library
        .preview_dj_sync(setup.request(DjSyncScope::Full, MixxxExistingTracks::Keep))
        .unwrap();
    assert!(
        add.warnings[0].starts_with("Mixxx: 1 track added; 0 replaced; 1 left unchanged"),
        "{:?}",
        add.warnings
    );
    setup.library.apply_dj_sync(&add.id).unwrap();
    assert_eq!(setup.titles(), ["Mixxx title for a", "Title b"]);

    // Manage library: CueRithm now replaces what "a" holds.
    let manage = setup
        .library
        .preview_dj_sync(setup.request(DjSyncScope::Full, MixxxExistingTracks::Replace))
        .unwrap();
    setup.library.apply_dj_sync(&manage.id).unwrap();
    assert_eq!(setup.titles(), ["Authored", "Title b"]);
    let connection = setup.mixxx();
    let (key, key_id, beats, bpm): (String, i64, String, f64) = connection
        .query_row(
            "SELECT key, key_id, beats_version, bpm FROM library WHERE title = 'Authored'",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .unwrap();
    assert_eq!(
        (key.as_str(), key_id, beats.as_str(), bpm),
        ("Am", 22, "BeatGrid-2.0", 120.0)
    );
    drop(connection);

    // Restoring both syncs returns Mixxx's own library untouched.
    setup.library.restore_dj_sync(&manage.id).unwrap();
    setup.library.restore_dj_sync(&add.id).unwrap();
    assert_eq!(setup.database_bytes(), original);
}

#[test]
fn playlist_scope_adds_only_its_tracks_and_that_playlist() {
    let setup = setup();
    setup.add_track("b");
    setup.add_track("c");
    setup.add_playlist("pl", "Peak Hour", &["b"]);
    let preview = setup
        .library
        .preview_dj_sync(setup.request(
            DjSyncScope::Playlist { id: "pl".into() },
            MixxxExistingTracks::Keep,
        ))
        .unwrap();
    assert_eq!((preview.tracks, preview.playlists), (1, 1));
    setup.library.apply_dj_sync(&preview.id).unwrap();
    assert_eq!(setup.titles(), ["Title b"], "neither a nor c was added");
    let connection = setup.mixxx();
    let names: Vec<String> = connection
        .prepare("SELECT name FROM Playlists")
        .unwrap()
        .query_map([], |row| row.get(0))
        .unwrap()
        .map(Result::unwrap)
        .collect();
    assert_eq!(names, ["Peak Hour"]);
}

#[test]
fn changed_scope_is_empty_after_a_full_sync_and_a_rename_updates_the_playlist_in_place() {
    let setup = setup();
    setup.add_track("b");
    setup.add_playlist("pl", "Warmup", &["a", "b"]);
    let full = setup
        .library
        .preview_dj_sync(setup.request(DjSyncScope::Full, MixxxExistingTracks::Keep))
        .unwrap();
    setup.library.apply_dj_sync(&full.id).unwrap();
    let id_before: i64 = setup
        .mixxx()
        .query_row("SELECT id FROM Playlists", [], |row| row.get(0))
        .unwrap();

    let idle = setup
        .library
        .preview_dj_sync(setup.request(DjSyncScope::Changed, MixxxExistingTracks::Keep))
        .unwrap();
    assert_eq!(idle.tracks, 0);
    assert!(idle.files.is_empty());
    setup.library.discard_dj_sync(&idle.id).unwrap();

    {
        let connection = setup.library.conn().unwrap();
        connection
            .execute(
                "UPDATE playlists SET name = 'Warm-up set' WHERE id = 'pl'",
                [],
            )
            .unwrap();
        Library::bump(&connection).unwrap();
    }
    let renamed = setup
        .library
        .preview_dj_sync(setup.request(DjSyncScope::Changed, MixxxExistingTracks::Keep))
        .unwrap();
    assert!(
        !renamed.files.is_empty(),
        "the rename is a change: {:?}",
        renamed.warnings
    );
    setup.library.apply_dj_sync(&renamed.id).unwrap();
    let (id_after, name): (i64, String) = setup
        .mixxx()
        .query_row("SELECT id, name FROM Playlists", [], |row| {
            Ok((row.get(0)?, row.get(1)?))
        })
        .unwrap();
    assert_eq!((id_after, name.as_str()), (id_before, "Warm-up set"));
    assert_eq!(
        setup
            .mixxx()
            .query_row("SELECT COUNT(*) FROM Playlists", [], |r| r.get::<_, i64>(0))
            .unwrap(),
        1
    );
}

#[test]
fn a_running_mixxx_blocks_the_service_preview_with_a_clear_reason() {
    let setup = setup();
    cuerithm_dj_sync::discovery::pretend_target_running(true);
    let error = setup
        .library
        .preview_dj_sync(setup.request(DjSyncScope::Full, MixxxExistingTracks::Keep))
        .unwrap_err()
        .to_string();
    assert!(error.contains("Close the target DJ software"), "{error}");
}
