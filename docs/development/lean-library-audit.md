# Lean library audit

The library is meant to hold tags, links to files and artwork. It must never hold audio.
Music removed from the library must leave nothing behind, and Undo is bounded: ten
operations. This page is the audit that drives issue #46. It lists every place CueRithm
retains data in its data folder or in the library database, what keeps it, how big it gets and
what was decided. The verdict column is the decision for this change, and the last section records what shipped and what was deliberately left, and
the last section says what it deliberately leaves.

How to read the numbers: sizes marked *measured* come from a read-only inspection of a copy of
one real library (26 tracks, a rekordbox collection of 3,417 tracks, 36,752 cue rows). Sizes marked
*estimate* are computed from the field widths in the code and are not measurements. No
per-track data from that library is recorded here.

## Measured on one real library

| Item | Before | After this change |
|---|---:|---:|
| `library.sqlite` | 252 MiB | 31 MiB |
| `library.sqlite-wal` | 129 MiB | 0 (truncated at open) |
| `tracks` (26 rows) | 0.7 MiB | 0.7 MiB |
| `native_records` (62,208 rows) | 99.4 MiB + 6.9 MiB index | 21.7 MiB + 6.1 MiB index |
| `native_track_links` (3,417 rows) | 20.8 MiB | 1.1 MiB |
| Free pages inside the database file | 31,248 pages (122 MiB, 48% of the file) | 0 |
| `cache/artwork` (26 files) | 35 MiB | unchanged (artwork is allowed) |
| `cache/waveforms` (29 files) | 4.1 MiB | unchanged (rebuilt on demand) |
| `analysis-components` | 18 MiB | unchanged (the bundled model, reinstalled when missing) |

Upgrading that copy from schema 28 to the new schema 30 took 1.2 seconds in a debug build: the
migration itself, one rewrite of the live data to enable incremental auto-vacuum, and a
truncated log.

The copy had no saved scans, reviews, previews, cleanup or metadata history and no tag-write
originals, so those rows below are *estimates* from the code, not measurements.

## 1. The SQLite file itself

| What | Where | Writer / reader | Why it exists | Bound today | Size | Verdict |
|---|---|---|---|---|---|---|
| Write-ahead log | `library.sqlite-wal` | SQLite; one long-lived connection (`store.rs`) | WAL journal mode, `synchronous=FULL` | None: no `journal_size_limit`, default autocheckpoint, no checkpoint on exit (the process ends without closing the connection) | measured 129 MiB | **Bound**: `journal_size_limit` 16 MiB, autocheckpoint 1000 pages, `TRUNCATE` checkpoint at open, after large operations and on a clean shutdown |
| Free pages | inside `library.sqlite` | SQLite | rows deleted by earlier versions | None: no `auto_vacuum`, no `VACUUM` anywhere | measured 122 MiB | **Reclaim**: new libraries use incremental auto-vacuum; an older one is converted once by a single `VACUUM` (only when the live data is up to 2 GiB and the disk has room for three times that), then freed in bounded steps |
| Library-switch and restore staging databases | `libraries/<id>/`, `library-backups/` | `generations.rs`, `library_backup.rs` | recovery after a library restore | None; superseded generations are kept | one full database each | **Keep** (explicit user recovery; not derived data) |
| Catalog runtime database | `catalog-runtime/catalog.sqlite3` | provider setup | provider response cache | rows and bytes capped (256 entries, 32 MiB) | measured 0.1 MiB | **Keep** |

## 2. DJ-software copies

