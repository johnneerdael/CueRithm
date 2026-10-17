# OpenKeyScan Rust Backend Implementation Plan

> **For agentic workers:** Execute inline with the executing-plans skill. Keep private feasibility diagnostics distinct from product code. One fresh whole-change review after production integration; no implementation agents.

**Goal:** Run the fixed OpenKeyScan3 audio-to-key pipeline offline from the Rust engine with measured parity and explicit provenance.

**Architecture:** Build the same portable SoXR source and use fixed CQT kernels with RustFFT. Feed the resulting complete spectrogram to the already validated ONNX/RTen network. Validate a private worker before adding the engine selection and CLI interface.

**Tech Stack:** Rust, Symphonia, RustFFT, C99 libsoxr/CMake, RTen, serde, pinned ONNX.

**Spec:** docs/research/openkeyscan-rust-backend.md.

## Global Constraints

- Audio and fixed configuration/model only; no runtime labels/tags/filenames as predictions, no per-track rules.
- Preserve upstream source/checkpoint/graph and the82group holdout; keep failed attempts and failures in denominators.
- Initial diagnostic profile: complete verified native44100Hz stereo16-bit FLAC,<=64000000frames; broader formats remain required later.
- CQT arguments and dtype/normalization/crop semantics exactly as the spec; internal tolerance1e-4absolute+relative, output class parity exact.
- No Python, platform DSP framework, original application or analysis service at runtime.
- Preserve third-party license notices and source provenance; no Git exists, retain hashes/snapshots instead of commits.

## Review Focus

- Long, short, silent, odd-length and nonfinite input must have deliberate semantics, with no fabricated confidence or hidden fallback.
- Kernel/bin/class ordering, boundary padding and crop must remain source-consistent for variable lengths.
- FFI lengths/ownership/error cleanup and cross-platform C ABI must not permit buffer overflow or stale pointers.
- Unsupported profiles, conflicting key models and changed source files must not produce apparently verified mixed results.
- Source/model/config identity and older engine settings/results must stay meaningful across backend selections.

### Task1: Component feasibility and private full frontend

**Files:** .analysis/experiments/openkeyscan-resample-native-v1; new openkeyscan-cqt-frontend-v1 diagnostic.

**Interfaces:** consumes generated PCM/reference CQT and pinned graph; produces fixed kernel/normalizer assets plus a PCM/file-to-CQT-to-key diagnostic.

- [x] Preserve the existing14network and15projection-stage parity records; validate the exact native SoXR half-rate and scalar rounding recipe on generated boundaries. Expected:15raw/scaled cases byte-exact; no Python in the Rust runtime.
- [x] Assemble the fixed frontend in a private diagnostic worker; compare generated PCM and five full-track tensors. Expected: all internal vectors within declared tolerance and class parity exact; no tuning from MiK labels.
- [x] Freeze source/assets/toolchain and numeric reports, with unsupported/silent/short behavior recorded. Expected: reproducible diagnostic, not a production-completion claim.

### Task2: Full independent tuning diagnostic

**Files:** new .analysis/experiments/openkeyscan-rust-tuning-v1 runner/scorer/reports.

**Interfaces:** consumes Task1 frozen worker/model; produces one result per same282 tuning identities.

- [x] Predict from private read-only input snapshots, with network and expected-data contents denied. Observe every worker terminate before opening references.
- [x] Compare all classes against the fixed upstream run, and independently grade MiK11 key agreement. Expected:282/282 upstream class parity and274/282 MiK11 agreement, including failures; retain contrary evidence if not met.
- [x] Run generated and five-track native component checks on Linux ARM64 offline, preserve cleanup and source identity. Expected: exact class parity, numerical errors measured separately; Windows remains unverified until executed.

### Task3: Production key selection

**Files:** `crates/camelot-core/src/openkeyscan.rs` and `openkeyscan/kernels.rs`; `crates/camelot-core/native/openkeyscan/{CMakeLists.txt,half.c}`; vendored `crates/camelot-core/vendor/soxr`; core `build.rs`/Cargo features; engine/analysis/lib exports; CLI arguments/features; backend/settings/CLI tests; provenance and third-party notices. Keep the three-crate workspace.

**Resolved behavior:** Optional `openkeyscan-key` feature with pinned RTen 0.24.0 and cmake 0.1.58. Load and verify one owned model-byte buffer, then parse those same bytes. Add `key_openkeyscan_model` and `--key-openkeyscan-model`, mutually exclusive with profile/S-KEY choices. Decode DSP/22050/44100 rates independently in one pass with deduplication and selection by actual rate. Record the OpenKeyScan input rate and model/frontend/native identities; omit the new rate/settings fields when absent and preserve the old configuration digest structure. The source digest changes for new source/build/native/kernel content. Inputs shorter than three seconds or RMS<=1e-5 return unknown; invalid rate/nonfinite/>64000000 analyzed44100Hz samples return errors. Explicit excerpts remain visibly partial. The old strict file-profile gate belongs to diagnostics, while the product uses the existing decoder formats and direct44100Hz resampling, with broader-format parity still requiring tests. Raw finite logits rank classes1A..12A then1B..12B; do not fabricate probabilities. No Python/platform DSP at runtime. Do not change frozen diagnostic or live All-In-One files.

**Interfaces:** consume validated PCM frontend/model; produce optional explicit OpenKeyScan setting and final KeyEstimate with raw uncalibrated ranking.

- [x] Add failing tests for all24class mappings, short/silent/error semantics, model checksum, settings conflicts, direct input-rate selection, old JSON and code/config identity. Expected: missing API/settings RED.
- [x] Implement the explicit backend without changing default/S-KEY behavior; include kernel/native/model identities. Expected: targeted tests GREEN.
- [x] Run full default/all-feature suites, fmt/Clippy/release and fresh end-to-end corpus checks for the final binary. Expected: green and unchanged measured parity.
- [x] Dispatch one independent whole-change review, fix material findings with RED-to-GREEN and full checks, and retain the final checkpoint. Full engine goal remains active until energy/cues/platform/heldout requirements are also met.


Task3 execution evidence: `openkeyscan-engine-integration-v1`. Final regular CLI
282/282 complete/integrity-verified,282/282 upstream key classes,274/282 MiK11
keys; fresh-request BPM/anchor282/282 exact binary64,199371 prior beat values
unchanged. Mac386 default/399 all-feature tests pass; Linux ARM64/Rust1.89
399 all-feature tests plus real model/Engine/combined CLI pass offline. Sources,
originals and snapshot cleanup verified. No Windows, held-out accuracy or full
engine-completion claim. Whole-change review has no code findings.


### Windows runtime follow-up, 2026-10-17

The existing x64 MSVC release executable and all26 cross-compiled test targets
now execute successfully on the user's AWS WorkSpace (Windows NT10.0.17763,
AMD64,64-bit process):399 tests pass,7 are normally ignored, and both pinned
OpenKeyScan model tests pass explicitly. No failures or timeouts occurred. A
generated-audio CLI smoke returns6A with the expected model checksum and the
same source/configuration identities as Mac. No product source changed; the
PowerShell reporting harness needed an architecture-property compatibility fix.
The original failure is retained and the corrected harness also passes on Mac.
Native Windows compilation, real-music Windows parity, held-out accuracy and
whole-engine acceptance remain separate gates. Private evidence is
`openkeyscan-windows-msvc-v1/workspaces-v1/verification.json`. The temporary
transfer server/tunnel are stopped; the user's Windows session remains open.
