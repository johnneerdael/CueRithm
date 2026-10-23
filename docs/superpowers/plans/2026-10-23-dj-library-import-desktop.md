# DJ Library Import Desktop and Qualification Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Deliver the import/refresh workflow with independent user controls, visible provenance and recovery, then verify the full branch without overstating native acceptance.

**Architecture:** Tauri commands wrap the shared library service through existing background-operation/cancellation leases. React consumes opaque server previews and paged data; it never reconstructs mutation authority from visible paths. Existing Sync output and Incoming remain available with their present behavior.

**Tech Stack:** Tauri 2, React 19, TypeScript 5.9, Vitest/Testing Library, Rust integration tests and existing cross-platform CI.

**Spec:** [Approved design](../specs/2026-10-23-dj-library-import-refresh-design.md); [shared service plan](2026-10-23-dj-library-import.md); [adapter plan](2026-10-23-dj-library-import-adapters.md).

## Global Constraints

- The default mode is **Store as imported**.
- Retagging and renaming must be opt-in even if enabled in global Incoming settings.
- Preserve current DSP calculations.
- Existing project instructions prohibit rekordbox XML transport.
- Native Mixed In Key Windows storage is uninspected in the pack.
- Production write capability remains disabled until its exact D5 native-app acceptance gates pass.
- Target macOS, Windows and Linux; all supported formats use the bundled media runtime.
- Report imported, unchanged, processed, blocked, failed, cancelled and recovery-required outcomes independently.

## Review Focus

1. Two import windows race or an old preview is restored after restart: reject stale apply and offer a fresh review (Tasks 19–21).
2. Global settings enable destructive processing: imported workflow still starts with every optional stage off (Task 20).
3. A capability disables one group or an unavailable file blocks one stage: retain other selections and explain the exact row/group limitation (Tasks 20–21).
4. A huge/deep library changes while a page is open: stable cursors/revision checks prevent skipped or duplicated records, and cancellation remains responsive (Task 22).
5. An imported row has no prepared analysis: metadata remains visible and playback/waveform loading never overwrites its key, energy or cues (Tasks 20–23).

---

## File map and remaining contracts

- Create `apps/desktop/src-tauri/src/import_commands.rs`; modify `lib.rs` command registration and `host.rs` service lifetime. Reuse `maintenance.rs` cancellation leases and `commands::work/AppError`.
- Create `apps/desktop/src/import-types.ts`, `import-api.ts`; follow existing organization/rekordbox bridge conventions. Types mirror serialized Rust types from the shared plan; add a checked fixture contract test rather than hand-maintaining incompatible property names.
- Create `apps/desktop/src/views/LibraryImport.tsx`, `ImportSources.tsx`, `ImportSelection.tsx`, `ImportPolicy.tsx`, `ImportReview.tsx`, `ImportHistory.tsx`, and `LibraryImport.css`. Split per workflow responsibility; use existing visual language and shared controls.
- Modify `views/Sync.tsx` entry point, `App.tsx` navigation, `types.ts`, `components/{TrackTable,Sidebar,InspectorFrame,PlayerPanel}.tsx` for imported row/detail/collection/playback integration; no broad reorganization.
- Create `apps/desktop/src/test/import-{api,policy,workflow,refresh,library}.test.tsx` (API-only tests may use `.test.ts`), sharing invented fixtures in `import-fixtures.ts`.
- Create `crates/cuerithm-library/tests/import_scale.rs`, `import_acceptance.rs`; expose only a feature-gated fixture generator if integration tests need it.
- Create `docs/development/library-import-api.md`, `docs/research/library-import-qualification.md`, `docs/research/library-import-implementation-record.md`; extend `.github/workflows/{ci,desktop}.yml` existing native checks without claiming a hosted run happened locally.

