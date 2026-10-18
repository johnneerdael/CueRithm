# Playlists and Smart Lists Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task by task. Preserve established native execution; use one fresh whole-branch reviewer at the end.

**Goal:** Deliver ordinary playlist organization/tools and all supported local Lexicon smart-list predicates without changing audio, analysis or cue history.

**Architecture:** A pure Rust smart-rule crate evaluates immutable track/catalog snapshots. The library owns transactional playlist trees, memberships, undo, dependency validation and coherent materialization. Tauri exposes typed commands to the existing persistent-player workspace.

**Tech Stack:** Rust 1.90, serde, rusqlite, chrono/chrono-tz, pinned adapted regress 0.12.0; Tauri 2, React/TypeScript, existing Dosis/charcoal/teal styles.

**Spec:** `docs/superpowers/specs/2026-10-18-playlists-workspace.md`.

## Global Constraints

- Target macOS arm64/x64, Windows and Linux. No external JS/Python runtime, separate model setup or XML transport.
- Preserve all key/BPM/grid/energy/cue calculations, source bytes and existing analysis/cue revisions. Playlist edits never trigger analysis/tagging.
- Retain raw source-helper outputs and label deliberate policy differences. Publish no extracted Lexicon code/assets or private music/library data.
- Do not add Plays, Last Played or Danceability. Unavailable services/fields remain explicit errors, not fabricated values.
- Smart combination is the actual flat Any/All plus OR-flag semantics. Validate dependencies including folder descendants.
- Current cleanup parent is PR3; rebase onto its reviewed delivered head before final integration. No concurrent Cargo operations against the shared target.

## Review Focus

1. A tree edit can introduce cycles indirectly through referenced descendant smart lists; Task3/4 tests cover graph mutations as well as rule saves.
2. Budget exhaustion inside negative regex/lookaround must never turn into a positive membership result; Task1/2 tests pin both polarities and independent concurrent meters.
3. Membership snapshots can race cleanup/preparation, clock boundaries or playlist renames; Task4 proves atomic revision checks, refresh and explicit stale status.
4. Reordering filtered/paged results must preserve hidden/unselected members and archive intent; Task3/5/6 tests prove full-match selection and stable slot substitution.
5. Moving/copying/deleting same-named nodes must not retarget a stable-ID reference or overwrite later edits during Undo; Task3/4 tests cover ambiguous fallback and concurrent operations.

---

### Task 1: Bounded ECMAScript regex adapter

**Files:** Create `crates/cuerithm-regex/{Cargo.toml,src/lib.rs,tests/compatibility.rs,tests/budget.rs}`, `vendor/regress/` from pinned registry archive with licenses/UPSTREAM/patch notes; modify workspace/lock/notices.

**Interfaces:** `Pattern::compile(source: &str) -> Result<Pattern>` parses the source-style pattern/flags and the 100-UTF-16-unit limit. `Pattern::is_match(&self, text: &str) -> Result<bool>` uses a 1 MiB input limit and 1,000,000 work units. Adapted regress exposes a bounded UTF-16/UCS-2 Boolean API with an explicit sticky/start-only switch and a distinct budget error; ordinary upstream APIs retain their behavior.

- [ ] Write compatibility and budget tests first: flags i/m/s/u/v/g/d/y, duplicate/unknown/u+v rejection, escaped slash, raw pattern, BMP/astral classes, Unicode properties/sets, captures/backreferences/lookbehind, sticky offset and long nonmatching sticky input, empty/invalid/101-unit patterns. Watch RED. Include `(a+)+$`, nested negative assertions, long repeat scans and independent simultaneous matches; budget exhaustion must be Err, never false/true.
- [ ] Vendor regress 0.12.0 archive SHA256 `32eef8b209c3c1c15dbad02c1f30f9539f00dc7253e0cbcdaae442a50a09d7c1`. Add per-execution fuel to instruction/backtrack/candidate/repeat work, shared through nested assertions, with early exhaustion and no global state. Keep original notices and a precise modification record. Use std/utf16/prohibit-unsafe features; no external runtime.
- [ ] Run `cargo test -p cuerithm-regex --locked` plus vendored upstream tests and Clippy; Expected: all pass, bounded adversarial inputs report the typed limit error. Commit.

