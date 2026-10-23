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
