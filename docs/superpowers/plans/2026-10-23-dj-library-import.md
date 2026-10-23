# DJ Library Import and Refresh Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Import and refresh original DJ libraries while preserving their metadata and giving users independent control over analysis, cues, retagging, tag writes and renaming.

**Architecture:** Add streaming, source-neutral observations alongside the outbound sync model. Store observations and selected local projections without preparation jobs; immutable previews govern reconciliation and independent optional processing. This plan builds the shared service; the linked adapter and desktop plans complete the approved scope.

**Tech Stack:** Rust workspace, rusqlite/SQLCipher, serde, bundled FFmpeg, existing DSP/tagging/organization services, Tauri 2, React 19 and TypeScript.

**Spec:** [Approved design](../specs/2026-10-23-dj-library-import-refresh-design.md).

## Global Constraints

- The default mode is **Store as imported**.
- Retagging and renaming must be opt-in even if enabled in global Incoming settings.
- Preserve current DSP calculations.
- Existing project instructions prohibit rekordbox XML transport.
- Native Mixed In Key Windows storage is uninspected in the pack.
- Production write capability remains disabled until its exact D5 native-app acceptance gates pass.
- Target macOS, Windows and Linux; retain repository Rust floors (engine 1.89, library/desktop 1.90), bundled decoding and no installed Python runtime.
- No private libraries, music, credentials, proprietary extracts or identifying reports in commits. No release, deployment or merge is part of execution.

## Review Focus

1. A database copy shares source IDs but diverges: require replica/new-library choice; never merge automatically (Task 3).
2. A source changes during a multi-artifact capture: reject mixed snapshots and retain the prior valid binding (Task 2).
3. A refresh omits a selected collection or encounters one malformed row: preserve local rows and baselines; no inferred deletion (Task 5).
4. A media file changes between processing and publication: refuse stale results and preserve both prior local state and recovery evidence (Tasks 7–8).
5. A provider credential disappears or a rename cannot repair a source reference: report a blocked stage without enabling a different stage (Task 8).

---

## Delivery map and order

These three subsystem plans form one delivery. Intermediate commits are reviewable units, not claims that the entire feature is shipped.

| Plan | Tasks | Working deliverable |
| --- | --- | --- |
| This plan | 1–8 | Tested observation, reconciliation and processing services, usable by Rust callers |
| [Source adapters](2026-10-23-dj-library-import-adapters.md) | 9–18b | Individually testable read adapters and gated DJ.Studio experiment |
| [Desktop and qualification](2026-10-23-dj-library-import-desktop.md) | 19–23 | Complete user workflow, recovery UI and qualification evidence |

Order: 1 → 2 → 3 → 4 → 5 → 6 → 7 → 8; adapters require Tasks 1–2 and may be implemented after Task 8; desktop requires the shared service and adapter registry. Finish with Tasks 22–23. Do not silently omit a source when native acceptance is unavailable: implement its documented reader, show exact evidence/capability limits, and record the unrun acceptance gate.

## File and interface map

- `crates/cuerithm-dj-sync/src/import/{mod,model,policy,validation,paths,clock,snapshot,registry,tags}.rs`: source-neutral contract and read infrastructure; adapter files are listed in the adapter plan.
- `crates/cuerithm-dj-sync/tests/import_{model,snapshot,paths,tags}.rs`: public-interface tests using generated/supplied synthetic fixtures.
- `crates/cuerithm-dj-sync/tests/fixtures/import/`: verified invented JSON fixtures, author manifest and provenance README. Research prose lives under `docs/research/dj-library-import-handover/` preserving relative links.
- `crates/cuerithm-library/src/import/{mod,model,store,identity,projection,collections,preview,reconcile,processing,publication,recovery}.rs`: durable service, split by responsibility.
- `crates/cuerithm-library/migrations/029_library_import.sql`: additive tables; `store.rs` registers it. Preserve migrations 001–028.
- `crates/cuerithm-library/src/{lib,model,store,playback,dj_sync_service,native_sync}.rs`: narrow integration points; retain normal Incoming and outbound sync semantics.
- `crates/camelot-core/src/selected.rs`, `analysis.rs`, `engine.rs`, `lib.rs`: selective orchestration only, leaving DSP implementations unchanged.
- Library tests are child modules `src/import/tests/{mod,store,projection,reconcile,processing,publication}.rs`, permitting existing test helpers without exposing DB internals.

### Shared contracts owned by Task 1

