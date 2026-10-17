> Superseded for cue generation by `docs/cuerithm-integration.md`. Do not
> continue MiK cue work from this historical plan.

# Offline Engine Implementation Plan

> **For agentic workers:** Use superpowers:executing-plans to implement inline,
> task by task. The active user goal authorizes engine work now; defer the UI.

**Goal:** Build and improve a portable Rust audio-analysis engine against the
user's Mixed In Key key/BPM/cue references, with measured agreement and no
reference leakage.

**Architecture:** Keep a PCM-only analysis core separate from file decoding and
the reference evaluator. Expose JSON through a CLI now and the same library to
Tauri later. Make every benchmark reproducible by recording configuration,
versions, corpus/split identity, successes and failures.

**Tech Stack:** Rust, Symphonia, RustFFT, serde, clap; evaluate Beat This! Rust
for learned beat/downbeat tracking with explicitly packaged model assets.

**Spec:** `docs/superpowers/specs/2026-10-16-camelotwheel-design.md` and
`docs/research/reference-corpus.md`; the latest user instruction sequences the
engine before all UI work.

## Global Constraints

- Target macOS, Windows and Linux; no platform-specific audio dependency.
- Preserve original application, source audio and CSV files.
- Keep `.analysis/`, music, reference rows and per-track results private.
- Never derive predictions from embedded key/BPM/energy/cue tags or filename.
- Preserve one audio group in one split. Tune on the tuning split only.
- Keep failed/unknown estimates visible in benchmark denominators.
- Do not call an engine settled based on tests or a narrow sample alone.
- Do not implement the UI during this engine phase.

## Review Focus

- Silence and nonmusical audio should not acquire fabricated confident keys.
- Decoding corruption, truncation and source changes must not become success.
- Sample-rate differences and chunk boundaries must not change time units.
- Duplicate reference aliases must not inflate scores or cross data splits.
- Multiple generated cues must not count against one reference cue.

## Tasks

### Task 1: Portable core, notation and PCM decoding

**Files:** `Cargo.toml`, `crates/camelot-core/{Cargo.toml,src/lib.rs,src/key.rs,src/audio.rs}`, tests.

**Interfaces:** `MusicalKey { tonic, mode }`, `camelot()`, `DecodedAudio`,
`decode(path, target_rate, max_seconds)`; analysis consumes mono PCM only.

- [x] Write failing tests for all 24 Camelot mappings and relative/adjacent keys.
- [x] Implement and verify notation, including 12/1 wraparound.
- [x] Write a failing generated-WAV decode test for sample count/downmix/rate.
- [x] Implement bounded mono decoding/resampling and verify errors for corrupt input.
- [x] Create project instructions, ignore rules and license without changing the installed app.

### Task 2: Independent key and tempo baselines

**Files:** `src/tonal.rs`, `src/rhythm.rs`, `src/analysis.rs` in camelot-core; generated-signal tests.

**Interfaces:** `analyze_pcm(samples, rate, config) -> Analysis`, including key
ranking, uncalibrated strength, BPM candidates and engine/config identity.

- [x] Write failing tests for synthetic tonal sequences, silence and detuning.
- [x] Implement spectral-peak chroma and public profile correlations.
- [x] Write failing tests for click tracks at several tempi/rates and silence.
- [x] Implement onset/periodicity tempo estimation with explicit alternatives.
- [x] Run core tests and document baseline limitations.

### Task 3: CLI and reference evaluator

**Files:** `crates/camelot-cli/`, `crates/camelot-core/src/evaluation.rs`.

**Interfaces:** `analyze PATH`, `benchmark --inputs --references --split tuning`,
saved predictions JSONL and aggregate JSON; input/reference manifests remain separate.

- [x] Test one-to-one cue matching and metrics with missing/failed predictions.
- [x] Test grouped reference joins, conflicting duplicate labels and split mismatch rejection.
- [x] Implement CLI analysis and deterministic benchmark selection/checkpointing.
- [x] Verify on generated audio with deliberately misleading analysis tags.
- [x] Run a tuning pilot, inspect errors, then the complete tuning partition.
- [x] Reproduce/fix changed-source resume and interrupted-final-record recovery.

### Task 4: Beat/downbeat candidate and calibration

**Files:** `src/neural.rs`, explicit model manifest/acquisition utility,
private benchmark artifacts.

- [x] Pin source/model hashes and preserve licenses; use no startup downloads.
- [x] Integrate Beat This! through PCM input, with model identity in results.
- [ ] Compare tuning-set BPM agreement, runtime and memory against baseline.
- [ ] Improve selection/calibration using tuning labels; preserve alternatives.

### Task 5: Cue inference and measured improvements

**Files:** `src/cues.rs`, cue fixtures, benchmark reports and engine documentation.

- [x] Use beat/downbeat/structure features for experimental cue candidates; keep vendor labels external.
- [x] Add an opt-in, PCM-only local structure/energy feature timeline with signal tests.
- [ ] Measure cue count, one-to-one timing agreement, names and boundary handling.
- [ ] Improve key/BPM/cue estimates or train/export a local model when justified by tuning errors.
- [ ] Freeze chosen engine/configuration before a held-out evaluation.
- [ ] Report held-out agreement, coverage and limitations; keep the goal active if material gaps remain.

## Completion evidence

Run formatting, lint, full tests, release build, complete tuning benchmark and
the frozen held-out benchmark. Verify source files remain unchanged and that
the CLI works without reference manifests. Use a fresh whole-engine review.
Document actual platform checks, not just an unexecuted CI matrix. The user
subsequently prioritized musical/DJ usefulness above exact vendor agreement.
Keep agreement as a secondary measurement, report remaining gaps and validate
key/BPM, energy, cue placement and section-label usefulness independently before
claiming the engine is settled. The optional All-In-One direction was approved
on 2026-10-17: preserve the complete structure, then derive and validate useful
DJ cues separately; app-specific export remains a separate integration gate.
