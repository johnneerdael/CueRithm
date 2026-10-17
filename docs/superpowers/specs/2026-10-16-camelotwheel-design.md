# CamelotWheel desktop application

Status: high-level Tauri/Rust desktop design approved in chat; updated for the
user-supplied reference corpus and first-class cue/Rekordbox requirements.

## Purpose and agreed scope

Build an independently implemented, open-source application inspired by the
user's purchased Mixed In Key 11. First prioritize musical key/Camelot, BPM and
a local music library. Include cue-point generation, editing and Rekordbox
interoperability as explicit project requirements rather than deferring them.
Use Tauri 2, a Rust backend and a React/TypeScript
interface, supporting macOS, Windows and Linux desktops.

Keep the installed application unchanged. Keep reverse-engineering copies and
decompiler records under the private, excluded `.analysis/` directory. Ship
original source/assets and explicitly licensed dependencies. The current
workspace starts with AGENTS.md and investigation documents, not an existing
application source tree.

## First-version user flow

1. Open the desktop app to an empty local library with an import action.
2. Select audio files or a folder through a native dialog, or drop files/folders.
3. Recursively discover supported files without following directory symlinks;
   deduplicate by canonical local path and report inaccessible inputs.
4. Show imported tracks immediately; run analysis in the background with visible
   queued/analyzing/ready/failed/cancelled states and per-file errors.
5. Browse title, artist, duration, musical key, Camelot label, BPM and status;
   search and sort results. No invented tracks populate the real library.
6. Select a track or wheel sector to see same-key, adjacent-number and relative
   major/minor compatibility. Explain the matching rule in the interface.
7. Allow manual key and tempo corrections, keeping them separate from estimates.
8. Reopen the app and recover saved tracks, estimates and corrections. Interrupted
   jobs become resumable queue entries rather than permanently analyzing rows.
9. Remove a library entry without deleting its audio file.
10. View, preview and edit suggested cue points, retaining generated estimates
    separately from user edits. Export a new Rekordbox XML file with track
    locations, cues and any verified beatgrid information.

## Reference-driven development

Use the supplied 434 FLAC files and CSV as the first vendor-reference corpus.
The local audit extracted source-labelled key, energy and cue results from all
files, plus a resolved CSV BPM reference. There are 364 declared-audio groups;
282 are assigned to tuning and 82 to a frozen holdout. Never allow an audio
copy into both sets. Independently verify decoded fingerprints before making
strong split-integrity claims beyond the declared metadata.

Compute predictions from audio only. Strip or ignore embedded key/BPM/energy/
cue answers when invoking engines. Store predictions and references separately.
Measure vendor agreement explicitly; do not confuse it with independent musical
ground truth or claim that reference extraction proves engine agreement.

Compare key/BPM engines first, then add cue inference informed by beat timing
and musical structure, calibrating or training against the tuning references.
Evaluate cue count, timing, order and names with one-to-one matching; retain
near-zero negative reference times and apply an explicit export policy. Freeze
engine choices before evaluating the holdout. Keep the later larger library
as a fresh external test and broaden missing key/tempo/style coverage.

The exact Mixed In Key server implementation is not a prerequisite for this
agreement-driven approach. Treat full agreement as a measurable target, not a
guarantee made before experiments.

## Architecture

- `crates/camelot-core`: platform-neutral Rust library for types, decoding,
  DSP/engine adapters, notation, analysis versioning and persistence.
- `src-tauri`: Tauri 2 host, native dialogs, application data paths, a bounded
  analysis worker and typed commands/events.
- `src`: React/TypeScript interface and a typed adapter for Tauri commands.
- `docs/research`: observed behavior, provenance, independent-engine research
  and unresolved original-app behavior.

Use Symphonia for decoding and RustFFT for the classical baseline. Persist with
SQLite through bundled rusqlite, avoiding a separately installed database.
Pin resolved versions and preserve lockfiles. No Node.js or Python runtime
should be required by an installed release.

## Analysis engines and provenance

Keep engine choice independent from the UI and database schema.

For key detection, implement a small transparent Rust baseline using spectral
peaks, tuning correction, pitch-class aggregation and public major/minor profile
methods. Benchmark it against licensed candidates before asserting useful
real-music accuracy. stratum-dsp is an evaluation candidate, but its numerical
labels must not be used as Camelot notation. libkeyfinder is a mature GPL
comparison candidate, not an implicit dependency of an MIT application.

For tempo, implement a classical onset/periodicity baseline and evaluate the
small Beat This! model through beat-this-rs. Prefer the neural engine when its
local accuracy, performance and platform checks justify it. Treat that choice
as an implementation result, not a predetermined accuracy claim. Package
selected model files with pinned hashes, source references and license notices;
do not download weights silently on app startup. If a neural adapter remains
unverified, label the baseline and defer that adapter explicitly.

