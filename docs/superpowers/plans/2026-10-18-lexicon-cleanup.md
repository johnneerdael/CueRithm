# Lexicon-style cleanup implementation plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Preserve the user's native execution choice; one fresh whole-branch review at the end.

**Goal:** Deliver the eleven Smart Fixes plus Genre/Artist Cleanup, shared by automatic Incoming and a previewable, reversible manual workspace.

**Architecture:** A pure Rust cleanup crate owns validated transformations and mapping behavior. The existing GPL tagging adapter exposes structured metadata access and applies cleanup after catalog selection. Library-owned preview plans and metadata journals reuse verified filesystem publication while preserving analysis/cues.

**Tech Stack:** Rust stable 1.90 desktop, independent Rust rule engine, SQLite, Tauri 2, React/TypeScript and the existing native decoder/tagging resources.

**Spec:** `docs/superpowers/specs/2026-10-18-cleanup-workspace.md`, refining the already approved `2026-10-18-cuerithm-library-workspace-design.md`.

## Global Constraints

- Keep source/publication isolated from PR2 while its native CI completes; rebase this branch onto the final tagging delivery before integration.
- Original open-source code only; no extracted Lexicon code/assets, private music, credentials or corpus labels in Git.
- Preserve key/BPM/grid/scalar energy/cue algorithms; no MIK cue inference or per-segment energy.
- Support all current audio formats throughout read/write/undo. No Python or Node runtime dependency in the installed app.
- Default rules disabled; apply only configured mappings; no fuzzy identity merge. Preserve multiple contributors and explicit exceptions.
- Manual metadata changes preserve analysis and cue revision counts. Source/ownership conflicts never overwrite newer bytes.
- Manual default is file+library with undo, as stated while awaiting the optional preference; incorporate later user steering without stalling pure transformation work.
- Reuse original backup durability/cleanup/reservation primitives. Retained originals are visible; no redundant staged audio after completion.
- Run Cargo validation serially across feature graphs and worktrees, using the shared target, incremental off and debug symbols disabled where appropriate.

## Review Focus

1. Multi-value contributors containing commas, Unicode names and protected canonical values must survive transformations; tests belong to Tasks1/2.
2. Chained/cyclic aliases, interacting rule order and empty/hostile options must converge safely or reject before writes; tests belong to Task1.
3. A preview or Undo may race preparation, file edits, archive changes or later metadata/custom-tag edits; Tasks3/4 pin ownership and non-loss behavior.
4. A crash between file replacement and metadata/custom-tag commit must restore consistent state without changing cues; Task3 tests recovery boundaries.
5. Library-only custom-tag conversion, raw provider provenance and physical tag output must not be conflated or silently lost across restart/direct sync; Tasks2/4/5 test this boundary.

---

### Task 1: Pure Smart Fixes and canonical mappings

**Files:** Create `crates/cuerithm-cleanup/{Cargo.toml,src/lib.rs,src/model.rs,src/fixes.rs,src/mappings.rs,tests/reference.rs,tests/mappings.rs,tests/fixtures/lexicon-v1.json}`; modify root `Cargo.toml`; add `docs/research/cleanup-behavior.md`.

**Interfaces:** `MetadataField` is a typed human-text field enum, including the five contributor roles. `Metadata` contains `BTreeMap<MetadataField, Vec<String>>` plus categorized custom tags. `CleanupConfig` owns ordered enabled `ConfiguredFix` rules, genre mappings/protection/conversion and role-scoped artist mappings/protection. `apply(&CleanupConfig,&Metadata) -> Result<CleanupReport>` returns final metadata, per-field before/after changes and rule provenance; it performs no I/O. Empty vectors remove a field; punctuation never guesses contributor boundaries.

- [ ] Write reference tests using the131 original synthetic cases, normalizing scalar/null fixtures into typed metadata. Record source outputs and separately labeled data-preserving expectations. Add invalid separator/part, unknown target, rule ordering, mixed case, multiple contributors, protected aliases, cycles, idempotency and genre-to-custom-tag tests.
- [ ] Run `cargo test -p cuerithm-cleanup`; observe missing APIs/behavior RED.
- [ ] Implement all eleven option-bearing variants and canonical mapping validation. Match scalar source behavior except the documented non-loss corrections. Protect binary/analysis/path fields. Resolve alias chains to a terminal canonical value; reject cycles. Deduplicate only identical resulting values while preserving order.
- [ ] Run pure tests/Clippy and inspect the behavior matrix; commit.

### Task 2: Structured metadata access and all-format cleanup writes

**Files:** Create `crates/cuerithm-autotag/src/metadata.rs`, tests `cleanup_writes.rs`; modify tag writer and selected format accessors only where necessary; add the pure cleanup dependency.

**Interfaces:** `read_metadata(path, media_info) -> Result<Metadata>` chooses actual format, preserves native value boundaries and never mutates. `TagWritePlan::with_cleanup(CleanupConfig)` applies enabled rules to effective tags after catalog selection/overwrite; `with_metadata_edits(Vec<FieldChange>)` applies an already-reviewed manual patch. Staging returns cleanup provenance/custom-tag changes separately from provider provenance. Reuse `StagedTagWrite` PCM/clock/durable publication contract.

