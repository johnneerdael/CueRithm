//! Making a Mixxx library from nothing: for a Mixxx that has not been started yet, and
//! for a sync that replaces a whole library.
//!
//! The layout is `schema39.sql`, what Mixxx 2.4 and 2.5 create for a new profile. Mixxx
//! finds the schema at its current version and has nothing to migrate; a newer Mixxx
//! upgrades it (39 to 40) as it would any older library.
use anyhow::{Context, Result, ensure};
use rusqlite::Connection;
use std::path::Path;

const SCHEMA: &str = include_str!("schema39.sql");

/// The schema version of the library this makes.
pub(super) const VERSION: u32 = 39;

/// Create the library at `path`, which must not exist: Mixxx's tables, the schema
/// settings its migrations write, and the two playlists they insert (Auto DJ and the
/// placeholder Mixxx keeps its history playlists under). Mixxx adds everything else,
/// such as today's history playlist, when it starts.
pub(super) fn create(path: &Path) -> Result<()> {
    ensure!(
        !path.exists(),
        "A Mixxx library already exists at that place"
    );
    let connection = Connection::open(path).context("Cannot create the Mixxx library")?;
    connection
        .execute_batch(SCHEMA)
        .context("Cannot create the Mixxx tables")?;
    connection
        .execute_batch(&format!(
            "INSERT INTO settings(name, value) VALUES
                 ('mixxx.schema.version', '{VERSION}'),
                 ('mixxx.schema.min_compatible_version', '3'),
                 ('mixxx.schema.last_used_version', '{VERSION}');
             INSERT INTO Playlists(id, name, position, hidden, date_created, date_modified, locked)
             VALUES (1, 'Auto DJ', 1, 1, datetime('now'), datetime('now'), 0),
                    (2, 'historyPlaceholder', 2, -1, datetime('now'), datetime('now'), 1);"
        ))
        .context("Cannot write the Mixxx schema settings")?;
    let check: String = connection.query_row("PRAGMA quick_check", [], |row| row.get(0))?;
    ensure!(
        check == "ok",
        "The new Mixxx library failed its integrity check: {check}"
    );
    Ok(())
}

/// What replacing `library` (a copy of the existing one) throws away, for the preview.
pub(super) fn describe_replaced(library: &Connection) -> Result<String> {
    let count = |sql: &str| -> Result<i64> { Ok(library.query_row(sql, [], |row| row.get(0))?) };
    let tracks = count("SELECT COUNT(*) FROM library WHERE COALESCE(mixxx_deleted, 0) = 0")?;
    let playlists = count("SELECT COUNT(*) FROM Playlists WHERE hidden = 0")?;
    let crates = count("SELECT COUNT(*) FROM crates")?;
    let history = count("SELECT COUNT(*) FROM Playlists WHERE hidden = 2")?;
    let plays = count("SELECT COALESCE(SUM(timesplayed), 0) FROM library")?;
    Ok(format!(
        "Replacing the whole Mixxx library: its {tracks} track(s), {playlists} playlist(s), \
         {crates} crate(s), {history} history playlist(s) and {plays} play count(s) are \
         replaced by what is in this sync. Restore backup in the sync history brings them back."
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mixxx::schema;

    #[test]
    fn a_created_library_is_one_mixxx_would_find_current_and_the_adapter_accepts() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("mixxxdb.sqlite");
        create(&path).unwrap();
        let connection = Connection::open(&path).unwrap();
        let layout = schema::inspect(&connection).unwrap();
        assert_eq!(layout.version, VERSION);
        let settings: Vec<(String, String)> = connection
            .prepare("SELECT name, value FROM settings ORDER BY name")
            .unwrap()
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
            .unwrap()
            .collect::<rusqlite::Result<_>>()
            .unwrap();
        assert_eq!(
            settings,
            vec![
                ("mixxx.schema.last_used_version".to_owned(), "39".to_owned()),
                (
                    "mixxx.schema.min_compatible_version".to_owned(),
                    "3".to_owned()
                ),
                ("mixxx.schema.version".to_owned(), "39".to_owned()),
            ]
        );
        let playlists: Vec<(i64, String, i64, i64)> = connection
            .prepare("SELECT id, name, hidden, locked FROM Playlists ORDER BY id")
            .unwrap()
            .query_map([], |row| {
                Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
            })
            .unwrap()
            .collect::<rusqlite::Result<_>>()
            .unwrap();
        assert_eq!(
            playlists,
            vec![
                (1, "Auto DJ".to_owned(), 1, 0),
                (2, "historyPlaceholder".to_owned(), -1, 1)
            ]
        );
        // Every table of Mixxx's schema 39, not only the ones CueRithm writes.
        let tables: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name NOT LIKE 'sqlite_%'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(tables, 26);
    }

    #[test]
    fn creating_over_an_existing_file_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("mixxxdb.sqlite");
        std::fs::write(&path, b"mine").unwrap();
        assert!(create(&path).is_err());
        assert_eq!(std::fs::read(&path).unwrap(), b"mine");
    }
}