| What | Where | Writer / reader | Why it exists | Bound today | Size | Verdict |
|---|---|---|---|---|---|---|
| `native_records.json` | table, one row per record of the rekordbox database | `native_sync::store_records`; read back only by `native_playlist_sync` for the four playlist and MyTag tables, plus `native_record()` (tests only) | baseline "kept for conflict detection" | None | measured 99 MiB (62k rows) | **Replace by a smaller form**: every record keeps its key and the SHA-256 of its content (all that a later observation compares); the content is kept only for `djmdPlaylist`, `djmdSongPlaylist`, `djmdMyTag` and `djmdSongMyTag`, which native playlist sync reads back |
| `native_track_links.observation_json` | table | `native_sync`; read by the reconciliation review for a *linked* track and by the portable library backup | last observed rekordbox track | None | measured 18 MiB for 3,417 tracks, none linked | **Replace by a smaller form**: a DJ-software track with no library track keeps `observation_hash` and no observation. A linked track keeps its observation. |
| `native_track_links.baseline_json` | table | `native_sync`, `rekordbox_sync_service` | the real three-way merge baseline (local, remote, owner, acknowledged per field) | one row per linked track | measured 7 KB | **Keep** |
| `rekordbox_sync_operations.journal_json` | table, one row per sync | `rekordbox_sync_service` | frozen plan, receipts and `ledger_before`: every row of the seven `native_*` tables for the target, needed to restore a sync | None: completed operations stay for ever | estimate: one full copy of the mirror per sync | **Bound** (see section 8): the ledger shrinks with the records; only the newest ten completed operations keep a restorable ledger |
| `native_reconciliation_plans`, `native_playlist_acknowledgements`, `native_playlist_bindings` | tables | native sync | reviews and ownership | bindings replaced on rebind | small | **Keep** |
| `sync_links` | table | no live writer | legacy baselines | none | small | **Keep** |
| `sync/baselines/*.json`, `sync/ownership/*.json` | files | `dj_sync_service` | id to hash maps per target | one per target, replaced | KBs | **Keep** |
| `sync/<id>/` for Engine DJ, Serato, VirtualDJ, djay Pro, Mixxx, Pioneer USB | files: `staged/N`, `backup/N` (previous version of each target file), `journal.json`, `library.json` | `cuerithm-dj-sync` `transaction.rs` | restore a sync | discarded only while the preview is still prepared; applied journals stay for ever | one copy of every file written or replaced (media files for Serato, Engine DJ and Pioneer USB) | **Bound** to the newest ten operations per target, see section 8 |
| `sync/rekordbox/operations/<id>/` | files: `before.db`, staged image, `build/`, `audio/` | `rekordbox_sync_service` | restore, crash recovery | prepared operations only | a copy of the rekordbox database per sync | **Bound** with the operation, see section 8 |
| `organization-references/<op>/` | files | `organization_references` | recovery of DJ-library path references | none | DJ library files, not audio | **Keep** (reported in storage); outside this change |

## 3. Audio copies in the data folder

Every row here is a whole audio file kept in the data folder. After this change none remains
once the operation has settled.

| Write | Where | Created by | Read after commit by | Bound today | Verdict |
|---|---|---|---|---|---|
| Automatic tagging (Incoming) | `tagging/cuerithm-tags-*/original.<ext>`, plus `art-original-N.jpg` for a cover file | `cuerithm-autotag` `write.rs` | nothing: automatic tagging has no Undo. Crash recovery and rollback use it only before commit. | none | **Purge**: removed with the other private files when the write settles; older ones are removed once, at open |
| Manual cleanup | `cleanup/cuerithm-tags-*/original.<ext>`; every Undo adds `cleanup/cuerithm-undo-*/` with two more full copies | `cleanup/mod.rs`, `artifacts.rs` | Undo copies the original bytes back | none | **Replace by a smaller form**: Undo restores the previous tag values recorded in the journal, through the same checked write |
| Manual metadata edit | `metadata-edits/staging/cuerithm-tags-*/`: two full copies per previewed track from Preview until apply, discard or supersede; `original.<ext>` kept after commit; Undo adds `current.audio` and `restore.audio` | `metadata_edit/service.rs`, `journal.rs` | Undo of an edit to a finished track copies the original bytes back | previews retired when stale or discarded; committed backups never | **Replace by a smaller form**: Undo restores the recorded previous values and replaced artwork bytes (artwork is allowed); backups go when the write settles |
| Duplicate merge with a donor tag copy | `maintenance/staging/cuerithm-tags-*/original.<ext>` (the keeper) plus `undo-*/` with two more copies | `maintenance/donors.rs` | Undo copies the original bytes back | none; archive-only merges never enter the ten-operation window | **Replace by a smaller form**: the keeper's previous tag snapshot is recorded; Undo restores values |
| Reviewed unused-file removal | `maintenance/removal/remove-*/original-N` (every removed file, and its Serato or VirtualDJ sibling) | `maintenance/removal.rs` | Restore | none; disposal only on explicit user action. The file itself is **permanently deleted**, not sent to the Trash | **Purge**: files go to the system Trash, no copy is kept |
| DJ-software sync (non-rekordbox) | `sync/<id>/staged`, `backup` | `cuerithm-dj-sync` | restore a sync | prepared previews only | **Bound** (section 2) |
| rekordbox sync | `sync/rekordbox/operations/<id>/audio/*`, converted WAV copies in `sync/rekordbox/assets/<id>/staged` | `rekordbox_sync_service` | restore, recovery | prepared operations only | **Bound** (section 8) |
| Decode scratch | `cache/media/cuerithm-*` | `cuerithm-media` | the decode in flight | cleaned at every start | **Keep** (transient) |
| DJ timing scratch | `cache/dj-timing/timing-*` | `dj_sync_service` | the sync in flight | `TempDir` | **Keep** (transient) |
| Fingerprint scratch | `cache/fingerprints/*` | `maintenance/scan.rs`, `broken.rs` | the scan in flight | `TempDir`, crash residue not swept | **Sweep** crash residue at open |
| Staging directories orphaned by a crash between staging and the journal insert | `tagging/`, `cleanup/`, `metadata-edits/staging/`, `maintenance/staging/` | the writers above | nothing | none | **Sweep** unreferenced `cuerithm-tags-*`, `cuerithm-undo-*` and `cuerithm-*` directories older than a day at open |

