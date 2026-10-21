-- Server-owned frozen native plans and crash-recoverable publication state.
CREATE TABLE rekordbox_sync_operations (
 id TEXT PRIMARY KEY, target_id TEXT NOT NULL, database_path TEXT NOT NULL,
 status TEXT NOT NULL, journal_json TEXT NOT NULL,
 created INTEGER NOT NULL, updated INTEGER NOT NULL
);
CREATE INDEX rekordbox_sync_operations_target ON rekordbox_sync_operations(database_path,status);
CREATE TABLE native_sync_track_ownership (
 target_id TEXT NOT NULL REFERENCES native_targets(id) ON DELETE CASCADE,
 local_track_id TEXT NOT NULL REFERENCES tracks(id), identity_json TEXT NOT NULL,
 cue_ids_json TEXT NOT NULL DEFAULT '[]', cue_proofs_json TEXT NOT NULL DEFAULT '{}', source_json TEXT,
 PRIMARY KEY(target_id,local_track_id)
);
PRAGMA user_version=24;
