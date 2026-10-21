# Local library backup and restore

Date: 2026-10-21
Status: implemented on the feature branch with same-installation restore guards. Native Windows and real power-loss acceptance remain unverified.

## Contract

Create dated directory bundles containing a coherent SQLite backup and a private
location manifest. SQLite contains preparation results, playlists, cue edits and
settings. Music files are excluded. No source tags or external DJ databases are
written. A database restore cannot undo tag edits.

The first delivery restores only on the same platform, application version and
library installation. Each existing source must still match its saved hash;
missing sources must remain missing. Engine models and the complete retained
tagging, cleanup, maintenance and DJ-sync resource trees must be unchanged.
Reject unsettled jobs/journals, organization reservations, incomplete preparation
checkpoints and Copy conflicts. Include owned sidecars, exact native object/ancestor
identities for Copy originals, source quarantines, abandoned/partial stages and
native reference receipts/recovery files. Keep settled cancelled/partial histories,
pre-proof ignored Copy decisions and historical moved/undone paths useful; bind
their actual current state rather than assuming obsolete destinations remain.
Reject unavailable required dependencies without pruning history.
Do not promise portable backups or recovery after external file edits.

## Publication and restoration

Use the existing lock order: DJ-sync service lease, organization gate, metadata publication gate,
library connection. Reject queued/running preparation and unsettled publication,
maintenance/disposal and sync journals. Capture a private SQLite snapshot under the connection mutex, then release it
before all source/model/resource hashing. Recheck same-connection changes, external
SQLite data version, schema and quiet state before publishing the bundle or
restore intent. Ordinary reads remain available and concurrent edits reject stale
results. Desktop maintenance lease blocks
parallel maintenance. Use SQLite backup API for coherent WAL snapshots.

Inspect only two regular bundle files with fixed names; reject links, extra
entries, excessive files/bytes, mismatched hashes, app identity/schema/version
and malformed SQLite. Derive sources/models from the database rather than
trusting the manifest alone. Compare the actual retained resource tree.

Restore preview shows date, track count, unavailable sources and scope. Explicit
restore stages a private validated copy and durable request; it does not mutate
the open database. Before Host/Library opens, acquire the original library lease,
revalidate both databases and dependencies, retain a dated pre-restore SQLite
snapshot and manifest, then use SQLite backup into the original database. This
keeps database identity/lease and SQLite transaction recovery. A failed preflight persists a blocked reason and opens the unchanged current
library with explicit UI Retry/Cancel. Persist mutation intent and the recovery
snapshot pointer before changing the original; an interruption during that update
must preserve the original snapshot and stop startup if replay cannot be validated.

## UI

A dedicated Backup view offers Create backup and Inspect backup folder, reviewed
restore with explicit restart instructions, and cancellation of a staged request.
Display same-installation/unchanged-source restrictions and music exclusion
before confirmation. Errors remain visible; changing selection invalidates preview.
