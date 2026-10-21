# Windows and Linux DJ Sync Parity Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Let DJs configure native or Windows/Wine/Proton libraries and sync the existing destinations using their actual path and database protocols, then merge PR #22 and verify a successful release.

**Architecture:** One portable target environment owns explicit host-to-Windows mappings and frozen filesystem observations. Host paths remain the source of file access; destination references are separate. Both sync services retain their existing publication, ownership, source proof and recovery boundaries.

**Tech Stack:** Rust, SQLite, Tauri 2, React/TypeScript.

**Spec:** `docs/superpowers/specs/2026-10-21-windows-linux-dj-sync-parity-design.md` (approved).

## Global Constraints

- Make the existing DJ sync destinations work on Windows and on Linux when the DJ application runs through Wine or Proton.
- Do not add rekordbox XML transport.
- Old serialized targets default to Native.
- Keep physical host paths separate from serialized destination paths.
- Unknown schemas remain read-only.
- Preserve existing preview, conflict, backup, source-proof, recovery and ownership guarantees.
- Store paths, environment, mappings and revision; never credentials or derived keys.
- Never run vendor programs, modify their routing settings, or test against the user's real library.
- Keep synthetic fixtures, cross-compilation and native-runtime acceptance claims separate.
- Use the existing shared Cargo target, four build/test workers, no incremental/debug artifacts.
- The user authorizes implementation, independent decisions, review rounds, merge, successful release verification and direct-to-main post-merge build fixes. Rerun only failed/affected stages after a fix.

## Review Focus

- Overlapping or case-colliding mappings must never silently select a different source; Task 1 tests longest-root resolution and collisions.
- Symlink or mount changes between preview and apply must invalidate publication; Tasks 1–3 test frozen canonical roots and filesystem observations.
- Imported native references and Windows asset locations must resolve through the selected context without altering source proofs; Tasks 2–3 test path aliases, existing IDs and escaped Windows names.
- Old journals and ownership baselines must retain Native behavior; Tasks 2–4 test omitted fields and stable target identities.
- Switching setup while a preview is in flight must discard the stale result; Task 4 tests async UI races and backend revalidation.

---

### Task 1: Portable target environment and mapping contract

**Files:** Create `crates/cuerithm-rekordbox/src/target_context.rs`; modify SDK `src/lib.rs`; test in the new module.

**Interfaces:** Produce serde-tagged `TargetEnvironment::{Native, WindowsCompatibility { prefix: PathBuf, installation: Option<PathBuf>, mappings: Vec<DriveMapping> }}`, `DriveMapping { windows_root: String, host_root: PathBuf }`, and methods `is_native()`, `validate() -> Result<()>`, `to_native(&Path) -> Result<PathBuf>`, `to_host(&Path) -> Result<PathBuf>`, `freeze() -> Result<TargetEnvironmentSnapshot>`. Snapshot `verify() -> Result<()>` revalidates captured canonical roots and installation file identity. Use bounded, portable Windows lexical parsing, not host Path semantics.

- [ ] Add failing authored tests for drive/UNC/Unicode mappings, longest component root, unresolved/duplicate/ambiguous roots, traversal, reserved/unrepresentable Windows components, case collisions and prefix/root symlink changes.
- [ ] Run focused SDK tests and observe failure before implementation.
- [ ] Implement exact mappings with bidirectional checks; Native remains identity. Preserve user-selected root spelling and capture canonical physical roots separately. Snapshot installation evidence must be separate from stable library identity.
- [ ] Run focused tests, formatting and strict SDK Clippy; commit the implementation.

### Task 2: General adapters and frozen context

**Files:** Modify `crates/cuerithm-dj-sync/src/{model.rs,sync.rs,engine_dj.rs,virtual_dj.rs,serato4.rs,djay_pro.rs}` and their focused modules/tests; modify `crates/cuerithm-library/src/dj_sync_service.rs` and focused service tests.

**Interfaces:** Consume Task 1. Add serde-default/skip-Native `environment: TargetEnvironment` to `SyncTarget`. Add optional serde-default `native_path: Option<PathBuf>` on `SyncTrack`, with `reference_path(&self) -> &Path`. This field is derived during preparation; caller-supplied projections are never trusted over the selected environment. Keep `path` physical for decoding, hashing, copies and artwork. Freeze/verify environment in prepared requests before publication/restore. Preserve the existing Serato `source_root` meaning.

- [ ] Write and run failing fixtures that sync host files into Windows-shaped djay, Engine, VirtualDJ and Serato databases; assert native references, preserved IDs/foreign records and no host path leakage.
- [ ] Add mapping-change refusal, repeated no-op, interruption recovery, restore and old-Native-journal tests through the actual services.
- [ ] Derive projections once after source proofs; use projected references in adapters without replacing physical paths. Reject unresolved/colliding selected paths before staged writes.
- [ ] Correct Engine desktop cross-drive absolute references while preserving explicit device-copy behavior. Route VirtualDJ Windows per-drive databases and select existing modern/legacy playlist layout by observed capability, preserving foreign libraries.
- [ ] Keep default Native serialization stable so existing baselines remain usable; map physical library roots separately from native drive roots.
- [ ] Run focused/full adapter and service tests plus strict Clippy; commit.

