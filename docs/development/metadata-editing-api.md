# Metadata and match-review services

The native editor shares `TagWritePlan`, PCM verification, file artifacts and retained native parent authority with existing publication services. Ready-track edits do not manufacture a `JobRun`, call analysis publication or regenerate cues. Incoming overrides use the real preparation owner and its protected decision/outbox contract.

## Draft and publication commands

| Command | Result |
| --- | --- |
| `metadata_editor_snapshot(operation_id, track_ids)` | Complete source-bound typed native snapshots and separate measured scalars. |
| `create_metadata_draft(operation_id, track_ids)` | Durable draft with stable selected IDs and native revision. |
| `metadata_draft(id)`, `metadata_drafts()` | Recover persisted draft state. |
| `update_metadata_draft(id, expected_revision, patch)` | Compare-and-set a new draft revision; invalidate earlier unused previews. |
| `preview_metadata_edits(operation_id, id, revision)` | Freeze and verify the complete requested draft, including per-field outcomes. |
| `apply_metadata_edits(operation_id, preview_id)` | Publish the exact server-side plan or atomically queue Incoming overrides. |
| `recover_metadata_edits(operation_id, batch_id)` | Resume retained native publication evidence. |
| `undo_metadata_edits(operation_id, batch_id)` | Apply a guarded inverse while preserving unrelated values and measured DSP. |
| `discard_metadata_draft(id, revision)` | Discard that revision without discarding required publication/recovery assets. |
| `metadata_edit_history_page(offset, limit)` | SQL-paged lightweight summaries with exact totals and eligibility. |
| `metadata_edit_batch(id)` | Bounded full detail for one opaque batch ID. |

Typed edits distinguish Keep/Set/Clear/Append from mixed display values. Invalid pending date/number strings can remain in durable draft form state but cannot authorize a native preview. `allow_id3_upgrade` defaults to false. Exact date precision or contributor values requiring a newer ID3 format need that explicit policy and a new preview.

A save acknowledges only its captured draft revision. A later revision remains pending even if the older write completes. Source identity, path/metadata revisions, current job/attempt and selected record identities remain server-side authority. Display paths never become mutation inputs.

Full snapshots/plans/batches have aggregate JSON budgets; oversized complete scopes fail explicitly rather than being silently truncated. History lists summaries and loads details separately so repeated bulk edits do not require reading every historical snapshot.

## Artwork

`choose_metadata_artwork()` reads a native picker result with an 8 MiB bound and stores a codec-validated private asset. `extract_metadata_artwork(operation_id, track_id, source_sha256, identity)` selects an existing embedded picture by native identity and source proof. Both return `MetadataArtworkAsset`; `metadata_artwork_asset(asset_id)` returns its validated image data URL.

Drafts contain asset IDs, not client filesystem paths. Images are limited to 4096×4096 and bounded decoder allocation. Native identities include type/description/digest where supported; MP4 uses ordinal/digest without invented cover types. ID3 rating owners/counters and comment languages/descriptions remain distinct.

The narrow vendored `id3` decoder patch preserves duplicate decoded records before the editor can apply identity checks. See `vendor/id3/CUERITHM_PATCH.md`; it does not claim complete binary-tag editing or support for opaque DJ records.

## Match review

`track_match_review(track_id)` resolves the current job server-side and returns retained review state or a review-only source state. Ready imported tracks can have no preparation job. Candidates are capped at ten and selectable payloads at 2 MiB, with source/configuration/native identity and generation binding.

`preview_match_review(operation_id, request)` expands the selected server-side candidate through the same shared `Arc<Tagger>` and provider gates used by Incoming. The returned preview freezes per-field decisions, conflicts and actual proposed values. `apply_match_review(operation_id, preview_id, revision)` atomically queues an Incoming decision or returns the metadata-only Ready publication result.

`requery_matches(operation_id, track_id, generation)` explicitly replaces the generation. It does not silently enable providers or authorize a write. `match_review_artwork(operation_id, preview_id, revision)` fetches only the selected frozen candidate's approved HTTPS image origin and returns a private artwork asset. Credentials and arbitrary client URLs are not accepted by that command; cross-origin redirects are rejected.

Keep-existing-once is an explicit protected resolution, not a fabricated Matched outcome or a global configuration change. Immutable decision provenance is separate from current execution binding. Rebinding after CueRithm's own tag write/restore requires that job's persisted native publication proof; arbitrary equal-byte replacement is insufficient. Published organization receipts are never rebound to new naming evidence.

## Cancellation, events and exit

Long commands hold the shared `MaintenanceTasks` lease and cooperative token. `maintenance_cancel(operation_id)` requests cancellation; the next inspector worker waits for the previous promise to settle. Worker/provider cancellation shares the same atomic flag rather than a detached polling task.

Draft changes emit `metadata-edit-change`. Publication emits library and metadata changes on both success and error because an error can follow partial publication. Empty `library-change.track_ids` means a global refresh. Match generation/decision changes emit `match-review-change`.

Settled preparation events also emit `metadata-edit-change`, so queued Incoming overrides update open results, pending drafts and history after the worker finishes or requires attention. They refresh catalog-access projections separately without invalidating unrelated review choices.

Provider operations compare catalog-access state before and after the request, including failed requests. A changed restriction emits `catalog-access-change`; saving transport settings or resetting detected restrictions also refreshes this projection. Access-only events update availability without replacing the review generation, selected fields, existing preview or editor draft. Explicit Requery captures current saved settings and replaces the generation.

Temporary retirement uses a separate, bounded `artifact_cleanup_moves` journal with native parent/file identity and a same-filesystem private quarantine. Recovery does not adopt an unknown object because its bytes match. Malformed auxiliary cleanup receipts remain retained with diagnostics; they do not authorize filesystem access. Undo backups are counted separately from pending temporary cleanup, including backups retained by Incoming overrides.

The frontend installs `app-exit-requested` before calling `register_exit_guard`. On Quit it disables new edits, drains pending draft/image-input persistence and calls `complete_exit(request_id)` only for the current request. Persistence failure calls `cancel_exit`; stale completions cannot stop services. A replacement listener receives the pending request from registration. Native shutdown then cancels/waits for workers and retained journals. Normal window close has its own draft-flush guard.
