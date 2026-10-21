CREATE TABLE organization_copy_conflicts (
 operation_id TEXT PRIMARY KEY REFERENCES organization_operations(id),
 state TEXT NOT NULL,
 digest TEXT NOT NULL,
 payload_json TEXT NOT NULL,
 queued_job_id TEXT REFERENCES jobs(id),
 created INTEGER NOT NULL,
 updated INTEGER NOT NULL
);
PRAGMA user_version=18;
