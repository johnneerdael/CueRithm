INSERT INTO meta VALUES ('maintenance_revision',0);
CREATE TABLE maintenance_fingerprints (
 track_id TEXT NOT NULL REFERENCES tracks(id) ON DELETE CASCADE,
 engine_identity TEXT NOT NULL,source_hash TEXT NOT NULL,fingerprint_json TEXT NOT NULL,created INTEGER NOT NULL,
 PRIMARY KEY(track_id,engine_identity)
);
CREATE TABLE maintenance_scans (
 id TEXT PRIMARY KEY,kind TEXT NOT NULL,payload_json TEXT NOT NULL,created INTEGER NOT NULL
);
CREATE TABLE maintenance_ignored_tracks (
 track_id TEXT PRIMARY KEY REFERENCES tracks(id) ON DELETE CASCADE
);
CREATE TRIGGER maintenance_ignored_insert AFTER INSERT ON maintenance_ignored_tracks BEGIN UPDATE meta SET value=value+1 WHERE key='maintenance_revision'; END;
CREATE TRIGGER maintenance_ignored_delete AFTER DELETE ON maintenance_ignored_tracks BEGIN UPDATE meta SET value=value+1 WHERE key='maintenance_revision'; END;
CREATE TRIGGER maintenance_ignored_update AFTER UPDATE ON maintenance_ignored_tracks BEGIN UPDATE meta SET value=value+1 WHERE key='maintenance_revision'; END;
PRAGMA user_version=7;