All import types are re-exported from `cuerithm_dj_sync::import`; all library service types from `cuerithm_library`. Use existing `anyhow::Result` and `cuerithm_media::CancellationToken`.

- `SourceProvider`: `Rekordbox, Serato3, Serato4, EngineDj, Traktor, VirtualDj, DjayPro, AppleMusicXml, PioneerUsb, M3u, MixedInKey11, DjStudio, Mixxx`. No selectable rekordbox XML variant.
- `SourceKey { provider: SourceProvider, library_instance_id: String }`; `RecordKey { source: SourceKey, external_id: String }`.
- `Presence<T>`: `Absent, Null, Empty(T), Value(T), Deleted`; serde representation preserves the handover's presence discriminant. Explicit deletion additionally requires complete authoritative scope.
- `FieldGroup`: descriptive tags, key, BPM, beatgrid, hotcues, markers, memory/main/load cues, loops, ratings/color/history, whole-track energy, artwork/custom tags, collections. Field masks also select individual descriptive fields and cue subfamilies; history and queue selections default off.
- `ImportPolicy`: versioned selected fields plus independent enrichment/provider references, cleanup rules, `BTreeMap<AnalysisField, AnalysisAdoption>`, cue generation, `replace_hotcues`, selected audio-tag fields, relocation and file-tag capture. `AnalysisField = Key | Bpm | Beatgrid | Energy`; `AnalysisAdoption = FillMissing | ReplaceSelected`. Relocation is `Off | RenameInPlace { rule } | Move { root_id, rule } | Copy { root_id, rule }` using existing organization configuration types. Credentials are references, never values.
- `ObservationHeader`: contract version 1, source/profile/schema/adapter identities, snapshot hash, scope, completeness (`WholeLibrary | SelectedScope | PartialWithErrors`), capture timestamp and record accounting.
- `TrackObservation`: stable external ID, recorded path evidence/availability, native field-presence map, cached-tag map, independent file-tag reference, typed musical groups, cues, grids, safe provider extensions and typed issues. Preserve raw numbers/blobs through content-addressed `BlobRef` values; filter account/settings/license tables.
- `CueObservation`: stable external ID, family, optional source slot, nullable label/color, native start/end and raw units/flags, optional canonical start/end with clock evidence. `GridObservation`: explicit native beats or tempo segments, optional bar phase, raw data and clock evidence. End-only ranges and negative pre-roll are valid.
- `CollectionObservation`: external ID, parent ID, kind, name, raw rules/order evidence. `MembershipObservation`: collection ID, unique per-collection occurrence ID, track ID, optional source position; repeated track IDs are allowed.
- `ObservationRecord = Track(TrackObservation) | Collection(CollectionObservation) | Membership(MembershipObservation) | Issue(ImportIssue)`.
- `ObservationSink::push(&mut self, record: ObservationRecord) -> Result<()>`; `validate_observation(header: &ObservationHeader, records: &mut dyn ObservationReader, limits: &ImportLimits) -> Result<ValidationReport>`. `ObservationReader::next(&mut self) -> Result<Option<ObservationRecord>>` allows DB-backed validation rather than collecting the library.
- `ImportLimits::default()`: page size 1000, hierarchy depth 128, per-blob inflated limit 64 MiB, per-text/JSON record limit 16 MiB, total snapshot budget 8 GiB; all arithmetic checked. Exceeding a limit produces a typed issue/rejection, never truncation presented as complete. Cancellation checked at each page and within large blob/archive loops.
- `SourceAdapter: Send + Sync`: `provider(&self) -> SourceProvider`, `inspect(&self, snapshot: &ImportSnapshot) -> Result<SourceProfile>`, `read(&self, snapshot: &ImportSnapshot, options: &ReadOptions, sink: &mut dyn ObservationSink, cancel: &CancellationToken) -> Result<ObservationHeader>`. Registry function `adapter(provider: SourceProvider) -> &'static dyn SourceAdapter`.
- `SourceProfile`: exact supported schema/codec and app-origin identities with per-field read/write/lossless capability, D0–D6 evidence and explanatory losses. Unknown profiles return actionable unsupported status; filename/product name is insufficient.

### Task 1: Verified contracts, semantic validation and independent policy

**Files:** Create the shared model/policy/validation modules and `tests/import_model.rs` above; modify dj-sync `lib.rs`; retain all 22 verified pack artifacts in the provenance directories above with original SHA-256 mapping (fixtures may be referenced from prose via a documented local mapping).

