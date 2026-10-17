# File-to-request engine implementation plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan inline.

**Goal:** Compute all ten validated analysis-request fields from a supported FLAC file in one offline Rust process.

**Architecture:** Add camelot-source-analysis with private arithmetic modules copied from the forty independent prototype libraries. Use typed adapters for decode, classic/Main/key/energy and a public feature-only result; expose it through camelot-core and camelot analyze-features. Keep existing final-analysis behavior.

**Tech Stack:** Rust1.89+/2024, existing Symphonia0.5.5/serde/serde_json(float_roundtrip)/sha2/anyhow plusbase640.22. No application SDK, Python or network at runtime.

**Spec:** docs/research/file-request-integration.md

## Global constraints

- Preserve the original engine-first cross-desktop goal and exact-output acceptance gate.
- Source-only44100Hz stereo16-bit FLAC initially; unsupported profiles return explicit errors.
- No source tags, filenames, original intermediate arrays, labels, request values or models as predictions.
- One fresh decode feeds all own components; full frame count and reader-prefix count remain distinct.
- Preserve original app/music/CSV/preferences/credentials; heldout82 stays unopened.
- No runtime .analysis/proprietary/Python dependency; no UI or release/version bump.
- Existing short-classic initialization failure remains unresolved and must propagate honestly.

## Review focus

- Namespace promotion changing same-named dependencies or crate-local FFT paths: audit deterministic source transformations and original hashes.
- Decoder tail/full-duration confusion: five fresh full-file comparisons and explicit frame-count metadata.
- Short/silent timing failure: public API returns error, never classifies fabricated timing.
- Unsupported metadata/profile or malformed PCM: numeric-only typed API and explicit decoder guard tests.
- JSON losing float bits or confusing features with final decisions: round-trip tests and all ten captured analysis-field comparisons.

### Task1: Bring independent arithmetic into workspace source

Create crates/camelot-source-analysis/Cargo.toml, src/lib.rs, src/dsp/mod.rs, src/dsp/<component>/src/*.rs, own synthetic FLACfixtures, and source-origins.json. Modify rootCargo.toml workspace/base64 entry. Preserve all forty dependency-selected library sources (including reached localFFT/table modules), their unit tests and numeric constants; replace only dependency/crate namespaces and synthetic fixture locations. Record every original/transformed source digest. Keep private CLI mains/native artifacts outside.

Interfaces: private dsp modules preserve reviewed signatures; workspace crate initially exposes only a scaffold.

- [ ] Add source-copy audit; verify it fails with a missing/mutated required module.
- [ ] Promote own sources with component-local dependency alias resolution; run the audit and component tests.
- [ ] Resolve only integration/compiler issues without changing arithmetic; retain any failed checks.

### Task2: Typed file-to-analysis-request pipeline

Create src/pipeline.rs, src/request.rs, src/energy.rs and src/waveform.rs, plus tests/pipeline.rs. Add typed key-bank aggregation and energy timeline accessor in the promoted modules; retain adapter-equivalence tests.

Consumes reviewed numeric methods. Produces pubfn analyze_file(path:&Path)->anyhow::Result<FeatureAnalysis> and pubfn analyze_pcm(pcm:&[f32],full_frames:usize)->anyhow::Result<FeatureAnalysis>; FeatureAnalysis.analysis is a serializable AnalysisRequest with ten fields.

- [ ] Write failing malformed/profile/short-input and exact JSON round-trip tests.
- [ ] Decode once, compute own classic timing, Main waveform/context/timing and key cache, then aggregate/wrap key segments.
- [ ] Feed own energyStream and use typed rawband/RMS access; compute temporal/183/dB results once, omitting empty energy arrays.
- [ ] Assemble versioned numeric payload with no final classifications/auth/metadata and make tests pass.

### Task3: Expose and verify the integrated engine

Modify coreCargo.toml/corelib.rs and CLI main.rs; create CLI feature-command tests; updateREADME/research maps. Public consume: camelot_core::analyze_file_features and camelot analyze-features PATH, returning feature-only envelope.

- [ ] Write/run failing CLI error/feature output test; implement the command and pass it.
- [ ] Run five fresh supported development files through the single worker; expected requests stay outside workers until all terminate. Compare all ten analysis fields, bitwise numeric arrays and cue payload strings, including failures in denominators.
- [ ] Run isolated offline Linux from workspace source plus same five copied inputs, with no expected values/mounts/network; remove worker container before grading.
- [ ] Run workspace tests/fmt/clippy/release; record current failures and unchanged full-library benchmark.
- [ ] Obtain one fresh whole-change review; fix material findings in one RED->GREEN pass. Preserve review/source/worker identities and exact scope.

NoGit repository is present: preserve source hashes/checkpoints instead of pretending to create commits or worktrees. Existing approved execution remains inline; this plan records implementation choices and does not introduce another approval gate.
