CREATE TABLE manual_publication_operations(id TEXT PRIMARY KEY,state TEXT NOT NULL,payload_json TEXT NOT NULL,created INTEGER NOT NULL);
CREATE TABLE manual_publication_state(track_id TEXT PRIMARY KEY REFERENCES tracks(id),json TEXT NOT NULL);
CREATE TABLE cue_templates(id TEXT PRIMARY KEY,name TEXT NOT NULL,json TEXT NOT NULL,updated INTEGER NOT NULL);
PRAGMA user_version=15;