### Task 2: Pure smart-list model and complete predicate engine

**Files:** Create `crates/cuerithm-smartlists/src/{lib,model,text,numeric,dates,tags,cues,membership}.rs`, tests/fixtures and `docs/research/smart-playlist-parity.md`; modify manifests/lock.

**Interfaces:** `SmartDefinition {match_all, rules: Vec<SmartRule>}`; each rule has stable ID, field ID, one of the 58 `Operator` variants, typed values and `or` flag. `SmartTrack` holds immutable real field values, tags/categories, cues, tempo markers, key and archive state. `EvaluationContext` supplies known catalogs, evaluated playlist membership, clock/time zone and key mixing mode. `SmartDefinition::validate_shape() -> Result<()>` checks structural limits/arity/types; `CompiledSmartList::new(definition, catalog) -> Result<Self>` checks availability and compiles once; `matches(track, context) -> Result<bool>` performs no I/O.

- [ ] Promote the 637 original synthetic helper cases from private research with provenance; expand to all 58 operators, Any/All/OR/empty lists, missing/zero/truthiness, reversed bounds, source decimal rounding, deleted IDs, loops/memory, all 24×24 keys in both modes, and local-day/calendar/DST cutoffs. Watch RED before implementation. Keep raw source expectations and explicit invalid-rule/non-loss policies separate.
- [ ] Implement typed field/operator validation and the exact source combination rules. Use Task1 for regex; invalid/budget/unavailable errors propagate before membership publication. Date context captures one now and IANA zone. No inferred contributor splitting, cue-type guessing or fake unavailable service metrics.
- [ ] Run pure crate debug/release tests, table-driven reference coverage and Clippy; Expected: every operator has passing positive/negative/empty coverage and no unaccounted parity rows. Commit.

### Task 3: Transactional tree, manual membership and Undo

**Files:** Add `crates/cuerithm-library/migrations/006_playlists.sql`, `src/playlists/{mod,model,tree,edits,history}.rs`, `tests/playlists.rs`; modify lib/store migrations.

**Interfaces:** `PlaylistNode {id,parent_id,kind,name,position,favorite_position,definition,deleted}` and `PlaylistState {revision,nodes}`. `Library::playlist_state()`, `apply_playlist_edit(edit: PlaylistEdit, expected_revision: u64) -> PlaylistChange`, `undo_playlist_edit(edit_id, expected_revision)`. Edits include create/rename/move/copy/remove/restore/favorite and ordinary add/remove/reorder members. State changes are SQLite-only and bump the library query revision too. Schema6 also creates generic pinned-preview, smart-status/member and independent library-preference tables consumed by Tasks4/5. A dedicated smart-input counter/transactional triggers cover relevant track/tag/cue data changes, excluding job progress and materialized outputs.

- [ ] Write RED tests for durable IDs/order/restart, nested folders, unique memberships, archive retention, no smart/manual confusion, recursive copy/reference intent, same-name ambiguity, tree/dependency cycles, unknown IDs, stale revisions, rollback/Undo preserving later changes and untouched file/analysis/cue identities.
- [ ] Implement schema6, structural validation from Task2, validated atomic edits, graph checks and durable before/after operation identities. Removed nodes/members remain recoverable. Stable-ID references outrank unambiguous path fallback; dangling references are explicit. Add `replace_playlist_track_refs(old_ids, keeper, expected_revision)` for later duplicate merging, preserving first-occurrence order.
- [ ] Run full library tests and migration inputs0..5; Expected: no source-file writes or new cue revisions and all failures preserve prior tree/membership. Commit.

### Task 4: Coherent smart evaluation, preferences and scoped queries

**Files:** Add `src/playlists/{snapshot,evaluate,queries,preferences}.rs`, `tests/smart_playlists.rs`; modify query/model/job invalidation as necessary.

**Interfaces:** `Library::smart_field_catalog()`, `preview_smart_list(definition, scope) -> SmartPreview`, `refresh_smart_lists(clock) -> SmartRefresh`, `playlist_members(id)`. Add `TrackQuery.playlist_id: Option<String>` and explicit playlist order. Library preferences persist key mixing mode/time zone separately from PreparationSettings, with `get/save_playlist_preferences` and their own revision.

