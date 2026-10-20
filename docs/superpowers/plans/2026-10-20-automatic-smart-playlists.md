# Automatic Smart Playlists Implementation Plan

> **For agentic workers:** Use superpowers:executing-plans inline, then one fresh whole-branch reviewer.

**Goal:** Create and maintain artist, BPM, month/energy and genre smartlists from Settings.
**Architecture:** Extend revisioned PlaylistPreferences; reconcile managed node identities from effective smart snapshots in the existing refresh service. Add normalized credit and release-month fields for readable exact smart rules.
**Tech Stack:** Rust, SQLite, React/TypeScript, Tauri 2.
**Spec:** docs/superpowers/specs/2026-10-20-automatic-smart-playlists.md

## Global Constraints
Preserve music, tags, manual nodes, existing preferences, cue DSP and user undo. Disabled by default. Up to ten inclusive ranges. No invented release month. No release/version changes.

## Review Focus
- Collaboration names with substrings and repeated/remixer credits must match whole individuals.
- Year-only, malformed, conflicting and month-only dates must preserve precision and source priority.
- New tracks must create missing folders without opening Settings, including after restart.
- Concurrent settings/library edits must not commit a stale generation snapshot.
- Disabling generation must retain existing lists and manual folder children.

### Task 1: Effective grouping metadata
- [x] Add failing tests for contributor parsing, genre membership and release-month source priority/precision.
- [x] Add credits module and smart snapshot fields artistCredit, genreCredit, releaseMonth; support exact credit matching in smart evaluator.
- [x] Run metadata/grouping tests and existing smart playlist tests.

### Task 2: Revisioned settings and automatic reconciliation
- [x] Add failing integration tests for all four dimensions, BPM boundaries/validation, idempotence, manual isolation, disabling, future month and restart/service expansion.
- [x] Extend preference models with serde defaults and add managed-key/stamp migration.
- [x] Implement Library::reconcile_automatic_playlists(now), guarded by input/preference/tree stamps; integrate save and refresh service. Use stable keys and preserve undo history.
- [x] Run backend suites and commit.

### Task 3: Smart Playlists settings UI
- [x] Add failing UI tests for toggles, ten-range cap, validation, persisted save and errors.
- [x] Add dedicated settings page using existing tokens and draft/save patterns, retaining Library preferences.
- [x] Run frontend suite/typecheck/build and inspect screenshot.

## Execution notes

- Task 1 complete: grouping fields preserve whole credits, effective native remixer/genre values, release-source priority and month precision. Unicode regression passes.
- Task 2 complete: ten automatic integration tests pass, including atomic rollback, future-month creation after service restart, default opt-in, harmonic aliases/wraparound, energy boundaries and manual isolation.
- Task 3 complete: all 300 frontend tests pass, production build and visual screenshot checked. Existing Library preferences drafts remain shared with Smart Playlists to avoid stale preference writes.
- Independent review: one effective-remixer mismatch identified; regression failed before correction and passed after native remixer overlay. GitHub review additionally requested maintaining enabled managed-list rules; a failing regression now verifies rule repair while preserving names and folders, and that disabled lists remain customizable. Follow-up Undo regression verifies that automatic definition repairs retain the last user edit identity and that Undo tolerates only managed-definition changes, preserving stacked Undo and rejecting newer user ABA edits. Smart playlist suite: 37 passed; playlist storage suite: 15 passed. Inactive BPM validation regression reproduced in UI and backend; disabling now ignores inactive bounds and omits incomplete drafts from the UI payload. No deferred minors. Reviewed organization decisions match the spec: preserve lists on disable/range removal, preserve user renames/moves, suppress deleted managed identities, and leave old manual releaseDate semantics intact.
- Clippy, release build, frontend suite/build and documentation build are required alongside the full workspace suite before merge; final evidence is recorded in the PR. Initial broad checks hit disk pressure; cleared only this worktree's disposable incremental cache and old object files and resumed with CARGO_INCREMENTAL=0.

### Task 4: Verify and merge
- [ ] Run workspace test/fmt/clippy/release commands and update docs with actual results.
- [ ] Request independent whole-branch review; resolve meaningful findings and verify final head.
- [ ] Commit, push and open PR; inspect required CI and merge when passing.
