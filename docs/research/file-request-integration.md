# File-to-request engine integration specification

Continue the approved engine-first Tauri2/Rust cross-desktop design. Build an offline Rust API and CLI that compute the ten analysis request fields directly from an audio file in one process, using the independently reconstructed components. Preserve the existing analysis API and CLI behavior. Final key/energy/cue classifications remain separate unfinished work; no request-feature output may masquerade as classified results.

Initial exact profile: Mixed In Key11.2.6.7446 default, source-rate44100Hz stereo16-bit FLAC, reader-returned prefix with full-file decoded frame count retained for energy duration. Reject other formats/configurations explicitly until independently validated. Decode/verify once; feed own512-frame schedules to key/energy/classic paths, compute Main waveform/context/timing, aggregate key banks, finish energy/resultfields, and serialize independent numeric arrays with exactfloat parsing. Predictions accept only audio bytes and fixed configuration. Ignore source tags, filename-derived labels, original timing/features, expected requests and service responses.

Public output: a FeatureAnalysis envelope with source/analyzed frame counts, profile identity, and AnalysisRequest containing Segments, EnergySegmentData, DurationInSeconds, Tempo, DownbeatTime, CuePointStartBeat, CuePointData, KeyAlgorithmVersion94, EnergyAlgorithmVersion"2" and CuePointAlgorithmVersion"3". No authentication, service call, proprietarybinary/runtime, Python runtime, privatefixture dependency, product impersonation or copied metadata. The captured five requests grade only these ten analysis fields after workers finish; complete HTTP body equality is not claimed.

All independent algorithm code needed at runtime must live under workspace source. Preserve the reviewed arithmetic, tables and numeric domain checks; document mechanical namespace changes and copied own-source hashes. Include only independently written Rust and synthetic fixtures, never original executables/decompiled listings/music/captures/labels. Keep low-level details private behind typed API. Preserve explicit unknown/error behavior for the unresolved short-classic phase; no invented zero, fallback algorithm or silent accuracy downgrade. Existing production full-run scores remain unchanged until rerun.

Verification: unchanged workspace baseline, source-copy audit, component synthetic tests, meaningful public API/CLI error and numerical serialization tests, full five-file fresh Mac/Linux workers with no expected inputs, ten-field capture comparison, format/unsupported/short-input controls, workspacefmt/clippy/tests/release, and one fresh whole-change review. Windows and broader/held-out agreement stay unverified until actually executed. Leave heldout82 untouched. No UI until engine acceptance gate is met.

## Current implemented boundary

The workspace contains `camelot-source-analysis`, exposed through
`camelot_core::analyze_file_features`, `analyze_pcm_features` and
`camelot analyze-features`. One fresh verified FLAC decode feeds every stage
inside one Rust process. Forty independent arithmetic libraries were promoted
with original/transformed source hashes, mechanical namespace changes, formatting
and a read-only energy timeline accessor. No original SDK, private experiment
intermediate, service, Python runtime or model download is used.

All ten analysis fields match five captured development requests after fresh
file runs: **16,921 numbers and 15 strings**, including key banks, 81 energy
segments, timing, packed cue scores and algorithm versions. Linux ARM64 rebuilds
the workspace and decodes the same files independently; all complete output JSON
bytes match macOS ARM64. Both platforms pass 369 workspace tests, with two
existing private-data tests explicitly ignored. Formatting, Clippy and release
checks pass on macOS. Windows execution remains unverified.

A separate control changes one copied file's basename and sets deliberately
false key/BPM/energy tags. Its complete output remains byte-identical to the
original prediction. The source input remains unchanged.

The first Linux attempt lacked optional `beat-this` registry metadata and
stopped before analysis. A second archive supplies public Cargo metadata and
153 cached crate archives verified against `Cargo.lock`. Networking and mounts
remain disabled, and the container is removed before expected-output reads.
The initial failed run is retained.

This integrates the earlier same-five cached Main/key/energy chain into a fresh
file-analysis path. A fresh whole-change review passed its isolated build, tests,
five file replays and capture comparison, with no Critical/Important findings.
Its sole Minor finding concerned documenting the raw-PCM API contract; the API
now states its fixed rate, signed16 normalization, analyzed-prefix and separate
full-source-count obligations. Existing final
analysis estimates and their full-library benchmark are unchanged. Final key,
energy and cue decisions, broader/held-out inputs, other formats/configurations,
Windows and the short-classic phase ambiguity remain open. Engine acceptance
is incomplete; the UI remains deferred.

## Broader tuning measurement

A subsequent fresh macOSARM64 run covers all282 tuning representatives.
All282 stored full-precision BPM values match exactly. All6182 energy-feature
segments match the retained native extraction, including1149852 timing, volume
and feature numbers. Reader-prefix/full-file counts and request durations also
match on all282. The15 initial sandbox filename failures are retained and all
succeeded after a quoting-only harness correction with the same frozen worker.
See [engine measurements](engine-benchmarks.md) for the denominator, barriers and
separate root bit check. This extends tempo/energy-feature evidence; it does not
extend captured-request coverage or establish final key/energy/cue classification.