**Interfaces:** Produces every contract in “Shared contracts”; consumes the verified author manifest at `/workspace/scratch/dj-library-sync-handover/pack-manifest.json` and the approved spec.

- [x] Write `policy_defaults_do_not_authorize_processing` asserting `analysis_fields.is_empty()`, generation off, enrichment off, cleanup off, audio-tag writes off and relocation off; toggle each alone and assert all other components remain unchanged. Write `semantic_validation_rejects_bad_graph_not_valid_schema` for duplicate IDs/occurrences, orphan references, depth 129, cyclic parent, nonfinite/invalid BPM and unresolved clock with canonical endpoints. Assert A/B/A membership, end-only range and negative pre-roll remain valid.
- [x] Run `cargo test -p cuerithm-dj-sync --test import_model`; expect missing API failure before implementation.
- [x] Implement the contracts and disk-backed semantic-validation indexes, with separate envelope/semantic errors. Copy only manifest-verified clean-room prose and invented fixtures; retain the research-only XML fixture enum as evidence, never as an enabled provider. Add byte/hash integrity checking to the fixture test.
- [x] Rerun the command; expect all tests pass including the normalized example and all 22 manifest entries. Record all optional policy values in serialization tests.
- [x] Commit `feat: define lossless library import observations and policies`.

### Task 2: Read-only snapshots, portable paths, clocks and tag observations

**Files:** Create `import/{snapshot,paths,clock,tags}.rs`, `tests/import_{snapshot,paths,tags}.rs`; modify dj-sync `Cargo.toml` only if safe plist parsing needs a pinned dependency.

**Interfaces:** Produces `capture_source(request: &SourceCaptureRequest, workspace: &Path, limits: &ImportLimits, cancel: &CancellationToken) -> Result<ImportSnapshot>`, `resolve_location(evidence: &LocationEvidence, mappings: &[PathMapping]) -> Result<LocationResolution>`, `align_time(value: &NativeTime, profile: Option<&QualifiedClockProfile>) -> Result<AlignedTime>`, and `read_file_tags(path: &Path, limits: &ImportLimits, cancel: &CancellationToken) -> Result<FileTagObservation>`. Snapshot owns private artifact handles, manifest and source guard; source paths never come back from the UI as authority.

`SourceCaptureRequest` holds provider, validated native source-root authority, explicit selected artifact IDs and origin profile. `ReadOptions` holds limits, history/queue inclusion, file-tag capture choice, origin profile and path mappings. `LocationEvidence` retains recorded bytes/platform/volume/case authority; `PathMapping` maps a source component prefix to a validated local root ID. `QualifiedClockProfile` binds source app/OS/renderer/codec/rate/media and decoder identities to an offset; `AlignedTime` retains native values and exposes canonical endpoints only for that exact match. `FileTagObservation` contains normalized fields, ordered raw tag records, attachment `BlobRef`s and exact capture losses.

- [x] Write `snapshot_reads_committed_wal_without_source_writes` asserting WAL-only rows present and source DB/WAL bytes unchanged; `multi_file_mutation_rejects_capture`; `path_mapping_preserves_drive_unc_unicode_and_case_authority`; `clock_inverse_applies_once_or_stays_unresolved`. Assert `canonical = native - offset`, absent profile ⇒ no canonical values, longest component-prefix mapping and no basename matching. For tags assert native/cache/file layers differ and unknown duplicate frames/atoms plus binary artwork remain recoverable.
- [x] Run the three test binaries; expect missing API failures.
- [x] Implement read-only online backup with bounded busy/cancel, multi-file inventory before/after and stable/closed-source requirements for nontransactional sets. Use native path serialization and explicit source-platform comparison rules; add safe URL/bookmark handling without mounting. Retain raw supported container structures alongside normalized tag values; report exact unsupported containers/fields. No audio decoding for tag capture. XML/plist parsing rejects DTD/entity/network expansion; JSON rejects trailing junk/nonfinite values; no archive traversal or source code/SQL execution.
- [x] Rerun all three tests; add truncated blobs, archive bombs, symlink escape, TOC/UID cycles and source-error accounting; expect pass. Tests inspect generated SQLite and file hashes, not merely mocked open flags.
- [x] Commit `feat: capture consistent read-only import snapshots`.

### Task 3: Durable observations and source identity

**Files:** Create migration 029 and `src/import/{mod,model,store,identity}.rs`, `src/import/tests/{mod,store}.rs`; modify library `lib.rs`, `store.rs`.

