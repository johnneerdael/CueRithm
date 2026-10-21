CREATE TABLE sync_target_configurations (
 id TEXT PRIMARY KEY,
 name TEXT NOT NULL,
 revision INTEGER NOT NULL CHECK(revision > 0),
 target_json TEXT NOT NULL,
 updated INTEGER NOT NULL
);
PRAGMA user_version=28;
