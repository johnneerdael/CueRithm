# Native playlist identity and owned deletion

Status: implemented ownership and mutation contract, 2026-10-20. This document
combines current independent Rust behavior with aggregate observations of Lexicon DJ
1.11.15's shipped converters. Native application/player runtime acceptance remains
separate. Scope is current VirtualDJ MyLists, Engine DJ schema 3, djay Pro
YapDatabase, Serato 4 and current Pioneer Device Library/Device Library Plus.

## Identity and removal boundary

`dj_sync_service::Baseline` caches acknowledged source track/playlist hashes and
options for Modified selection. A separate local ownership ledger now persists the
source-to-native bindings needed for rename, move and deletion. Full and Changed
(the internal name for Modified) no longer reject every established local removal:
they stage verified owned deletions and report retained external matches.

| Target | Implemented native binding | Mutation behavior |
| --- | --- | --- |
| VirtualDJ | Verified relative MyLists file/directory locator | Rename/move updates the owned tree and order files, including omitted descendants and case-only paths; dotted names remain intact. |
| Engine DJ | Database identity and ordinary playlist ID or smartlist UUID | Bound objects retain identity across rename/move and representation transitions; sibling and derived membership references are repaired. |
| djay Pro | Yap collection/key UUID with creation proof | Rename/move updates the same object and both parent arrays; deletion removes only owned playlist/link records. |
| Serato 4 | Location identity and native container locator, plus master mapping | Crate rename/move/deletion reconciles location and aggregate references without adopting unrelated same-name containers. |
| Pioneer USB | Device/library identity and playlist ID | SQL and PDB share the merged hierarchy and preserve unrelated catalog content. |

Source IDs survive rename/move. Local removal records `deleted`/`deleted_by` for the
removed subtree. Deletion contexts are derived from that authoritative complete
source tree and intersected with prior bindings. Omission from a selected playlist
or a Modified active projection is not removal. Native smart or ordinary output is
chosen independently of this ownership decision.

## Durable local ledger

The version-1 `OwnershipLedger` is stored beside local sync journals, keyed by the
canonical target selector. It contains a generation, last journal, persistent-volume
and catalog-object registration, per-native-store ownership state and audit tombstones. It adds no
tracking columns or marker records to proprietary libraries.

Each `PlaylistBinding` records `source_id`, native `locator`, `parent_source_id`,
`created` and a structural `fingerprint`. `created = true` requires before/after
staging proof that this sync created the object. A matched external record retains
`created = false`, even after an update. Only a created binding authorizes automatic
deletion. Reports return the current bindings and separate deleted/retired source IDs;
retirement keeps the external native object.

The replaceable Changed hash cache is not ownership proof. Legacy hash baselines,
matching names/contents, source-derived UUIDs or filename suffixes alone cannot
authorize deletion. The ledger separately guards native identity and filesystem
registration, rejecting ordinary catalog replacement at the same displayed path.
Symlink registrations are rejected. Unsettled apply/restore acknowledgement
blocks a new preview for that target.

Registration uses macOS volume UUIDs, Linux ext UUID-derived filesystem IDs or
matched `/dev/disk/by-uuid` entries, and Windows volume serials. Mount-session
device numbers are not persisted. FAT/exFAT allocated inode numbers are excluded;
catalog creation times supply object proof there. Pioneer includes both visible
and hidden catalog objects, without binding its host mount-folder identity.
Engine's native database UUID is checked by the existing staged adapter snapshot.
Missing persistent volume identity or required creation-time proof stops sync.
Identical FAT catalog replacements that also preserve creation metadata cannot be
distinguished through these filesystem APIs.

