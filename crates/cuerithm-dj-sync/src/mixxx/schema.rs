//! Which Mixxx databases the adapter may write. The library's own schema version
//! decides, with the compatibility marker Mixxx stores beside it and the shape of the
//! tables the writer fills, so a Mixxx released after this code keeps working as long
//! as it does not change the format.
use anyhow::{Context, Result, bail, ensure};
use rusqlite::{Connection, OptionalExtension};
use std::{collections::BTreeSet, path::Path};

/// Released Mixxx 2.4 and 2.5 use schema 39. Nothing older is written: it predates the
/// beat, key and cue formats this adapter writes.
pub(super) const OLDEST: u32 = 39;
/// The newest schema this adapter has been checked against (the 2.6/2.7 development
/// line). A newer library is accepted when it passes the tests in `inspect`.
pub(super) const NEWEST_CHECKED: u32 = 40;

/// Columns the writer names, per table. A database missing any of them is not one
/// this adapter understands, whatever its version number says.
const REQUIRED: &[(&str, &[&str])] = &[
    ("settings", &["name", "value"]),
    (
        "track_locations",
        &[
            "id",
            "location",
            "filename",
            "directory",
            "filesize",
            "fs_deleted",
            "needs_verification",
        ],
    ),
    (
        "library",
        &[
            "id",
            "artist",
            "title",
            "album",
            "album_artist",
            "year",
            "genre",
            "composer",
            "grouping",
            "tracknumber",
            "filetype",
            "location",
            "color",
            "comment",
            "url",
            "rating",
            "key",
            "key_id",
            "cuepoint",
            "bpm",
            "replaygain",
            "replaygain_peak",
            "timesplayed",
            "played",
            "mixxx_deleted",
            "header_parsed",
            "source_synchronized_ms",
            "channels",
            "samplerate",
            "bitrate",
            "duration",
            "beats_version",
            "beats_sub_version",
            "beats",
            "bpm_lock",
            "keys_version",
            "keys_sub_version",
            "keys",
            "coverart_source",
            "coverart_type",
            "coverart_location",
            "coverart_hash",
        ],
    ),
    (
        "cues",
        &[
            "id", "track_id", "type", "position", "length", "hotcue", "label", "color",
        ],
    ),
    (
        "Playlists",
        &[
            "id",
            "name",
            "position",
            "hidden",
            "date_created",
            "date_modified",
            "locked",
        ],
    ),
    (
        "PlaylistTracks",
        &[
            "id",
            "playlist_id",
            "track_id",
            "position",
            "pl_datetime_added",
        ],
    ),
    (
        "crates",
        &["id", "name", "count", "show", "locked", "autodj_source"],
    ),
    ("crate_tracks", &["crate_id", "track_id"]),
    ("directories", &["directory"]),
];

/// What `inspect` learned about a supported database.
#[derive(Debug, Clone)]
pub(super) struct Layout {
    pub version: u32,
    /// The oldest schema Mixxx says can still use this library.
    pub min_compatible: u32,
    library: BTreeSet<String>,
}
impl Layout {
    /// For a library newer than anything this adapter was checked against: why the sync
    /// goes ahead anyway.
    pub(super) fn advisory(&self) -> Option<String> {
        (self.version > NEWEST_CHECKED).then(|| {
            format!(
                "This Mixxx library is schema {}, newer than the newest CueRithm has been \
                 checked against ({NEWEST_CHECKED}). Mixxx marks it as usable by software \
                 that knows schema {}, and its tables have the layout CueRithm writes, so the \
                 sync goes ahead. Look at the result in Mixxx.",
                self.version, self.min_compatible
            )
        })
    }

    /// Whether `library` has this column; optional columns such as the schema-40
    /// `tuning_frequency_hz` are written only when present.
    pub(super) fn has_library_column(&self, column: &str) -> bool {
        self.library.contains(column)
    }
}

fn columns(connection: &Connection, table: &str) -> Result<BTreeSet<String>> {
    let mut statement = connection.prepare(&format!("PRAGMA table_info(\"{table}\")"))?;
    let names = statement
        .query_map([], |row| row.get::<_, String>(1))?
        .collect::<rusqlite::Result<BTreeSet<_>>>()?;
    Ok(names)
}