Also on the user's music folder: while a write is in flight a dot-file `.cuerithm-<id>.tmp`
holds the displaced original. It is the live restore point and is removed after commit; it is not
an app-data copy.

## 4. Saved scans, reviews, previews and reports

All of these are rows of `maintenance_scans (id, kind, payload_json, created)`. Path byte arrays
serialise as JSON number arrays (about 0.35 KB per path). *Estimates* from the code, per track in
the library unless noted.

| Kind | Content | Written / read by | Needed by | Bound today | Verdict |
|---|---|---|---|---|---|
| `duplicates` | every non-archived track with id, path, title, artist, hash, fingerprint and file proof, about 1.7 KB per track (50,000 tracks: about 85 MiB) | `scan.rs`; review creation | beginning a review | a removal deletes all, a relocation deletes all; a new scan adds another | **Bound**: keep the newest 3; never one an open review points to |
| `duplicate_review` | a complete copy of the scan plus the groups, rewritten on every edit | `review.rs`; merge preview | the preview and the apply | purged on any removal | **Bound**: keep the newest 3; never one a live plan points to |
| `duplicate_merge_plan` | a third full copy; kept after it is consumed | `merge.rs`; apply | apply | purged on any removal | **Bound**: delete when consumed; at most 3 unconsumed |
| `broken` | about 0.8 KB per track | `broken.rs`; missing previews | previews | purged by id on a removal, deleted on relocation | **Bound**: keep the newest 3 |
| `unused` | every library path (as byte arrays) and a listing of every scanned folder | `unused.rs` | removal preview and apply | only relocation clears it; **not** purged on a removal | **Bound**: keep the newest 3; purge on a removal |
| `unused_removal_plan` | up to 10,000 files at about 0.6 KB | `removal.rs` | apply | kept after it is consumed | **Bound**: delete when consumed or after a day |
| `unused_removal_operation`, `unused_backup_disposal*` | history of unused removals and their backup disposals | `removal.rs`, `retention.rs` | Restore | never pruned | **Replace**: with the Trash there is no backup to restore from; history lines keep a small summary and the ten newest keep their detail |
| `missing_removal_preview`, `missing_relink_preview` | previews | `missing.rs` | apply | pruned after a day on the next save | **Keep** |
| `missing_repair` | one history line per repair, removal lines trimmed to a summary by the ten-operation window | `missing.rs` | Undo | removals bounded; relinks and Undo lines are not | **Bound**: relink and Undo lines keep a summary after the window |
| `duplicate_merge`, `duplicate_merge_undo` | legacy path, no longer called by the app | `merge.rs` | none | none | **Keep** (legacy) |
| `maintenance_fingerprints` | one row per track per engine identity; a new identity per release leaves the old rows behind | `scan.rs` | duplicate scans | cascades on track delete | **Bound**: rows of an engine identity other than the current one are deleted |
| `maintenance_writes`, `maintenance_write_files` | duplicate-merge journals, with before and after track images | `donors.rs` | Undo, recovery | only removals leave the window | **Bound**: archive-only merges join the ten-operation window; settled journals lose their file rows |
| UI references | the last scan and review ids in browser storage | `MaintenanceShared.tsx` | reopening a view | n/a | a missing row already means "scan again" |

A cleanup must never delete: a prepared or conflicting write, a write with unfinished cleanup or an
unacknowledged rollback; the committed writes inside the ten-operation window and every History
line; the newest scans the UI still points to; a `duplicate_merge_plan` and the review it points
to; a live unused scan and its plan; `maintenance_track_epochs` and `maintenance_ignored_tracks`
of live tracks.

