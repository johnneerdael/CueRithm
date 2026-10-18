# Cuerithm playlists and smart lists

This delivery implements the approved ordinary/full smart-playlist scope from
`2026-10-18-cuerithm-library-workspace-design.md`. The intended outcome is a
useful local DJ library: organize prepared music into ordered folders/playlists,
build dynamic selections from actual metadata/analysis/cues, and preserve their
intent for later direct rekordbox sync and duplicate merging. Keep Tauri 2,
React/TypeScript and Rust across macOS arm64/x64, Windows and Linux.

## Ordinary playlists

Persist stable node IDs, folder parentage, sibling order, favorite order, manual
track membership and track order. Support creation, rename, move/reorder,
recursive duplication, deletion/restore, adding/removing tracks and internal
member order. Membership is unique by track ID. Archive preserves ordinary
membership and order; the default track view hides archived records until the
user asks for them. Incoming is excluded until preparation succeeds.

Expose the requested tools: merge/union, cross-reference/intersection, folder
sorting, number/text prefixes and rewriting member order from the current sort.
Source lists survive creation of a merged/cross-referenced ordinary list.
Partial sorting/reordering preserves unselected or filtered-out members in their
positions. Reject unknown IDs and nonmember substitutions rather than silently
dropping membership. Manual edits cannot masquerade as smart-rule results.

Every tree/membership operation is transactional and revision-checked. Keep a
recoverable operation record with before/after identity. Undo refuses to replace
newer edits. Playlist removal never deletes source audio. Referenced deletion
leaves an explicit unavailable dependency rather than retargeting another
same-named list. Stable IDs take priority over path fallback; fallback must be
unambiguous. Duplicate copies get new IDs and preserve their rule references;
Snapshot as ordinary evaluates and freezes current membership separately.

## Smart-rule contract

Inventory all 58 source operators in `docs/research/lexicon-smart-playlists.md`.
Implement text/regex, numeric/date/color/boolean/existence, tags/categories,
tempo-marker/exact-BPM, playlist/folder/smart membership, cue/loop/memory and key
similarity predicates. Fields use actual persisted values and measured analysis;
never source labels as predictions. Do not add Plays, Last Played or Danceability.
Cloud/streaming predicates and absent service metrics are unavailable, not
invented false/zero values. Other real file metadata remains selectable through
a typed field catalog.

Use the source's flat Any/All rules with per-rule OR flags. Any needs one true
rule. All needs a nonempty list and all rules, except that two or more OR-marked
rules form one alternative group alongside every unmarked rule. A single OR
mark does not change All. Empty lists match nothing. Archived records require
an explicit Archived=true rule. Do not replace this with arbitrary Boolean
nesting and call it source parity.

Pin source semantics for whitespace/case, missing/zero values, reversed bounds,
BPM rounding and exactness, tag/category existence, loops/memory cues and all
24-by-24 key combinations in Harmonic and Fuzzy modes. Source-helper fixtures
keep raw outputs and separate any intentional policy expectation. Invalid or
unavailable rules must be visible; a failed evaluation cannot silently publish
partial membership or synchronize stale results.

## Regex and time

Regex matching runs in Rust without an external JavaScript runtime. Reuse pinned
MIT/Apache regress 0.12.0 with a documented minimal bounded-execution adaptation;
retain its licenses and archive SHA. Preserve valid ECMAScript flags, UTF-16
length/character behavior, lookarounds, backreferences and Unicode sets. The
source pattern limit is 100 UTF-16 units. Validate flags, including duplicates
and incompatible u/v; g/d do not change a new regex's first Boolean test and y
requires a match at offset zero. Budget exhaustion is an explicit evaluation
error, including inside negative lookarounds, never a successful negation.

Bound each predicate to 1,000,000 execution work units and a 1 MiB field input.
The budget covers candidate searches, instructions, backtracks, repeated
character scans and nested assertions. No global mutable budget or leaked
worker is acceptable. Test catastrophic patterns and independent concurrent
matches before using the adaptation. Invalid syntax/oversize input is reported
at validation instead of silently matching a malformed rule; record this
intentional difference from source's caught-regex no-match behavior.

Dates use one captured evaluation clock and an explicit IANA time zone, defaulting
to the system zone. Store milliseconds. Equality is by local day, After includes
the chosen day, Before excludes it, and Between includes reversed endpoints.
Recent uses calendar subtraction and a strict cutoff, with source missing-value
and zero-duration cases pinned by tests. Include month-end/leap/DST fixtures.
Relative rules refresh at least every minute and when the clock/zone changes.

## Storage and evaluation

The pure smart-list crate accepts an immutable track/catalog/membership snapshot
and clock; it performs no filesystem/database/audio work. The library supplies
actual metadata, categorized tags, measured key/BPM/energy, current cue/tempo
state and file availability. Preserve complete analysis/cue revisions. Imported
DJ states can later provide native memory/loop/variable-tempo facts through the
same typed input; do not infer them from cue names or fabricate extra markers.

Persist definitions separately from evaluated membership. Validate the complete
dependency graph, including descendant-folder dependencies, and reject cycles.
Use a dedicated smart-input revision that ignores job-progress and output-only
materialization changes, alongside the playlist-edit/preference revisions.
Evaluate dependencies in order from that coherent snapshot; publish a
complete result only if its source and playlist revisions still match. Report
stale/unavailable status while retaining the last good result. Refresh on
preparation/metadata/cleanup/playlist edits and relative time. File availability
errors are distinguishable from confirmed absence.

Provide stable query pagination and full-match selection scoped to playlist ID,
library revision, search and sort. A late request from playlist A cannot replace
playlist B or select A's tracks in B. Manual order is an explicit sort option.
Key mixing mode and time zone belong in persistent Settings.

## Generation and interface

Support bulk smart-list generation by observed genre/artist/tag/category, key,
energy/rating, year/decade, BPM ranges and bitrate/size/duration buckets, plus
cartesian combinations. Preview generated names/rules/counts before atomically
creating at most 5,000 nodes. Reject name/ID/reference conflicts without a
half-created tree. Excluded metrics do not reappear as generator options.

Extend the existing charcoal/teal sidebar with scrollable folder trees and
favorites, accessible create/edit/context tools and drag/drop backed by equivalent
buttons/keyboard actions. Keep the table, art, full waveform/cues and player
persistent. A smart editor exposes typed fields/operators, source Any/All/OR
semantics, complete option values and preview/error states. Settings hold global
preparation choices; opening a playlist never starts tagging or reanalysis.

The later direct adapter decides which smart definitions rekordbox represents
natively. Preserve original Cuerithm definitions and expose native/static status
there; this delivery does not add XML or claim target round trips. Duplicate
merging will reuse the transactional membership-replacement API from this store.

## Acceptance

Expand the 637 initial private synthetic source-helper cases to cover every
operator and combination boundary. Publish only original synthetic inputs and
scalar outputs/provenance, never extracted JS/assets or private music data.
Test tree cycles, references, transactional tools/Undo, archived membership,
restart, stale snapshots/requests, automatic refresh and relative clock changes.
Prove analysis/cue/file bytes unchanged by playlist operations. Verify native UI
with an isolated generated-audio library and package/test all four desktop
targets. One whole-branch fresh review and a single TDD fix pass precede merge.
