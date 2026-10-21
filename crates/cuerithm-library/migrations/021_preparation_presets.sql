CREATE TABLE preparation_presets (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    name_key TEXT NOT NULL,
    current_revision TEXT NOT NULL,
    deleted INTEGER NOT NULL DEFAULT 0,
    created INTEGER NOT NULL,
    updated INTEGER NOT NULL
);
CREATE UNIQUE INDEX preparation_presets_active_names ON preparation_presets(name_key) WHERE deleted=0;
CREATE TABLE preparation_preset_revisions (
    preset_id TEXT NOT NULL REFERENCES preparation_presets(id),
    revision TEXT NOT NULL,
    payload_json TEXT NOT NULL,
    created INTEGER NOT NULL,
    PRIMARY KEY(preset_id,revision)
);
CREATE TABLE preparation_preset_folders (
    id TEXT PRIMARY KEY,
    path BLOB NOT NULL UNIQUE,
    authority_json TEXT,
    created INTEGER NOT NULL
);
CREATE TABLE preparation_preset_assignments (
    folder_id TEXT PRIMARY KEY REFERENCES preparation_preset_folders(id),
    revision TEXT NOT NULL,
    choice_json TEXT NOT NULL,
    preset_id TEXT REFERENCES preparation_presets(id),
    updated INTEGER NOT NULL
);
CREATE TABLE preparation_preset_capture_authorities (
    settings_revision TEXT PRIMARY KEY REFERENCES settings(revision),
    authority_json TEXT NOT NULL
);
PRAGMA user_version=21;
