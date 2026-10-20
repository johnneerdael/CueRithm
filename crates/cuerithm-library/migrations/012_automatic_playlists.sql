CREATE TABLE automatic_playlist_nodes (managed_key TEXT PRIMARY KEY, playlist_id TEXT NOT NULL UNIQUE REFERENCES playlists(id) ON DELETE CASCADE);
CREATE TABLE automatic_playlist_state (singleton INTEGER PRIMARY KEY CHECK(singleton=1),input_revision INTEGER NOT NULL,preference_revision INTEGER NOT NULL,playlist_revision INTEGER NOT NULL);
PRAGMA user_version=12;
