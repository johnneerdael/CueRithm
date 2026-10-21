INSERT INTO meta(key,value) VALUES('curation_taxonomy_revision',0);
CREATE TABLE curation_group_properties(category_id TEXT PRIMARY KEY REFERENCES custom_tag_categories(id),mode TEXT NOT NULL DEFAULT 'multiple' CHECK(mode IN ('single','multiple')),position INTEGER NOT NULL DEFAULT 0,deleted INTEGER NOT NULL DEFAULT 0);
CREATE TABLE curation_value_properties(tag_id TEXT PRIMARY KEY REFERENCES custom_tags(id),position INTEGER NOT NULL DEFAULT 0,deleted INTEGER NOT NULL DEFAULT 0);
CREATE TABLE curation_category_aliases(name TEXT PRIMARY KEY,category_id TEXT NOT NULL REFERENCES custom_tag_categories(id));
CREATE TABLE curation_value_aliases(category_id TEXT NOT NULL REFERENCES custom_tag_categories(id),name TEXT NOT NULL,tag_id TEXT NOT NULL REFERENCES custom_tags(id),PRIMARY KEY(category_id,name));
INSERT INTO curation_category_aliases SELECT name,id FROM custom_tag_categories;
INSERT INTO curation_value_aliases SELECT category_id,name,id FROM custom_tags;
CREATE TABLE curation_tracks(track_id TEXT PRIMARY KEY REFERENCES tracks(id),revision INTEGER NOT NULL,acknowledged_revision INTEGER NOT NULL,desired_json TEXT NOT NULL,committed_json TEXT NOT NULL,state TEXT NOT NULL,error TEXT,active_publication_id TEXT,last_publication_id TEXT,native_stale INTEGER NOT NULL DEFAULT 0,input_json TEXT NOT NULL,updated INTEGER NOT NULL,notes_search TEXT NOT NULL DEFAULT '',native_required INTEGER NOT NULL DEFAULT 0,pending_mappings_json TEXT NOT NULL DEFAULT '[]',pending_taxonomy_revision INTEGER NOT NULL DEFAULT 0,pending_reexport INTEGER NOT NULL DEFAULT 0,last_native_publication_id TEXT,input_proof_json TEXT);
CREATE INDEX curation_pending ON curation_tracks(state,updated,track_id);
CREATE TABLE curation_mutations(id TEXT PRIMARY KEY,kind TEXT NOT NULL,digest TEXT NOT NULL,receipt_json TEXT NOT NULL,created INTEGER NOT NULL);
CREATE TABLE curation_mappings(id TEXT PRIMARY KEY,payload_json TEXT NOT NULL,updated INTEGER NOT NULL);
CREATE TABLE curation_publications(id TEXT PRIMARY KEY,track_id TEXT NOT NULL REFERENCES tracks(id),revision INTEGER NOT NULL,before_revision INTEGER NOT NULL,before_json TEXT NOT NULL,after_json TEXT NOT NULL,taxonomy_revision INTEGER NOT NULL,mappings_json TEXT NOT NULL,native_required INTEGER NOT NULL,state TEXT NOT NULL,error TEXT,child_preview_id TEXT,child_batch_id TEXT,undo_of TEXT REFERENCES curation_publications(id),predecessor_id TEXT REFERENCES curation_publications(id),reexport INTEGER NOT NULL DEFAULT 0,input_json TEXT NOT NULL DEFAULT '{}',input_proof_json TEXT,notified INTEGER NOT NULL DEFAULT 0,created INTEGER NOT NULL,updated INTEGER NOT NULL);
CREATE INDEX curation_history ON curation_publications(created,id);
CREATE INDEX curation_child ON curation_publications(child_batch_id,track_id);
CREATE INDEX curation_track_history ON curation_publications(track_id,revision);
CREATE TABLE curation_metadata_drafts(draft_id TEXT PRIMARY KEY REFERENCES metadata_edit_drafts(id),publication_id TEXT NOT NULL REFERENCES curation_publications(id));
CREATE TRIGGER curation_category_insert AFTER INSERT ON custom_tag_categories BEGIN INSERT OR IGNORE INTO curation_category_aliases VALUES(NEW.name,NEW.id); UPDATE meta SET value=value+1 WHERE key='curation_taxonomy_revision'; END;
CREATE TRIGGER curation_category_update AFTER UPDATE ON custom_tag_categories BEGIN INSERT OR IGNORE INTO curation_category_aliases VALUES(OLD.name,OLD.id); INSERT OR IGNORE INTO curation_category_aliases VALUES(NEW.name,NEW.id); UPDATE meta SET value=value+1 WHERE key='curation_taxonomy_revision'; END;
CREATE TRIGGER curation_category_delete AFTER DELETE ON custom_tag_categories BEGIN UPDATE meta SET value=value+1 WHERE key='curation_taxonomy_revision'; END;
CREATE TRIGGER curation_value_insert AFTER INSERT ON custom_tags BEGIN INSERT OR IGNORE INTO curation_value_aliases VALUES(NEW.category_id,NEW.name,NEW.id); UPDATE meta SET value=value+1 WHERE key='curation_taxonomy_revision'; END;
CREATE TRIGGER curation_value_update AFTER UPDATE ON custom_tags BEGIN INSERT OR IGNORE INTO curation_value_aliases VALUES(OLD.category_id,OLD.name,OLD.id); INSERT OR IGNORE INTO curation_value_aliases VALUES(NEW.category_id,NEW.name,NEW.id); UPDATE meta SET value=value+1 WHERE key='curation_taxonomy_revision'; END;
CREATE TRIGGER curation_value_delete AFTER DELETE ON custom_tags BEGIN UPDATE meta SET value=value+1 WHERE key='curation_taxonomy_revision'; END;
CREATE TRIGGER curation_smart_update AFTER UPDATE OF committed_json ON curation_tracks WHEN OLD.committed_json IS NOT NEW.committed_json BEGIN UPDATE meta SET value=value+1 WHERE key='smart_input_revision'; END;

-- Mapped legacy assignments can need explicit export before a Curate head exists.
CREATE TABLE curation_native_stale (
    track_id TEXT PRIMARY KEY REFERENCES tracks(id) ON DELETE CASCADE
);
PRAGMA user_version=22;
