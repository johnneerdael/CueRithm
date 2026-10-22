//! Which Mixxx databases the adapter may write, decided from the stored schema
//! version and from the tables and columns it actually needs.
use anyhow::{Context, Result, bail, ensure};
use rusqlite::{Connection, OptionalExtension};
use std::{collections::BTreeSet, path::Path};

/// Released Mixxx 2.4 and 2.5 use schema 39; the 2.6/2.7 development line uses 40.
pub(super) const OLDEST: u32 = 39;
pub(super) const NEWEST: u32 = 40;

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
];

/// What `inspect` learned about a supported database.
#[derive(Debug, Clone)]
pub(super) struct Layout {
    pub version: u32,
    library: BTreeSet<String>,
}
impl Layout {
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

/// Refuse anything but schema 39 or 40 with every required table and column.
pub(super) fn inspect(connection: &Connection) -> Result<Layout> {
    let settings = columns(connection, "settings")?;
    ensure!(
        !settings.is_empty(),
        "This is not a Mixxx library: it has no settings table"
    );
    let stored: Option<String> = connection
        .query_row(
            "SELECT value FROM settings WHERE name = 'mixxx.schema_version'",
            [],
            |row| row.get(0),
        )
        .optional()?;
    let stored = stored.context(
        "This Mixxx library has no schema version; start Mixxx once so it can set it up",
    )?;
    let version: u32 = stored
        .trim()
        .parse()
        .with_context(|| format!("Unreadable Mixxx schema version {stored:?}"))?;
    if version < OLDEST {
        bail!(
            "Mixxx library schema {version} is older than the supported {OLDEST}-{NEWEST}; \
             open and quit Mixxx 2.4 or later once to upgrade it, then sync again"
        );
    }
    if version > NEWEST {
        bail!(
            "Mixxx library schema {version} is newer than the supported {OLDEST}-{NEWEST}; \
             update CueRithm before syncing to this Mixxx"
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
        if *table == "library" {
            library = present;
        }
    }
    Ok(Layout { version, library })
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
    fn rejects_38_and_41_with_window_in_message() {
        let directory = tempfile::tempdir().unwrap();
        let old = inspect(&fixture::create(&directory.path().join("38.sqlite"), 38))
            .unwrap_err()
            .to_string();
        assert!(
            old.contains("38") && old.contains("39-40") && old.contains("upgrade"),
            "{old}"
        );
        let new = inspect(&fixture::create(&directory.path().join("41.sqlite"), 41))
            .unwrap_err()
            .to_string();
        assert!(
            new.contains("41") && new.contains("39-40") && new.contains("update CueRithm"),
            "{new}"
        );
    }

    #[test]
    fn rejects_missing_or_unreadable_version() {
        let directory = tempfile::tempdir().unwrap();
        let connection = fixture::create(&directory.path().join("a.sqlite"), 39);
        connection
            .execute(
                "DELETE FROM settings WHERE name = 'mixxx.schema_version'",
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
                "INSERT INTO settings(name, value) VALUES('mixxx.schema_version', 'forty')",
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
