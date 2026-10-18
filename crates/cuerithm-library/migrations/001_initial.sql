CREATE TABLE meta (key TEXT PRIMARY KEY,value INTEGER NOT NULL);
INSERT INTO meta VALUES ('revision',0);
CREATE TABLE settings (revision TEXT PRIMARY KEY,json TEXT NOT NULL,created INTEGER NOT NULL);
CREATE TABLE selected_settings (singleton INTEGER PRIMARY KEY CHECK(singleton=1),revision TEXT NOT NULL REFERENCES settings(revision));
CREATE TABLE tracks (
 id TEXT PRIMARY KEY,path BLOB NOT NULL UNIQUE,title TEXT NOT NULL,
 artist TEXT NOT NULL DEFAULT '',album TEXT NOT NULL DEFAULT '',genre TEXT NOT NULL DEFAULT '',
 search_text TEXT NOT NULL DEFAULT '',sort_title TEXT NOT NULL DEFAULT '',sort_artist TEXT NOT NULL DEFAULT '',
 source_hash TEXT NOT NULL,state TEXT NOT NULL DEFAULT 'incoming',
 archived INTEGER NOT NULL DEFAULT 0,current_job TEXT,analysis_json TEXT,metadata_json TEXT NOT NULL DEFAULT '{}',
 musical_key TEXT,bpm REAL,energy INTEGER,duration REAL,created INTEGER NOT NULL,updated INTEGER NOT NULL
);
CREATE TABLE jobs (
 id TEXT PRIMARY KEY,track_id TEXT NOT NULL REFERENCES tracks(id),source_hash TEXT NOT NULL,
 settings_revision TEXT NOT NULL REFERENCES settings(revision),state TEXT NOT NULL,stage TEXT NOT NULL,
 error TEXT,attempts INTEGER NOT NULL DEFAULT 0,created INTEGER NOT NULL,updated INTEGER NOT NULL,
 UNIQUE(track_id,source_hash,settings_revision)
);
CREATE INDEX jobs_pending ON jobs(state,created);
CREATE INDEX tracks_state_title ON tracks(state,title COLLATE NOCASE,id);
CREATE TABLE cue_revisions (id TEXT PRIMARY KEY,track_id TEXT NOT NULL REFERENCES tracks(id),
 job_id TEXT NOT NULL REFERENCES jobs(id),json TEXT NOT NULL,created INTEGER NOT NULL);
CREATE TABLE sync_links (track_id TEXT NOT NULL REFERENCES tracks(id),target TEXT NOT NULL,
 external_id TEXT NOT NULL,baseline_json TEXT NOT NULL,PRIMARY KEY(track_id,target),UNIQUE(target,external_id));
PRAGMA application_id=1129665093;
PRAGMA user_version=1;