**Interfaces:** `Library::inspect_import(&self, request: ImportInspectRequest, cancel: &CancellationToken) -> Result<ImportInspection>` stages a snapshot and registry output in a private SQLite spool, returns opaque `inspection_id`. `Library::import_page(&self, inspection_id: &str, query: ImportPageQuery) -> Result<ImportPage>` returns bounded records. `ImportInspectRequest` includes provider, native-selected source authority, explicit library binding/origin choice and path mappings; optional file-tag read policy. `ImportInspection` includes profile, completeness, counts/issues and capability summary. Selection uses source-native IDs, not display paths.

- [x] Write `import_store_migrates_without_touching_rekordbox_state`, `source_replica_requires_explicit_binding`, `same_path_is_not_global_identity`, and `inspection_is_paged_and_private`. Assert old native targets/links/owners/journals byte-for-byte unchanged, source ID reused by different libraries remains distinct, same-source established link precedes exact canonical path, content matching is disabled by default, malformed rows are accounted.
- [x] Run `cargo test -p cuerithm-library import::tests::store`; expect missing module/API failure.
- [x] Add `import_sources`, `import_snapshots`, `import_observations`, `import_blobs`, `import_bindings`, `import_items`, `import_active_fields`, `import_applied_baselines`, `import_collections`, `import_memberships`, `import_policies`, `import_previews`, `import_operations`, `import_processing_receipts`. Enforce namespaced record/occurrence uniqueness, transaction ownership and private blob references. Track observed vs applied hashes separately. Persist source-link bridges to existing rekordbox identities explicitly, never backfill acknowledged baselines just from observation. Hash semantic fields/scope/order excluding timestamps.
- [x] Rerun the store tests against fresh and migrated databases, crash/reopen and partial inspections; expect pass and no dangling snapshot/blob references.
- [x] Commit `feat: persist source observations and import bindings`.

### Task 4: Visible imported items and occurrence-preserving collections

**Files:** Create `src/import/{projection,collections}.rs`, `tests/projection.rs`; modify library `model.rs`, `store.rs`, `playback.rs`, `dj_sync_service.rs` and relevant collection query modules at their existing query entry points.

**Interfaces:** `Library::imported_item(&self, item_id: &str) -> Result<ImportedItemDetail>`; `Library::imported_collection_page(&self, collection_id: &str, cursor: Option<&str>, limit: u32) -> Result<ImportedCollectionPage>`. `ImportedItemDetail` includes active fields, provenance, availability, `playback_allowed`, analysis state and source links. Existing `TrackQuery/TrackPage` includes imported rows exactly once; allocate the eventual stable `TrackId` in `import_items`, promote to playable `tracks` only when required, preserving that ID. Bound existing tracks retain their IDs.

Add serde-defaulted `TrackSummary.import_state: Option<ImportedItemState>` and matching optional `Track.import_state` in desktop `types.ts`. `ImportedItemState` contains source IDs, availability, analysis state and playback permission; absent means existing library behavior. Imported collection pages include occurrence IDs independently of track IDs so row identity survives repeated membership.

- [x] Write `unavailable_import_visible_without_audio_or_jobs`, `unchecked_new_fields_remain_provenance_only`, `existing_unchecked_fields_are_unchanged`, and `imported_collections_preserve_occurrences`. Assert metadata visible with missing/streaming media, zero preparation/analysis rows, null unselected active values, A/B/A order, same-time memory+hot cues, unordered crate marked unordered, and unanalysed export blocked with a specific capability reason.
- [x] Run `cargo test -p cuerithm-library import::tests::projection`; expect missing API failure.
- [x] Implement query projections, source-native occurrence pages and imported/unanalysed/unavailable states. Reuse existing playback decoding for resolved supported media lazily, preserving imported musical fields. Keep immutable playback snapshots stable across refresh. Outbound eligibility must examine required active data/clock evidence explicitly; never fabricate completed preparation or measured `FileAnalysis`.
- [x] Rerun projection tests plus existing manual-playlist/playback/output tests; expect manual playlist dedup semantics unchanged and imported repeated occurrences intact.
- [x] Commit `feat: expose imported library items without automatic preparation`.

### Task 5: Immutable preview, initial conflicts and three-way refresh

**Files:** Create `src/import/{preview,reconcile}.rs`, `tests/reconcile.rs`; extend `model.rs` and `store.rs` within the import module.

