CREATE TABLE preparation_match_snapshots (
    job_id TEXT PRIMARY KEY REFERENCES jobs(id),
    track_id TEXT NOT NULL REFERENCES tracks(id),
    attempt INTEGER NOT NULL CHECK(attempt > 0),
    source_hash TEXT NOT NULL,
    settings_revision TEXT NOT NULL REFERENCES settings(revision),
    payload_json TEXT NOT NULL CHECK(length(payload_json) <= 2097152),
    created INTEGER NOT NULL
);
PRAGMA user_version = 15;