### Task 3: Native rekordbox manual targets and protocol capability

**Files:** Modify `crates/cuerithm-rekordbox/src/{discovery.rs,key_provider.rs,write_profile.rs}`; modify `crates/cuerithm-library/src/rekordbox_sync_service{.rs,/types.rs,/planning.rs,/publication.rs,/assets.rs}` and focused tests.

**Interfaces:** Consume Task 1. Add serde-default/skip-Native environment to `RekordboxSyncTarget`. Expose manual native target configuration through the service (Task 4 persists/selects it). Determine Windows key source from the selected environment, not host OS. Stable target identity binds database/environment/options location, with installation fingerprint a separate preview precondition.

- [ ] Write failing supported-family/schema/header tests showing compatible patch builds and Windows environments can write, while unknown schemas, unsupported major, malformed fingerprint and unsupported asset edits remain blocked.
- [ ] Replace exact executable hash/version whitelist with family 6/7 plus the two recognized schema/header profiles. Retain executable/version hash in preview validation and evidence; do not imply new runtime acceptance.
- [ ] Support explicit executable, database, agent options and asset root on every host. Parse bounded portable PE version resources or require a validated family/version hint, without executing vendor code or reading unrelated settings.
- [ ] Resolve native music/analysis/artwork references through the selected environment. Keep SourceProof physical paths and mapping-aware native-ID matching separate. Freeze source and environment observations in operations and verify at publication/recovery/restore boundaries.
- [ ] Add authored service preview/apply/no-op/recovery/restore tests, incompatible asset refusal, installation update and mapping-change refusal; run SDK/service checks and commit.

### Task 4: Saved target configurations and desktop setup

**Files:** Create additive library migration `028_sync_target_configurations.sql` and configuration service module; register through the existing migration/library pattern. Modify Tauri sync command modules/registration, frontend `sync-types.ts`, `rekordbox-sync-types.ts`, APIs/preferences, `views/Sync.tsx`, `views/RekordboxSync.tsx`; create shared `components/SyncTargetSetup.tsx` and tests.

**Interfaces:** Consume Tasks 1–3. Persist named configurations with an ID, revision, kind and complete target JSON (no keys). Library methods list/save/delete configurations validate their target shape/environment. Backend prepares manual rekordbox targets into the same target resolution path as discovery. Tauri exposes list/save/delete/manual-target commands; frontend bridges remain typed and tests exercise real request shapes.

- [ ] Add failing migration/configuration roundtrip, validation, revision and old-journal tests. Add frontend tests for Native/Wine setup, prefix/install/library pickers, mappings, saved target selection, unresolved errors and stale-preview invalidation.
- [ ] Implement shared setup controls using the existing charcoal/teal components and folder/file picker conventions. Native defaults require no extra setup. Saving never enables automatic publication.
- [ ] Persist/reload selected configurations without replacing operation-captured context; changing/deleting a saved setup invalidates prepared work while recovery/restore retains its original context.
- [ ] Show format compatibility and tested-version evidence separately in rekordbox copy. Remove user-facing exact-build restrictions.
- [ ] Run library/Tauri checks, complete desktop tests/build and screenshot the live setup; commit.

### Task 5: Branch verification, Codex review, merge and release

**Files:** Update public contract/status docs, relevant user documentation and PR #22 body/release notes. No version bumps or manual release sections.

- [ ] Review spec coverage and final behavior, documenting remaining native-runtime evidence limits without claiming missing Traktor/import coverage.
- [ ] Run required workspace fmt/test/Clippy and desktop checks, strict docs and fresh whole-branch review; fix confirmed findings with regressions.
- [ ] Push PR #22, mark ready and request `@codex review`. Fix and reply to every valid finding, resolve threads, repeat until the current head has a clean Codex review. Refresh the PR gate and wait for required Linux checks.
- [ ] Squash merge when the current-head review/gate/checks are green. Monitor main checks and the resulting release.
- [ ] For post-merge failures, patch main directly, declare verified baseline/affected components and rerun only failed/invalidated stages. Preserve passing artifacts/checks. Verify a published new release has the expected packages and source revision before reporting completion.

## Execution decisions

The user explicitly requested continued implementation and independent decisions through release. Proceed without additional approval checkpoints. Use focused agents for disjoint adapter/native/UI work after the shared contract is fixed, with a fresh task review and final branch review. Coordinate Cargo through one shared slot and serialize Git mutations. Missing native-runtime acceptance is an evidence limit, not permission to weaken source/process/publication checks.