**Interfaces:** `Library::preview_import(&self, request: ImportPreviewRequest, cancel: &CancellationToken) -> Result<ImportPreview>`; `Library::apply_import(&self, preview_id: &str, cancel: &CancellationToken) -> Result<ImportApplyResult>`; `Library::discard_import_preview(&self, preview_id: &str) -> Result<()>`; `Library::inspect_import_refresh(&self, source_id: &str, cancel: &CancellationToken) -> Result<ImportInspection>`. Request includes inspection ID, selected IDs/fields, frozen policy and source/local/custom conflict decisions. Response has opaque preview ID, paged effects/conflicts, exact counts and blocked reasons. Apply accepts only preview ID; repeated apply returns the same operation receipt.

- [x] Write `all_off_import_has_no_external_effects`, `initial_conflicts_keep_local_until_decided`, `refresh_uses_applied_not_observed_baseline`, `unchanged_refresh_is_zero_work`, `partial_scope_never_tombstones`, `stale_preview_cannot_apply`, and `hotcue_selection_is_not_generation_authorization`. Assert no jobs/DSP/provider/tag/rename calls; unchanged refresh has zero track/cue/collection revisions and no baseline advance for unchecked groups; native hotcue adoption over occupied local family requires both decision and authorization.
- [x] Run `cargo test -p cuerithm-library import::tests::reconcile`; expect missing API failure.
- [x] Store immutable proposals with local revision, source guard, resolved media identity, decisions and policy revision. Apply observations, selected projections/baselines, bindings and operation receipt in one local transaction. Preserve differing source/local values, explicit null/empty/deleted semantics, atomic performance families and collection hierarchy/order conflicts. Incomplete reads never infer tombstones; complete deletions require review and never delete media. Source changes invalidate preview before mutation. Optional processing is scheduled separately only after commit.
- [x] Rerun reconciliation tests with source-only/local-only/equal/divergent edits, source ID reuse, deletion, reordered duplicate memberships, double apply and cancelled transaction; expect pass and atomic rollback.
- [x] Commit `feat: preview and reconcile repeat library imports`.

### Task 6: Selective engine execution without DSP changes

**Files:** Create `crates/camelot-core/src/selected.rs`; modify `engine.rs`, `analysis.rs`, `lib.rs`; tests in `selected.rs` child module and existing engine regression modules.

**Interfaces:** `AnalysisSelection { key: bool, bpm: bool, beatgrid: bool, energy: bool, cues: bool }`; `Engine::analyze_prepared_selected(&mut self, audio: &cuerithm_media::PreparedAudio, selection: AnalysisSelection) -> Result<FileAnalysis>`; existing `analyze_prepared` behavior preserved. Disabled outputs are absent, and their stages/models are not initialized/evaluated unless a disclosed prerequisite of a selected stage. Selection is part of cache/config identity.

- [x] Write `key_only_skips_timing_energy_and_cues`, `energy_only_skips_key_and_cues`, `cue_prerequisites_do_not_publish_unselected_fields`, and `full_selection_matches_existing_engine`. Use stage spies plus generated PCM assertions: key-only output equals existing key, energy-only equals existing whole-track energy, full selected results equal existing analysis, existing cue calculations unchanged. Compare actual values and implementation identities, not just presence.
- [x] Run `cargo test -p camelot-core selected`; expect missing selective API failure.
- [x] Add a selective orchestration path, sharing existing stage implementations and direct-rate prepared inputs. Do not alter `lexicon_cues.rs`, key, rhythm or energy algorithms, model promotion or product labels. Read facts register revision 20 in full before touching orchestration; record applicable provenance/origin constraints and the absence of a modelling change in the implementation record. An all-off request returns before decoding, enforced by its caller and tested here where applicable.
- [x] Rerun selected tests and existing DSP regression tests; expect identical full-path results, independent key/energy execution, no disabled-stage initialization and no extra direct-rate inputs.
- [x] Commit `feat: orchestrate explicitly selected analysis stages`.

### Task 7: Frozen import-processing coordinator

**Files:** Create `src/import/processing.rs`, `tests/processing.rs`; extend import `model/store`; integrate service lifetime with existing host only in desktop Task 19.

**Interfaces:** `ImportProcessingService::start(library: Library, runtime: MediaRuntime) -> Result<Self>`, `run_pending(&self, cancel: &CancellationToken) -> Result<ImportProcessingSummary>`, `shutdown(&self)`; production dependencies come from existing provider/tag/organization services. `ImportProcessingSummary` distinguishes processed/blocked/failed/cancelled/recovery-required. Each stage receipt binds operation/item/policy/attempt and source/PCM/settings/implementation identities.

