# Source Timing Engine Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans inline. One fresh whole-change review after implementation; no implementation agents.

**Goal:** Use validated source-profile BPM in normal engine results without inventing strength or claiming unfinished final outputs.

**Architecture:** Extract the unchanged timing calculation into a shared source-analysis module. Add an explicit engine/CLI timing selection, preserve existing JSON readers and record the selected code/configuration identity.

**Tech Stack:** Rust, serde, Symphonia, existing own arithmetic, clap.

**Spec:** docs/research/source-timing-engine.md.

## Global Constraints

- Predictions consume audio and fixed configuration only; holdout stays unopened.
- Source profile:44100Hz stereo signed16 FLAC, full integrity verification, <=64000000frames, no fabricated short/silent timing.
- Exact full-precision BPM; energy features are not classified energy levels.
- No proprietary runtime/model/UI dependency or original-target mutation.
- No Git exists; preserve source snapshots/hashes instead of commits.

## Review Focus

- Shared timing extraction must leave every existing request-feature bit unchanged.
- Selected source mode must reject unsupported or partial input without falling back.
- Unknown strength and single anchor must not masquerade as measured confidence or a complete downbeat sequence.
- Old JSON/configuration and normal modes must retain usable behavior; compiled source identity must cover the new dependency.
- Separate decodes must not combine results from a changed file; inspect identity guard and profile/frame metadata.

### Task1: Typed source timing API

**Files:** source-analysis src/timing.rs, src/pipeline.rs, src/lib.rs, build.rs, Cargo.toml; tests/timing.rs.

**Interfaces:** produce `TimingAnalysis`, `analyze_timing_file(&Path)->Result<TimingAnalysis>`, `source_sha256()->&'static str`; share private timing computation with analyze_pcm.

- [x] Write tests for timing/file-feature BPM+anchor equality on own64second FLAC, finite ordered grid, full/prefix counts, unsupported-rate/short errors and64hex code identity. Run: `cargo test -p camelot-source-analysis --test timing`; Expected: missing API RED.
- [x] Extract the existing validation/legacy/waveform/Main code without changing its arithmetic. Add public typed wrapper and a build-time digest of sorted runtime Rust sources, Cargo.toml and build.rs.
- [x] Run timing tests and all source-analysis tests; Expected: all pass, existing314 tests preserved. Audit generated-source promotion separately; do not edit copied arithmetic modules.
- [x] Snapshot code/logs and ledger task completion (no Git).

### Task2: Main engine timing selection

**Files:** core engine.rs, rhythm.rs, neural.rs, analysis.rs, lib.rs; tests/engine.rs, tests/cues.rs, tests/json_precision.rs.

**Interfaces:** consume Task1 API; produce default-false `EngineSettings.source_profile_timing`, optional timing profile metadata in FileAnalysis, source TempoEstimate with optional strength/anchor.

- [x] Add engine tests for selected source mode's exact typed BPM/grid/anchor and absent strength; settings validation for partial/models/cues; old JSON compatibility; unsupported source errors; code/config identity differences. Run targeted tests; Expected: missing settings/optional fields RED.
- [x] Wire the selected timing before key/structure processing, disable unused classical tempo calculation, include code/config identity and file guard. Keep optional learned key path usable. Make existing strengths Some(score) and old/new anchor values explicit.
- [x] Run core tests with default/all features; Expected: green, source mode no fabricated cues/energy or confidence.
- [x] Snapshot/log/ledger completion.

### Task3: CLI and complete verification

**Files:** cli main.rs/tests/cli.rs and source timing tests; README/docs research.

**Interfaces:** consume source_profile_timing setting; expose `--source-profile-timing` to analyze/benchmark with full-input/model/cue conflicts.

- [x] Add CLI tests for successful synthetic source-mode output, unsupported rate, conflicting partial/model/cue flags and unchanged default behavior. Run; Expected: unknown flag RED.
- [x] Add flag and settings mapping, document validated scope/extra decode/no final-classifier claim.
- [x] Run fmt, Clippy default/all features, workspace tests/default/all features, release; Expected: all pass. Fresh five-request output regression and282 main-result BPM predictions, grading only after workers finish; Expected: all old captured fields and282 BPM bits exact, with failures retained.
- [x] Dispatch one fresh whole-change review on the most capable model with spec/plan/ledger and Review Focus verbatim. Fix material findings once with RED→GREEN; ledger minors/declined scope rulings. Retain final checkpoint; full engine goal remains active.

Completed bounded timing integration; evidence and review fix are retained in `.analysis/experiments/source-timing-engine-integration-v1`. The full engine objective remains active: key backend integration, energy/cue decisions, broader formats/platforms and held-out validation are unfinished.
