-- Empty factual Mixxx schema; generated from pinned migration history.

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
        header_parsed INTEGER DEFAULT 0, filetype varchar(8) DEFAULT '?', replaygain FLOAT DEFAULT 0, timesplayed INTEGER DEFAULT 0, rating INTEGER DEFAULT 0, key varchar(8) DEFAULT '', beats BLOB, beats_version TEXT, composer varchar(64) DEFAULT '', bpm_lock INTEGER DEFAULT 0, beats_sub_version TEXT DEFAULT '', keys BLOB, keys_version TEXT, keys_sub_version TEXT, key_id INTEGER DEFAULT 0, grouping TEXT DEFAULT '', album_artist TEXT DEFAULT '', coverart_source INTEGER DEFAULT 0, coverart_type INTEGER DEFAULT 0, coverart_location TEXT DEFAULT '', coverart_hash INTEGER DEFAULT 0, replaygain_peak REAL DEFAULT -1.0, tracktotal TEXT DEFAULT '//', color INTEGER, coverart_color INTEGER, coverart_digest BLOB, last_played_at DATETIME DEFAULT NULL, source_synchronized_ms INTEGER DEFAULT NULL, tuning_frequency_hz FLOAT DEFAULT 0.0);

CREATE TABLE track_locations (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        location varchar(512) UNIQUE,
        filename varchar(512),
        directory varchar(512),
        filesize INTEGER,
        fs_deleted INTEGER,
        needs_verification INTEGER);

CREATE TABLE cues (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        track_id INTEGER NOT NULL REFERENCES "library_old"(id),
        type INTEGER DEFAULT 0 NOT NULL,
        
        position INTEGER DEFAULT -1 NOT NULL,
        length INTEGER DEFAULT 0 NOT NULL,
        hotcue INTEGER DEFAULT -1 NOT NULL,
        label TEXT DEFAULT '' NOT NULL, color INTEGER DEFAULT 4294901760 NOT NULL);

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

CREATE TABLE crates (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        name varchar(48) UNIQUE NOT NULL,
        count INTEGER DEFAULT 0,
        show INTEGER DEFAULT 1, locked INTEGER DEFAULT 0, autodj_source INTEGER DEFAULT 0);

CREATE TABLE crate_tracks (
        crate_id INTEGER NOT NULL REFERENCES crates(id),
        track_id INTEGER NOT NULL REFERENCES "library_old"(id),
        UNIQUE (crate_id, track_id));

CREATE TABLE directories (
        directory TEXT UNIQUE
      );

CREATE TABLE settings (
        name TEXT UNIQUE NOT NULL,
        value TEXT,
        locked INTEGER DEFAULT 0,
        hidden INTEGER DEFAULT 0);

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

INSERT INTO settings(name,value) VALUES('mixxx.schema.version','40');

INSERT INTO settings(name,value) VALUES('mixxx.schema.last_used_version','40');

INSERT INTO settings(name,value) VALUES('mixxx.schema.min_compatible_version','3');
