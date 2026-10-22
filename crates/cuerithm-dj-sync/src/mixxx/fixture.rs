//! Authored Mixxx-shaped databases for tests. The DDL is written from the observed
//! table and column contract (see `docs/research/mixxx-sync-contract.md`); no upstream
//! schema text is copied. Only the tables and columns the adapter touches, plus the
//! ones Mixxx's own loader reads, are modelled.
use rusqlite::{Connection, params, types::Value};
use std::path::Path;

/// A fresh database at `path` shaped like Mixxx schema `version` (any number, so tests
/// can also build unsupported ones). Version 40 adds `library.tuning_frequency_hz`.
pub fn create(path: &Path, version: u32) -> Connection {
    let connection = Connection::open(path).unwrap();
    connection
        .execute_batch(
            "CREATE TABLE settings (
                 name TEXT UNIQUE NOT NULL, value TEXT,
                 locked INTEGER DEFAULT 0, hidden INTEGER DEFAULT 0);
             CREATE TABLE track_locations (
                 id INTEGER PRIMARY KEY AUTOINCREMENT, location TEXT UNIQUE,
                 filename TEXT, directory TEXT, filesize INTEGER,
                 fs_deleted INTEGER, needs_verification INTEGER);
             CREATE TABLE cues (
                 id INTEGER PRIMARY KEY AUTOINCREMENT,
                 track_id INTEGER NOT NULL REFERENCES library(id),
                 type INTEGER DEFAULT 0 NOT NULL, position INTEGER DEFAULT -1 NOT NULL,
                 length INTEGER DEFAULT 0 NOT NULL, hotcue INTEGER DEFAULT -1 NOT NULL,
                 label TEXT DEFAULT '' NOT NULL, color INTEGER DEFAULT 4294901760 NOT NULL);
             CREATE TABLE Playlists (
                 id INTEGER PRIMARY KEY, name TEXT, position INTEGER,
                 hidden INTEGER DEFAULT 0 NOT NULL, date_created TEXT,
                 date_modified TEXT, locked INTEGER DEFAULT 0);
             CREATE TABLE PlaylistTracks (
                 id INTEGER PRIMARY KEY, playlist_id INTEGER REFERENCES Playlists(id),
                 track_id INTEGER REFERENCES library(id), position INTEGER,
                 pl_datetime_added TEXT);
             CREATE TABLE crates (
                 id INTEGER PRIMARY KEY AUTOINCREMENT, name TEXT UNIQUE NOT NULL,
                 count INTEGER DEFAULT 0, show INTEGER DEFAULT 1,
                 locked INTEGER DEFAULT 0, autodj_source INTEGER DEFAULT 0);
             CREATE TABLE crate_tracks (
                 crate_id INTEGER NOT NULL REFERENCES crates(id),
                 track_id INTEGER NOT NULL REFERENCES library(id),
                 UNIQUE (crate_id, track_id));
             CREATE TABLE directories (directory TEXT UNIQUE);",
        )
        .unwrap();
    let tuning = if version >= 40 {
        ", tuning_frequency_hz REAL DEFAULT 0.0"
    } else {
        ""
    };
    connection
        .execute_batch(&format!(
            "CREATE TABLE library (
                 id INTEGER PRIMARY KEY AUTOINCREMENT,
                 artist TEXT, title TEXT, album TEXT, album_artist TEXT DEFAULT '',
                 year TEXT, genre TEXT, composer TEXT DEFAULT '', grouping TEXT DEFAULT '',
                 tracknumber TEXT, tracktotal TEXT DEFAULT '//', filetype TEXT DEFAULT '?',
                 location INTEGER REFERENCES track_locations(id), color INTEGER,
                 comment TEXT, url TEXT, rating INTEGER DEFAULT 0,
                 key TEXT DEFAULT '', key_id INTEGER DEFAULT 0, cuepoint INTEGER,
                 bpm REAL, replaygain REAL DEFAULT 0, replaygain_peak REAL DEFAULT -1.0,
                 wavesummaryhex BLOB, timesplayed INTEGER DEFAULT 0,
                 last_played_at TEXT DEFAULT NULL, played INTEGER, mixxx_deleted INTEGER,
                 header_parsed INTEGER DEFAULT 0, source_synchronized_ms INTEGER DEFAULT NULL,
                 channels INTEGER, samplerate INTEGER, bitrate INTEGER, duration REAL,
                 beats_version TEXT, beats_sub_version TEXT DEFAULT '', beats BLOB,
                 bpm_lock INTEGER DEFAULT 0, keys_version TEXT, keys_sub_version TEXT,
                 keys BLOB, coverart_source INTEGER DEFAULT 0, coverart_type INTEGER DEFAULT 0,
                 coverart_location TEXT DEFAULT '', coverart_color INTEGER,
                 coverart_digest BLOB, coverart_hash INTEGER DEFAULT 0,
                 datetime_added TEXT DEFAULT CURRENT_TIMESTAMP{tuning});"
        ))
        .unwrap();
    connection
        .execute(
            "INSERT INTO settings(name, value) VALUES('mixxx.schema_version', ?1)",
            [version.to_string()],
        )
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