- [ ] Write `global_incoming_settings_cannot_change_import_policy`, `key_and_energy_choices_are_independent`, `fill_missing_is_not_replace_unknown`, `generation_respects_occupied_hotcue_family`, `source_change_blocks_publication`, and `restart_does_not_replay_completed_stage`. Assert key-only/energy-only leave active hotcues intact; overwrite alone runs no generator; no source-field value is used as a measured prediction.
- [ ] Run `cargo test -p cuerithm-library import::tests::processing`; expect missing service failure.
- [ ] Implement independent stage planning and attempt/checkpoint claims outside long DB locks. Use Task 6 API; cache only exact validated identities. Store generated results as proposals without active adoption; Task 8's actual-effects preview/apply publishes selected groups under revision guards. Distinguish private generation prerequisites from published BPM/grid. Preserve ordinary Incoming validation and workers; never enqueue imports through `enqueue_paths` or `enqueue_one`.
- [ ] Rerun processing tests with missing media, cancellation per stage, worker restart, cache mismatch and later user edits; expect accounted blocked/cancelled outcomes and no unchecked stage calls.
- [ ] Commit `feat: process imports with frozen independent policies`.

### Task 8: Independent enrichment, file publication, renaming and recovery

**Files:** Create `src/import/{publication,recovery}.rs`, `tests/publication.rs`; narrowly extend existing tagging/publication and `organization/service/` APIs where importing unprepared items needs a guarded entry point.

**Interfaces:** `Library::preview_import_effects(&self, operation_id: &str, cancel: &CancellationToken) -> Result<ImportEffectsPreview>`; `Library::apply_import_effects(&self, preview_id: &str, cancel: &CancellationToken) -> Result<ImportProcessingSummary>`; `Library::recover_import_operation(&self, operation_id: &str, cancel: &CancellationToken) -> Result<ImportRecoveryResult>`; `Library::undo_import_operation(&self, operation_id: &str, cancel: &CancellationToken) -> Result<ImportUndoResult>`. Effect preview freezes actual generated/provider results and file/reference changes; if unavailable at initial import review, it requires a separate explicit apply before publication.

- [ ] Write `enrichment_is_library_only_without_tag_authorization`, `accepted_imported_tags_write_without_provider_lookup`, `rename_in_place_needs_no_tag_write`, `missing_credential_blocks_only_requested_stage`, and `reference_repair_failure_blocks_rename`. Assert zero unrelated calls, PCM hash unchanged after supported tag writes, exact filenames unchanged when renaming off, stable item IDs/analysis/membership after rename, collision/stale-file rejection, original-byte restoration and later-edit protection on Undo.
- [ ] Run `cargo test -p cuerithm-library --features autotag import::tests::publication`; expect missing effect API failure.
- [ ] Adapt existing provider proposal/cleanup, staged tag-write PCM proof/backups and organization reference coordinator behind the frozen independent policy. Use final accepted metadata for naming; rename in place unless move/copy explicitly chosen. Never call a preparation-specific publisher by fabricating a job. Persist journals/receipts before file replacement, require closed targets and qualified reference repairs, and retain recovery evidence across cancellation/restart. Do not roll back a valid local import merely because optional processing blocks.
- [ ] Rerun publication tests with generated media for MP3, AAC/ADTS, AAC/M4A, ALAC/M4A, FLAC, WAV, AIFF, Ogg Vorbis, Opus and WMA; retain exact field/container limitations and unsupported repair blockers. Inject failure at every file/journal transition and verify byte hashes, idempotency and no half-applied completion claim.
- [ ] Commit `feat: guard optional import tagging and renaming with recovery`.

## Self-review and execution handoff

Coverage: observation/selection/provenance → 1–4; identity and existing rekordbox compatibility → 3; default import/conflicts/refresh → 5; independent DSP → 6–7; retagging/audio writes/renaming/recovery → 8; all named sources and DJ.Studio experiment → linked Tasks 9–18; user flows and acceptance → linked Tasks 19–23. Review Focus cases are assigned above. No source/app/device acceptance is supplied by this plan or by the research pack.

Execution has not started. Review all three linked plans together. Recommended method: **Native** execution, preserving one implementer's context across the shared projection, reconciliation and publication interfaces, followed by a fresh whole-branch review. Subagent-driven execution is available if independent review after every task is preferred.
