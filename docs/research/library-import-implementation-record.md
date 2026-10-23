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
