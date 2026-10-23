//! Mixxx-shaped databases for tests. `create` builds them from the table definitions
//! Mixxx 2.5.6 itself wrote (`schema39.sql`), not from an authored approximation: a first
//! version of these fixtures was written from the observed contract and hid three
//! defects that only a real library showed (the schema version key, and the foreign
//! keys Mixxx's schema declares but does not satisfy).
use rusqlite::{Connection, params, types::Value};
use std::path::Path;

/// A fresh database at `path` shaped like Mixxx schema `version` (any number, so tests
/// can also build unsupported ones). Version 40 adds `library.tuning_frequency_hz`.
/// Like Mixxx, the returned connection has foreign keys off. The two playlists a new
/// Mixxx library starts with are removed, so tests can count the playlists they make;
/// `with_default_playlists` puts them back.
pub fn create(path: &Path, version: u32) -> Connection {
    super::create::create(path).unwrap();
    let connection = Connection::open(path).unwrap();
    connection
        .pragma_update(None, "foreign_keys", false)
        .unwrap();
    connection
        .execute("DELETE FROM Playlists WHERE id IN (1, 2)", [])
        .unwrap();
    if version >= 40 {
        connection
            .execute_batch("ALTER TABLE library ADD COLUMN tuning_frequency_hz REAL DEFAULT 0.0;")
            .unwrap();
    }
    connection
        .execute(
            "UPDATE settings SET value = ?1 WHERE name = 'mixxx.schema.version'",
            [version.to_string()],
        )
        .unwrap();
    connection
}

/// The Auto DJ and history-placeholder playlists every new Mixxx library has.
pub fn with_default_playlists(connection: &Connection) {
    connection
        .execute_batch(
            "INSERT INTO Playlists(id, name, position, hidden, date_created, date_modified, locked)
             VALUES (1, 'Auto DJ', 1, 1, datetime('now'), datetime('now'), 0),
                    (2, 'historyPlaceholder', 2, -1, datetime('now'), datetime('now'), 1);",
        )
        .unwrap();
}

/// Open an existing test library the way Mixxx does: foreign keys off.
pub fn open(path: &Path) -> Connection {
    let connection = Connection::open(path).unwrap();
    connection
        .pragma_update(None, "foreign_keys", false)
        .unwrap();
    connection
}

/// A track the way Mixxx leaves one after scanning: tags parsed, no grid, no key,
/// no cues. Returns its `library.id`.
pub fn add_track(connection: &Connection, location: &str, title: &str) -> i64 {
    let (directory, filename) = location.rsplit_once('/').unwrap_or(("", location));
    connection
        .execute(
            "INSERT INTO track_locations(location, filename, directory, filesize, fs_deleted, needs_verification)
             VALUES(?1, ?2, ?3, 1000, 0, 0)",
            params![location, filename, directory],
        )
        .unwrap();
    let location_id = connection.last_insert_rowid();
    connection
        .execute(
            "INSERT INTO library(artist, title, album, filetype, location, rating, key, key_id,
                 cuepoint, bpm, played, mixxx_deleted, header_parsed, channels, samplerate,
                 bitrate, duration, bpm_lock)
             VALUES('Mixxx Artist', ?1, 'Mixxx Album', 'mp3', ?2, 3, '', 0, -1.0, 0.0, 0, 0, 1,
                 2, 44100, 192, 200.0, 0)",
            params![title, location_id],
        )
        .unwrap();
    connection.last_insert_rowid()
}

/// A user playlist (`hidden = 0`) with ordered members; returns its id.
pub fn add_playlist(connection: &Connection, name: &str, tracks: &[i64]) -> i64 {
    let position: i64 = connection
        .query_row(
            "SELECT COALESCE(MAX(position), 0) + 1 FROM Playlists",
            [],
            |row| row.get(0),
        )
        .unwrap();
    connection
        .execute(
            "INSERT INTO Playlists(name, position, hidden, date_created, date_modified)
             VALUES(?1, ?2, 0, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)",
            params![name, position],
        )
        .unwrap();
    let id = connection.last_insert_rowid();
    for (index, track) in tracks.iter().enumerate() {
        connection
            .execute(
                "INSERT INTO PlaylistTracks(playlist_id, track_id, position, pl_datetime_added)
                 VALUES(?1, ?2, ?3, CURRENT_TIMESTAMP)",
                params![id, track, index as i64 + 1],
            )
            .unwrap();
    }
    id
}

