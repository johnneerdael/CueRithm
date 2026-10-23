# DJ library import implementation record

The user approved the design and all three linked implementation plans on 23 October 2026 and selected Native execution, with a fresh whole-branch review after implementation.

- Base: `446aab645b40dcaad82182c79d96daa7fa27f9cc`.
- Isolated feature branch: `codex/dj-library-import`.
- Author-supplied handover: all 22 entries match the recorded sizes and SHA-256 in `dj-library-import-handover/pack-manifest.json`.
- The separate baseline checkout uses that exact base. Earlier onboarding checks on `2219143` are not this baseline.
- A missing bundled-runtime resource in the new worktrees was resolved by linking the already verified local runtime into each ignored runtime path. The initial baseline build failure is environment setup evidence, not a product-test failure.

Execution details and in-progress logs live in the ignored per-plan workspace. Completed test evidence, decisions, qualification levels and remaining native-app gates will be recorded here as implementation proceeds. No source-app or hardware acceptance has been performed by this feature work so far.

## Execution decisions

1. The installed cloud skill package does not expose its referenced execution helper scripts. Equivalent brief, BASE, ledger and test-log bookkeeping is performed locally. The cost is manual enforcement of their checks.
2. Per-task tests use the release profile to reuse native dependencies and bound resource use. Final checks retain debug-specific coverage, including overflow behavior.
3. The reader registry will advertise an unavailable capability until its reader exists. Shared-service tests can inject a synthetic reader before the adapters land; final acceptance tests exercise the actual registry to catch seam drift.

## Shared observation contract

Initial red tests failed on the missing import module. Eight contract tests then passed, including independent default processing, distinct presence states, ordered repeated membership, duplicate/orphan/cycle rejection, depth 128/129, native versus canonical timing, negative/end-only cues, finite BPM and author-manifest integrity. An additional malformed Unicode key case reproduced a validator panic; an ASCII guard now returns a validation error. The broader dj-sync suite passed 394 tests with three explicitly ignored tests including final verification after the minimal cleanup.

## Baseline result

The current-base run completed with 2072 Rust tests passed, 48 failures all caused by the unavailable block-device source, and 11 ignored tests. The frontend passed all 535 tests in 59 files. Exact failed test names and evidence boundaries are recorded in [qualification](library-import-qualification.md).

## Read-side infrastructure

Task 2 passed the full dj-sync suite: 412 passed, three ignored; formatting and all-target Clippy with warnings denied passed. Red/green controls cover committed SQLite WAL data without source DB/WAL changes, stale multi-artifact sets, cancellation/size/symlink limits, explicit drive/UNC/Unicode/non-UTF8 path handling, exact clock-profile inversion, native tag multiplicity, opaque binary payloads, deduplicated storage, bounded record counts, and artwork MIME/type/description/original bytes. Container controls cover ID3v1/v2/APE, FLAC, MP4, RIFF/AIFF, Ogg/Opus and ASF; malformed/unsupported structured values retain raw records with precise interpretation issues. These generated metadata containers are D2 controls, not source-app or decoder acceptance.

Additional execution decisions:

- Per-task tests may set the actively changed repository package to release opt-level 0; dependencies remain optimized. This bounds compile latency. Default optimized release checks remain required at final qualification.
- The concrete SourceAdapter/ReadOptions interfaces landed with ImportSnapshot in Task 2, avoiding a placeholder authority type in Task 1. Later consumers compile against this interface.
- Track observation records use a Box to reduce the enum from a 960-byte largest inline variant; serialization is unchanged. Rust callers must use the boxed variant.
- Archive/XML/TOC/UID malformed-input controls run with their owning reader tasks (9/14/15), where those formats are implemented. Shared snapshot, allocation/count and native-container bounds are exercised in Task 2. Reader qualification remains contingent on those owner tests.

## Durable observations and identity

Task 3 adds migration 029, disk-spooled inspections, bounded record paging, content-addressed raw blobs, source bindings and runtime-bound snapshot authority. Ten store tests pass, including fresh/reopened storage, copied-library replica/new-library choices, artifact-order independence, existing rekordbox observations/links/field owners/reconciliation decisions/journals preserved, atomic cancellation, backup contents and field/row issue accounting. Two regressions reproduced before fixes: nonfinite numbers could become JSON null, and a byte-identical source replacement with a restored timestamp could retain snapshot authority. Validation now runs before JSON serialization; source guards also retain a digest of the native file-identity key.

