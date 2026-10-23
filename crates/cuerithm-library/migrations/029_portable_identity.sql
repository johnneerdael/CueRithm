ALTER TABLE tracks ADD COLUMN source_locator_json TEXT;
ALTER TABLE tracks ADD COLUMN origin TEXT NOT NULL DEFAULT 'cuerithm';
ALTER TABLE tracks ADD COLUMN analysis_status TEXT NOT NULL DEFAULT 'not_analyzed' CHECK(analysis_status IN ('complete','imported','not_analyzed','stale'));
ALTER TABLE tracks ADD COLUMN selected_musical_revision TEXT;
UPDATE tracks SET analysis_status='complete' WHERE analysis_json IS NOT NULL;
CREATE TABLE accepted_musical_revisions(id TEXT PRIMARY KEY,track_id TEXT NOT NULL REFERENCES tracks(id),json TEXT NOT NULL,created INTEGER NOT NULL);
CREATE INDEX accepted_musical_track ON accepted_musical_revisions(track_id,created);
PRAGMA user_version=29;
CREATE TABLE portable_domain_records(kind TEXT NOT NULL,id TEXT NOT NULL,json TEXT NOT NULL,PRIMARY KEY(kind,id));
CREATE TABLE portable_library_meta(key TEXT PRIMARY KEY,value TEXT NOT NULL);

CREATE INDEX cue_revisions_track_created ON cue_revisions(track_id,created);
