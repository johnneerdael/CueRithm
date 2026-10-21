# Direct rekordbox Sync Completion Implementation Plan

> **For agentic workers:** Use the executing-plans skill for inline execution,
> or subagent-driven-development if that method is selected. Read the linked
> spec before implementing each task. Checkboxes track implementation, not
> application acceptance.

**Goal:** Complete direct native rekordbox 6/7 synchronization with explicit
conflict decisions, coordinated database/assets publication and guarded restore.

**Architecture:** Extend the existing rekordbox crate and native library state.
Use the current desktop Sync workspace and journal concepts, preserving the five
implemented external-player adapters. Keep schema-specific writes separate from
per-field reconciliation and UI state.

**Tech Stack:** Rust, bundled SQLCipher/rusqlite, Tauri 2, React/TypeScript.

**Spec:** `docs/superpowers/specs/2026-10-21-rekordbox-sync-completion-design.md`

**Status:** Approved on 2026-10-21; inline execution in progress. No task is
complete until its implementation and stated evidence are recorded.

## Global constraints

- No collection XML transport is introduced.
- Preserve validated cue detection and use the saved playable timeline.
- No Python or Node runtime dependency in the installed application.
- Never kill rekordbox or its agent, or delete WAL/SHM files to enable a write.
- Keep runtime key material out of settings, errors and diagnostics.
- Development and acceptance use isolated collections and artifact copies.
- Keep implementation, fixture tests, application acceptance and release separate.
- Preserve automatic release versioning and existing player integrations.

## Review focus

1. An initially equal local/remote field still needs explicit acknowledgement;
   observing a value is not evidence of successful publication. Task 2 tests it.
2. A background agent or WAL-only edit may change a supposedly closed target.
   Tasks 1 and 5 must reject stale or busy publication without deleting sidecars.
3. An ID reused for a different native UUID must not steal an old local link.
   Tasks 2 and 4 must preserve identities and report a conflict.
4. A library commit followed by an asset failure is an interrupted operation,
   never a completed sync. Task 5 tests every publication boundary.
5. A later target edit or an unknown binary chunk must survive restore/staging.
   Tasks 3 and 5 test preservation and guarded conflict reporting.

## Task 1: Verified native write profiles and preflight

**Files:** Create `crates/cuerithm-rekordbox/src/write_profile.rs` and
`crates/cuerithm-rekordbox/tests/write_profile.rs`; modify
`crates/cuerithm-rekordbox/src/lib.rs` and `src/discovery.rs`.

**Interfaces:** Introduce observed/staging profile status, `WriteCapability` and
`validate_write_profile(&DatabaseSchema) -> Result<WriteCapability>` in the
rekordbox crate. Schema-only validation returns read-only restrictions and cannot
authorize publication. A later accepted production capability must additionally
bind installed app/build and native database/asset protocol. Reuse
`DatabaseSchema`, `DatabaseIdentity`, `TargetDiscovery` and `DatabaseKey`; never
serialize keys or deserialize an accepted capability from caller-supplied data.

- [ ] Add failing tests for unknown fingerprints, required-column/type changes,
  unsupported versions and foreground/background-agent activity. Assert no
  target bytes change, and distinguish readable from writable.
- [ ] Run `cargo test -p cuerithm-rekordbox --test write_profile`; confirm the
  missing profile API causes the intended failure.
- [ ] Capture sanitized observed/staging profile contracts from isolated copies,
  validate fingerprints and process preflight. Keep the production write
  allowlist empty until Task 6 acceptance. Do not
  invent a rekordbox 6 profile from the inspected rekordbox 7 schema.
- [ ] Run profile, discovery and encrypted database tests. Require rejection
  of mismatches and coherent handling of WAL-only source changes.
- [ ] Record observed/staging profile evidence and its remaining acceptance in
  `docs/research/rekordbox-direct-contract.md` and commit this unit.

**External dependency:** An isolated actual rekordbox 6 installation/profile is
required to close 6 support. Implement profile-independent work while unavailable;
retain an explicit incomplete state, rather than widening the write allowlist.

