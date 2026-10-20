CREATE TABLE organization_operations (
 id TEXT PRIMARY KEY,
 track_id TEXT NOT NULL REFERENCES tracks(id),
 state TEXT NOT NULL,
 payload_json TEXT NOT NULL,
 error TEXT,
 created INTEGER NOT NULL,
 updated INTEGER NOT NULL
);
CREATE UNIQUE INDEX organization_active_track ON organization_operations(track_id)
 WHERE state NOT IN ('complete','unchanged','undone','cancelled');
CREATE TABLE organization_reservations (
 path BLOB PRIMARY KEY,
 operation_id TEXT NOT NULL REFERENCES organization_operations(id)
);
CREATE TABLE organization_alternates (
 operation_id TEXT PRIMARY KEY REFERENCES organization_operations(id),
 track_id TEXT NOT NULL REFERENCES tracks(id),
 path BLOB NOT NULL,
 source_hash TEXT NOT NULL,
 state TEXT NOT NULL,
 observed_hash TEXT,
 ignored_hash TEXT
);
CREATE INDEX organization_alternate_path ON organization_alternates(path);
PRAGMA user_version=14;
