CREATE TABLE catalog_access (
    source TEXT PRIMARY KEY,
    requires_solver INTEGER NOT NULL CHECK (requires_solver IN (0, 1))
);
PRAGMA user_version=13;