## 5. History and payload tables

| Table | Content | Bound today | Verdict |
|---|---|---|---|
| `tag_writes`, `cleanup_writes`, `metadata_edit_writes` (+ `_files`, `_rows`, `_batches`) | journals with `analysis_json` (a second full analysis), metadata, receipts and path BLOBs | none; only a flag marks cleanup done | **Keep** the journal (History needs the summary); the audio paths go with the audio copies (section 3) |
| `cleanup_plans`, `cleanup_batches` | frozen plan with a proposal per track, result | none | **Keep** (small per use) |
| `metadata_edit_drafts`, `_previews`, `_overrides` | drafts and previews, up to 64 MiB per row | previews retired when stale or discarded | **Keep**; the preview's staged audio is the transient cost in section 3 |
| `metadata_artwork_assets` + `metadata-edits/assets/<id>` | artwork bytes | never deleted | **Keep** (artwork is allowed); referenced by Undo |
| `preparation_checkpoints` | evidence copy of the analysis and metadata (only when organisation is on) | completion only sets a state | **Keep** (outside this change) |
| `preparation_match_snapshots`, `_generations`, `_review_artwork` | match evidence, up to 2 MiB per row | per track, except review artwork of older generations | **Purge**: review artwork of every generation of a removed track |
| `preparation_analysis` | a second copy of the analysis per source hash and settings revision | removed with orphaned hashes | **Keep** |
| `playlist_edits`, `playlist_previews` | history diffs including member lists; previews expire on use | never swept | **Keep** history; sweep expired previews |
| `organization_*` | plans, batches, operations, references | discard only | **Keep** (outside this change) |
| `curation_*`, `manual_publication_*` | publications and mutations, never pruned | none | **Keep** (small) |
| `relocation_plans` | 24 hours | garbage-collected at open | **Keep** |
| `artifact_cleanup_moves` | rows stay after they are removed | none | **Keep** (small) |
| `jobs`, `cue_revisions`, `accepted_musical_revisions` | analysis history | none | **Keep**; rebuilt only on request |

## 6. Caches

| What | Bound today | Verdict |
|---|---|---|
| `cache/artwork/<source_hash>` (each up to 8 MiB) | removed when no track or removal still needs it | **Keep** (artwork is allowed; part of the cover kept for Undo) |
| `cache/waveforms/<key>.json` | removed for the two cached resolutions only | **Purge**: remove every resolution of a removed track's content |
| `analysis-components/<sha>.onnx` | older hashes are never removed | **Keep**: one fixed bundled model |

## 7. Purge on removal

`maintenance/purge.rs` is the one list. The audit found these gaps:

| Gap | Fix |
|---|---|
| `maintenance_scans` kind `unused` names every library path and was not purged | purged on any removal |
| `maintenance_writes` journals keep the full image of a track removed by another merge | settled journals are trimmed |
| `preparation_review_artwork` of older generations stays | all generations of the removed tracks |
| waveform keys for resolutions other than the two cached ones | every resolution |
| `tests/removal_engine.rs` exempts `maintenance_scans` and `maintenance_writes` from its trace check | the check covers every table with a track id column and fails for a table nobody listed |

## 8. What this change does and does not do

Sync operations (sections 2 and 8): a completed sync keeps its summary for ever, but only the newest
ten completed operations of a target keep what a restore needs (the ledger, the before-image and
the backups of replaced files); an older one reports that its restore is no longer available.

Done: bounded and truncated log, reclaimable space, the lean DJ-software copy with a migration
that converts existing rows, and the audio copies and saved-scan bounds in sections 3 and 4.

Deliberately kept, with the reason:

- `tracks.analysis_json` (about 27 KB per track) and `preparation_analysis`: they are the
  product, not a copy of audio. A second copy in `tag_writes.analysis_json` stays because the
  journal needs it to finish an interrupted write.
- The four playlist and MyTag record tables keep their content in `native_records` (9.5 MiB for
  the measured collection): native playlist sync compares them with its bindings.
- Artwork, in the cache and in `metadata-edits/assets`.
- Library backups, retained library generations and the pre-restore snapshot: they are the
  user's own recovery data.
- `organization-references`, `curation_*`, `organization_*`, `preparation_checkpoints`: small,
  and a bound there needs its own review.

<!-- measured on a copy of one real library, read-only; sizes of tables without data are estimates from the code. -->
