# CamelotWheel engine guidelines

## Project Structure & Module Organization

This workspace contains an independent Rust audio-analysis engine. The user's current priority is the engine; defer the Tauri 2 UI until it is settled. Target macOS, Windows and Linux. The reference app is `/Applications/Mixed In Key 11.app`, outside this workspace.

- `crates/camelot-core/`: PCM decoding, notation, DSP and evaluation.
- `crates/camelot-cli/`: analysis and benchmark CLI.
- `docs/research/`: public aggregate findings and provenance.
- `.analysis/`: private binary copies, corpus labels and experiments; exclude from publication.

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

No existing Git history establishes commit conventions. Do not publish music, private library paths, per-track labels, decompiled code or packaged proprietary artifacts. Record actual validation and remaining agreement gaps. Do not manually cut releases.

## Bundle Integrity

Preserve the installed application, source music and CSV. Work on separate artifact copies. Never edit signing manifests, write music tags during benchmarks, or read/export license credentials. Keep identifying reports under `.analysis/`; public docs use aggregate findings.
