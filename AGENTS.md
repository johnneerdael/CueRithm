# Cuerithm engine guidelines

## Project Structure & Module Organization

This workspace contains an independent Rust audio-analysis engine and is expanding into the Cuerithm desktop library application. Use Tauri 2 with React/TypeScript and a Rust backend; target macOS, Windows and Linux. The user approved a library sidebar, track table and persistent waveform/cue panel with OneTagger's charcoal/teal visual direction. Incoming automatically applies configured tagging/cleanup, full analysis and replacement cues before moving completed tracks into Tracks. Include Beatport/Traxsource auto-tagging, Lexicon-style cleanup and playlists, and direct-only rekordbox 6/7 sync; do not add XML transport. See the current design under `docs/superpowers/specs/` for delivery boundaries and review status. Cue generation uses the validated Lexicon-compatible DSP in `lexicon_cues.rs`; preserve its calculations. Keep key analysis, BPM and a single whole-track energy rating. Do not restart Mixed In Key cue inference, attach energy labels to structural cues, or expose per-segment energy in normal analysis.

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

## Commit & Pull Request Guidelines

The canonical remote is `https://github.com/johnneerdael/Cuerithm`. Preserve upstream changes and use descriptive commits. Do not publish music, private library paths, per-track labels, decompiled code or packaged proprietary artifacts. Record actual validation and remaining agreement gaps. Do not manually cut releases.

## Bundle Integrity

Preserve the installed application, source music and CSV. Work on separate artifact copies. Never edit signing manifests, write music tags during benchmarks, or read/export license credentials. Keep identifying reports under `.analysis/`; public docs use aggregate findings.
