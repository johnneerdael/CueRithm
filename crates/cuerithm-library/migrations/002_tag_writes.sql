ALTER TABLE tracks ADD COLUMN tagging_json TEXT;
CREATE TABLE tag_writes (
 id TEXT PRIMARY KEY,job_id TEXT NOT NULL REFERENCES jobs(id),track_id TEXT NOT NULL REFERENCES tracks(id),
 attempt INTEGER NOT NULL,settings_revision TEXT NOT NULL,before_hash TEXT NOT NULL,after_hash TEXT NOT NULL,
 pcm_hash TEXT NOT NULL,frames INTEGER NOT NULL,decoder TEXT NOT NULL,
 analysis_json TEXT NOT NULL,metadata_json TEXT NOT NULL,provenance_json TEXT NOT NULL,
 workspace BLOB NOT NULL,state TEXT NOT NULL DEFAULT 'prepared',error TEXT,created INTEGER NOT NULL
);
CREATE INDEX tag_writes_pending ON tag_writes(state,created);
CREATE TABLE tag_write_files (
 write_id TEXT NOT NULL REFERENCES tag_writes(id),ordinal INTEGER NOT NULL,
 target BLOB NOT NULL,staged BLOB NOT NULL,backup BLOB,before_hash TEXT,after_hash TEXT NOT NULL,
 publish_path BLOB NOT NULL,displaced_path BLOB NOT NULL,
 PRIMARY KEY(write_id,ordinal)
);
PRAGMA user_version=2;
