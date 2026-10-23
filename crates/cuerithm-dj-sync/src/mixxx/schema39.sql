-- The structure of the library Mixxx 2.5.6 creates for a new profile (schema version 39),
-- read from sqlite_master in creation order. Mixxx 2.4 and 2.5 create exactly this and
-- newer versions upgrade it themselves. Mixxx is GPL-2.0-or-later
-- (https://github.com/mixxxdj/mixxx); this is the layout of its database, reproduced so
-- that CueRithm can create a library Mixxx accepts as current. Statements are verbatim,
-- including the `library_old` references Mixxx's own migrations left behind.
-- sqlite_sequence is SQLite's own table and is not created here.
CREATE TABLE settings (
        name TEXT UNIQUE NOT NULL,
        value TEXT,
        locked INTEGER DEFAULT 0,
        hidden INTEGER DEFAULT 0);
CREATE TABLE track_locations (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        location varchar(512) UNIQUE,
        filename varchar(512),
        directory varchar(512),
        filesize INTEGER,
        fs_deleted INTEGER,
        needs_verification INTEGER);
CREATE TABLE LibraryHashes (
        directory_path VARCHAR(256) PRIMARY KEY,
        hash INTEGER,
        directory_deleted INTEGER, needs_verification INTEGER DEFAULT 0);
CREATE TABLE Playlists (
        id INTEGER PRIMARY KEY,
        name varchar(48),
        position INTEGER,
        hidden INTEGER DEFAULT 0 NOT NULL,
        date_created datetime,
        date_modified datetime, locked INTEGER DEFAULT 0);
CREATE TABLE PlaylistTracks (
        id INTEGER PRIMARY KEY,
        playlist_id INTEGER REFERENCES Playlists(id),
        track_id INTEGER REFERENCES "library_old"(id),
        position INTEGER, pl_datetime_added);
CREATE TABLE cues (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        track_id INTEGER NOT NULL REFERENCES "library_old"(id),
        type INTEGER DEFAULT 0 NOT NULL,
        
        position INTEGER DEFAULT -1 NOT NULL,
        length INTEGER DEFAULT 0 NOT NULL,
        hotcue INTEGER DEFAULT -1 NOT NULL,
        label TEXT DEFAULT '' NOT NULL, color INTEGER DEFAULT 4294901760 NOT NULL);
CREATE TABLE crates (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        name varchar(48) UNIQUE NOT NULL,
        count INTEGER DEFAULT 0,
        show INTEGER DEFAULT 1, locked INTEGER DEFAULT 0, autodj_source INTEGER DEFAULT 0);
CREATE TABLE crate_tracks (
        crate_id INTEGER NOT NULL REFERENCES crates(id),
        track_id INTEGER NOT NULL REFERENCES "library_old"(id),
        UNIQUE (crate_id, track_id));
CREATE TABLE library (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        artist varchar(64),
        title varchar(64),
        album varchar(64),
        year varchar(16),
        genre varchar(64),
        tracknumber varchar(3),
        location INTEGER REFERENCES track_locations(location),
        comment varchar(256),
        url varchar(256),
        duration FLOAT,
        bitrate INTEGER,
        samplerate INTEGER,
        cuepoint INTEGER,
        bpm FLOAT,
        wavesummaryhex BLOB,
        channels INTEGER,
        datetime_added DEFAULT CURRENT_TIMESTAMP,
        mixxx_deleted INTEGER,
        played INTEGER,
        header_parsed INTEGER DEFAULT 0, filetype varchar(8) DEFAULT '?', replaygain FLOAT DEFAULT 0, timesplayed INTEGER DEFAULT 0, rating INTEGER DEFAULT 0, key varchar(8) DEFAULT '', beats BLOB, beats_version TEXT, composer varchar(64) DEFAULT '', bpm_lock INTEGER DEFAULT 0, beats_sub_version TEXT DEFAULT '', keys BLOB, keys_version TEXT, keys_sub_version TEXT, key_id INTEGER DEFAULT 0, grouping TEXT DEFAULT '', album_artist TEXT DEFAULT '', coverart_source INTEGER DEFAULT 0, coverart_type INTEGER DEFAULT 0, coverart_location TEXT DEFAULT '', coverart_hash INTEGER DEFAULT 0, replaygain_peak REAL DEFAULT -1.0, tracktotal TEXT DEFAULT '//', color INTEGER, coverart_color INTEGER, coverart_digest BLOB, last_played_at DATETIME DEFAULT NULL, source_synchronized_ms INTEGER DEFAULT NULL);
CREATE TABLE itunes_library (
        id INTEGER PRIMARY KEY,
        artist varchar(48), title varchar(48),
        album varchar(48), year varchar(16),
        genre varchar(32), tracknumber varchar(3),
        location varchar(512),
        comment varchar(60),
        duration INTEGER,
        bitrate INTEGER,
        bpm INTEGER,
        rating INTEGER, grouping TEXT DEFAULT '', album_artist TEXT DEFAULT '');
CREATE TABLE itunes_playlists (
        id INTEGER PRIMARY KEY,
        name varchar(100) UNIQUE);
CREATE TABLE itunes_playlist_tracks (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        playlist_id INTEGER REFERENCES itunes_playlist(id),
        track_id INTEGER REFERENCES itunes_library(id), position INTEGER DEFAULT 0);
CREATE TABLE traktor_library (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        artist varchar(48), title varchar(48),
        album varchar(48), year varchar(16),
        genre varchar(32), tracknumber varchar(3),
        location varchar(512) UNIQUE,
        comment varchar(60),
        duration INTEGER,
        bitrate INTEGER,
        bpm FLOAT,
        key varchar(6),
        rating INTEGER
        );
CREATE TABLE traktor_playlists (
        id INTEGER PRIMARY KEY,
        name varchar(100) UNIQUE
        );
CREATE TABLE traktor_playlist_tracks (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        playlist_id INTEGER REFERENCES traktor_playlist(id),
        track_id INTEGER REFERENCES traktor_library(id)
        , position INTEGER DEFAULT 0);
CREATE TABLE rhythmbox_library (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        artist varchar(48), title varchar(48),
        album varchar(48), year varchar(16),
        genre varchar(32), tracknumber varchar(3),
        location varchar(512) UNIQUE,
        comment varchar(60),
        duration INTEGER,
        bitrate INTEGER,
        bpm FLOAT,
        key varchar(6),
        rating INTEGER
        );
CREATE TABLE rhythmbox_playlists (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        name varchar(100) UNIQUE
        );
CREATE TABLE rhythmbox_playlist_tracks (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        playlist_id INTEGER REFERENCES rhythmbox_playlist(id),
        track_id INTEGER REFERENCES rhythmbox_library(id)
        , position INTEGER DEFAULT 0);
CREATE TABLE track_analysis (
      id INTEGER PRIMARY KEY AUTOINCREMENT,
      track_id INTEGER NOT NULL REFERENCES track_locations(id),
      type varchar(512),
      description varchar(1024),
      version varchar(512),
      created DEFAULT CURRENT_TIMESTAMP,
      data_checksum varchar(512)
    );
CREATE INDEX track_analysis_track_id_index ON track_analysis (track_id);
CREATE TABLE directories (
        directory TEXT UNIQUE
      );
CREATE INDEX idx_PlaylistTracks_playlist_id_track_id ON PlaylistTracks (
          playlist_id,
          track_id
      );
CREATE INDEX idx_PlaylistTracks_track_id ON PlaylistTracks (
          track_id
      );
CREATE INDEX idx_crate_tracks_track_id ON crate_tracks (
          track_id
      );
CREATE TABLE rekordbox_library (    id INTEGER PRIMARY KEY AUTOINCREMENT,    rb_id INTEGER,    artist TEXT,    title TEXT,    album TEXT,    year INTEGER,    genre TEXT,    tracknumber TEXT,    location TEXT UNIQUE,    comment TEXT,    duration INTEGER,    bitrate TEXT,    bpm FLOAT,    key TEXT,    rating INTEGER,    analyze_path TEXT UNIQUE,    device TEXT,    color INTEGER);
CREATE TABLE rekordbox_playlists (    id INTEGER PRIMARY KEY,    name TEXT UNIQUE);
CREATE TABLE rekordbox_playlist_tracks (    id INTEGER PRIMARY KEY AUTOINCREMENT,    playlist_id INTEGER REFERENCES rekordbox_playlists(id),    track_id INTEGER REFERENCES rekordbox_library(id),    position INTEGER);
CREATE TABLE serato_library (    id INTEGER PRIMARY KEY AUTOINCREMENT,    title TEXT,    artist TEXT,    album TEXT,    genre TEXT,    comment TEXT,    grouping TEXT,    year INTEGER,    duration INTEGER,    bitrate TEXT,    samplerate TEXT,    bpm FLOAT,    key TEXT,    location TEXT,    bpm_lock INTEGER,    datetime_added DEFAULT CURRENT_TIMESTAMP,    label TEXT,    composer TEXT,    filename TEXT,    filetype TEXT,    remixer TEXT,    size INTEGER,    tracknumber TEXT,    serato_db TEXT);
CREATE TABLE serato_playlists (    id INTEGER PRIMARY KEY,    name TEXT,    serato_db TEXT);
CREATE TABLE serato_playlist_tracks (    id INTEGER PRIMARY KEY AUTOINCREMENT,    playlist_id INTEGER REFERENCES serato_playlists(id),    track_id INTEGER REFERENCES serato_library(id),    position INTEGER);