The Linux distinction follows the kernel's
[ext filesystem ID](https://github.com/torvalds/linux/blob/master/fs/ext4/super.c),
[FAT](https://github.com/torvalds/linux/blob/master/fs/fat/inode.c) and
[exFAT](https://github.com/torvalds/linux/blob/master/fs/exfat/inode.c) implementations;
[udev's persistent-storage rules](https://github.com/systemd/systemd/blob/main/rules.d/60-persistent-storage.rules.in)
provide UUID links for matching block devices.

## Adapter planning and reporting

`PlaylistOwnershipRequest` passes previous bindings plus explicit removed source IDs.
Adapters return `PlaylistOwnershipReport` with current bindings, deleted IDs and
retired external bindings. The staged report is proposed state until publication
and acknowledgement; it does not itself advance the local ledger or Modified cache.

Resolve bound native objects before name matching, verify fingerprints and reject
changed/missing/reused identities that cannot be reconciled safely. Expand owned
folder deletion deepest first. Check the actual native subtree and references before
removing a parent; foreign children/content block deletion. Playlist tombstones never
authorize deleting audio, native tracks or unrelated records.

Batch staging handles name swaps, moves and uniqueness constraints together.
Reordering partial scopes preserves omitted sibling slots. Empty active input still
processes removals. After native staging, exact file hashes, directory manifests and
case-rename preconditions become the recoverable publication plan.
Adapter build copies are temporary. After the journal durably owns its staged
publications, the build tree is removed; preparation errors also clean up build
copies while retaining journal and recovery evidence.

## Native operations

### VirtualDJ MyLists

The locator is a relative file/directory path under the selected VirtualDJ home, together with the known `CueRithm.subfolders` root. A leaf is a `.vdjfolder` XML file; a folder is a `.subfolders` directory. Each containing directory has an `order` file whose entries are filename stems, not source UUIDs. Song database rows are independent of playlist paths.

Rename/move a leaf by publishing its new owned path and removing its verified old owned path. For an ancestor rename/move, calculate new paths for **every owned descendant**, including descendants omitted from Changed input. Preserve unrelated siblings in both old/new `order` files. A directory containing any unknown file, child or native custom content cannot be recursively removed. Deleting a playlist must not delete audio or `database.xml` song entries.

Typed directory operations are implemented with expected child manifests, explicit empty-directory creation/removal, ancestor/symlink guards and inverse recovery. Subtree moves use verified file operations and deepest-first empty-directory removals. Dedicated case-rename journal entries use checked temporary paths on case-insensitive filesystems. Apply and restore preserve exact spelling, empty folders, files and both order files; unchecked recursive removal is not the mutation policy.

The shipped converter's whole-MyLists replacement on Full is evidence of layout, not a deletion policy to copy: it would remove unrelated target lists.

### Engine DJ

Bind `(database UUID, Playlist.id)`. Update `title`, `parentListId` and ordering in place. `nextListId` is a sibling linked list with a single `0` tail; `(parentListId,nextListId)` and `(title,parentListId)` are unique. Reconnect old/new sibling chains and preserve omitted siblings. Plan a uniqueness-safe temporary phase for multi-row reorder/move operations.

Direct membership is `PlaylistEntity(listId, trackId, databaseUuid, nextEntityId, membershipReference=0)`. Derived ancestor membership has `membershipReference>0`, counts shared descendant membership, and uses its own linked ordering. Rebuild derived membership after moves/deletions, including tombstone-only batches; the current writer handles these without requiring active playlists.

Delete verified owned leaves' `PlaylistEntity` rows, repair predecessor `nextListId`, then delete `Playlist` rows deepest first. The current schema has membership cascades and an after-delete trigger that repairs sibling links **and deletes children**. Validate the whole descendant closure before executing it; explicit link repair must agree with the installed trigger behavior rather than double-adjusting links. Preserve `Track`, `PerformanceData`, art, prepare lists and unrelated smartlists. Do not reset `sqlite_sequence` as the shipped bulk-clear path does.

Include native `Smartlist` path references in the graph for native and materialized representation transitions. `parentPlaylistPath` and `nextPlaylistPath` contain semicolon-terminated playlist paths; `nextListUuid` links smartlists. A folder rename/move requires boundary-aware old/new path-reference repair while preserving smartlist UUIDs/rules. A foreign smartlist beneath a removed folder blocks parent deletion; a surviving smartlist pointing to a removed sibling needs its ordering reference repaired. Unknown path encoding or mixed-list ordering must produce a conflict rather than an unrestricted string replacement.

### djay Pro

Bind `(database2 collection='mediaItemPlaylists', key=UUID)`. Current source-derived UUIDs can be reported directly, but still require creation evidence before automatic deletion. Keep the native root `mediaItemPlaylist-root` and all unrelated children.

Maintain both directions: a playlist object's `parentUUID` and its old/new parent's ordered `childUUIDs`. Its `itemUUIDs` name records in `mediaItemPlaylistItems`; each link has `playlistUUID` and `mediaItemUUID`. Delete only links belonging to the owned playlist, remove that UUID from parent arrays, and delete its playlist object. Validate referenced link ownership instead of deleting an arbitrary foreign link merely because its key appears in a malformed array. Delete owned descendants before parents; reject unowned children.

Preserve unknown TSAF object properties and `database2.metadata`. Invalidate the existing playlist/relationship YapDatabase index version tags in the same transaction, using the current adapter's index invalidation machinery. Do not remove media items, title IDs, local bookmarks or user analysis when deleting a playlist.

### Serato 4

Bind the location database identity, `space.id`, and `container.id`; record that a source folder maps to a native crate, not a separate folder table. Current ordinary crates use `type=1`; root `type=0` is never deleted. Maintain `parent_id`, `name`, `list_order` and `revision` in place. Keep the global revision counters in `master`/`serato` consistent with existing writer behavior.

Remove membership rows for an owned container before its `container` row. Inspect the current schema's references first: besides `container_asset.container_id`, the shipped model exposes `smart_crate_rules.container_id`, `alias_container.container_id`, `container_keep_alive_list_tbl`, and master `location_container`. The shipped unreferenced-crate routine deletes membership then container; it does **not** establish all cascade behavior. Validate foreign keys/triggers in the supported current schema and remove only references to the planned owned IDs. Report target-added aliases/children as affected references or conflicts. Never delete `asset` or `space_asset` for a playlist tombstone.

Treat the master aggregate as part of the same publication. Resolve its `location_container(location_id,external_container_id)` before name matching; preserve the existing aggregate `container_id` during rename/move. Remove obsolete `container_asset` rows referring to that location mapping, remove the matching mapping, and remove an aggregate container only when it is owned and has no remaining locations, aliases or children. Never remove another location's memberships. The aggregate refresh reconciles the planned location/master mutations; deletion is not confined to `root.sqlite`/`location.sqlite`. Preserve all location registration and portable asset identity records. No Serato 3 crate-file work is involved.

### Pioneer USB

Bind the device/library identity and native playlist ID. In Plus SQL update `playlist(playlist_id,playlist_id_parent,name,sequenceNo,attribute)` in place. Delete only `playlist_content` for planned owned IDs before deepest-first `playlist` deletion; preserve other memberships and all content/cue/art/history/settings tables.

Regenerate the twin PDB playlist tree (table 7: parent, sibling order, ID, folder flag, name) and playlist entries (table 8: order, track ID, playlist ID) from that same merged catalog. PDB-only import must keep playlist IDs before applying the plan. Preserve opaque unrelated PDB rows/config and publish Plus/PDB together. A deleted playlist does not authorize deleting its tracks, copied audio, ANLZ or artwork: existing file cleanup still operates only on assets unreferenced by the complete merged catalog. Removing playlist membership is separate from track/media ownership.

## Preview, acknowledgement and recovery

1. Under the existing service lease, capture one source snapshot, including global/playlist revisions, active hierarchy and explicit local removal groups. Read the ownership ledger generation and complete current native graph.
2. Resolve bound IDs and calculate all mutations, affected parent/reference fingerprints, path/directory changes and conflicts. Guard targets by native incarnation and logical owned structure. Allow unrelated native changes since the last sync; do not equate a different whole-database hash with loss of ownership.
3. Stage native mutations on copies, return the proposed report and verify readback: IDs, parent links, ordered memberships, deletion closure, no dangling references, and unchanged unrelated records. Capture exact current target file hashes/directory manifests for the publication journal.
4. At Apply, recheck app closure, device identity, source revision, source audio hashes, ownership generation and every captured target precondition. Retain the current local write transaction/publication gates. An external edit after preview makes it stale.
5. Publish and verify the complete file transaction. Atomically install the next local ledger with `last_journal` and generation, then update Changed hashes and mark the library journal acknowledged. Store prior and proposed ledger snapshots in the transaction's local metadata. Do not acknowledge failed/unverified deletes.
6. Recover the crash window where native apply succeeded but local acknowledgement did not: verify journal after-state, finish the same ledger transition idempotently, and do not allocate another playlist. If after-state no longer matches, require reconciliation/restore. A failed or interrupted apply retains prior ownership and the pending report.
7. Restore native backups and the corresponding prior ledger snapshot only after verification. Keep audit tombstones. Invalidate Changed cache separately; partial/stale restore remains unsettled and cannot erase ownership proof or acknowledge completion.

Pending source restore cancels an unapplied tombstone. Restoring a source after an acknowledged native deletion creates/rebinds a current native object; never blindly reuse an ID now occupied by another playlist. An already missing owned target can be reported as verified absent and tombstoned without deleting a same-name replacement.

## Focused verification contract

| Area | Acceptance evidence |
| --- | --- |
| Identity | Rename, parent move, case-only rename, A/B name swap, duplicate-name collisions: one source binding and no orphan duplicate. Missing/reused ID and changed target incarnation refuse destructive operations. |
| Scope | Full/Changed remove only proven owned local tombstones. Omission from Playlist or Modified active input deletes nothing without an explicit owned source tombstone. Removing the final playlist works with zero exported tracks. Version-1 baseline cannot authorize deletion. |
| Hierarchy | Owned subtree deletion succeeds deepest first; moved-out children survive. Unowned native child blocks parent deletion. Partial reorder retains unselected sibling order. |
| Native references | Engine next-pointer uniqueness and derived rollups; djay old/new parent arrays and link objects; Serato location/master mappings plus foreign locations; Pioneer SQL/PDB hierarchy agreement and opaque-row retention. |
| VDJ journal | Empty folders, unknown child files, old/new order files, case-only moves, failure at every operation, stale child manifests and symlink substitution. Restore reproduces the exact previous directory tree. |
| Recovery | Fail before/after target publication and before/after ledger/cache acknowledgement. Reopen/recover idempotently; restore reinstates prior bindings; stale target/source/ledger revisions cannot be acknowledged. |

Focused regressions use synthetic native fixtures, directory manifests and independent reader checks. Supported-schema trigger/FK behavior constrains Serato/Engine mutations. Synthetic passes establish mutation/rollback contracts, not successful reopening in each DJ application; record that later runtime evidence separately.

## Evidence and implementation entry points

Repository observations refer to the current working implementation's symbols, not a released feature:

- `crates/cuerithm-library/src/dj_sync_service.rs`: `Baseline`, `select_changed`, `apply_dj_sync`, `restore_dj_sync`, `invalidate_baseline`; `dj_sync_service/ownership.rs` and `ownership_tests.rs`: ledger registration, generation and acknowledgement recovery.
- `crates/cuerithm-library/src/dj_sync.rs`: `project_playlists`; `playlists/edits.rs`: Remove/Restore; `playlists/mod.rs`: durable IDs, removal groups and revision bumps.
- `crates/cuerithm-dj-sync/src/virtual_dj.rs`: playlist path/ownership planning; `transaction.rs` and `transaction/case_renames.rs`: typed file, directory and case-rename apply/restore.
- `engine_dj.rs`: `write_playlists`; `djay_pro.rs`: `write_playlists`, `merge_children`, `stable_id`, `INDEXES`; `serato4.rs`: `container`, `write_playlists`; `serato4/aggregate.rs`: `refresh_master`; `pioneer_usb/database.rs`: `write`, `import_pdb`, `pdb_tables`.

Shipped converter static evidence, expressed as aggregate format findings only:

| Artifact range | Finding |
| --- | --- |
| Lexicon DJ 1.11.15 renderer, 363438–363606 | Current MyLists path/order layout and bulk replacement behavior. |
| 269315; 358731–359030; 359110–359168 | Engine playlist constraints/delete triggers, ID/path CRUD, smartlist path links and derived ancestor membership. |
| 365338–365365; 365847–366003 | djay index invalidation, playlist UUID graph, membership links and bulk cleanup. |
| 253390–253876; 275351–275408; 275491–275506 | Serato 4 crate upsert/order/delete, reference tables and database UUID fields. |
| 207539–207626; 210312–210590; 211874–212115 | Pioneer current SQL schema, carried device state and shared SQL/PDB playlist projection. |

Private extraction provenance, artifact hash and working-source hashes are recorded under the ignored local `.analysis/dj-software-sync/playlist-ownership/` evidence directory. No proprietary implementation snippets, user library paths, credentials or music are included in this document.

The focused suite covers native identity, ownership, tombstone-only changes, foreign
children, representation transitions, file/directory rollback, case-only subtree
rename/restore and acknowledgement crash windows. Test totals are intentionally not
frozen here. Current-head checks and integration evidence are recorded in
[PR 12](https://github.com/johnneerdael/CueRithm/pull/12). Target DJ/physical player
acceptance remains a separate verification step.
