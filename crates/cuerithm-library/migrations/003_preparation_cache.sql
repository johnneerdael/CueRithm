ALTER TABLE tag_writes ADD COLUMN cleanup_complete INTEGER NOT NULL DEFAULT 0 CHECK(cleanup_complete IN (0,1));
CREATE TABLE preparation_analysis (
 source_hash TEXT NOT NULL,settings_revision TEXT NOT NULL,engine_identity TEXT NOT NULL,decoder TEXT NOT NULL,
 analysis_json TEXT NOT NULL,
 PRIMARY KEY(source_hash,settings_revision,engine_identity,decoder)
);
PRAGMA user_version=3;
