//! Keeping a synced Mixxx track attached to its file when CueRithm's Organize flow
//! moves or renames it. Only the `track_locations` row changes: tracks, cues,
//! playlists and crates refer to that row's id, so they follow the file.
use super::tracks::{location_key, split_location};
use anyhow::{Context, Result, bail, ensure};
use rusqlite::{Connection, OpenFlags, params};
use std::path::Path;

/// Rows for a location: the exact text, else, where the file system folds case, the
/// same text ignoring ASCII case (SQLite's `lower` folds only ASCII).
fn find(connection: &Connection, key: &str) -> Result<Vec<(i64, String)>> {
    let read = |sql: &str| -> Result<Vec<(i64, String)>> {
        let mut statement = connection.prepare(sql)?;
        let rows = statement
            .query_map([key], |row| Ok((row.get(0)?, row.get(1)?)))?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(rows)
    };
    let exact = read("SELECT id, location FROM track_locations WHERE location = ?1")?;
    if !exact.is_empty() {
        return Ok(exact);
    }
    // SQLite's lower() folds only ASCII, so `\u{c9}` and `\u{e9}` would never meet: fold in
    // Rust, with the rules the track index uses. A row spelled differently is only this
    // file's when the file system says so. This reads every location, which only happens
    // when no row has the exact spelling, on a system that folds case.
    let folded = key.to_lowercase();
    let same = super::tracks::file_check();
    let mut statement = connection.prepare("SELECT id, location FROM track_locations")?;
    let rows = statement
        .query_map([], |row| {
            Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows
        .into_iter()
        .filter(|(_, location)| location.to_lowercase() == folded && same(key, location))
        .collect())
}

fn key_of(path: &Path) -> Result<String> {
    location_key(
        &std::fs::canonicalize(path)
            .with_context(|| format!("Cannot resolve {}", path.display()))?,
    )
}

/// The Mixxx identity of the track stored for `source`: its `track_locations.id`.
/// It does not change when the location is repaired.
pub(crate) fn reference_id(database: &Path, source: &Path) -> Result<String> {
    let connection = Connection::open_with_flags(database, OpenFlags::SQLITE_OPEN_READ_ONLY)
        .context("Cannot open the Mixxx library")?;
    match find(&connection, &key_of(source)?)?.as_slice() {
        [(id, _)] => Ok(id.to_string()),
        [] => bail!("Mixxx has no track for {}", source.display()),
        many => bail!(
            "Mixxx library holds {} rows for {}; remove the duplicates in Mixxx first",
            many.len(),
            source.display()
        ),
    }
}

/// Point the row identified by `expected_id` at `to`. Both files exist when this runs
/// (the copy was verified and the original is not yet removed), so both are resolved
/// the way Mixxx would record them.
pub(crate) fn repair_reference(
    database: &Path,
    from: &Path,
    to: &Path,
    expected_id: &str,
) -> Result<()> {
    let mut connection =
        Connection::open(database).context("Cannot open the staged Mixxx library")?;
    // Mixxx runs with foreign keys off, and its own schema could not be written with
    // them on: `library.location` references `track_locations(location)` but holds an
    // id, and several tables reference a `library_old` that no longer exists. The
    // SQLite bundled here turns them on by default.
    connection.pragma_update(None, "foreign_keys", false)?;
    let transaction = connection.transaction()?;
    let from_key = key_of(from)?;
    let to_key = key_of(to)?;
    let current = find(&transaction, &from_key)?;
    ensure!(
        matches!(current.as_slice(), [(id, _)] if id.to_string() == expected_id),
        "The Mixxx track for {} is no longer the one inspected",
        from.display()
    );
    let id: i64 = expected_id.parse()?;
    // A case-only rename on a case-folding system finds the track's own row.
    ensure!(
        find(&transaction, &to_key)?
            .iter()
            .all(|(other, _)| *other == id),
        "Mixxx already has a different track at {}",
        to.display()
    );
    let (directory, filename) = split_location(&to_key);
    let size = std::fs::metadata(to)?.len();
    transaction.execute(
        "UPDATE track_locations
         SET location = ?1, directory = ?2, filename = ?3, filesize = ?4,
             fs_deleted = 0, needs_verification = 0
         WHERE id = ?5",
        params![to_key, directory, filename, size as i64, id],
    )?;
    transaction.commit()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        DjSoftware, SourcePathChange, SyncTarget, apply_path_repair, inspect_path_reference,
        mixxx::fixture, prepare_path_repair, transaction::file_hash,
    };
    use std::path::PathBuf;

    struct Env {
        root: PathBuf,
        _dir: tempfile::TempDir,
        database: PathBuf,
    }
    fn env() -> Env {
        crate::discovery::pretend_target_running(false);
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().canonicalize().unwrap();
        let database = root.join("settings/mixxxdb.sqlite");
        std::fs::create_dir_all(database.parent().unwrap()).unwrap();
        drop(fixture::create(&database, 39));
        Env {
            root,
            _dir: dir,
            database,
        }
    }
    impl Env {
        fn audio(&self, name: &str) -> PathBuf {
            let path = self.root.join(name);
            std::fs::write(&path, b"invented fixture audio").unwrap();
            path
        }
        fn connection(&self) -> Connection {
            fixture::open(&self.database)
        }
        fn track(&self, path: &Path, title: &str) -> i64 {
            fixture::add_track(&self.connection(), &key_of(path).unwrap(), title)
        }
        fn target(&self) -> SyncTarget {
            SyncTarget {
                software: DjSoftware::Mixxx,
                environment: Default::default(),
                path: self.root.join("settings"),
                source_root: None,
            }
        }
    }

    #[test]
    fn reference_id_finds_the_track_and_rejects_unknown_or_ambiguous() {
        let env = env();
        let known = env.audio("known.wav");
        let other = env.audio("other.wav");
        env.track(&known, "Known");
        let id = reference_id(&env.database, &known).unwrap();
        assert_eq!(id, "1");
        assert!(
            reference_id(&env.database, &other)
                .unwrap_err()
                .to_string()
                .contains("no track")
        );
        // Two rows that differ only by case, both the same file, are ambiguous.
        {
            let _same = crate::mixxx::tracks::pretend_same_file(|_, _| true);
            let key = key_of(&other).unwrap();
            fixture::add_track(&env.connection(), &key.to_uppercase(), "Upper");
            fixture::add_track(&env.connection(), &key.replace("other", "OTHER"), "Mixed");
            assert!(
                reference_id(&env.database, &other)
                    .unwrap_err()
                    .to_string()
                    .contains("duplicates")
            );
        }
    }

    #[test]
    fn a_row_that_differs_only_by_non_ascii_case_is_found_where_case_folds() {
        // A case-only rename of a file with an accented capital: SQLite's lower() cannot
        // fold it, so the match has to be made in Rust.
        let _same = crate::mixxx::tracks::pretend_same_file(|_, _| true);
        let env = env();
        let source = env.audio("caf\u{e9}.mp3");
        let key = key_of(&source).unwrap();
        let stored = key.replace("caf\u{e9}", "CAF\u{c9}");
        assert_ne!(stored, key);
        let id = fixture::add_track(&env.connection(), &stored, "Cafe");
        assert_eq!(
            reference_id(&env.database, &source).unwrap(),
            id.to_string()
        );
    }

    #[test]
    fn a_case_variant_row_is_another_track_when_the_file_system_says_so() {
        // On a case-sensitive volume the differently cased row is a different file.
        let _different = crate::mixxx::tracks::pretend_same_file(|_, _| false);
        let env = env();
        let source = env.audio("Song.mp3");
        let key = key_of(&source).unwrap();
        fixture::add_track(&env.connection(), &key.to_uppercase(), "Other file");
        assert!(
            reference_id(&env.database, &source)
                .unwrap_err()
                .to_string()
                .contains("no track")
        );
    }

    #[test]
    fn repair_moves_the_location_row_and_keeps_library_data() {
        let env = env();
        let old = env.audio("old.wav");
        let new = env.audio("new name.wav");
        let id = env.track(&old, "Kept title");
        {
            let connection = env.connection();
            connection
                .execute(
                    "UPDATE library SET key='Am', key_id=22, bpm=128.0, beats_version='BeatGrid-2.0', beats=x'0a'",
                    [],
                )
                .unwrap();
            connection
                .execute(
                    "INSERT INTO cues(track_id, type, position, length, hotcue, label, color) VALUES(?1, 1, 10, 0, 0, 'Drop', 255)",
                    [id],
                )
                .unwrap();
            fixture::add_playlist(&connection, "Set", &[id]);
        }
        let library_before = fixture::dump(&env.connection(), "SELECT * FROM library");
        let cues_before = fixture::dump(&env.connection(), "SELECT * FROM cues");
        let tracks_before = fixture::dump(&env.connection(), "SELECT * FROM PlaylistTracks");
        repair_reference(&env.database, &old, &new, &id.to_string()).unwrap();
        let connection = env.connection();
        let (location, filename, directory, size): (String, String, String, i64) = connection
            .query_row(
                "SELECT location, filename, directory, filesize FROM track_locations",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .unwrap();
        assert_eq!(location, key_of(&new).unwrap());
        assert_eq!(filename, "new name.wav");
        assert_eq!(directory, key_of(&new).unwrap().rsplit_once('/').unwrap().0);
        assert_eq!(size, 22);
        assert_eq!(
            fixture::dump(&connection, "SELECT * FROM library"),
            library_before
        );
        assert_eq!(
            fixture::dump(&connection, "SELECT * FROM cues"),
            cues_before
        );
        assert_eq!(
            fixture::dump(&connection, "SELECT * FROM PlaylistTracks"),
            tracks_before
        );
        assert_eq!(reference_id(&env.database, &new).unwrap(), id.to_string());
    }

    #[test]
    fn repair_rejects_a_changed_track_or_a_colliding_destination() {
        let env = env();
        let old = env.audio("old.wav");
        let new = env.audio("new.wav");
        let taken = env.audio("taken.wav");
        let id = env.track(&old, "Mine");
        env.track(&taken, "Someone else's");
        let error = repair_reference(&env.database, &old, &taken, &id.to_string())
            .unwrap_err()
            .to_string();
        assert!(error.contains("already has a different track"), "{error}");
        let error = repair_reference(&env.database, &old, &new, "999")
            .unwrap_err()
            .to_string();
        assert!(error.contains("no longer the one inspected"), "{error}");
        let error = repair_reference(&env.database, &new, &old, &id.to_string())
            .unwrap_err()
            .to_string();
        assert!(
            error.contains("no longer the one inspected") || error.contains("no track"),
            "{error}"
        );
        // Nothing was written by the refusals.
        assert_eq!(reference_id(&env.database, &old).unwrap(), id.to_string());
    }

    #[test]
    fn organize_can_move_a_track_synced_to_mixxx() {
        let env = env();
        let old = env.audio("old.wav");
        let new = env.root.join("new.wav");
        std::fs::copy(&old, &new).unwrap();
        let id = env.track(&old, "Synced");
        let target = env.target();
        let snapshot = inspect_path_reference(&target, &old).unwrap();
        assert_eq!(snapshot.external_id, id.to_string());
        let change = SourcePathChange {
            track_id: "t".into(),
            from: old.clone(),
            to: new.clone(),
            source_sha256: file_hash(&old).unwrap().unwrap(),
            approved_from_sha256: None,
        };
        let journals = env.root.join("journals");
        let original = std::fs::read(&env.database).unwrap();
        let plan = prepare_path_repair(&journals, &snapshot, &change).unwrap();
        assert_eq!(
            plan.after_external_id,
            id.to_string(),
            "identity is preserved"
        );
        assert_eq!(
            std::fs::read(&env.database).unwrap(),
            original,
            "preview writes nothing"
        );
        apply_path_repair(&journals, &plan.id).unwrap();
        assert_eq!(reference_id(&env.database, &new).unwrap(), id.to_string());
        assert!(reference_id(&env.database, &old).is_err());
        assert!(
            old.is_file() && new.is_file(),
            "the audio files are not touched"
        );
    }

    #[test]
    fn organize_refuses_a_running_mixxx_a_wine_target_and_a_stale_library() {
        let env = env();
        let old = env.audio("old.wav");
        env.track(&old, "Synced");
        crate::discovery::pretend_target_running(true);
        assert!(
            inspect_path_reference(&env.target(), &old)
                .unwrap_err()
                .to_string()
                .contains("Close the target DJ software")
        );
        crate::discovery::pretend_target_running(false);
        let mut wine = env.target();
        wine.environment = cuerithm_rekordbox::TargetEnvironment::WindowsCompatibility {
            prefix: env.root.clone(),
            installation: None,
            mappings: vec![cuerithm_rekordbox::DriveMapping {
                windows_root: "C:\\".into(),
                host_root: env.root.clone(),
            }],
        };
        assert!(
            inspect_path_reference(&wine, &old)
                .unwrap_err()
                .to_string()
                .contains("native")
        );
        // An edit after inspection makes the snapshot stale.
        let snapshot = inspect_path_reference(&env.target(), &old).unwrap();
        env.connection()
            .execute("INSERT INTO settings(name, value) VALUES('x','y')", [])
            .unwrap();
        let new = env.audio("new.wav");
        let change = SourcePathChange {
            track_id: "t".into(),
            from: old.clone(),
            to: new,
            source_sha256: file_hash(&old).unwrap().unwrap(),
            approved_from_sha256: None,
        };
        assert!(prepare_path_repair(&env.root.join("journals"), &snapshot, &change).is_err());
    }
}
