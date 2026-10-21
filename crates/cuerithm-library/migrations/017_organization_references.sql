-- Native reference receipts are independent of full metadata/cue/grid synchronization.
CREATE TABLE organization_reference_intents (
 operation_id TEXT PRIMARY KEY REFERENCES organization_operations(id),
 track_id TEXT NOT NULL REFERENCES tracks(id),
 digest TEXT NOT NULL,
 payload_json TEXT NOT NULL,
 verification_digest TEXT,
 state TEXT NOT NULL CHECK(state IN ('bound','repairing','repaired','acknowledged')),
 error TEXT,
 created INTEGER NOT NULL,
 updated INTEGER NOT NULL
);
CREATE INDEX organization_reference_track ON organization_reference_intents(track_id,updated);
CREATE TABLE organization_reference_snapshots (
 digest TEXT PRIMARY KEY,
 track_id TEXT NOT NULL REFERENCES tracks(id),
 payload_json TEXT NOT NULL,
 created INTEGER NOT NULL
);
CREATE TABLE organization_reference_paths (
 track_id TEXT NOT NULL REFERENCES tracks(id),
 target_key TEXT NOT NULL,
 path BLOB NOT NULL,
 operation_id TEXT NOT NULL REFERENCES organization_operations(id),
 updated INTEGER NOT NULL,
 PRIMARY KEY(track_id,target_key)
);
CREATE TABLE organization_reference_edges (
 operation_id TEXT NOT NULL REFERENCES organization_operations(id),
 target_key TEXT NOT NULL,
 before_digest TEXT NOT NULL,
 after_digest TEXT NOT NULL,
 payload_json TEXT NOT NULL,
 PRIMARY KEY(operation_id,target_key)
);
CREATE INDEX organization_reference_edge_from ON organization_reference_edges(target_key,before_digest);
PRAGMA user_version=17;