**Additional library APIs owned by Task 19:** `Library::discover_import_sources(&self) -> Result<Vec<ImportSourceCandidate>>`, `Library::import_operation_page(&self, query: ImportOperationQuery) -> Result<ImportOperationPage>`, `Library::import_preview_page(&self, preview_id: &str, query: ImportEffectPageQuery) -> Result<ImportEffectPage>`. Discovery returns candidates only; explicit native selection plus backend validation creates source authority. All paging has max1000, defaults100 and revision-bound cursors. `ImportSourceCandidate` includes provider, display location, profile hints and exact read/acceptance state, never credentials.

### Task 19: Bounded desktop API and operation lifecycle

**Files:** Create `import_commands.rs`, `import-types.ts`, `import-api.ts`, `test/import-api.test.ts`; modify Tauri `lib.rs`, `host.rs`; implement discovery/history/preview-page library APIs above in focused import modules.

**Interfaces:** Tauri snake_case commands mirror library methods: `discover_import_sources`, `inspect_import`, `import_page`, `preview_import`, `import_preview_page`, `apply_import`, `discard_import_preview`, `inspect_import_refresh`, `import_operation_page`, `preview_import_effects`, `apply_import_effects`, `recover_import_operation`, `undo_import_operation`, `imported_item`, `imported_collection_page`. Long commands take `operation_id: String` and use existing cancellation leases; apply takes `preview_id: String` only. TS `importApi` exports camelCase equivalents returning typed promises. Existing operation cancellation command remains the cancellation transport; emit `library-import-progress` with operation ID, stage/counts and redacted issue summary.

- [x] Write `opaque_preview_id_is_the_only_apply_authority`, `ipc_contract_matches_rust_fixture`, `cancelled_inspection_releases_lease`, `preview_history_is_paged_and_redacted` and `restart_rejects_stale_preview`. Assert tampered display paths/counts cannot authorize work, concurrent same-operation claim fails, typed `stale_preview/unsupported_profile/source_busy/recovery_required` states survive IPC and experimental DJ.Studio writes have no command.
- [x] Run `pnpm --dir apps/desktop test -- src/test/import-api.test.ts --maxWorkers=1` and Rust command-module tests; expect missing bridge/commands.
- [x] Implement thin async command wrappers with bounded background work, structured typed errors and capability validation. Own processing service shutdown/cancellation in Host; keep DB locks short. Add a serialized Rust/TS fixture parity check for policies/presence/clock/capability/result states. Document precise command payloads, pagination and invalidation behavior.
- [x] Rerun bridge and command tests; run `pnpm --dir apps/desktop typecheck` and desktop Rust check with its existing manifest/features; expect no TS or Rust errors. Validate no command takes a client-supplied staged path as authority.
- [x] Commit `feat: expose cancellable library import desktop commands`.

### Task 20: Import source, selection, processing choices and review

**Files:** Create `views/{LibraryImport,ImportSources,ImportSelection,ImportPolicy,ImportReview}.tsx`, `LibraryImport.css`, `test/import-{policy,workflow,library}.test.tsx`, `test/import-fixtures.ts`; modify `views/Sync.tsx`, `App.tsx` and library row/detail/playlist components where imported states are displayed.

**Interfaces:** `LibraryImport({ initialSourceId?: string, onClose: () => void })`; children receive typed state and callbacks, with one reducer owning the flow `source → selection → policy → review → result`. Server records hold all authority. `ImportPolicy` edits the full versioned Rust/TS policy; displayed controls never collapse enrichment/cleanup/audio-tag writes into one boolean.

