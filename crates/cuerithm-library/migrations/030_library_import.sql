-- Additive source-neutral observations. Existing native rekordbox state is untouched.
CREATE TABLE import_sources (
 id TEXT PRIMARY KEY,provider TEXT NOT NULL,native_instance_id TEXT NOT NULL,
 location_key TEXT NOT NULL,location_json TEXT NOT NULL,profile_json TEXT NOT NULL,created INTEGER NOT NULL,updated INTEGER NOT NULL
);
CREATE INDEX import_sources_native ON import_sources(provider,native_instance_id);
CREATE TABLE import_blobs (sha256 TEXT PRIMARY KEY,data BLOB NOT NULL,bytes INTEGER NOT NULL CHECK(bytes>=0),media_type TEXT);
CREATE TABLE import_snapshots (
 id TEXT PRIMARY KEY,source_id TEXT NOT NULL REFERENCES import_sources(id),
 header_json TEXT NOT NULL,profile_json TEXT NOT NULL,request_json TEXT NOT NULL,
 snapshot_hash TEXT NOT NULL,runtime_nonce TEXT NOT NULL,created INTEGER NOT NULL
);
CREATE INDEX import_snapshots_source ON import_snapshots(source_id,created,id);
CREATE TABLE import_observations (
 snapshot_id TEXT NOT NULL REFERENCES import_snapshots(id) ON DELETE CASCADE,
 sequence INTEGER NOT NULL,kind TEXT NOT NULL,record_id TEXT NOT NULL,
 collection_id TEXT,json TEXT NOT NULL,semantic_hash TEXT NOT NULL,
 PRIMARY KEY(snapshot_id,sequence)
);
CREATE INDEX import_observations_kind ON import_observations(snapshot_id,kind,sequence);
CREATE INDEX import_observations_record ON import_observations(snapshot_id,kind,record_id);
CREATE TABLE import_items (
 id TEXT PRIMARY KEY,local_track_id TEXT REFERENCES tracks(id),state TEXT NOT NULL DEFAULT 'imported',
 availability TEXT NOT NULL DEFAULT 'not_probed',path BLOB,source_hash TEXT,
 revision INTEGER NOT NULL DEFAULT 1,created INTEGER NOT NULL,updated INTEGER NOT NULL
);
CREATE INDEX import_items_path ON import_items(path);
CREATE INDEX import_items_hash ON import_items(source_hash);
CREATE TABLE import_bindings (
 source_id TEXT NOT NULL REFERENCES import_sources(id),external_id TEXT NOT NULL,
 item_id TEXT NOT NULL REFERENCES import_items(id),identity_json TEXT NOT NULL,
 legacy_target_id TEXT,legacy_content_id TEXT,observed_hash TEXT,
 PRIMARY KEY(source_id,external_id)
);
CREATE INDEX import_bindings_item ON import_bindings(item_id);
CREATE TABLE import_active_fields (
 item_id TEXT NOT NULL REFERENCES import_items(id),field TEXT NOT NULL,value_json TEXT NOT NULL,
 provenance_json TEXT NOT NULL,revision INTEGER NOT NULL DEFAULT 1,PRIMARY KEY(item_id,field)
);
CREATE TABLE import_applied_baselines (
 source_id TEXT NOT NULL REFERENCES import_sources(id),external_id TEXT NOT NULL,field TEXT NOT NULL,
 source_json TEXT NOT NULL,local_json TEXT NOT NULL,source_hash TEXT NOT NULL,policy_revision TEXT NOT NULL,
 PRIMARY KEY(source_id,external_id,field)
);
CREATE TABLE import_collections (
 id TEXT PRIMARY KEY,source_id TEXT NOT NULL REFERENCES import_sources(id),external_id TEXT NOT NULL,
 parent_id TEXT,kind TEXT NOT NULL,name TEXT NOT NULL,json TEXT NOT NULL,revision INTEGER NOT NULL DEFAULT 1,
 UNIQUE(source_id,external_id)
);
CREATE TABLE import_memberships (
 collection_id TEXT NOT NULL REFERENCES import_collections(id) ON DELETE CASCADE,
 occurrence_id TEXT NOT NULL,item_id TEXT REFERENCES import_items(id),external_track_id TEXT NOT NULL,
 position INTEGER,json TEXT NOT NULL,PRIMARY KEY(collection_id,occurrence_id)
);
CREATE TABLE import_policies (id TEXT PRIMARY KEY,source_id TEXT NOT NULL REFERENCES import_sources(id),json TEXT NOT NULL,created INTEGER NOT NULL);
CREATE TABLE import_previews (
 id TEXT PRIMARY KEY,source_id TEXT NOT NULL REFERENCES import_sources(id),snapshot_id TEXT NOT NULL REFERENCES import_snapshots(id),
 local_revision INTEGER NOT NULL,policy_id TEXT NOT NULL REFERENCES import_policies(id),json TEXT NOT NULL,
 runtime_nonce TEXT NOT NULL,state TEXT NOT NULL DEFAULT 'ready',created INTEGER NOT NULL
);
CREATE TABLE import_operations (
 id TEXT PRIMARY KEY,preview_id TEXT UNIQUE REFERENCES import_previews(id),source_id TEXT NOT NULL REFERENCES import_sources(id),
 policy_id TEXT NOT NULL REFERENCES import_policies(id),state TEXT NOT NULL,receipt_json TEXT NOT NULL,undo_json TEXT NOT NULL DEFAULT '{}',
 created INTEGER NOT NULL,updated INTEGER NOT NULL
);
CREATE TABLE import_processing_receipts (
 operation_id TEXT NOT NULL REFERENCES import_operations(id),item_id TEXT NOT NULL REFERENCES import_items(id),
 stage TEXT NOT NULL,attempt TEXT NOT NULL,state TEXT NOT NULL,identity_json TEXT NOT NULL,receipt_json TEXT NOT NULL,
 updated INTEGER NOT NULL,PRIMARY KEY(operation_id,item_id,stage,attempt)
);
CREATE UNIQUE INDEX import_processing_running ON import_processing_receipts(operation_id,item_id,stage) WHERE state='running';
CREATE TABLE import_preview_effects (
 preview_id TEXT NOT NULL REFERENCES import_previews(id) ON DELETE CASCADE,
 sequence INTEGER NOT NULL,kind TEXT NOT NULL,external_id TEXT NOT NULL,
 proposal_json TEXT NOT NULL,effect_json TEXT NOT NULL,PRIMARY KEY(preview_id,sequence)
);
CREATE INDEX import_preview_effects_item ON import_preview_effects(json_extract(proposal_json,'$.candidate.item_id'),preview_id) WHERE kind='track';
CREATE INDEX import_operations_source ON import_operations(source_id,created,id);
CREATE INDEX import_processing_signature ON import_processing_receipts(stage,json_extract(identity_json,'$.stage_signature')) WHERE state='proposed';
PRAGMA user_version=30;
