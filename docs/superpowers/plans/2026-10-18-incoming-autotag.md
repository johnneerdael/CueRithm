# Automatic Incoming Tagging Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Execution remains native, as already selected by the user. Steps use checkbox syntax for tracking.

**Goal:** Automatically apply configured Beatport/Traxsource tagging during Incoming preparation, retain Cuerithm analysis/cues, and promote only after required preparation succeeds.

**Architecture:** Reuse pinned GPLv3 OneTagger provider/matcher/tag-format code behind a Rust adapter with explicit inputs, candidates and staged write plans. Integrate source-checked, recoverable file publication into the existing durable job service. Keep provider traffic limits distinct from the user's 75%-of-available-cores analysis policy.

**Tech Stack:** Rust, selected OneTagger modules, reqwest/rustls, SQLite, Tauri 2, React/TypeScript, bundled native media runtime.

**Spec:** `docs/superpowers/specs/2026-10-18-cuerithm-library-workspace-design.md`, especially Tagging and persistent Settings and Automatic preparation contract. GPLv3 desktop reuse explicitly approved 2026-10-18. Cleanup, playlist parity and direct rekordbox sync remain following deliveries; they are not silently reduced.

## Global Constraints

- macOS/Windows/Linux; native decoding and playback remain available for MP3, ADTS AAC, AAC/M4A, ALAC/M4A, FLAC, WAV, AIFF, Vorbis, Opus and WMA.
- Retain the key/BPM/grid, scalar whole-track energy and validated Lexicon-compatible cues. Do not restart MIK cue or segment-energy work.
- All relevant Beatport/Traxsource matching, fields, overwrite, separators, filename, artwork and album options live in Settings, not an import wizard.
- OneTagger upstream: `36523f71f2d9a5947912f3cb930f1a31fcb2e3ee`. Record selected files, hashes and local patches. The combined desktop and imported modules are GPLv3; retain existing MIT engine notices and original upstream notices.
- Import no Spotify features, Shazam, standalone tag-editor/renamer, executable post-command or global batch-stop runtime. Do not misrepresent omitted options as working settings.
- User excluded Plays, Last Played and Danceability. No XML transport.
- Never publish user music, private captures, license credentials or decompiled Lexicon source. Tests use generated audio and synthetic metadata.
- Work against copied/generated files first. Source publication must preserve PCM and be recoverable across restart; original bytes must remain recoverable.

## Review Focus

1. Provider failure/timeout/403/malformed responses must be recoverable Incoming errors, not panic, false success or silent loss of configured work (Task 1/4).
2. Concurrent source, settings, retry or archive changes must not let an obsolete attempt rewrite a newer file or commit old tags/cues (Task 3/4).
3. A crash between file replacement and SQLite completion must recover from persisted identities/journal, preserving user edits and prior cues (Task 3).
4. Lossy priming, artwork and uncommon containers must retain the same complete playable PCM/clock after tagging; extension alone is insufficient evidence (Task 2).
5. Provider key/BPM and untrusted custom fields must remain distinct from measured analysis, while new metadata remains discoverable and settings survive upgrade/restart (Task 4/5).

---

### Task 1: Licensed providers, matching and configuration

**Files:** Create `vendor/onetagger/` (selected tag/tagger/provider source plus license/provenance), `crates/cuerithm-autotag/{Cargo.toml,src/lib.rs,src/config.rs,src/providers.rs,tests/matching.rs,tests/providers.rs,tests/http.rs,tests/concurrency.rs}`; modify workspace manifest, desktop license metadata and notices.

**Interfaces:** `AutotagConfig` owns validated selected-provider `TaggerConfig`, no-match policy and field-source preference. `TagInput` contains native source path, actual media information and playable duration. `Tagger::match_track(&TagInput,&AutotagConfig) -> Result<TagMatchOutcome>` returns explicit matched/no-match with provider, reasons, confidence and selected metadata; performs no file writes. `Tagger::provider_options()` supplies only the two selected providers' options.

- [ ] Write failing tests for first-success order, configured multi-provider merge, duration/version strictness, provider IDs, custom fields, invalid settings, no-match, timeout/403 and malformed/empty responses. Use injectable HTTP fixtures. No test depends on live catalog availability.
- [ ] Run `cargo test -p cuerithm-autotag --tests`; expect missing adapter errors first.
- [ ] Vendor the minimum source boundaries. Preserve matching arithmetic; make necessary parser failures return errors, bound HTTP requests, avoid token logging, and fix token refresh ownership rather than retaining recursive lock acquisition. Keep Traxsource at one simultaneous request. Add explicit AAC/WMA input modeling rather than labeling them another format.
- [ ] Apply GPLv3 desktop distribution metadata/notices, preserving the separately MIT engine. Record every source adaptation.
- [ ] Run the tests and Clippy, require all fixtures pass; commit the independently testable adapter. Record actual live native-client results separately from fixture results.

### Task 2: Staged tag writes across common formats

**Files:** Create `crates/cuerithm-autotag/src/write.rs`, `tests/writing.rs`; adapt selected OneTagger format helpers and media-runtime recipe only where required for AAC/WMA support.

**Interfaces:** `TagWritePlan::build(input,match_result,config,analysis_tags)` resolves explicit selected fields and overwrite rules. `TagWritePlan::stage(&MediaRuntime,cache_dir) -> Result<StagedTagWrite>` produces a temporary same-format file, before/after source hashes, PCM proof and a recoverable backup description, without replacing the source. Analysis tags are the measured Camelot key/BPM/energy; provider values retain separate provenance.