/// A crate with members; returns its id.
pub fn add_crate(connection: &Connection, name: &str, tracks: &[i64]) -> i64 {
    connection
        .execute("INSERT INTO crates(name) VALUES(?1)", [name])
        .unwrap();
    let id = connection.last_insert_rowid();
    for track in tracks {
        connection
            .execute(
                "INSERT INTO crate_tracks(crate_id, track_id) VALUES(?1, ?2)",
                params![id, track],
            )
            .unwrap();
    }
    id
}

/// Every row of `sql` as owned values, for before/after comparisons.
pub fn dump(connection: &Connection, sql: &str) -> Vec<Vec<Value>> {
    let mut statement = connection.prepare(sql).unwrap();
    let count = statement.column_count();
    statement
        .query_map([], |row| {
            (0..count)
                .map(|index| row.get::<_, Value>(index))
                .collect::<rusqlite::Result<Vec<_>>>()
        })
        .unwrap()
        .map(Result::unwrap)
        .collect()
}

/// Column names of `table` in declaration order.
pub fn column_names(connection: &Connection, table: &str) -> Vec<String> {
    let mut statement = connection
        .prepare(&format!(
            "SELECT name FROM pragma_table_info('{table}') ORDER BY cid"
        ))
        .unwrap();
    statement
        .query_map([], |row| row.get(0))
        .unwrap()
        .map(Result::unwrap)
        .collect()
}

pub fn column_index(connection: &Connection, table: &str, column: &str) -> usize {
    column_names(connection, table)
        .iter()
        .position(|name| name == column)
        .unwrap()
}

/// A valid silent 16-bit PCM WAV, so audio-property probing has something to read.
pub fn write_wav(path: &Path, channels: u16, sample_rate: u32, frames: u32) {
    let data = frames * u32::from(channels) * 2;
    let mut bytes = Vec::with_capacity(44 + data as usize);
    bytes.extend_from_slice(b"RIFF");
    bytes.extend_from_slice(&(36 + data).to_le_bytes());
    bytes.extend_from_slice(b"WAVEfmt ");
    bytes.extend_from_slice(&16u32.to_le_bytes());
    bytes.extend_from_slice(&1u16.to_le_bytes());
    bytes.extend_from_slice(&channels.to_le_bytes());
    bytes.extend_from_slice(&sample_rate.to_le_bytes());
    bytes.extend_from_slice(&(sample_rate * u32::from(channels) * 2).to_le_bytes());
    bytes.extend_from_slice(&(channels * 2).to_le_bytes());
    bytes.extend_from_slice(&16u16.to_le_bytes());
    bytes.extend_from_slice(b"data");
    bytes.extend_from_slice(&data.to_le_bytes());
    bytes.resize(44 + data as usize, 0);
    std::fs::write(path, bytes).unwrap();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fixture_shapes_differ_only_by_the_schema_40_column() {
        let directory = tempfile::tempdir().unwrap();
        let v39 = create(&directory.path().join("a.sqlite"), 39);
        let v40 = create(&directory.path().join("b.sqlite"), 40);
        let count = |c: &Connection| -> i64 {
            c.query_row(
                "SELECT COUNT(*) FROM pragma_table_info('library')",
                [],
                |r| r.get(0),
            )
            .unwrap()
        };
        assert_eq!(count(&v40), count(&v39) + 1);
        let track = add_track(&v40, "/music/a.mp3", "A");
        let playlist = add_playlist(&v40, "Set", &[track]);
        let crate_id = add_crate(&v40, "Crate", &[track]);
        assert!(track > 0 && playlist > 0 && crate_id > 0);
    }
}