/// Accept a library the writer understands: schema 39 or newer, with every table and
/// column it names, and nothing it would have to fill but does not know about.
///
/// A library newer than `NEWEST_CHECKED` is accepted on Mixxx's own rule. Mixxx keeps
/// `mixxx.schema.min_compatible_version` beside the schema version, the oldest software
/// that can still use the library, and refuses to open a newer library from software
/// below it (`SchemaManager::upgradeToSchemaVersion`). Software that knows schema 40 may
/// therefore write a schema 41 library that still says 40 or less. A release that does
/// change the format raises that marker, or adds a required column or drops one the
/// writer names, and is refused here.
pub(super) fn inspect(connection: &Connection) -> Result<Layout> {
    let settings = columns(connection, "settings")?;
    ensure!(
        !settings.is_empty(),
        "This is not a Mixxx library: it has no settings table"
    );
    let setting = |name: &str| -> Result<Option<String>> {
        Ok(connection
            .query_row(
                "SELECT value FROM settings WHERE name = ?1",
                [name],
                |row| row.get(0),
            )
            .optional()?)
    };
    let stored = setting("mixxx.schema.version")?.context(
        "This Mixxx library has no schema version; start Mixxx once so it can set it up",
    )?;
    let version: u32 = stored
        .trim()
        .parse()
        .with_context(|| format!("Unreadable Mixxx schema version {stored:?}"))?;
    if version < OLDEST {
        bail!(
            "Mixxx library schema {version} is older than the oldest CueRithm writes ({OLDEST}); \
             open and quit Mixxx 2.4 or later once to upgrade it, then sync again"
        );
    }
    // Mixxx's own fallback: a library that does not say is compatible only with itself.
    let min_compatible = setting("mixxx.schema.min_compatible_version")?
        .and_then(|value| value.trim().parse().ok())
        .unwrap_or(version);
    if version > NEWEST_CHECKED && min_compatible > NEWEST_CHECKED {
        bail!(
            "Mixxx library schema {version} needs software that knows schema {min_compatible}, \
             beyond the {NEWEST_CHECKED} CueRithm has been checked against; update CueRithm \
             before syncing to this Mixxx"
        );
    }
    let mut library = BTreeSet::new();
    for (table, required) in REQUIRED {
        let present = columns(connection, table)?;
        for column in *required {
            ensure!(
                present.contains(*column),
                "Mixxx library is missing {table}.{column}; it does not match schema {version}"
            );
        }
        if let Some(column) = unfilled_column(connection, table, required)? {
            bail!(
                "Mixxx library schema {version} has a column CueRithm does not know how to \
                 fill ({table}.{column}); update CueRithm before syncing to this Mixxx"
            );
        }
        if *table == "library" {
            library = present;
        }
    }
    Ok(Layout {
        version,
        min_compatible,
        library,
    })
}

/// A column an insert must give a value: `NOT NULL`, no default, not the row id, and not
/// one the writer names. A newer Mixxx that adds one has changed what a row needs.
fn unfilled_column(connection: &Connection, table: &str, known: &[&str]) -> Result<Option<String>> {
    let mut statement = connection.prepare(&format!("PRAGMA table_info(\"{table}\")"))?;
    let rows = statement.query_map([], |row| {
        Ok((
            row.get::<_, String>(1)?,
            row.get::<_, i64>(3)? != 0,
            row.get::<_, Option<String>>(4)?,
            row.get::<_, i64>(5)? != 0,
        ))
    })?;
    for row in rows {
        let (name, not_null, default, primary_key) = row?;
        if not_null && default.is_none() && !primary_key && !known.contains(&name.as_str()) {
            return Ok(Some(name));
        }
    }
    Ok(None)
}