## Task 2: Field reconciliation and acknowledged baselines

**Files:** Create `crates/cuerithm-library/src/native_reconcile.rs` and
`crates/cuerithm-library/tests/native_reconcile.rs`; modify
`crates/cuerithm-library/src/native_sync.rs` and `src/lib.rs`. Add a new numbered
migration only if persisted decision/acknowledgement data cannot fit the current
schema; choose the next unused number at implementation time.

**Interfaces:** Introduce typed `NativeFieldDecision`, `NativeConflict` and
`NativeReconciliation` values. Add a pure
`reconcile_field(baseline: &NativeFieldBaseline, local: &NativeFieldObservation,
remote: &NativeFieldObservation) -> NativeReconciliation` function. A typed
observation distinguishes unavailable data from a present JSON null. A clear
instruction is a separate explicit decision. Keep a separate explicit decision
for first acknowledgement and conflict resolution; default
behavior never overwrites an unresolved divergent field.

- [ ] Add table-driven failing tests for unacknowledged equal/divergent values,
  local-only changes, remote-only changes, convergent/divergent dual edits,
  null versus clear, owner policy and reused native identity.
- [ ] Run `cargo test -p cuerithm-library --test native_reconcile` and confirm
  failures exercise the missing behavior.
- [ ] Implement reconciliation and persisted reviewed decisions. Bind decisions
  to current local/remote proofs and mapping configuration. Advance agreed
  baselines only after successful acknowledgement/publication.
- [ ] Verify restart, repeated observation and stale-decision rejection; retain
  existing native import behavior and ready-track/manual-edit preservation.
- [ ] Update reconciliation documentation and commit the tested unit.

## Task 3: Cue/grid/artwork staging and native record writer

**Files:** Create `crates/cuerithm-rekordbox/src/writer.rs`,
`src/asset_staging.rs`, `tests/writer.rs` and `tests/asset_staging.rs`; modify
`src/anlz.rs`, `src/lib.rs` and `tests/anlz.rs` within that crate.

**Interfaces:** Introduce `NativeWritePlan` and `StagedNativeWrite`. Implement
`stage_native_write(plan: &NativeWritePlan, workspace: &std::path::Path,
key: &DatabaseKey, cancel: &cuerithm_media::CancellationToken)
-> Result<StagedNativeWrite>`. It writes only owned staging paths and returns
before/after proofs and asset manifests, never publishing the target directly.

- [ ] Add failing authored fixtures for metadata/foreign-table updates, native
  IDs and update counters, memory/hot cues, active loops, variable/constant
  grids, nonzero source timing and unknown binary chunks.
- [ ] Run targeted writer/staging tests to verify intended failure.
- [ ] Implement owned staging against Task 1's observed/staging profiles and Task
  2's accepted field
  decisions. Preserve unknown/unowned rows and chunks; validate every staged
  native relationship and resource path before publication.
- [ ] Run writer, ANLZ and staging tests with malformed paths, cancellation,
  repeated no-op plans and all required source audio layouts.
- [ ] Document exact profile and timing coverage and commit the unit.

## Task 4: Playlist/MyTag import and owned native export

**Files:** Create `crates/cuerithm-library/src/native_playlist_sync.rs` and
`crates/cuerithm-library/tests/native_playlist_sync.rs`; modify
`crates/cuerithm-rekordbox/src/collection.rs` and Task 3's writer.

**Interfaces:** Introduce `NativePlaylistPlan` for typed import/export decisions
with scoped identities, parent/order changes, owned removals and explicit native
rule versus materialized-list representation. Feed its accepted mutations into
`NativeWritePlan`. Reuse existing playlist/custom-tag transactions and smart-rule
semantics rather than creating another local playlist system.

- [ ] Test initially imported native lists/tags, ID reuse, nested order, rename,
  move, external-name collisions, owned deletion and preservation of external
  entries. Assert unsupported complete rules use explicit materialization.