- [ ] Write failing tests for every supported format, wrong suffix/Unicode paths, native contributor arrays, role/genre fields, field removal, protected/unselected tags, automatic cleanup with catalog disabled, and effective catalog-before-cleanup order.
- [ ] Run `cargo test -p cuerithm-autotag --test cleanup_writes`; expect RED before adapter methods exist.
- [ ] Implement typed metadata bindings for standard fields and explicit producer/composer/lyricist/comment/grouping frame mappings. Preserve native separators and unknown tags. Keep original provider genres and cleanup reports distinct. Do not modify musical-analysis tags or artwork unless independently configured.
- [ ] Require complete PCM/frame equality and exact round trips; commit.

### Task 3: Preview plans, metadata publication and durable Undo

**Files:** Add library cleanup models, service/store/journal modules and an additive migration; refactor shared artifact publication out of preparation-specific ownership code; add `tests/cleanup.rs` and recovery fixtures.

**Interfaces:** `preview_cleanup(scope:CleanupScope,config:CleanupConfig) -> CleanupPreview` persists stable proposal IDs, per-track before fingerprints and before/after fields without mutation. `apply_cleanup(plan_id, proposal_ids) -> CleanupBatchResult` publishes only selected proposals. `undo_cleanup(batch_id) -> CleanupBatchResult` restores originals only when post-apply identities still match. Results distinguish applied/unchanged/conflicted/failed track IDs. `cleanup_history()` survives restart. Scope accepts Ready track IDs or all unarchived Ready tracks.

- [ ] Write RED tests proving preview read-only behavior, selective apply, current/older cue and analysis preservation, all-file+DB rollback, partial-batch reporting, archive/preparation/file/metadata races, restart at each publication boundary, and Undo after later changes.
- [ ] Run focused library cleanup tests before implementation.
- [ ] Extract reusable artifact reserve/copy/replace/rollback/cleanup helpers without weakening preparation ownership. Add metadata-specific journals and the persistent category/tag/link tables needed for transactional genre-to-tag changes, rather than repurposing immutable analysis job identities. Share destination reservation and watcher ownership across both journal types. Flush originals before publication; commit metadata/source/custom-tag changes together. Update backup storage accounting across workflows.
- [ ] Run cleanup and existing tagging/storage tests; require exact byte restoration on valid Undo and preservation on conflicts; commit.

### Task 4: Categorized tags, counted cleanup catalogs and Incoming integration

**Files:** Add cleanup catalog queries over Task3's persistent custom-tag/category tables; modify immutable settings/preparation and publication payloads; add `tests/cleanup_preparation.rs` and catalog/mapping tests.

**Interfaces:** `CleanupCatalog` returns distinct genres and contributor values with unique affected-track counts, role counts and protected state. `PreparationSettings.cleanup: Option<CleanupConfig>` persists rules; `cleanup_enabled` requires the real stage. `CleanupReport` adds/removes explicit categorized tags transactionally. Original/provider values remain available in provenance.

- [ ] Write RED tests for saved rules/restart, cleanup-only offline Incoming, tagging+cleanup sequence, custom-tag conversion, counted multiple roles, locked genres/artists, unchanged completed-file discovery and stale settings.
- [ ] Implement persistent categorized tags with stable IDs and exact-name matching, counted catalogs, and configured Incoming cleanup before a single staged publication. Preserve75%-CPU allocation and existing retry cache semantics. Metadata-only manual operations do not run DSP or create cue revisions.
- [ ] Run full library/adapter tests plus explicit built-in analysis regressions; commit.

### Task 5: Cleanup workspace and set-and-forget Settings

**Files:** Create `views/Cleanup.tsx`, cleanup subviews/components and `CleanupSettings.tsx`; extend bridge/types/native commands/library-change events; update sidebar and docs; add UI tests.

**Interfaces:** Typed commands expose preview/apply/ignore/protection/mappings/catalog/history/undo. A library-change event refreshes metadata and loaded detail without restarting playback. Existing selection can supply scope; later playlist membership supplies the same track-ID scope.

- [ ] Write RED UI tests for all eleven option controls, counted genre/artist mapping, protected values, scan-only behavior, selected proposals, stale/partial results, Undo/restart, scrollability, pending/late responses and preservation of the loaded player.
- [ ] Implement the approved charcoal/teal workspace: Smart Fixes/Genres/Artists tabs, scan and before/after selection, saved rules/ignore/protection, custom-tag conversion and durable history. All automatic options live in Settings, not an ingestion wizard.
- [ ] Run20existing UI regressions plus new tests, TS/production/native builds, then native acceptance against a separate generated-audio library. Verify files/library/undo and zero cue-revision/DSP changes for manual cleanup. Commit.

### Task 6: Review and cross-platform delivery

**Files:** CI, notices and behavior/development docs as required.

- [ ] Rebase onto final PR2/main, verify source compatibility and run full local checks. Run one fresh-context whole-delta review; reproduce/fix Important findings in one pass, ledger rulings/minors.
- [ ] Push a separate Cuerithm PR. Require native Windows/Linux/macOSarm64/macOSx64 checks and actual packaged-resource validation. Publish no proprietary source or private data; distinguish source-helper tests from app UI/native acceptance.
- [ ] Preserve the remaining ordinary/full smart playlist and direct-only rekordbox deliveries; this plan alone does not complete the active overall goal.