- [ ] Write RED tests for actual native metadata/custom tags/measured scalar analysis/current cues, unknown fields/services, cycle paths through folders, coherent dependency ordering, race rejection after cleanup/preparation/tree edits, atomic result publication, previous-good retention with visible errors, relative-time expiry and file-unavailable states. Job progress and a result's own publication must not invalidate its input snapshot or cause endless refresh. Query cursors/full selection must bind playlist scope; changing library preferences must not invalidate preparation snapshots or enqueue analysis.
- [ ] Build immutable snapshots, resolve references, evaluate Task2 in dependency order, and commit only at matching library/playlist revisions. Track evaluation status/last-good membership. Relative rules refresh at least every minute; source or preference changes invalidate results. Preserve analysis/cue JSON and expose no invented memory/tempo facts.
- [ ] Run full library/engine regressions; Expected: dynamic memberships update correctly, no stale partial result or mis-scoped cursor/selection, and no extra DSP/tag publication. Commit.

### Task 5: Playlist tools and bulk generation

**Files:** Add `src/playlists/{tools,generator}.rs`, `tests/playlist_tools.rs` and generation fixtures.

**Interfaces:** `preview_playlist_tool(request) -> PlaylistToolPreview`; `apply_playlist_tool(preview_id, expected_revision) -> PlaylistChange`. `preview_smart_generation(request) -> GenerationPreview`; `apply_smart_generation(preview_id, expected_revision)`. Requests cover merge/union, cross-reference/intersection, sibling sort, number/text prefixes, visible-member order, smart snapshot and generation dimensions/combinations.

- [ ] Write RED tests for ordered union/intersection, source preservation, partial sibling/member slot substitution, prefix padding, recursive smart snapshot, observed-value buckets, empty dimensions, cartesian combinations, the 5,000-node cap, stale previews and transaction rollback halfway through creation. Hidden/archived members must survive reorder.
- [ ] Implement tools using Task3 edits and Task4 complete membership snapshots; validate before any atomic mutation. Generate only real supported values and preserve original dynamic definitions when snapshotting as ordinary.
- [ ] Run full library tests; Expected: exact deterministic member/name/order fixtures, all-or-nothing application and unchanged audio/analysis/cues. Commit.

### Task 6: Native playlist workspace and smart editor

**Files:** Extend typed bridge/native commands, App scope and Sidebar; add `views/{PlaylistEditor,PlaylistTools,PlaylistGenerator,PlaylistPreferences}.tsx` and focused UI tests; update styles/docs.

**Interfaces:** All Task3–5 operations have typed Tauri commands. Library-change events can include playlist IDs. Settings offers separately persisted library preferences; preparation setting revisions stay independent. Table queries and selection carry the active playlist ID and manual order.

- [ ] Write RED UI tests for tree/favorites/CRUD/Undo, equivalent accessible drag/drop actions, all operator groups/values and unavailable fields, source Any/All/OR, generation/tool previews, manual-vs-smart state, filtered order/full CmdCtrl+A, late A-vs-B requests, errors/restart, small-window scrolling and loaded-player preservation.
- [ ] Implement within the existing design system, preserving the player's audio session across scope changes. Clear selection on playlist changes, bind late results to full scope, show smart loading/error/last-good status and prevent manual edits of dynamic results. No ingestion/tagging wizard or XML controls.
- [ ] Run existing and new UI tests, TS/build/native package. Native acceptance uses isolated generated audio, verifies restart/order/dynamic refresh and unchanged source hashes, analysis/cue versions and playback identity. Expected: all pass. Commit.

### Task 7: Whole-branch review and desktop delivery

**Files:** CI/notices/development and parity docs as required.

- [ ] Rebase onto final cleanup/main; verify the complete local suite and actual package resources. Run one fresh whole-delta review; reproduce/fix Important findings in a single TDD pass, and ledger all rulings/minors.
- [ ] Push a separate Cuerithm PR, require native Windows/Linux/macOSarm64/macOSx64 checks and packaged diagnostics. Distinguish helper fixtures, native UI, live target DJ sync and unsupported services honestly.
- [ ] Preserve pending duplicate/unused/missing maintenance and direct-only rekordbox6/7 delivery; this plan does not by itself complete the active overall goal.
