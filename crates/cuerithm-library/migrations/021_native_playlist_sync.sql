-- Scoped native identity and both representations are independent of display names.
CREATE TABLE native_playlist_bindings (
 target_id TEXT NOT NULL REFERENCES native_targets(id) ON DELETE CASCADE,
 kind TEXT NOT NULL CHECK(kind IN ('playlist','category','tag')),
 native_id TEXT NOT NULL,uuid TEXT,local_id TEXT NOT NULL,
 owned INTEGER NOT NULL DEFAULT 0 CHECK(owned IN (0,1)),
 remote_json TEXT NOT NULL,local_json TEXT NOT NULL,native_revision INTEGER NOT NULL,
 owned_links_json TEXT NOT NULL DEFAULT '[]',
 PRIMARY KEY(target_id,kind,native_id),UNIQUE(target_id,kind,local_id)
);
CREATE TABLE native_playlist_acknowledgements (
 target_id TEXT NOT NULL REFERENCES native_targets(id) ON DELETE CASCADE,
 plan_hash TEXT NOT NULL,receipt_hash TEXT NOT NULL,
 PRIMARY KEY(target_id,plan_hash)
);
PRAGMA user_version=21;
