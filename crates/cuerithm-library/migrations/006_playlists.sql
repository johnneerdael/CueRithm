INSERT INTO meta VALUES ('playlist_revision',0),('smart_input_revision',0),('playlist_preferences_revision',0);
CREATE TABLE playlists (
 id TEXT PRIMARY KEY,parent_id TEXT REFERENCES playlists(id) DEFERRABLE INITIALLY DEFERRED,
 kind TEXT NOT NULL CHECK(kind IN ('playlist','folder','smartlist')),name TEXT NOT NULL,
 position INTEGER NOT NULL CHECK(position>=0),favorite_position INTEGER,
 definition_json TEXT,deleted INTEGER NOT NULL DEFAULT 0 CHECK(deleted IN (0,1)),deleted_by TEXT,last_edit_id TEXT NOT NULL
);
CREATE INDEX playlists_tree ON playlists(parent_id,deleted,position,id);
CREATE TABLE playlist_members (
 playlist_id TEXT NOT NULL REFERENCES playlists(id) ON DELETE CASCADE,
 track_id TEXT NOT NULL REFERENCES tracks(id),position INTEGER NOT NULL CHECK(position>=0),
 PRIMARY KEY(playlist_id,track_id),UNIQUE(playlist_id,position)
);
CREATE TABLE playlist_edits (
 id TEXT PRIMARY KEY,kind TEXT NOT NULL,before_json TEXT NOT NULL,after_json TEXT NOT NULL,
 source_edit_id TEXT REFERENCES playlist_edits(id),undone_by TEXT REFERENCES playlist_edits(id),created INTEGER NOT NULL
);
CREATE TABLE playlist_previews (
 id TEXT PRIMARY KEY,kind TEXT NOT NULL,payload_json TEXT NOT NULL,playlist_revision INTEGER NOT NULL,
 smart_input_revision INTEGER NOT NULL,preference_revision INTEGER NOT NULL,created INTEGER NOT NULL,expires INTEGER
);
CREATE TABLE library_preferences (singleton INTEGER PRIMARY KEY CHECK(singleton=1),json TEXT NOT NULL);
INSERT INTO library_preferences VALUES(1,'{"key_mixing_mode":"harmonic","time_zone":null}');
CREATE TABLE smart_playlist_status (
 playlist_id TEXT PRIMARY KEY REFERENCES playlists(id) ON DELETE CASCADE,state TEXT NOT NULL,error TEXT,
 input_revision INTEGER,playlist_revision INTEGER,preference_revision INTEGER,
 evaluated_at INTEGER,next_refresh INTEGER,last_good_revision INTEGER,definition_hash TEXT
);
CREATE TABLE smart_playlist_members (
 playlist_id TEXT NOT NULL REFERENCES playlists(id) ON DELETE CASCADE,track_id TEXT NOT NULL REFERENCES tracks(id),
 position INTEGER NOT NULL CHECK(position>=0),PRIMARY KEY(playlist_id,track_id),UNIQUE(playlist_id,position)
);
CREATE TRIGGER smart_tracks_insert AFTER INSERT ON tracks BEGIN UPDATE meta SET value=value+1 WHERE key='smart_input_revision'; END;
CREATE TRIGGER smart_tracks_delete AFTER DELETE ON tracks BEGIN UPDATE meta SET value=value+1 WHERE key='smart_input_revision'; END;
CREATE TRIGGER smart_custom_tag_categories_insert AFTER INSERT ON custom_tag_categories BEGIN UPDATE meta SET value=value+1 WHERE key='smart_input_revision'; END;
CREATE TRIGGER smart_custom_tag_categories_delete AFTER DELETE ON custom_tag_categories BEGIN UPDATE meta SET value=value+1 WHERE key='smart_input_revision'; END;
CREATE TRIGGER smart_custom_tag_categories_update AFTER UPDATE ON custom_tag_categories BEGIN UPDATE meta SET value=value+1 WHERE key='smart_input_revision'; END;
CREATE TRIGGER smart_custom_tags_insert AFTER INSERT ON custom_tags BEGIN UPDATE meta SET value=value+1 WHERE key='smart_input_revision'; END;
CREATE TRIGGER smart_custom_tags_delete AFTER DELETE ON custom_tags BEGIN UPDATE meta SET value=value+1 WHERE key='smart_input_revision'; END;
CREATE TRIGGER smart_custom_tags_update AFTER UPDATE ON custom_tags BEGIN UPDATE meta SET value=value+1 WHERE key='smart_input_revision'; END;
CREATE TRIGGER smart_track_custom_tags_insert AFTER INSERT ON track_custom_tags BEGIN UPDATE meta SET value=value+1 WHERE key='smart_input_revision'; END;
CREATE TRIGGER smart_track_custom_tags_delete AFTER DELETE ON track_custom_tags BEGIN UPDATE meta SET value=value+1 WHERE key='smart_input_revision'; END;
CREATE TRIGGER smart_track_custom_tags_update AFTER UPDATE ON track_custom_tags BEGIN UPDATE meta SET value=value+1 WHERE key='smart_input_revision'; END;
CREATE TRIGGER smart_cue_revisions_insert AFTER INSERT ON cue_revisions BEGIN UPDATE meta SET value=value+1 WHERE key='smart_input_revision'; END;
CREATE TRIGGER smart_cue_revisions_delete AFTER DELETE ON cue_revisions BEGIN UPDATE meta SET value=value+1 WHERE key='smart_input_revision'; END;
CREATE TRIGGER smart_cue_revisions_update AFTER UPDATE ON cue_revisions BEGIN UPDATE meta SET value=value+1 WHERE key='smart_input_revision'; END;
CREATE TRIGGER smart_tracks_update AFTER UPDATE OF path,title,artist,album,genre,source_hash,state,archived,analysis_json,metadata_json,cleanup_json,native_metadata_json,musical_key,bpm,energy,duration,created,updated ON tracks WHEN NEW.path IS NOT OLD.path OR NEW.title IS NOT OLD.title OR NEW.artist IS NOT OLD.artist OR NEW.album IS NOT OLD.album OR NEW.genre IS NOT OLD.genre OR NEW.source_hash IS NOT OLD.source_hash OR NEW.state IS NOT OLD.state OR NEW.archived IS NOT OLD.archived OR NEW.analysis_json IS NOT OLD.analysis_json OR NEW.metadata_json IS NOT OLD.metadata_json OR NEW.cleanup_json IS NOT OLD.cleanup_json OR NEW.native_metadata_json IS NOT OLD.native_metadata_json OR NEW.musical_key IS NOT OLD.musical_key OR NEW.bpm IS NOT OLD.bpm OR NEW.energy IS NOT OLD.energy OR NEW.duration IS NOT OLD.duration OR NEW.created IS NOT OLD.created OR NEW.updated IS NOT OLD.updated BEGIN UPDATE meta SET value=value+1 WHERE key='smart_input_revision'; END;
CREATE TRIGGER smart_protected_genres_update AFTER UPDATE OF revision ON selected_settings
WHEN COALESCE((SELECT json_extract(json,'$.settings.cleanup.genres.protected_values') FROM settings WHERE revision=NEW.revision),'[]')
 IS NOT COALESCE((SELECT json_extract(json,'$.settings.cleanup.genres.protected_values') FROM settings WHERE revision=OLD.revision),'[]')
BEGIN UPDATE meta SET value=value+1 WHERE key='smart_input_revision'; END;
PRAGMA user_version=6;
