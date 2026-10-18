# Lexicon timing implementation plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan inline task-by-task. One whole-branch review at the end; no per-task sub-agents.

**Goal:** Use the independently reproduced complete Lexicon timing path with the existing Lexicon cues in automatic desktop preparation.

**Architecture:** Add focused Rust tempo, onset/phase and anchor modules behind a complete timing API. Capture source outputs at component and whole-worker boundaries, then integrate the verified backend with the existing canonical media clock and cache identity. Preserve legacy timing for comparison.

**Tech Stack:** Rust, existing FFT/media dependencies, Tauri 2; original synthetic fixture recipes and original-worker numeric references.

**Spec:** `docs/superpowers/specs/2026-10-18-lexicon-timing.md`

## Global constraints

- Preserve camelot-core's Rust floor and existing key, scalar energy and cue calculations.
- Rust runtime only; no proprietary runtime or external timing model in the desktop.
- Complete finite first-channel Float32 PCM, rates 8000..192000; tempo bounds 50..300, min < max; defaults 98..195.
- Return unknown promptly for empty/short/silent/no-peak inputs; invalid options/nonfinite PCM are errors.
- No Analysis settings page, model picker or download; migrate unfinished jobs without rewriting completed results.
- No music, credentials, per-track labels or proprietary code in published artifacts; no tag/library writes during comparison.
- Support all existing common audio layouts across Linux, Windows, macOS ARM and Intel.

## Review focus

- Extremely quiet or denormal audio must not stall threshold searches or acquire a fabricated fallback rating.
- A strong right channel must not silently replace a weak left channel in timing input.
- Source replacement and encoder delay must not shift grids or associate cues with a different PCM snapshot.
- Fractional BPM must remain unrounded through cue generation, while display/export rounding remains consistent.
- Partial ports and tests must not select a new desktop default before complete anchor coverage.

### Task 1: Exact tempo evidence and guarded inputs

**Files:** Create `crates/camelot-core/src/lexicon_timing/{mod,tempo}.rs`, `crates/camelot-core/tests/lexicon_timing.rs`; modify `src/lib.rs`; retain `tests/fixtures/lexicon-timing/`.

**Interfaces:** `LexiconTempoOptions { min_tempo: f64, max_tempo: f64 }` defaults 98/195; `TempoEvidence { bpm: f64, confidence: f64, peaks: Vec<usize> }`; `analyze_tempo(pcm: &[f32], sample_rate: u32, options: &LexiconTempoOptions) -> anyhow::Result<Option<TempoEvidence>>`. This is component evidence, not the full timing/default backend.

- [ ] RED: regenerate all eight recipes in Rust and verify their PCM SHA-256; assert exact peak arrays, exact best BPM and confidence within 1e-12 against the original module 745 references. Assert all rows were consumed.
- [ ] RED: empty, sub-window, silence, near-zero/no-peak input return None promptly; NaN/infinity, invalid rates/bounds return errors. Confirm failures are missing APIs/behavior.
- [ ] Implement adaptive energy-change peaks, interval/tempo voting with stable insertion/tie order, correlation and source harmonic/fractional refinement. Preserve Float64 operation order. Do not return a source 120/zero fallback as successful timing.
- [ ] Run release component tests, existing core regressions, Fmt/Clippy and fixture recipe verification; record source boundary and exceptions. Commit.

### Task 2: Onset and grid-correction primitives

**Files:** Create `src/lexicon_timing/{onset,phase,dp}.rs`, `tests/lexicon_timing_primitives.rs`; extend original numeric fixtures.

**Interfaces:** Internal explicit-input helpers consume Float32 PCM/rate and BPM/anchor, returning correction evidence with an `applied` flag. DP envelope/chain preserve Float32 arrays and source tie order; no mutable process-global tracking.

- [ ] Capture original helper outputs for attack/onset, subsequent-beat, half-beat, riser, correlation, silent/weak rescue and DP chain/phase branches. Publish only authored recipes/numeric outputs.
- [ ] RED each helper's boundary, unchanged branch, applied branch and short/invalid guard before implementing it. Include asymmetric channels and quiet/denormal input where relevant.
- [ ] Implement all source-used primitives from modules 974, 522 and 809. Preserve correction tracking explicitly within an invocation.
- [ ] Verify reference cases and all Task 1 tests, Fmt/Clippy; document exceptions. Commit.

### Task 3: Complete anchor decision tree

**Files:** Create `src/lexicon_timing/{intro,anchors,walkback}.rs`; extend `mod.rs` and timing tests/fixtures.

**Interfaces:** `TimingEvidence { bpm: f64, first_beat_seconds: f64, confidence: f64 }`; `analyze(pcm, sample_rate, options) -> Result<Option<TimingEvidence>>`, consuming Tasks 1/2 without changing cue DSP.

- [ ] Capture branch-covering original intro/drop/DnB/walkback and complete-worker outputs, including longer intros and different ranges/rates. Verify exact input hashes.
- [ ] RED complete timing and branch references before porting modules 473, 578, 382 and 358. Include the eight initial final-anchor references; integer/fractional tempo, pickup, riser, half-beat and weak-grid branches must be represented.
- [ ] Implement complete correction sequence and guards. Do not port the unrelated unused fluid-analysis path merely because it is imported by the bundle.
- [ ] Pass all component/full references and core tests, Fmt/Clippy. Record any output tolerance based on measured numeric propagation, never adjust fixtures to the Rust result. Commit.

### Task 4: Media clock and engine integration

**Files:** Modify `src/{audio,engine,prepared}.rs`, relevant source-identity tests, `crates/cuerithm-library/tests/` common-format tests and desktop `builtin.rs` tests.

**Interfaces:** Add `EngineSettings.lexicon_timing: bool` default false; mutually exclusive with source-profile and learned timing. Full `TempoEstimate` uses method `lexicon-timing-v1`, unrounded BPM, first-beat anchor and generated constant beat times, with no invented downbeat list.

- [ ] RED first-channel/stereo-asymmetry, complete-input/mode validation, source-replacement, clock/delay and algorithm/options cache identity tests. Preserve historical serialized settings/results.
- [ ] Retain first-channel PCM in direct decoding and read canonical prepared first-channel PCM on its existing playable clock. Do not change key/energy downmix or cue arithmetic.
- [ ] Integrate complete timing with normal analysis; test fractional BPM arrives unchanged at cue generation and rendered grid. Keep desktop selection unchanged until this task's common-format gates pass.
- [ ] Verify all 14 supported media layouts and full engine/library regressions, cache/migration/Ready guarantees and Fmt/Clippy. Commit.

### Task 5: Desktop selection, corpus and final delivery

**Files:** Modify `apps/desktop/src-tauri/src/builtin.rs`, migration/integration tests and timing research docs; add original corpus runner if needed.

**Interfaces:** Desktop selects Lexicon timing, migrates obsolete unfinished snapshots, preserves completed results. Research legacy backend remains explicit and callable.

- [ ] RED desktop configuration/migration and native-grid/cue agreement tests. Switch selection only after complete timing gates; no new Analysis settings UI.
- [ ] Run an owned full-track comparison; record source/decoder/algorithm identity, failures, BPM/anchor/cue disagreement and throughput. Keep per-track data private; do not infer superiority from agreement alone.
- [ ] Native isolated-library preparation/playback/reanalysis on generated files; confirm key, energy, full-grid view, cues and source music integrity.
- [ ] One fresh whole-branch review, one tested fix pass, full local gates and four-platform hosted checks. Push PR with concrete evidence and remaining limits. Update ledgers; do not mark the larger library/sync goal complete.