- [x] Write `store_as_imported_starts_with_all_processing_off_even_when_incoming_is_on`; independently toggle key, energy, generation, replacement, enrichment, cleanup, audio-tag writes and renaming and assert all unrelated values unchanged. Write `overwrite_alone_generates_nothing`, `rename_defaults_to_in_place`, `fields_gate_new_and_existing_publication`, `capability_losses_are_visible`, and `missing_media_stays_in_library`. Assert exact source profile, history/queue off, native path mapping, duplicate collection occurrences, unresolved clock and no fake prepared badge. Show Camelot for recognized keys without changing raw notation.
- [x] Run `pnpm --dir apps/desktop test -- src/test/import-policy.test.tsx src/test/import-workflow.test.tsx src/test/import-library.test.tsx --maxWorkers=1`; expect missing components.
- [x] Implement accessible source/profile selection and explicit artifact choice, paged hierarchy/tracks, field/subfield selection, independent optional processing and actual effects review. Existing local conflicts default keep-local; source hotcue replacement requires both conflict selection and replacement authorization. Expose fill-missing/replace-selected for requested analysis; rename pattern/mode/root shown explicitly. Disable unsupported fields with reasons. Final apply submits only server preview ID; generated/provider/file proposals needing later results get their own effects review. Render imported metadata/provenance/availability with playback only for resolved media; lazy waveform does not change musical results.
- [x] Rerun targeted tests plus existing Sync/rekordbox/library/player tests; expect keyboard navigation, correct defaults and unchanged outgoing Sync behavior. Assert no `enqueuePaths` call anywhere in import UI. Build the frontend to catch route/component integration errors.
- [x] Commit `feat: add import review with independent processing controls`.

### Task 21: Refresh, conflicts, policy revision and recovery UI

**Files:** Create `views/ImportHistory.tsx`, `test/import-refresh.test.tsx`; extend `LibraryImport.tsx`, `ImportReview.tsx`, `import-api.ts` only with typed shared-service integration.

**Interfaces:** Refresh starts with `inspectImportRefresh(sourceId, operationId)` and the saved policy revision. New policy selections create an explicit revision through the next `previewImport` request. History actions call opaque operation/preview IDs, preserving the library's guards; no reconstructed custom apply payload.

- [x] Write `refresh_shows_saved_policy_and_keeps_local_conflict_default`, `unchanged_refresh_has_no_effects`, `partial_scan_is_not_source_deletion`, `custom_conflict_requires_new_preview`, `recovery_and_undo_respect_later_edits`, and `blocked_optional_stage_preserves_successful_import`. Assert separate result counts and exact disabled explanations, no source-removal “delete music” action, no silent policy adoption from Incoming, stale preview offers rescan and cancellation is not reported as successful processing.
- [x] Run `pnpm --dir apps/desktop test -- src/test/import-refresh.test.tsx --maxWorkers=1`; expect missing refresh/history behavior.
- [x] Implement source binding/replica conflict decisions, field/atomic-family comparisons with provenance, saved-policy review and explicit policy revisions. Show immutable operation history and recover/retry/Undo only when allowed by backend. Empty/unavailable outcomes remain actionable; source/profile changes return to inspection. Preserve paging selections by native ID, not row position.
- [x] Rerun refresh/workflow tests with racing local edit, stale source, removed media, interrupted rename/tag journal and missing provider credential; expect existing items remain visible and only authorized retries run.
- [x] Commit `feat: review import refresh conflicts and recovery`.

### Task 22: Whole-system acceptance fixtures and bounded scale

**Files:** Create library `tests/import_{scale,acceptance}.rs`; extend import fixture support, `docs/research/library-import-qualification.md` and `docs/research/library-import-implementation-record.md`.

**Interfaces:** Public service APIs only, plus generated fixture helpers. Scale test uses 100,000 tracks, 1000-record read batches, repeated A/B/A membership, hierarchy depth128 valid/depth129 rejected, paged previews and cancellation. A benchmark mode in the test binary prints aggregate stage times/max resident memory/cancellation latency with no private paths.

