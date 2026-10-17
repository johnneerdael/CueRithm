# Source-profile timing in engine results

Continue the approved engine-first Rust design and exact-output requirement.
The completed282-track tuning run establishes exact source-profile BPM and
energy-feature extraction. Make the validated timing usable through the main
engine result; final key/energy classifiers and cue selection remain required.

Expose a typed `TimingAnalysis` and `analyze_timing_file(&Path)` from
camelot-source-analysis. Share PCM validation and the existing legacy/waveform/
Main calculation with full request generation, preserving arithmetic, source
reader-prefix behavior and explicit unresolved short/silent errors. Return
profile identity, full/analyzed frame counts, BPM, first adjusted grid anchor
and computed beat times. A timing-only call does not compute unused key/energy
feature arrays. The broader282-run proves BPM; complete beat-grid equality
outside the earlier five cases remains a separate unverified diagnostic.

Add opt-in `EngineSettings.source_profile_timing` and CLI
`analyze --source-profile-timing` (also available to benchmark). Require complete
input, reject learned-beat model and cue-suggestion combinations, and reject
unsupported source profiles instead of substituting timing. Continue normal
key/structure analysis. Return the source BPM/grid/anchor with a distinct method
and profile/frame-count metadata in normal `FileAnalysis`. Existing mode stays
available for broader-format development; this is an explicit backend selection,
not completion of the full fidelity gate.

Tempo strength becomes `Option<f64>` with missing values omitted: classical/
neural scores remain numeric in existing JSON, and source timing has no invented
strength/candidate scores. A separate optional downbeat anchor records the
source value without pretending to supply a complete metrical downbeat list.
Old serialized settings/results must remain readable. Source timing is included
in configuration identity, and compiled source-analysis code is included in the
engine source identity. No model download or proprietary runtime dependency.

Implementation initially verifies/decodes the input for source timing and uses
the existing independent decoder for key/structure buffers. Keep an outer file
identity guard across both decodes. The whole-change review identified that
pathname hashes alone cannot bind the two opens during a transient A-to-B-to-A
replacement. Both readers must therefore open one private encoded snapshot,
which is removed on success/error, and compare its captured content/metadata
identity with the original before returning. This costs temporary disk space
and extra decoding but preserves both
validated reader contracts; a unified reader is a later performance task.
Unsupported rate/channel/bit-depth and unresolved timing fail the selected mode.

Validate through synthetic API/CLI/protocol errors, exact serialization,
old-settings/results compatibility, complete existing feature regression,
five captured-request comparisons and a fresh282-track main-result BPM run.
Use one fresh whole-change review. No original-app execution, holdout, UI,
Windows certification or final-classifier accuracy claim follows from this work.
