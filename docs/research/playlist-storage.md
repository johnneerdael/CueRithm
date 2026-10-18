# Playlist storage boundary

The library stores stable playlist/folder/smart IDs, parentage and sibling and
favorite order in schema6. Ordinary membership is unique and ordered. Incoming
and archived track IDs remain in manual membership to preserve intent; the
eligible track-query/evaluation layer controls visibility. Playlist operations
perform SQLite work only, without reading/writing audio or rerunning analysis.

Creation, rename, move, recursive duplication, removal/restoration, favorites,
member changes and duplicate-reference replacement use one revision-checked
transaction. A batch applies at most5000 edits, with bounded nesting. Partial
member reorder substitutes only the selected members' existing slots. Union,
intersection and generated-list previews are the next tools layer, not part of
this storage boundary.

Duplicate nodes receive new IDs and retain original smart references. A known
ID takes priority over a stale path/name. A removed/unknown ID stays unavailable;
path-only legacy references resolve only when the requested type and full path
are unambiguous. The store rejects folder and smart dependency cycles, including
smart lists reached through folder descendants. Syntactically valid unavailable
rules may remain stored; native catalog evaluation reports their availability.

Removal preserves rows and manual members, with the operation ID marking which
nodes were removed together. Restoring a folder does not resurrect a child
removed independently earlier. Favorite restoration inserts the restored slots
without using UUID tie order.

Undo history records before/after deltas for affected nodes and members. Each
node carries an internal last-edit identity, so changing a value away and back
does not make an older Undo safe. Undo checks both identity and content, rejects
newer child/reference dependencies, and refuses position conflicts. Restoring a
prior snapshot also restores its identity, allowing ordinary stacked Undo.
History and data changes commit together; injected SQL failure rolls back every
node, revision and history row.

Separate counters cover playlist edits, source smart inputs and future library
preferences. Source inputs include metadata, analysis, archive/readiness, native
metadata cache, categorized tags, cue revisions and protected-genre settings.
Job progress and smart materialization do not increment the source-input counter.
Changing unrelated preparation preferences does not increment it. Settings use
an UPDATE upsert so protected-genre changes trigger invalidation; SQLite REPLACE
would bypass that UPDATE trigger.

Schema6 also reserves normalized result/status, pinned preview and independent
library-preference tables for coherent evaluation and tools. Those tables alone
are not a usable smart-playlist workspace or proof of refresh/UI correctness.
Tests cover migration from schemas0–5 with original analysis/cue JSON preserved,
restart, atomic failures, stale edits, membership order and reference intent.