- [x] Write `all_off_roundtrip_is_lossless_and_has_zero_processing` across all adapters and library service; `refresh_retains_unselected_and_local_only_changes`; `hundred_thousand_tracks_remain_paged_and_cancellable`; `media_loading_preserves_imported_fields`. Assert 100000 accounted rows, max adapter buffer≤1000 records, selection/revision paging has no skipped/duplicate records, no full UI payload, peak incremental memory budget≤512 MiB for the authored fixture and cancel acknowledgment≤2 seconds between bounded stages on the recorded cloud host. These budgets are plan acceptance targets, not research-pack measurements; report actual results.
- [x] Run `cargo test -p cuerithm-library --features autotag --test import_acceptance` and `cargo test -p cuerithm-library --test import_scale -- --nocapture`; expect any absent end-to-end seam or bound to fail.
- [x] Connect independent generated fixtures and stage/provider/file-operation spies; instrument real page/cancel boundaries rather than sleeps. Exercise key-only/energy-only, source-cues-only, generation blocked/allowed, library-only enrichment, imported-tag writing without lookup, rename without tag writing, partial scans, identity reuse and concurrent edits. Cover generated media in all required codecs; independently verify source DB/WAL/media hashes, tag multiplicity/artwork and PCM after authorized writes.
- [x] Rerun acceptance and scale commands separately from resource-heavy builds. Record exact OS/CPU/runtime/build identities, peak memory, cancellation latency, limits and all failures. Add per-provider/field/profile D0–D6 matrix, fixture hashes, tested schema/codec origins, losses and unrun source-app/device gates. No unrun native test may be marked passed; retain explicit read-only/preview-only or unavailable capability where the approved qualification policy requires it.
- [x] Commit `test: verify import preservation refresh and scale`.

### Task 23: Current-base baseline, full verification and branch review

**Files:** Update qualification/implementation records and plan checkboxes. Modify code only for demonstrated failures; extend tests owning each failure. Inspect existing CI workflows and add missing import tests to current macOS/Windows/Linux jobs without altering volume/device safety checks.

**Interfaces:** Entire three-plan implementation and repository-required checks; no new public API.

- [ ] Before product changes begin, capture current-base test evidence on `446aab6` in a separate temporary baseline worktree or using its already unchanged source state. Activate `/workspace/.cuerithm-env/activate.sh`, use bounded compiler jobs and configure the sealed media runtime for this worktree. Install desktop dependencies with frozen lockfile. Record `cargo test --workspace` and frontend test outcomes including environmental failures; earlier onboarding on `2219143` is not this baseline. Baseline runs may happen at the start of Task 1; their results are consumed here.
- [ ] Run final `cargo test --workspace`, `cargo test -p cuerithm-library --features autotag`, experimental DJ.Studio tests, `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings`, and `cargo build --workspace --release`. Run the desktop manifest checks/build required by its existing workflow. After compilers finish, run `pnpm --dir apps/desktop test -- --maxWorkers=1` and `pnpm --dir apps/desktop build`. Expect passing applicable checks; list any environment-blocked checks with exact evidence and compare to the new baseline.
- [ ] Exercise a generated library through desktop/available headless harness: inspect, select, preview, import, metadata visibility, permitted playback, no-op refresh, divergent conflict, optional stage and recovery. Validate platform-native path fixtures locally; run available actual macOS/Windows CI and record job URLs/status if authorized by repository workflow. Cross-compilation/static checks do not establish native runtime success.
- [ ] Obtain the fresh whole-branch review required by the chosen execution skill. Address findings with reproducing tests; repeat only affected checks plus any required final gates. Review all source capability flags against recorded D-level evidence, all optional-stage defaults and current DSP diffs. Do not weaken mount/source identity checks to accommodate overlay/tmpfs. If native D4/D5/D6 access is unavailable, provide a precise remaining acceptance list and honest capability status.
- [ ] Commit final qualification/fixes with actual test results. Report branch/worktree, shipped behavior, check results and material remaining native gates; do not create a release, merge or deploy. If a PR is created as part of later user direction or applicable workflow, attach it using `codex_app.attach_artifact`.

## Self-review

Desktop behavior, field/policy choices, actual effect previews, refresh conflicts and recovery all have owner tests. Scale, media-format coverage, existing Incoming/output regressions and all per-source acceptance levels have explicit final gates. All three plans implement the approved design; generated-codec/native-app/device evidence remains separated. No product code was changed while writing these plans.