- [ ] Write failing round-trip tests for all required audio formats, Unicode/wrong-suffix paths, selected/unselected fields, raw/custom fields, artwork replacement, ID3 v2.3/v2.4, separators, album/date/genre/style handling and no-op plans.
- [ ] Run `cargo test -p cuerithm-autotag --test writing`; expect missing writer errors.
- [ ] Implement staged writing with preserved permissions and bounded image/network input. Compare full canonical PCM hashes and exact playable duration before allowing publication; reject changes in audio/timeline, leaving the original untouched. Do not skip AAC/WMA checks.
- [ ] Run the full writing fixtures and media decoding/linkage tests. Require unchanged input hashes before publication, identical complete decoded audio, exact requested tags and preserved unselected fields. Commit.

### Task 3: Durable publication and recovery

**Files:** Create `crates/cuerithm-library/src/tagging.rs`, `tests/tagging.rs`; modify schema/migrations, job mutation helpers and common-format fixture support.

**Interfaces:** `Library::begin_tag_write(&JobRun,&StagedTagWrite,&FileAnalysis)` persists a publication journal. `Library::publish_preparation(&JobRun,journal_id)` validates ownership, publishes the staged file and atomically commits updated source identity, metadata, analysis/cue version and readiness. `Library::recover_tag_writes()` reconciles pending publications before watcher/worker startup. The watcher recognizes an owned in-flight publication rather than enqueuing its own write as an unrelated source change.

- [ ] Write failing tests for crashes before/after replacement, SQLite commit failure, stale attempt/settings/source, archive/retry, user edits during recovery, interrupted backups and source A→B→A. Assert old source/cues remain recoverable and no premature Ready state.
- [ ] Run `cargo test -p cuerithm-library --test tagging`; expect missing publication APIs.
- [ ] Implement a versioned migration and persisted identities/backups. Hold the final ownership check and file publication within the library's serialized commit boundary. Recovery only replaces or completes a file whose hash matches the recorded before/after identity; unrelated edits require attention. Do not hold the SQLite mutex during network, decode or staged preparation.
- [ ] Run migration/restart/stale-attempt tests, including existing metadata/cue history tests. Require no corruption, no self-trigger loop and exactly one successful completion revision. Commit.

### Task 4: Automatic preparation stage

**Files:** Modify `crates/cuerithm-library/src/{prepare.rs,settings.rs}`, native host configuration and tests; add preparation/tagging integration fixtures.

**Interfaces:** Preparation consumes `AutotagConfig` from the immutable settings snapshot, obtains provider candidates and stages/publishes required tags after analysis. Existing unconfigured/no-tagging engine/CLI use stays offline. Enabled tagging is a real required stage; network failures retain resumable Incoming state. Explicit no-match policy chooses keep-source versus attention and records that no catalog tags were added.

- [ ] Write failing tests for enabled pipeline order, provider limits with multiple analysis workers, disabled offline mode, saved configuration upgrades, source changes during provider wait, failed tagging retaining previous cues, and no-match policy.
- [ ] Run `cargo test -p cuerithm-library --test preparation --test tagging` with deterministic provider fixtures; expect required-stage failure before integration.
- [ ] Wire the adapter and journal without changing DSP calculations. Preserve the 75%-CPU analysis allocation. Expose matching/tagging/writing stages and actionable errors. User-selected key/BPM field-source policy must not replace the measured analysis stored in the library with provider predictions.
- [ ] Run complete library/media/engine tests and real bundled model regressions. Require Ready only after success and no repeated work from the app's own tag changes. Commit.

### Task 5: Persistent auto-tag Settings and native acceptance

**Files:** Create `apps/desktop/src/views/TaggingSettings.tsx`; modify typed bridge/settings, progress labels, license UI and frontend tests; update developer/user docs.

**Interfaces:** Persist provider order/selection, all relevant common options, per-provider options, no-match policy and field-source preferences in `PreparationSettings`. No path picker or one-off auto-tag wizard. Unsupported tools stay excluded, not inert settings.

- [ ] Write failing UI tests for provider reorder/fallback, selected/overwrite fields, Beatport custom options, save/restart, no-match behavior, late responses and required-stage progress/error presentation.
- [ ] Run desktop UI tests; expect missing settings controls.
- [ ] Implement within the existing charcoal/teal workspace, using scrollable settings and readable labels. Keep analysis/cue options built in and current table selections intact.
- [ ] Run UI/typecheck/production/native builds; use a separate fixture library to verify incoming import → complete analysis/tagged bytes/replacement cues → automatic Tracks promotion, and restart/error recovery. Do not claim live provider success from static parsing fixtures. Commit.

### Task 6: Delivery

**Files:** Modify CI, notices, format/provider documentation and any required pinning/rebuild recipe.

- [ ] Run full local validation, actual packaged-resource checks and one fresh whole-delta review; reproduce/fix Important findings with regressions.
- [ ] Push a separate reviewed PR to Cuerithm; require native builds/checks on all desktop targets. Distinguish live catalog success, deterministic provider fixtures, copied-file writing and actual private-library operation.
- [ ] Preserve subsequent Lexicon cleanup, smart-playlist parity and direct-only rekordbox requirements. Publish no personal track data or vendor code extracted from Lexicon.
