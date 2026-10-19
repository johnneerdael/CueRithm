-- Native observations are separate from synchronized baselines and legacy links.
CREATE TABLE native_targets (
 id TEXT PRIMARY KEY,provider TEXT NOT NULL CHECK(provider='rekordbox'),
 database_id TEXT NOT NULL,device_id TEXT NOT NULL,
 identity_json TEXT NOT NULL,schema_fingerprint TEXT NOT NULL,observation_hash TEXT NOT NULL,issues_json TEXT NOT NULL,
 revision INTEGER NOT NULL CHECK(revision>0),created INTEGER NOT NULL,updated INTEGER NOT NULL,
 UNIQUE(provider,database_id,device_id)
);
CREATE TABLE native_records (
 target_id TEXT NOT NULL REFERENCES native_targets(id) ON DELETE CASCADE,
 table_name TEXT NOT NULL,record_id TEXT NOT NULL,json TEXT NOT NULL,hash TEXT NOT NULL,
 PRIMARY KEY(target_id,table_name,record_id)
);
CREATE TABLE native_track_links (
 target_id TEXT NOT NULL REFERENCES native_targets(id) ON DELETE CASCADE,content_id TEXT NOT NULL,
 local_track_id TEXT REFERENCES tracks(id),identity_json TEXT NOT NULL,
 observation_json TEXT,observation_hash TEXT,baseline_json TEXT NOT NULL,
 status TEXT NOT NULL,updated INTEGER NOT NULL,
 PRIMARY KEY(target_id,content_id)
);
CREATE INDEX native_track_links_local ON native_track_links(local_track_id);
PRAGMA user_version=11;