Broad library testing initially found 495 passes, 57 failures and two ignored tests. Forty-eight failures match the baseline block-device limitation. Nine were older migration expectations/fixtures that assumed schema 28 or retained newer tables while pretending to be schema 18/23. Their expected version and fixture teardown were updated without relaxing migration or restore checks; all 97 affected integration checks passed afterward. The store tests passed 10/10, shared validator/snapshot regressions 12/12, and the dj-sync suite 413 passed/three ignored. All-target Clippy with warnings denied, formatting and diff checks passed. A final whole-workspace run remains required after all tasks.

Additional execution decisions:

- ObservationSink now transports raw bytes through `put_blob(bytes, media_type) -> BlobRef`. References alone could not preserve payloads. Every sink must implement storage; actual blob read-back is tested.
- Backup schema inspection accepts version 29; strict same-schema restore and integrity guards remain intact. Source hashing runs outside the main library transaction, and row/blob transfer checks cancellation between bounded batches. Preview/apply must revalidate the captured source before authorizing changes.

## Imported item projection

Task 4 exposes selected fields without preparation, retains unchecked raw observations, preserves repeated source occurrences and explicit unselected member references, and blocks unqualified outbound items with a capability reason. Missing items can be archived without media access. Eight projection checks pass, including generated WAV playback/waveform decoding without field adoption or jobs. A failing Unicode search witness led to Rust Unicode indexing for linked native tracks without splitting artist names. Existing playlists/tools/playback checks passed 25 tests with one device-only ignore; frontend typecheck and all-target Clippy passed. Private projection helpers are temporarily annotated until their Task 5 preview-service consumers land.

## Immutable previews and refresh

Task 5 freezes selected records, field decisions, media identities and policy behind a backend preview ID. Apply creates bindings, accepted active values, agreed per-field baselines and an idempotent operation receipt atomically; inspection never acknowledges source changes. Unchanged refresh creates no active revisions. Partial scans and failed fields preserve applied values. Hotcue replacement requires both an explicit family decision and separate permission. Changed source/location identities require review. Legacy metadata edits and cleared scalar values remain local edits in both reconciliation and detail/table views.

Forty-one import checks passed, covering source/local/equal/divergent changes, null/empty/reviewed deletion, rollback/retry, discarded/cancelled/restarted authority, source/media races, reused IDs, custom collection ordering/cycles, three thousand streamed membership rows and large-comment byte paging. Fifty-four storage/backup/legacy-import checks passed. All-target Clippy with warnings denied, formatting and diff checks passed. Exact-path auto-linking reuses the qualified native-volume check; unavailable volume proof yields no automatic link. Opt-in content matches are revalidated candidates for review, never automatic links. File-tag key/BPM fallback applies only to absent native fields and keeps its provenance. No source-app/hardware acceptance is implied.

Execution decision: Add a fifteenth import table, `import_preview_effects`, to the unreleased migration 029 for disk-backed immutable proposals and bounded paging. The cost is one more table in legacy-schema fixture teardown and future schema maintenance. Collection fingerprints preserve metadata and ordered occurrence identity while avoiding whole-playlist JSON in previews.

## Selective analysis orchestration

Facts register revision 20 was read in full before Task 6. Applicable boundaries are F001/F003 (captured versus PCM qualification), F005 (versioned profiles), F006 (container versus PCM identity), F009/F051 (authored/detected/native clocks), F049/F077 (ordered multiplicity and negative values), and F118/F120/F128–F131 (no hidden model/branch replacement or promotion). This work changes stage orchestration only; no feature extraction, key/rhythm/energy/cue algorithm, fitted model, weights, training/evaluation split or product label changes. Private findings are cited as constraints, not claimed reproduced.

AnalysisSelection and a selected constructor remove disabled model settings before loading. The selected path skips tonal/timing computation and unnecessary mixed-rate inputs, retaining the existing stage implementations. Additive optional FileAnalysis selection/grid fields keep scalar BPM and grid adoption independent; cue timing prerequisites remain private unless selected. All-off is refused before model loading, and the import coordinator must return before decoding. Existing FileAnalysis fixture literals explicitly retain the legacy None shape. Six generated-PCM controls pass with desktop OpenKeyScan support enabled, comparing actual values and implementation identities with full analysis and testing invalid disabled model paths. Full core verification passed 161 tests with 2 explicitly ignored tests across 28 test targets, including supported-format and original timing/cue controls. All-target core/library Clippy passed. The broad run included one redundant settings-only unit control subsequently removed; the final six selective runtime/value controls passed again with OpenKeyScan enabled after the selected-entry guard was added. Final whole-workspace optimized validation remains pending.

## Frozen import processing