/// A crashed Mixxx can leave a rollback journal or write-ahead log beside the
/// database. Reading around it would show stale data, and replacing the file would
/// strand it, so the sync stops until Mixxx has recovered.
pub(super) fn ensure_closed_cleanly(database: &Path) -> Result<()> {
    for suffix in ["-journal", "-wal"] {
        let mut name = database.as_os_str().to_owned();
        name.push(suffix);
        let sidecar = std::path::PathBuf::from(name);
        let leftover = match std::fs::metadata(&sidecar) {
            Ok(metadata) => metadata.len() > 0,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => false,
            Err(error) => return Err(error).context("Cannot inspect the Mixxx library folder"),
        };
        ensure!(
            !leftover,
            "Mixxx did not close cleanly: a {suffix} file is left beside {}. \
             Open Mixxx and quit it normally, then sync again",
            database
                .file_name()
                .map_or_else(|| "its library".into(), |name| name.to_string_lossy())
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mixxx::fixture;

    #[test]
    fn accepts_schema_39_and_40() {
        let directory = tempfile::tempdir().unwrap();
        for version in [39, 40] {
            let connection =
                fixture::create(&directory.path().join(format!("{version}.sqlite")), version);
            assert_eq!(inspect(&connection).unwrap().version, version);
        }
    }

    #[test]
    fn rejects_38_with_the_oldest_supported_in_the_message() {
        let directory = tempfile::tempdir().unwrap();
        let old = inspect(&fixture::create(&directory.path().join("38.sqlite"), 38))
            .unwrap_err()
            .to_string();
        assert!(
            old.contains("38") && old.contains("(39)") && old.contains("upgrade"),
            "{old}"
        );
    }

    fn set(connection: &Connection, name: &str, value: &str) {
        connection
            .execute(
                "INSERT INTO settings(name, value) VALUES(?1, ?2)
                 ON CONFLICT(name) DO UPDATE SET value = excluded.value",
                [name, value],
            )
            .unwrap();
    }

    #[test]
    fn a_newer_schema_is_accepted_when_mixxx_says_old_software_can_use_it() {
        let directory = tempfile::tempdir().unwrap();
        let connection = fixture::create(&directory.path().join("45.sqlite"), 45);
        // `create` records what Mixxx records today: usable by software from schema 3.
        let layout = inspect(&connection).unwrap();
        assert_eq!((layout.version, layout.min_compatible), (45, 3));
        let advisory = layout.advisory().unwrap();
        assert!(
            advisory.contains("schema 45") && advisory.contains("(40)"),
            "{advisory}"
        );
        // Anything Mixxx marks as needing up to schema 40 is as acceptable.
        set(&connection, "mixxx.schema.min_compatible_version", "40");
        assert!(inspect(&connection).is_ok());
        // The versions checked get no advisory.
        let connection = fixture::create(&directory.path().join("40.sqlite"), 40);
        assert!(inspect(&connection).unwrap().advisory().is_none());
    }

    #[test]
    fn a_newer_schema_that_mixxx_marks_as_needing_newer_software_is_refused() {
        let directory = tempfile::tempdir().unwrap();
        let connection = fixture::create(&directory.path().join("45.sqlite"), 45);
        set(&connection, "mixxx.schema.min_compatible_version", "41");
        let error = inspect(&connection).unwrap_err().to_string();
        assert!(
            error.contains("45") && error.contains("41") && error.contains("update CueRithm"),
            "{error}"
        );
        // A library that says nothing is compatible only with itself, as in Mixxx.
        connection
            .execute(
                "DELETE FROM settings WHERE name = 'mixxx.schema.min_compatible_version'",
                [],
            )
            .unwrap();
        assert!(inspect(&connection).is_err());
        set(&connection, "mixxx.schema.version", "40");
        assert!(inspect(&connection).is_ok(), "40 is what was checked");
    }

    #[test]
    fn a_column_a_newer_mixxx_requires_is_refused_but_one_with_a_default_is_not() {
        let directory = tempfile::tempdir().unwrap();
        let connection = fixture::create(&directory.path().join("a.sqlite"), 45);
        connection
            .execute(
                "ALTER TABLE library ADD COLUMN mood TEXT NOT NULL DEFAULT 'calm'",
                [],
            )
            .unwrap();
        assert!(inspect(&connection).is_ok(), "a default fills it");
        connection.execute("DROP TABLE crate_tracks", []).unwrap();
        connection
            .execute(
                "CREATE TABLE crate_tracks (crate_id INTEGER NOT NULL, track_id INTEGER NOT NULL,
                 rank INTEGER NOT NULL)",
                [],
            )
            .unwrap();
        let error = inspect(&connection).unwrap_err().to_string();
        assert!(
            error.contains("crate_tracks.rank") && error.contains("update CueRithm"),
            "{error}"
        );
    }

    #[test]
    fn rejects_missing_or_unreadable_version() {
        let directory = tempfile::tempdir().unwrap();
        let connection = fixture::create(&directory.path().join("a.sqlite"), 39);
        connection
            .execute(
                "DELETE FROM settings WHERE name = 'mixxx.schema.version'",
                [],
            )
            .unwrap();
        assert!(
            inspect(&connection)
                .unwrap_err()
                .to_string()
                .contains("no schema version")
        );
        connection
            .execute(
                "INSERT INTO settings(name, value) VALUES('mixxx.schema.version', 'forty')",
                [],
            )
            .unwrap();
        assert!(
            inspect(&connection)
                .unwrap_err()
                .to_string()
                .contains("Unreadable")
        );
        let foreign = Connection::open(directory.path().join("other.sqlite")).unwrap();
        assert!(
            inspect(&foreign)
                .unwrap_err()
                .to_string()
                .contains("not a Mixxx library")
        );
    }

    #[test]
    fn rejects_missing_required_column_by_name() {
        let directory = tempfile::tempdir().unwrap();
        let connection = fixture::create(&directory.path().join("a.sqlite"), 39);
        connection
            .execute("ALTER TABLE cues DROP COLUMN color", [])
            .unwrap();
        let error = inspect(&connection).unwrap_err().to_string();
        assert!(error.contains("cues.color"), "{error}");
        let connection = fixture::create(&directory.path().join("b.sqlite"), 40);
        connection
            .execute("ALTER TABLE library DROP COLUMN bpm_lock", [])
            .unwrap();
        assert!(
            inspect(&connection)
                .unwrap_err()
                .to_string()
                .contains("library.bpm_lock")
        );
        let connection = fixture::create(&directory.path().join("c.sqlite"), 40);
        connection.execute("DROP TABLE crate_tracks", []).unwrap();
        assert!(
            inspect(&connection)
                .unwrap_err()
                .to_string()
                .contains("crate_tracks.")
        );
    }

    #[test]
    fn schema_40_column_is_optional_and_reported() {
        let directory = tempfile::tempdir().unwrap();
        let v39 = inspect(&fixture::create(&directory.path().join("39.sqlite"), 39)).unwrap();
        let v40 = inspect(&fixture::create(&directory.path().join("40.sqlite"), 40)).unwrap();
        assert!(!v39.has_library_column("tuning_frequency_hz"));
        assert!(v40.has_library_column("tuning_frequency_hz"));
        assert!(v39.has_library_column("beats") && v40.has_library_column("beats"));
    }

    #[test]
    fn rejects_hot_journal_or_wal() {
        let directory = tempfile::tempdir().unwrap();
        let database = directory.path().join("mixxxdb.sqlite");
        std::fs::write(&database, b"x").unwrap();
        ensure_closed_cleanly(&database).unwrap();
        // An empty sidecar is what a clean close leaves behind on some systems.
        std::fs::write(directory.path().join("mixxxdb.sqlite-journal"), b"").unwrap();
        ensure_closed_cleanly(&database).unwrap();
        std::fs::write(directory.path().join("mixxxdb.sqlite-journal"), b"pending").unwrap();
        let error = ensure_closed_cleanly(&database).unwrap_err().to_string();
        assert!(
            error.contains("did not close cleanly") && error.contains("-journal"),
            "{error}"
        );
        std::fs::remove_file(directory.path().join("mixxxdb.sqlite-journal")).unwrap();
        std::fs::write(directory.path().join("mixxxdb.sqlite-wal"), b"pending").unwrap();
        assert!(
            ensure_closed_cleanly(&database)
                .unwrap_err()
                .to_string()
                .contains("-wal")
        );
    }
}