Return tonic/mode as structured values. Compute all 24 Camelot mappings and
compatibility relations in one independently tested module. Return null/unknown
for insufficient tonal or rhythmic evidence. Preserve alternative keys and
tempi where available. Heuristic strength/margin values must not be displayed
as calibrated probability percentages.

Decode incrementally and use a bounded rolling DSP window. Keep whole-audio
buffers out of the UI and avoid unbounded per-file concurrency. Start with one
analysis worker; support cancellation between decode/feature chunks. If the
selected neural backend has an indivisible inference step, cancellation may
wait for that step, must be reported accurately, and must not commit its stale
result after cancellation.

## Supported input contract

Initially target unencrypted WAV/AIFF, FLAC, MP3, Ogg/Vorbis and M4A/MP4
AAC/ALAC where the selected decoder features support them. Verify representative
fixtures before advertising each format. A registered extension is not decoder
proof. Unsupported codecs, encrypted files, malformed inputs and files that
change during analysis produce actionable per-file errors.

Use source duration and sample-rate metadata when valid; otherwise derive
duration from decoded samples. Handle multichannel input deliberately before
mono analysis. Do not silently label partial/corrupt decoding as full success.

## Stored data and commands

Track identity: local path, display metadata, file size/modification identity,
duration, codec/rate where known, import time and stable track ID. Store analysis
version and engine/model identifiers alongside estimates, alternatives and
analysis timestamps. Keep user overrides in separate fields so reanalysis
cannot erase them. Store cues with stable local IDs, raw source timing when
imported, normalized seconds, names and optional slot/color provenance. Preserve
reference/generated/user-edited origins distinctly. Use schema migrations and
transactions.

Expose commands for listing/importing/removing tracks, starting/cancelling
analysis and updating corrections. List commands should return a durable state
snapshot, including progress and errors, so the frontend can recover after
missing events. Commands that write source-file metadata are outside v1.

Use Tauri's application-data directory on each OS. Restrict the webview to
bundled content and declared capabilities. Do not expose a generic shell or
arbitrary write command. Treat track metadata as text rather than HTML.

## Interface direction

Create an original, focused music-library workspace: clear table hierarchy,
readable key badges, restrained color, visible analysis progress and a usable
Camelot wheel. Use one layout on all desktops with sensible resizing and
keyboard access. Keep empty, loading, failed and cancelled states complete.
The interface should reflect actual backend state; browser previews may use
an explicit fixture adapter, never fabricated success in the desktop app.

## Cross-platform delivery and verification

Build the same Rust core and frontend on all three desktops. Account for path
handling and file-dialog differences without using macOS-only audio APIs.
Document WebKitGTK requirements on Linux and WebView2/MSVC requirements on
Windows. Add CI jobs on native OS runners; an unexecuted workflow is not evidence
of passing builds.

Verify locally:

- Core DSP and notation tests, including all 24 Camelot keys, wraparound and
  relative-mode matching.
- Synthetic key/tempo fixtures across sample rates, silence/noise, short/corrupt
  input and cancellation. These establish mechanics, not broad music accuracy.
- Temporary-database tests for persistence, duplicate imports, corrections,
  interrupted jobs and removal without source-file deletion.
- Type checking, frontend production build, Rust formatting/lint and tests.
- Corpus agreement evaluation with separate tuning/holdout reports, failed and
  unmatched items visible, and no tag-derived predictions.
- Cue export validation against the official Rekordbox XML format, followed by
  an actual disposable-playlist import/round-trip check when available.
- macOS Tauri desktop build and an actual import/analyze/relaunch smoke test
  using generated audio fixtures, plus screenshot inspection of the interface.

Report real-music evaluation separately, with independent annotations and
held-out parameter selection. Do not claim equivalent results to Mixed In Key.
Report Windows/Linux build and runtime coverage as unverified until native
checks actually run.

## Explicit later work

Energy calibration beyond cue needs, stems, tag writes, other DJ-library
formats and streaming-service integrations remain later work. Cue generation
and Rekordbox XML are part of the revised project scope. Preserve extension
points only where the first-version
code already requires them; do not scaffold unused subsystems.

## Evidence and rationale

- `docs/research/mixed-in-key-11.md`: artifact identity and package observations.
- `docs/research/analysis-service-contract.md`: local feature/server result split.
- `docs/research/offline-analysis.md`: upstream options and validation limits.
- `docs/research/reference-corpus.md`: vendor-labelled corpus, split and cue
  export requirements.

The inspected original path sends features to a vendor service; its server
classifier and weights are not present in the recovered client contract.
The supplied outputs nevertheless allow supervised calibration/training and
repeatable agreement tests. The independent app uses its own offline pipeline.