Task 7 freezes independent policy, selected engine settings, model-file hashes and implementation identities at preview. A host-started processor additionally freezes the verified decoder identity; a headless caller with no runtime yet records its decoder when processing and still requires the later actual-effects review. Shared durable stage claims survive cancellation and database reopen. Completed results are not replayed; incomplete proposal attempts are abandoned and retried without publishing active values. Qualified unknown results and unavailable dependencies block their requested stage. Global Incoming settings and mandatory Incoming validation are unchanged. No import is routed through ordinary enqueue/preparation jobs.

Key, BPM, grid, energy and generation create separate proposals. Private generation timing reaches the existing publication template without becoming active BPM/grid. Default all-off and overwrite-only policies create no processing receipts or decode cache. Present null, empty and unknown values are not fill-missing candidates. Preview and worker share the same stage planner, so no-op fill requests and identical refreshes schedule no processing. Validated stage reuse checks source/PCM/clock/decoder, selected settings, model/engine/coordinator identity, result shape and proposal checksum; a mismatch recomputes. Source hashes advance when changed media is accepted. History and queue adoption remain independently off even if an earlier inspection captured them.

Verification: 66 shared import checks passed, including cancellation after claim, concurrent wakes, a panic between stages followed by database reopen, missing media, source/model/implementation/decoder mismatch, cache reuse/rejection, unknown-result handling and a later local edit preserved alongside the proposal's older guard. Fifty-four storage/backup/legacy-import regressions and the six selective core controls passed. All-target core/library Clippy with warnings denied, formatting and diff checks passed. Provider, cleanup and file-publication slots remain explicitly blocked at this intermediate commit; Task 8 supplies their actual handlers and requires a separate effect preview/apply. These are fixture and generated-media controls, not native source-app/device acceptance.


## Desktop import review

The import route uses a single reducer for source, selection, policy, review and
result states. Native selections and mapping roots stay opaque; candidate values
and previews remain server-owned. Users choose an observed DJ.Studio SQL/JSON
candidate by immutable ID, then resolve any independent local conflict in a new
preview. Unchecked analysis, retagging, tag writes and naming remain off.

Existing prepared cues are local conflict evidence; an empty later cue revision
is an explicit local edit. Unknown native pad occupancy blocks generation and
performance export until separately authorized replacement for the current
snapshot. Saved loops remain independent, while loops occupying hot slots protect
those slots. Removed/placeholder native entries remain raw observations but do
not become active cues. Accepted duration and descriptive aliases do not require
analysis. Outgoing Sync uses accepted measured fields and explicit grid anchors,
preserving saved FileAnalysis and existing DSP calculations.


## Refresh and guarded operation history

Refresh uses the saved source binding and last applied choices, then requires a
new inspection and immutable preview. Local conflicts start at Keep local; custom
values require typed backend validation in a rebuilt preview. Partial scans do
not remove source records or music. Saved choices appear in readable form, with
independent controls available for the next explicit revision.

History exposes durable import counts separately from latest-stage state counts.
Backend flags govern retry, effects review, recovery and Undo. A scoped retry
uses the original processing service and gate, retaining completed proposals;
publication journals keep their own recovery/Undo authority. Recovery failures
persist with each effect, including later-edit Undo refusals. Returned summaries
and history issues are bounded without truncating outcome counts. Applying and
applied preview authority cannot be discarded.


## Public-service acceptance and observation guards

Portable authored native artifacts exercise all13 real readers through public
Library APIs. The all-off control exposed a duplicate `locked` field in Mixxx
ScalarBpm serialization: its typed value now carries the lock, while the original
native value remains under `native_bpm_lock`. The generic SDK fixture helper also
checks observation serialization. No DSP calculation changed.

Source process checks now cover original-media tag preview, pre-apply,
publication/recovery and Undo, using the existing bounded Linux executable and
Wine/Proton parser plus fresh Windows/macOS enumeration. M3U remains a transport
without a native app. Imported copy Undo protects later source links. A
current-snapshot unknown pad replacement control exposed that no-field refreshes
skipped the observation guard update; binding snapshot/occupancy guards now
advance even when accepted fields remain unchanged. This does not advance
unaccepted field baselines or trigger optional processing.

The scale fixture exercises100000 available generated media paths, repeated
A/B/A membership,1000-record import pages,500-record Tracks pages, actual SHA I/O,
bounded cancellation and depth128/129. Native app/device acceptance remains
separate from these D2 parser, publication and media controls. Exact fixture
hashes, profile/field evidence and measured host/results are in qualification.


Acceptance review found that stored imported collections lacked persistent UI
navigation. A bounded read-only collection list and Imported playlists browser
now expose hierarchy and occurrence pages after restart, including repeats and
missing entries, without changing editable manual-playlist deduplication. Cursor
filters/runtime/revision and byte limits stay backend-owned. Browser playback
uses the existing resolved-media route; source smart rules remain observations.
