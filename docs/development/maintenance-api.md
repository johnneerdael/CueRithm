# Maintenance native API

This is the native contract behind the desktop Maintenance screens
(`apps/desktop/src/views/Maintenance.tsx` and `views/maintenance/`). Import
`nativeMaintenance` from `apps/desktop/src/maintenance-api.ts`; its
`MaintenanceBridge` type and `maintenance-types.ts` define the requests and
results. `use-maintenance-task.ts` owns the operation-ID, progress and
cancellation protocol described below: one task slot shared by every tab, a
fresh `crypto.randomUUID()` per operation, progress filtered by ID and kind, and
the slot held until the cancelled promise settles.

## Scanning and cancellation

Create a fresh `crypto.randomUUID()` for every long operation, retain it for
cancellation, and subscribe to progress before starting. Filter progress by both
`operation_id` and `kind`. Unsubscribe when the view leaves; ignore late results
from an older operation. One maintenance operation runs at a time. Cancel it
with `cancel(operationId)` and wait for its original promise to settle before
starting another; cancellation acknowledgement does not release the slot.

- `scanDuplicates(operationId, request)`: full library, selected playlists, or
  full-library groups intersecting selected playlists. Specify all request fields.
  The `method` distinguishes exact decoded audio from artist/title candidates.
  Exact grouping requires matching native PCM digest, sample rate and channel
  count. Rescan reports made with an older scanner implementation before review
  or merge; raw fingerprint caching remains separate from classification.
- `scanUnused(operationId, request)`: explicit folder with all/include/exclude
  extension filters. Filters choose removal candidates; sidecar association uses
  the complete regular-file inventory, including excluded base audio files.
  Show `complete` and `issues`; partial is not a complete scan.
- `scanBroken(operationId, {inspect_audio})`: availability or strict decoding.
  Preserve distinct missing, unavailable, unsupported, timed-out and decode-error
  states. Presence alone does not establish decodability.
- `duplicateScan`, `unusedScan` and `brokenScan`: reload a persisted report using
  a fresh operation ID and its report ID. Reload rechecks source proofs and
  revisions; always surface `stale` and `stale_reason`.
- `exportUnused(operationId, reportId)`: return the path-list text for the report.
  This writes no disk file and performs no removal. Retain report/staleness context
  when offering its text to the user.

Native errors use `cancelled`, `maintenance_busy`, `worker_failed` or the existing
`operation_failed` code and a message. App shutdown cancels active maintenance
work and waits for the worker to release its slot. Scans do not change track
metadata, audio files, cues, archives, preparation jobs or playlist membership.

## Duplicate review

1. `beginReview(operationId, scanId)` creates a review from a fresh scan.
2. `getReview(operationId, reviewId)` reloads its current choices and rechecks
   source/scope proofs. Choices survive restart.
3. `editReview(operationId, reviewId, expectedRevision, edit)` applies a group
   edit or a batch preference. Use the revision from the last successful response.
   On a revision conflict, reload before editing again. On stale inputs, rescan.

Group edits select a keeper, set/clear the independent tag/cue donor, lock,
unlock or toggle a track ignore. Unlocking a locked group with a donor enters
`copy_cues`; forced unlock enters normal merge selection. Selecting the keeper
as donor clears the donor. Preferences optionally override locks and select a
separate best-cue donor when they change the keeper. Streaming/play statistics
are outside the approved scope.

An ignored selected keeper remains visible; it cannot be applied. Manual
unavailable or ignored keeper/donor choices are rejected. A review's own ignore
edit adopts the resulting ignore revision; other reviews/scans remain tied to
their earlier inputs and become stale. `onChange` lets other open views reload.

`ignores()` provides a coherent stamped list independent of any review.
`setIgnores(trackIds, ignored, expectedStamp)` can restore ignored tracks even
when their original review is stale. All IDs must still exist; unknown IDs or a
stale stamp reject the entire edit. Reload the list after a conflict.

## Merge preview and history

`previewMerge(operationId, reviewId, expectedRevision, groups)` persists a concrete
plan for the selected group IDs. It checks current review, library, source and
scope identities. Its groups retain the mode and exact-audio/candidate distinction,
keeper, archive/unarchive actions and explicit donor-to-target copy tasks. Playlist
changes include ordered before/after membership arrays.

`mergeHistory()` returns committed archive/reference transaction records and
`undone_by` markers, ordered newest first. These are history records; they do not
promise that undo remains available after later source or library changes.

