---
title: Curate and preparation presets
type: feat
status: ready-for-review
date: 2026-10-21
parent: docs/plans/2026-10-20-002-feat-library-preparation-five-pr-delivery-plan.md
---

# Curate and preparation presets

This is implementation detail for PR4 of the approved five-PR program, following
the ready-for-review match/editor delivery in #16. It implements U9 and the
preset portion of U10 without activating new providers. The user manages PR
review and merge.

## Curate

Reuse the existing category/value IDs and track links. Existing groups begin as
multiple-select; add single-select groups without silently discarding conflicting
assignments. Rename preserves identity. Retain aliases/tombstones for frozen
cleanup evidence, and handle deleted smart-rule references explicitly rather
than removing predicates and broadening a playlist.

Categories, user rating and notes start library-only. Opening Curate does not
need readable audio. Keep user rating and notes distinct from native tag values,
and measured key/BPM/energy read-only. Optional file mappings identify proved
text, rating-owner or comment targets and reject collisions. Unproved native DJ
category destinations remain unavailable. Use PR3's writer and recovery evidence
for mapped publication.

Persist a revision-checked, idempotent local outbox before navigation. Coalesce
pending publication while preserving an immutable in-flight revision. Distinguish
desired, locally saved and published revisions; an older completion never marks
a newer edit Saved. Debounce publication rather than relying on a hidden Save
gesture. Pending/history use lightweight pages and exact detail. Retry/Discard
addresses unpublished intent; guarded Undo reverses published work.

Use the existing inspector. Freeze the complete native ordered scope and retain
the edited stable track ID even if a category filter removes it from the live
table. Next/Previous changes selection independently of playback. Navigation and
Quit wait for the latest local receipt, with the current focus, compact-window
and both-theme contracts.

## Presets

Use typed portable rules for cues, tagging/matching/writing, cleanup and
organization recipes, plus separate local destination connections. Keep watched
folders, ingestion controls, model paths/backends, solver settings, detected
restrictions and credentials outside portable rules. Never export the complete
legacy tagging configuration: it includes arbitrary custom values and secret
slots.

Create immutable preset revisions with compare-and-set CRUD. Import a bounded,
versioned allowlist as a new inactive preset; reject unknown fields/versions and
require destination reconnection for Move/Copy. Load rules into the existing
Settings draft without replacing unrelated unsaved global settings. Commit
folder assignment drafts atomically with Save settings.

Resolve each newly admitted source against the deepest verified assigned folder.
Inherit removes a local rule; explicit Global default stops parent inheritance.
Do not use display-string prefixes or silently fall back when a folder/preset is
unavailable. Preserve low-level captured-revision enqueue and recovery semantics.
Materialize effective snapshots without changing selected global settings;
claimed workers never resolve presets again. Keep failed discovery admissions
retryable after reconnection without touching the audio file.

## Implementation and verification

- [x] Native preset rules, immutable capture and source routing;
  `021_preparation_presets.sql`.
- [x] Stable taxonomy, local scalars, durable outbox and shared mapped
  publication; `022_curation.sql`.
- [x] Curate/filter/navigation and Settings preset/folder controls; native
  command and event wiring.
- [x] Preservation, recovery, accessibility, documentation and both-theme
  browser/controller acceptance. Packaged native-window acceptance remains separate.

Current validation: 1,463 Rust tests passed with seven ignored; workspace format,
strict Clippy and release build passed. Desktop tests passed 419 cases in 47
files, with production build and strict documentation checks. No-autotag
storage/Clippy and desktop custom-protocol checks also passed. No personal
provider keys were read or provider API requests made for this delivery.

Tests cover mixed single/multiple operations; rename/delete with captured
cleanup and smart rules; offline local edits; lost responses, older receipts and
final-track close/restart; Retry/Discard versus Undo; complete filtered scopes
and stale cursors; mapping collisions and native multivalue/owner preservation;
Incoming Move/Ready/Undo with unchanged PCM/DSP/cues; preset precedence,
aliases/replaced/offline folders, per-source captures, immutable claimed jobs,
current global solver inheritance, secret/path-free export, reconnection and
no-tagging builds. Run the required workspace format, Clippy, tests and release
build, plus desktop tests/build and strict documentation checks after integration.
