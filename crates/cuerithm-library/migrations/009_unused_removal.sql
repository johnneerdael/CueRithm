-- Older readers do not recover unused_removal_operation journal payloads.
-- Refuse downgrade while these operations and their durable backups exist.
PRAGMA user_version=9;