## Coordinated publication and undo

`applyMerge(operationId, previewId)` runs the complete donor/archive/reference
coordinator. It stages all selected recipient files and validates their native
metadata, decoded PCM and source identities before reserving publication targets.
Durable backups and a prepared journal precede any exchange. Metadata, custom tags,
manual cue revisions, grid publication, archives, playlist membership, consumed
preview token and committed journal share one SQLite commit. The raw recipient
analysis remains intact; its presentation publication can reflect the imported
cues/grid with manual-import provenance. Duplicate merging removes no music files.

`undoMerge(operationId, operationIdToUndo)` uses a new reverse file journal and
restores the exact original file bytes plus the captured library/cue/tag/member
state. Use `mergeHistory()` to discover operations after restart. Track/playlist
entity epochs reject relevant edits and A/B/A cycles. Unrelated Incoming results
and derived smart-list refreshes do not invalidate a version2 undo. Older version1
archive-only history retains its original conservative undo contract.

Both operations emit `library-change` and `maintenance-change` after success.
`updated_ids`, `archived_ids`, `restored_ids` and `affected_playlists` identify what
to refresh. Reload tracks and playlist state after the event; do not infer
publication from a staging/progress event. Copy-only mode imports selected tags,
cues and grid without archive/reference actions.

App startup rolls back interrupted prepared operations. An interrupted undo
returns to the previously committed after-state and remains retryable. Later user
file edits are preserved and recorded as recovery conflicts with backups retained.
`recovery()` lists pending/conflicted operations and incomplete recovery cleanup;
`retryRecovery(operationId, journalId)` retries guarded rollback/cleanup after the
user resolves the conflicting source/location. It cannot overwrite an unknown
later file version. Show these errors rather than presenting a partial merge as
successful. Cancellation propagates through hashing, decoding and tag staging;
once exchanges have started, rollback finishes before the operation settles.

Schema8 permits manual cue revisions with a null `job_id`; it preserves older
analysis-job cue ownership. A no-tagging build refuses pending file journals so
it cannot skip required recovery. Native tag, cleanup and duplicate writes share
target reservations and the publication gate. Source/scope/review/epoch checks
remain server-side.

Generated tests exercise all fourteen audio layouts, SQL failure, restart at
prepared/exchanged points, interrupted undo, subsequent file edits, relevant ABA,
unrelated Incoming changes and migration. A separate packaged macOS test app
exercised real invoke Apply/Undo, cue/grid display through track details, history,
playlist replacement/restoration and zero pending recovery on generated WAVs.
These checks do not establish branded maintenance-screen or full user-library
acceptance. The user-owned UI can now connect its review/apply/undo controls to
these commands. Reviewed disk-removal APIs are described below; their dedicated
screens remain unfinished.


## Reviewed unused-file removal

Use `previewUnusedRemoval(operationId, scanId, selectedIds)` with IDs from a
complete, fresh unused-file report. The persisted preview returns the exact
selected files, the first present Lexicon-style Serato/VirtualDJ sibling, hashes,
roles and total bytes. It accepts no arbitrary source paths. Show all preview
entries, including siblings, before the explicit removal action.

`applyUnusedRemoval(operationId, previewId)` captures and flushes verified
original backups before recording a prepared operation. It rechecks the scan,
current references (including archived records and physical aliases), source
identities, root containment and shared destination reservations. Same-volume,
no-replace moves precede the final journal/preview-consumption commit. Scanning,
normal preparation and duplicate merging never call this method automatically.
The action changes no track metadata, jobs, analysis, cues or playlist membership.

`unusedRemovalHistory()` discovers operations and recoverable backups after
restart. `restoreUnusedRemoval(operationId, removalId)` stages the original bytes
and restores only unoccupied original paths. It refuses altered/missing backups
and newer files at those paths. A successful restore marks the removal undone;
a repeated restore is rejected. Both operations use the existing native task
lease/cancellation protocol and emit maintenance/library change events.

Interrupted prepared operations roll back on startup. A later user file stays
in place and yields a visible conflict with the captured source/backups retained.
Unresolved recovery reserves its source against another removal, tagging,
cleanup or donor publication. Missing recovery storage does not prevent library
access. `retryUnusedRecovery(operationId, journalId)` retries guarded recovery
after the conflicting source or storage is resolved. Never hide `state`, `error`
or `cleanup_pending`; a failed action may require history refresh.

