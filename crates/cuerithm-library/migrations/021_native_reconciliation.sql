-- Reviewed choices are intents, not acknowledged synchronization baselines.
CREATE TABLE native_reconciliation_plans (
 id TEXT PRIMARY KEY,
 target_id TEXT NOT NULL,
 content_id TEXT NOT NULL,
 snapshot_json TEXT NOT NULL,
 decisions_json TEXT NOT NULL DEFAULT '{}',
 revision INTEGER NOT NULL DEFAULT 0 CHECK(revision>=0),
 created INTEGER NOT NULL,
 updated INTEGER NOT NULL,
 FOREIGN KEY(target_id,content_id) REFERENCES native_track_links(target_id,content_id)
);
CREATE INDEX native_reconciliation_target ON native_reconciliation_plans(target_id,content_id);
PRAGMA user_version=21;
