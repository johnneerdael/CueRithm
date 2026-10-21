CREATE TABLE catalog_rate_state(
 service TEXT PRIMARY KEY CHECK(length(service) BETWEEN 1 AND 64),
 last_start_ms INTEGER,
 not_before_ms INTEGER NOT NULL DEFAULT 0,
 observed_limit INTEGER,
 observed_remaining INTEGER,
 observed_ms INTEGER,
 manual_hold INTEGER NOT NULL DEFAULT 0 CHECK(manual_hold IN (0,1)),
 manual_hold_observed_ms INTEGER
);
CREATE TABLE catalog_request_starts(
 id INTEGER PRIMARY KEY,
 service TEXT NOT NULL,
 started_ms INTEGER NOT NULL
);
CREATE INDEX catalog_request_starts_service_time ON catalog_request_starts(service,started_ms);
CREATE TABLE catalog_response_cache(
 key TEXT PRIMARY KEY CHECK(length(key)=64),
 body BLOB NOT NULL,
 expires_ms INTEGER NOT NULL,
 used_ms INTEGER NOT NULL
);
CREATE INDEX catalog_response_cache_used ON catalog_response_cache(used_ms,key);
ALTER TABLE preparation_review_artwork ADD COLUMN source_json TEXT;
PRAGMA user_version=23;