Schema10 prevents older readers from opening journals that include parent-folder
restoration or backup disposal. Version1 file-removal journals remain readable.
These operations also recover in builds without catalog tagging support.

Previews include `directories` and explicit `folder_metadata` entries when parent
cleanup would remove `.DS_Store` or `Thumbs.db`. Folder planning follows selected
primary-file/sidecar order. It preserves cover art and the reviewed root. Cleanup
removes only reviewed empty parents, never recurses, and leaves a parent containing
new files alone. Restore recreates missing parents shallow-first and restores
captured permissions; it preserves existing replacement directories. Restart
rolls back owned folder creation when an interrupted restore has not committed.

### Retained originals and disposal

`unusedRemovalHistory()` supplies `restore_available` as history eligibility;
actual restore still validates current paths, backup contents and reservations.
Backups occupy application storage until the user chooses to dispose of them.
`backups_disposed` is true once a removal's backups have been permanently disposed of. A screen that offers
disposal lists the original removals (not the reverse journals) whose `backups_disposed` is false and whose
`cleanup_pending` is false, whether or not they were restored, since a restored removal keeps its copies until
they are disposed of.

`previewBackupDisposal(operationId, removalIds)` returns the exact owned backup
files, total bytes and `restores_lost` IDs. Select original removal operations,
including already-restored operations, rather than reverse-journal IDs. Related
restore rows share one backup set and are counted once. Show the loss of Restore
before invoking the explicit irreversible `disposeBackups(operationId, previewId)`
action. A scan, normal preparation, duplicate merge or file removal never invokes
backup disposal automatically.

Disposal requires unchanged related history and backup identities. A durable
prepared journal precedes moves within recovery storage; committed disposal marks
related originals unavailable for Restore before deleting verified retired copies.
A restore after preview invalidates that preview. Recovery returns backups on a
pre-commit interruption and finishes cleanup after a committed interruption.
Later backup versions are preserved as visible conflicts. No music path is
deleted, restored or rewritten by disposal.

Use `backupDisposalHistory()` and `retryBackupDisposal(operationId, disposalId)`
for failures or incomplete cleanup. Always surface `state`, `error` and
`cleanup_pending`. `size_bytes` is the reviewed backup amount; incomplete cleanup
is not proof those bytes were released. Filesystem snapshots and clones may also
retain physical storage after logical file removal. Branded views remain
user-owned; their confirmation and final live-library acceptance remain required.


## Retained preparation and cleanup originals

Tagging and metadata cleanup keep a copy of each file they rewrite. These stay in
application storage until the user disposes of them. Use `previewRetainedOriginals(
operationId, includeUndoable)`, `disposeRetainedOriginals(operationId, previewId)`,
`retainedDisposalHistory()` and `retryRetainedDisposal(operationId, id)`.
`getTaggingStorage()` reports the current count and bytes.

- **Eligible originals.** A preparation tag write's original is offered once the write is
  committed or rolled back and its own cleanup finished. A cleanup write's original is
  offered when the write was undone or rolled back, and for the undo record's own copy.
  A committed, not-yet-undone cleanup original is what Undo restores from; it is offered
  only when `includeUndoable` is true, and `undo_lost_writes` / each file's `undo_lost`
  say so. Writes that are prepared, in conflict or still cleaning up are never offered.
- **Preview and apply.** The preview is persisted and consumed by one apply. Apply rechecks
  that the eligible set is identical, that each file is a regular file inside its own write
  workspace under application data, and that its SHA-256 still matches the recorded
  original. Any difference refuses the whole disposal.
- **Order of effects.** One transaction clears the database's references to the files and
  records the journal. File deletion follows. An interruption can therefore leave only
  unreferenced files; the history shows `cleanup_pending` and a retry (or the next
  startup) finishes it. A file whose bytes no longer match is kept and reported, never
  deleted. An emptied workspace folder is removed; a non-empty one is left alone.
- **Undo afterwards.** Cleanup Undo reports "Undo is no longer available: the retained
  original was disposed of" instead of guessing. The applied file and library rows stay as
  they were. No music file in a user folder is touched by disposal.
- **Space.** Counts are logical file sizes. Filesystem snapshots or clones can keep the
  physical space in use after deletion.

The cleanup source is covered by library tests (offer rules, Undo loss, replay refusal,
changed-original refusal, interrupted deletion and retry). The tagging source shares the
same code path but is not exercised by a dedicated end-to-end test here.