- [ ] Verify tests fail before implementation.
- [ ] Implement typed local import and reviewed native export using existing
  identity/ownership patterns in `dj_sync_service/ownership.rs`.
- [ ] Verify round-trip authored collections, no-op repeats and consistent smart
  membership after imported changes; commit with updated documentation.

## Task 5: Coordinated publication, history and recovery

**Files:** Create `crates/cuerithm-library/src/rekordbox_sync_service.rs` and
`crates/cuerithm-library/tests/rekordbox_sync_service.rs`; modify `src/lib.rs`
and existing journal integration where needed. Keep existing
`src/dj_sync_service.rs` APIs compatible for the other targets.

**Interfaces:** Add typed request/preview/history models and library methods
`preview_rekordbox_sync`, `apply_rekordbox_sync` and `restore_rekordbox_sync`.
Preview persists a token bound to current source/target identities, decisions,
settings and staged assets. Apply/restore accept that persisted operation ID.
Library acknowledgements settle the same durable publication journal.

- [ ] Add failing tests for stale local/remote data, foreground/background
  activity, file reservation conflicts, no-op sync, DB/asset exchange failures,
  restart before/after each acknowledgement and later external edits.
- [ ] Verify targeted tests fail at the missing orchestration boundary.
- [ ] Implement staged publication and pre-mutation backups. Reuse durable
  journal primitives where their contracts match; add explicit handling where
  encrypted database/assets publication needs a different coordinated manifest.
  Retain native exclusive ownership across the write boundary and test an agent
  starting during exchange. Do not assume SQLite file replacement transfers
  native locks or manages outstanding WAL state.
- [ ] Run restart/fault-injection and guarded restore tests. Require a settled
  operation before another preview; prove unrelated native data survives.
- [ ] Document recovery/retention semantics and commit the unit.

## Task 6: Desktop delivery, documentation and actual app acceptance

**Files:** Create `apps/desktop/src/rekordbox-sync-types.ts`,
`src/components/RekordboxConflicts.tsx`, `src-tauri/src/rekordbox_sync_commands.rs`
and `src/test/rekordbox-sync.test.tsx` under `apps/desktop`; modify `src/api.ts`,
`src/types.ts`, `src/views/Sync.tsx`, `src-tauri/src/lib.rs` and
`docs/user-guide/guides/sync.md`.

**Interfaces:** Expose Task 5's typed requests/results through Tauri. The Sync
workspace adds a separate rekordbox target state and explicit conflict choices;
other targets retain existing requests, preferences and controls.

- [ ] Add failing UI tests for readable-but-not-writable profiles, preview
  decisions, open agents, stale previews, changed destination, late responses
  and restore/recovery after restart.
- [ ] Implement the native bridge and branded UI; capture rendered states using
  synthetic data. Show the actual change/skip/conflict result, not staged intent.
- [ ] Run desktop tests/type checks/build and the existing five-target Sync
  regressions. Update guides/screenshots and limitations.
- [ ] Run `cargo test --workspace`, `cargo fmt --all -- --check`,
  `cargo clippy --workspace --all-targets -- -D warnings` and
  `cargo build --workspace --release` on the final implementation head.
- [ ] Exercise isolated actual rekordbox 6 and 7 libraries as specified in the
  design, including reopen, playback, edits in both applications, repeat sync
  and recovery. Record exact installed versions and results separately.
- [ ] Promote each individually accepted application/build and native protocol
  profile into the production allowlist only after its isolated acceptance passes.
- [ ] Review the complete implementation and resolve findings before integration.
  Keep missing native acceptance explicit; do not mark 6/7 complete prematurely.

## Dependencies and handoff

Tasks 1 and 2 define profile and reconciliation contracts. Task 3 consumes both;
Task 4 adds playlist/tag mutations to the staged plan; Task 5 coordinates their
publication; Task 6 exposes the complete workflow. Native rekordbox 6 evidence
is an external acceptance dependency, not a reason to guess a compatible schema.

This plan is a proposed implementation breakdown. Existing uncommitted work in
the main checkout remains separate, and no live library has been changed.
