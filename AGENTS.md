# Cuerithm engine guidelines

## Project Structure & Module Organization

This workspace contains an independent Rust audio-analysis engine and is expanding into the Cuerithm desktop library application. Use Tauri 2 with React/TypeScript and a Rust backend; target macOS, Windows and Linux. The user approved a library sidebar, track table and persistent waveform/cue panel with OneTagger's charcoal/teal visual direction. Incoming automatically applies configured tagging/cleanup, full analysis and replacement cues before moving completed tracks into Tracks. Include Beatport/Traxsource auto-tagging, Lexicon-style cleanup and playlists, and direct-only rekordbox 6/7 sync; do not add XML transport. See the current design under `docs/superpowers/specs/` for delivery boundaries and review status. Cue generation uses the validated Lexicon-compatible DSP in `lexicon_cues.rs`; preserve its calculations. Keep key analysis, BPM and a single whole-track energy rating. Private Mixed In Key cue-model research is permitted. Do not attach energy labels to product structural cues or expose per-segment energy in normal analysis.

- `crates/camelot-core/`: PCM decoding, notation, DSP and evaluation.
- `crates/camelot-cli/`: analysis and benchmark CLI.
- `docs/research/`: public aggregate findings and provenance.
- `.analysis/`: private binary copies, corpus labels and experiments; exclude from publication.

The first usable desktop delivery must support MP3, AAC/ADTS and AAC/M4A,
ALAC/M4A, FLAC, WAV, AIFF, Ogg Vorbis, Opus and WMA throughout analysis and
playback. Do not ship a FLAC-only stage or mark other formats complete after
skipping required timing/energy work. Bundle required native decoder resources;
users must not install a decoder runtime themselves.

## Build, Test, and Development Commands

Run these commands from the workspace root:

```sh
cargo test --workspace
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo build --workspace --release
```

These commands validate the independent engine, not the purchased application.

## Coding Style & Naming Conventions

Use focused Rust modules, explicit errors and standard formatting. Keep analysis APIs independent of Tauri. Quote paths with spaces. Do not require platform-only DSP APIs or a Python runtime in the installed engine.

## Testing Guidelines

Compute predictions from PCM and explicit configuration only. Never use source key/BPM/energy/cue tags, filenames or reference labels as predictions. Keep reference labels separate from engine inputs; tune on the tuning split and keep audio copies in one split. Leave held-out results untouched until engine/configuration freeze. Report exact key, BPM and cue agreement separately, including failures/unknowns in denominators. Synthetic tests do not establish real-music accuracy. Record engine/config/model identity for comparisons.

## Required Facts Register Before Analysis or Modelling

Before changing feature extraction, key/BPM/energy analysis, training, evaluation or model promotion, read [the facts register and modelling constraints](docs/research/mik-facts-and-modelling-constraints.md). Cite its revision and relevant fact IDs from the current revision in the experiment record and PR; explain when a change has no applicable findings. Historical notes do not override a scoped counterexample in the register.

- Before rendering or fitting, list the relevant facts and scopes, and check proposed acceptance gates against any reused baseline's recorded values. Authored timing and an analyser's detected grid are separate measurements.
- Use 100% agreement as the research target for key, whole-track energy and complete ordered cue lists; qualify genuinely unseen music and the native PCM path separately. A perfect training/development fit is not completion.
- Begin each proposed correction with a case-level error analysis: exact failures, corrected cases, regressed known matches, unchanged failures, affected classes/contexts and a falsifiable explanation. Aggregate percentage alone does not establish a verified fix or justify replacement. Preserve known-correct cases as explicit regression witnesses; investigate candidates with tradeoffs without calling them recovered rules. A proposed combination must use audio-derived inputs rather than target-based model switching.
- Keep key, window energy, overall energy, cue placement and cue labelling separate when their failure modes differ. Choose techniques per component; timing observations alone do not identify the server's model families.
- State inputs, targets, preprocessing, invariances and output space before fitting. Check existing counterexamples before tuning; do not revive refuted sufficiency claims, silently remove witnesses or add per-file exceptions. Label intentionally lossy baselines explicitly.
- Treat verified model-experiment outcomes as scoped research findings alongside measured MIK observations, source/data audits and exclusions. State what each result establishes; a failed configuration does not exclude its entire technique or identify server architecture. New retained-data analysis can add a fact without new synthetic audio. Use new controlled audio for specific falsifiable gaps.
- Preserve immutable fact/source/cache snapshots pinned by completed runs. Advance the current register and private provenance through explicit versions; never rewrite an earlier pinned artifact.
- Keep observations, hypotheses and modelling choices distinct. New contradictory evidence requires a versioned reproduction record and an explicit addition or supersession; preserve existing fact IDs and dated evidence. Update the tracked aggregate and private provenance together without publishing identifying material.
- Freeze track-lineage groups before fitting. Copies, formats, edits, transpositions, dither and stem descendants stay in one split. Newly captured or renamed known tracks are not unseen validation. Reserve overlap-checked new music before target inspection if it will serve as the final test.
- Report captured-feature prediction separately from end-to-end PCM results, with failures and unknowns in denominators. Use out-of-fold track-group predictions when training an overall learner on window-model outputs. Returned labels are oracle diagnostics, not deployable inference inputs.
- Rebuild base outputs and learned transforms within matched or nested group exclusions for meta-model inner selection; an existing row-wise OOF cache alone does not certify a new validation split. Resolve frozen capture versions by source identity and verify semantic request identity when original filenames have been reused.
- Private Mixed In Key cue-model research may use the captured cue observations and counterexamples. Preserve the validated product Lexicon cue path; research authorization does not promote a replacement. Default promotion still requires native Rust/ONNX parity, supported-format checks and a frozen independent evaluation.

## Commit & Pull Request Guidelines

The canonical remote is `https://github.com/johnneerdael/Cuerithm`. Preserve upstream changes and use descriptive commits. Do not publish music, private library paths, per-track reference labels or measurements, decompiled code or packaged proprietary artifacts. The one exception is the user guide, whose screenshots show the owner's own library as the app displays it (titles, artists, keys, BPMs, energy values and cues): the guide documents what the app shows. Those images must not show audio files, file paths, user names, credentials or licence details, and each one is opened and checked for them before it is kept. Record actual validation and remaining agreement gaps. Do not manually cut releases.

## Release Notes

- Use the supplied dark 1600x480 banner at `docs/assets/brand/release-banner.png` unchanged. Do not substitute the white GitHub banner.
- Follow `.github/scripts/release_notes.py` and `docs/development/pr-workflow.md`: banner and tagline, What's new, Downloads, then collapsed build/source details.
- Lead each public bullet with a short bold benefit and explain what the DJ can do in plain words. Group related fixes and retain setup requirements or unverified behavior.
- Link platform downloads to their published assets. Keep source hashes, internal changes and commit lists inside the collapsed section.
- When correcting published notes, preserve the release tag, application source commit and downloadable assets. Pin a replacement banner to its own commit if it was corrected after release.

## Bundle Integrity

Preserve the installed application, source music and CSV. Work on separate artifact copies. Never edit signing manifests, write music tags during benchmarks, or read/export license credentials. Keep identifying reports under `.analysis/`; public docs use aggregate findings.
