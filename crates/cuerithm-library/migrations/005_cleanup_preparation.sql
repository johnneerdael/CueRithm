ALTER TABLE tracks ADD COLUMN native_metadata_json TEXT;
ALTER TABLE tag_writes ADD COLUMN cleanup_json TEXT;
ALTER TABLE tag_writes ADD COLUMN native_metadata_json TEXT;
PRAGMA user_version=5;
