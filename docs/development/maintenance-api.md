# Maintenance native API

This is the backend handoff for the user-owned UI. Import `nativeMaintenance`
from `apps/desktop/src/maintenance-api.ts`; its `MaintenanceBridge` type and
`maintenance-types.ts` define the requests and results. No styling or navigation
changes are included in this checkpoint.

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
- `scanUnused(operationId, request)`: explicit folder with all/include/exclude
  extension filters. Show `complete` and `issues`; partial is not a complete scan.
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

## Publication boundary

There is no apply/merge/delete/undo command in this checkpoint. Do not add an
active Apply or Remove button for these reports yet. Copying tags/cues, archive
changes, ordinary-playlist replacement, durable undo and separately reviewed file
removal still need their guarded publication implementation and native acceptance.
No user collection has been modified by these generated-fixture tests.
