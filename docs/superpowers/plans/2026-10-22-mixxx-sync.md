# Mixxx native library sync Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add Mixxx (macOS, Windows, Linux) as a CueRithm sync destination that writes tracks, grids, keys, cues and playlists/crates straight into `mixxxdb.sqlite`, in *Manage library* or *Add only* mode.

**Architecture:** A new `mixxx` module in `cuerithm-dj-sync` mutates a staged SQLite copy (the djay Pro pattern), returning an ownership report; `prepare_sync` queues the replacement through the existing journal, backup, restore and process-guard machinery. Library service, Tauri and the React Sync view get a Mixxx entry and options.

**Tech Stack:** Rust 2024 (`rusqlite`, `lofty`, `serde`, `sha2`), Tauri 2, React/TypeScript, Vitest.

**Spec:** `docs/superpowers/specs/2026-10-22-mixxx-sync-design.md`; contract in `docs/research/mixxx-sync-contract.md`.

## Global Constraints

- Supported Mixxx schema: **39 and newer** (checked against 39 and 40); read from `settings` row `mixxx.schema.version`; a schema above 40 is accepted only when `mixxx.schema.min_compatible_version` is 40 or lower and the tables keep the shape the writer needs; older and incompatible libraries are refused with a message naming the version.
- Cue and loop `position`/`length` are REAL **frames × 2**; hot cue slots `0..=35`; types `1` hot cue and `4` loop only are written or replaced.
- New `library` rows: `header_parsed = 1`, `source_synchronized_ms` NULL, `mixxx_deleted = 0`; existing rows never get `source_synchronized_ms` changed.
- `beats_version` is `BeatGrid-2.0` or `BeatMap-1.0`; `keys_version` is `KeyMap-1.0`; frames are at the file's own sample rate.
- Key text uses the ID3v2 spelling; `key_id` is the `ChromaticKey` number (major 1–12, minor 13–24).
- Mixxx timing offsets (samples), from file-header facts and not from `Mp3TimingCase`: MP3 without an Info/Xing frame `−F` (`F` = 1152 at ≥ 32 kHz, else 576); Info/Xing frame without a `LAME`/`Lavf`/`Lavc` identifier `0`; with such an identifier `+(encoder delay + 529)`; lossless/PCM/Ogg/Opus 0; AAC (`.m4a`/`.mp4`/`.aac`) and WMA 0 with a warning.
- Cue colours are written as exact RGB; `nearest_color` is ignored for Mixxx.
- Native environment only; Wine/Proton environments are rejected.
- Never write music tags, edit a `directories` list that has a row (an empty one is seeded so Mixxx does not prompt at every start), delete Mixxx tracks, or add markers/columns to the Mixxx database. Hidden playlists (`hidden != 0`) and locked containers are never modified.
- Authored fixture DDL must be written independently; no upstream schema text is copied. Fixtures, tests and docs contain no private paths or music.
- Run from the worktree root. Commit messages end with `Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>`.

## Review Focus

1. Mixxx running, or a crashed session leaving `mixxxdb.sqlite-journal`: the sync must stop with a readable reason (Task 5, Task 11).
2. One file spelled two ways (case on macOS/Windows, `C:\` vs `C:/`, UNC, verbatim prefix) must update one row, never insert a duplicate; an ambiguous match is an error (Task 6).
3. A CueRithm track with no key, no cues or no grid must never blank Mixxx's value in *Manage library* (Tasks 6, 7).
4. A second identical sync must publish nothing, and an edited or missing owned playlist must be reset (Manage), appended to (Add only) or kept (removal), never error out (Tasks 8, 11).
5. A track the user removed from Mixxx or whose file Mixxx flagged missing returns to a usable state when it is back in scope (Task 6).

---

### Task 1: Model, options and discovery

**Files:**
- Modify: `crates/cuerithm-dj-sync/src/model.rs`, `crates/cuerithm-dj-sync/src/discovery.rs`, `crates/cuerithm-dj-sync/src/lib.rs`
- Create: `crates/cuerithm-dj-sync/src/mixxx.rs` (module root; later tasks add submodules)
- Modify to compile (`DjSoftware::Mixxx` arms that bail with "Mixxx is not wired yet" until their task): `sync.rs`, `smart_rules.rs`, `path_repair.rs`, `timing.rs`, and any other exhaustive match the compiler reports in `cuerithm-library` and `apps/desktop/src-tauri`.

**Interfaces:**
- Produces: `DjSoftware::Mixxx` (serde `"mixxx"`); in `model.rs`
  `pub enum MixxxExistingTracks { #[default] Keep, Replace }`,
  `pub enum MixxxContainer { #[default] Playlists, Crates }` (both `snake_case`),
  `pub struct MixxxOptions { existing_tracks, container, replace_metadata: bool, replace_key: bool, replace_cues: bool }` with `Default` = Keep, Playlists, true, true, true, `#[serde(default)]`, and `fn is_default(&self) -> bool`;
  `SyncOptions.mixxx: MixxxOptions` with `#[serde(default, skip_serializing_if = "MixxxOptions::is_default")]`.
  In `mixxx.rs`: `pub fn database_candidates(home: &Path, local_app_data: Option<&Path>) -> Vec<PathBuf>` returning, in order, `~/Library/Application Support/Mixxx/mixxxdb.sqlite`, `~/.mixxx/mixxxdb.sqlite`, `~/.var/app/org.mixxx.Mixxx/.mixxx/mixxxdb.sqlite`, `<local_app_data or ~/AppData/Local>/Mixxx/mixxxdb.sqlite`; `pub fn resolve_database_path(path: &Path) -> Result<PathBuf>` (directory → `mixxxdb.sqlite` inside it; a file must be named `mixxxdb.sqlite` and exist).
  `discovery.rs`: `discover` adds existing Mixxx candidates as `DiscoveredTarget { software: Mixxx, path }`; `process_matches(Mixxx, name)` true for `mixxx` (case-insensitive, `.exe` stripped); Linux `/proc` scan in `discovery/linux_process.rs` treats executable name `mixxx` as running.

- [ ] **Step 1: Failing tests** in `model.rs` and `discovery.rs`: `mixxx_options_default_is_omitted_from_serialization`; `old_options_json_deserializes_with_default_mixxx_block`; `mixxx_options_round_trip_non_default`; `mixxx_candidates_cover_every_platform_layout`; `mixxx_discovery_lists_only_existing_files_and_dedups`; `process_matches_accepts_mixxx_spellings` (`Mixxx`, `/Applications/Mixxx.app/Contents/MacOS/Mixxx`, `mixxx.exe`, `MIXXX.EXE`) and rejects `mixxx-helper`; `resolve_database_path_accepts_folder_or_file_and_rejects_other_names`.
- [ ] **Step 2:** Run `cargo test -p cuerithm-dj-sync mixxx -- --nocapture`; expect FAIL (types missing).
- [ ] **Step 3:** Implement the types, `mixxx.rs` helpers and discovery arms; add the temporary bailing arms so `cargo build --workspace` passes.
- [ ] **Step 4:** Run `cargo test -p cuerithm-dj-sync` and `cargo build --workspace`; expect PASS.
- [ ] **Step 5:** `git add -A && git commit -m "feat(mixxx): target model, options and discovery"`.

### Task 2: Protobuf encoders

**Files:** Create `crates/cuerithm-dj-sync/src/mixxx/proto.rs`; modify `mixxx.rs` (`mod proto;`).

**Interfaces:**
- Produces (all `pub(super)`): `fn beat_grid(bpm: f64, first_beat_frame: i32) -> Vec<u8>`; `fn beat_map(frames: &[i32]) -> Vec<u8>`; `fn key_map(key_id: u8, id3_text: &str, source: u8) -> Vec<u8>`; test-only decoders `decode_beat_grid(&[u8]) -> Result<(f64, i32)>`, `decode_beat_map`, `decode_key_map -> (u8, String, u8)`.
- Encoding: `BeatGrid`: field 1 `{ field 1 double }`, field 2 `{ field 1 int32 varint }`; `BeatMap`: repeated field 1 `{ field 1 int32 }`; `KeyMap`: field 1 enum, field 2 string, field 4 enum. Negative `int32` encodes as a 10-byte sign-extended varint.

- [ ] **Step 1: Failing tests:** golden bytes written out by hand in the test: `beat_grid(128.0, 44100)` = `0a 09 09 <128.0 as f64 LE> 12 04 08 c4 d8 02`; `beat_map(&[0, 22050, 44100])` as three `0a <len> 08 <varint>` entries; `key_map(22, "Am", 2)` = `08 16 12 02 41 6d 20 02`; round trips through the test decoders including a negative frame and 1-byte, 3-byte and 5-byte varints.
- [ ] **Step 2:** Run `cargo test -p cuerithm-dj-sync mixxx::proto`; expect FAIL.
- [ ] **Step 3:** Hand-encode (no protobuf dependency).
- [ ] **Step 4:** Run the tests; PASS. Then validate the goldens once against Mixxx's own definitions: write the golden bytes to a scratch file and run `protoc --decode=mixxx.track.io.BeatGrid` (and `BeatMap`, `mixxx.track.io.key.KeyMap`) with `-I <mixxx>/src/proto <mixxx>/src/proto/beats.proto` / `keys.proto`; record the decoded output in the task commit message.
- [ ] **Step 5:** Commit `feat(mixxx): beat and key blob encoders`.

### Task 3: Beat plan, key and colour mapping

**Files:** Create `mixxx/beats.rs`, `mixxx/keys.rs`, `mixxx/colors.rs`; modify `mixxx.rs`.

**Interfaces:**
- Consumes: `proto::{beat_grid, beat_map, key_map}`, `crate::formatting::parse_key`, `SyncTrack`.
- Produces: `pub(super) struct BeatPlan { pub version: &'static str, pub blob: Vec<u8>, pub bpm: f64 }`; `fn plan_beats(track: &SyncTrack) -> Result<Option<BeatPlan>>` (empty `beatgrid` → `None`; one anchor, or equal-BPM anchors lying on the first anchor's grid within 0.5 ms → `BeatGrid-2.0` at the first anchor; otherwise a dense `BeatMap-1.0` that contains every anchor exactly, at least two beats, strictly increasing frames; `bpm` = beats-per-minute over first to last beat for a map; frames = `round(seconds × track.sample_rate)`; non-finite or non-positive BPM → error).
  `fn mixxx_key(text: &str) -> Result<Option<MixxxKey>>` with `struct MixxxKey { id: u8, text: &'static str }` (empty text → `None`; unparsable → error naming the value).
  `pub(super) const HOTCUE_PALETTE: [u32; 8]` = `C50A08, 32BE44, 42D4F4, F8D200, 0044FF, AF00CC, FCA6D7, F2F2FF`; `fn slot_color(index: u8) -> u32` (`HOTCUE_PALETTE[index % 8]`).

- [ ] **Step 1: Failing tests:** `constant_grid_uses_beatgrid_2_at_first_anchor_in_source_frames` (48 kHz track, anchor 0.5 s → frame 24000); `repeated_equal_bpm_anchors_on_grid_stay_constant`; `tempo_change_builds_beatmap_with_every_anchor`; `phase_jump_at_equal_bpm_builds_beatmap`; `empty_grid_is_none`; `invalid_bpm_is_rejected`; `every_camelot_open_key_and_musical_spelling_maps_to_the_same_chromatic_id` (24 keys × `8A`, `1m`, `Am`, `A minor`); `key_text_uses_id3v2_spelling` (`F#`, `Db`, `Ebm`, `G#m`, `Bbm`); `empty_key_is_none`; `bad_key_is_error`; `slot_color_wraps_the_eight_color_palette`.
- [ ] **Step 2:** Run `cargo test -p cuerithm-dj-sync mixxx::` ; FAIL.
- [ ] **Step 3:** Implement. Key id: major `tonic + 1`, minor `tonic + 13` (tonic 0 = C).
- [ ] **Step 4:** Run; PASS. **Step 5:** Commit `feat(mixxx): beat plans, key and colour mapping`.

### Task 4: Cue rows

**Files:** Create `mixxx/cues.rs`; modify `mixxx.rs`.

**Interfaces:**
- Consumes: `colors::slot_color`, `SyncTrack`.
- Produces: `pub(super) struct CueRow { pub kind: i64, pub position: f64, pub length: f64, pub hotcue: i64, pub label: String, pub color: i64 }`; `fn plan_cues(track: &SyncTrack, warnings: &mut Vec<String>) -> Vec<CueRow>`; kind 1 for a cue without `end_seconds`, kind 4 with `end_seconds`; `position = round(start × track.sample_rate) × 2`, `length = round((end − start) × sample_rate) × 2` computed on whole frames; `hotcue = cue.index`; label = name; color = exact RGB, or `slot_color(index)` when absent or `0`.
- A start before zero (possible after the MP3 no-Info offset) is moved to frame 0 and counted in one summary warning for the track. Skips with a warning naming the track and slot: index > 35; duplicate index (first wins); a loop whose end is at or before zero or shorter than one frame; non-finite times.

- [ ] **Step 1: Failing tests:** `hot_cue_position_is_frames_times_two`; `loop_has_length_and_hotcue_slot`; `missing_or_black_color_takes_slot_palette`; `exact_rgb_is_kept_even_with_nearest_color_set`; `slot_36_is_skipped_with_warning`; `duplicate_slot_keeps_first`; `zero_length_loop_is_skipped`; `start_before_zero_is_moved_to_the_first_frame_with_one_warning`; `loop_ending_before_zero_is_skipped`.
- [ ] **Step 2:** FAIL. **Step 3:** Implement. **Step 4:** PASS. **Step 5:** Commit `feat(mixxx): cue row planning`.

### Task 5: Schema layer and authored fixtures

**Files:** Create `mixxx/schema.rs`, `mixxx/fixture.rs` (`#[cfg(test)] pub(crate)`); modify `mixxx.rs`.

**Interfaces:**
- Produces: `pub(super) struct Layout { pub version: u32, pub library: BTreeSet<String>, pub crate_columns: BTreeSet<String>, pub playlist_columns: BTreeSet<String> }`; `fn inspect(connection: &Connection) -> Result<Layout>` — errors, each with its own message: missing `settings` or version row; version outside 39..=40 (message contains the version and `39`–`40`); any required table or column missing (name it); `mixxxdb.sqlite-journal` beside a file-backed database (hot journal: "Mixxx did not close cleanly; open and quit Mixxx once"). `Layout::has(&self, column: &str) -> bool`.
  Fixture: `pub(crate) fn create(path: &Path, version: u32) -> Connection` building independently authored DDL for `settings`, `track_locations`, `library`, `cues`, `Playlists`, `PlaylistTracks`, `crates`, `crate_tracks`, `directories`, `LibraryHashes` with schema-40's extra `tuning_frequency_hz` only when `version == 40`; plus `pub(crate) fn add_track(connection, location: &str, title: &str) -> i64` returning the library id of a Mixxx-shaped existing row (header_parsed 1, beats/keys empty).

- [ ] **Step 1: Failing tests:** `accepts_schema_39_and_40`; `rejects_38_and_41_with_window_in_message`; `rejects_missing_version_row`; `rejects_missing_required_column_by_name`; `rejects_hot_journal`; `schema_40_column_is_optional_for_39`.
- [ ] **Step 2:** FAIL. **Step 3:** Implement. **Step 4:** PASS. **Step 5:** Commit `feat(mixxx): schema inspection and authored fixtures`.

### Task 6: Track identity and writing

**Files:** Create `mixxx/tracks.rs`; modify `mixxx.rs`.

**Interfaces:**
- Consumes: `schema::Layout`, `beats::plan_beats`, `keys::mixxx_key`, `proto`, `options.mixxx`, `SyncLibrary`.
- Produces: `pub(super) fn location_key(path: &Path) -> Result<String>` (strip `\\?\` and `\\?\UNC\` → `//server/share`, backslashes → `/`, error for non-Unicode); `pub(super) struct TrackOutcome { pub library_ids: BTreeMap<String, i64>, pub added: usize, pub replaced: usize, pub kept: usize, pub restored: usize }`; `pub(super) fn write_tracks(tx: &Transaction<'_>, layout: &Layout, library: &SyncLibrary, options: &SyncOptions, warnings: &mut Vec<String>) -> Result<TrackOutcome>`.
- Match: exact `track_locations.location`; on macOS and Windows additionally case-folded (`to_lowercase`), error "Mixxx library holds N rows for this file" when more than one distinct row matches. Source id → library id for every track (needed by playlists).
- Existing row, `Keep`: no column written except `mixxx_deleted = 0` and `track_locations.fs_deleted = 0` (both counted as restored); this also holds in `Replace`. Existing row, `Replace`: write only groups enabled by `replace_metadata` (title, artist, album, genre, comment, year, rating, color), `replace_key` (`key`, `key_id`, `keys*`; skipped when the track key is empty), grid (`bpm`, `beats*`; skipped by `skip_beatgrids` or empty grid), `bpm_lock` per `beatgrid_lock` (Lock → 1, Unlock → 0, Default → unchanged). Cue replacement is Task 7.
- New row: insert `track_locations(location, directory, filename, filesize, fs_deleted 0, needs_verification 0)` and `library` with only columns present in `layout`; audio facts `channels`, `bitrate` read with `lofty` read-only (a probe failure, for example WMA, falls back to 2 and 0 with a warning), `samplerate` and `duration` from the track, `filetype` = lowercase extension with `aif`→`aiff`, `cuepoint -1.0`, `replaygain 0.0`, `replaygain_peak -1.0`, `header_parsed 1`, `bpm_lock` as above (Default → 0), `datetime_added` left to its default. Empty key/grid leave blob and version columns empty.

- [ ] **Step 1: Failing tests** (each builds a fixture with `fixture::create` for versions 39 and 40 where noted): `location_key_normalizes_windows_and_unc_spellings`; `exact_match_updates_the_existing_row_only`; `case_insensitive_match_on_macos_and_windows_but_exact_on_linux` (`cfg`-split with a `case_insensitive: bool` parameter on an inner function so all three run everywhere); `ambiguous_case_match_is_error`; `keep_mode_leaves_existing_tracks_byte_for_byte_but_restores_deleted_and_missing_flags`; `replace_mode_updates_selected_groups_only` (toggle each of the three flags and `skip_beatgrids`); `empty_key_and_empty_grid_never_clear_existing_values`; `lock_option_sets_bpm_lock`; `new_row_has_header_parsed_and_null_source_sync`; `new_row_on_schema_39_omits_tuning_column`; `new_row_filetype_normalizes_aif`; `added_track_blobs_decode_through_test_decoders`.
- [ ] **Step 2:** FAIL. **Step 3:** Implement with prepared statements built from `layout` columns. **Step 4:** PASS. **Step 5:** Commit `feat(mixxx): track identity, insert and replace`.

### Task 7: Cue writing

**Files:** Modify `mixxx/tracks.rs` (or new `mixxx/cue_write.rs`); tests alongside.

**Interfaces:**
- Consumes: `cues::plan_cues`, `TrackOutcome`.
- Produces: `pub(super) fn write_cues(tx: &Transaction<'_>, track_id: i64, rows: &[CueRow]) -> Result<usize>` deleting `cues` rows of type 1 and 4 for the track and inserting `rows`; called by `write_tracks` for new rows, and for existing rows only in `Replace` with `replace_cues` and a non-empty plan (replaced count added to `TrackOutcome.replaced_cues`).

- [ ] **Step 1: Failing tests:** `replace_swaps_hot_cues_and_loops_but_keeps_main_intro_and_outro_rows`; `no_cuerithm_cues_leaves_existing_cues`; `keep_mode_never_touches_cues_of_existing_tracks`; `new_track_gets_cues`; `written_rows_pass_the_three_loader_discard_rules` (no zero-length loop, no black hot cue at 0, unique slots).
- [ ] **Step 2–4:** FAIL → implement → PASS. **Step 5:** Commit `feat(mixxx): hot cue and loop writing`.

### Task 8: Playlists, crates and ownership

**Files:** Create `mixxx/owned.rs`; modify `mixxx.rs`.

**Interfaces:**
- Consumes: `TrackOutcome.library_ids`, `ownership::{PlaylistBinding, PlaylistOwnership, PlaylistOwnershipReport, PlaylistOwnershipRequest, fingerprint}`, `options.mixxx.container`, `existing_tracks`.
- Produces: `pub(super) const IDENTITY: &str = "mixxx:mixxxdb.sqlite"`; `pub(super) fn write_containers(tx: &Transaction<'_>, layout: &Layout, library: &SyncLibrary, options: &SyncOptions, request: &PlaylistOwnershipRequest, library_ids: &BTreeMap<String, i64>, warnings: &mut Vec<String>) -> Result<PlaylistOwnershipReport>`.
- Locator `playlist:<id>` or `crate:<id>`. Fingerprint = `fingerprint(&(kind, name, member ids))` (playlist: ordered; crate: sorted). Names: folders flatten to `Parent / Child`; folders emit nothing themselves; duplicate flattened names among active lists take ` (2)`, ` (3)` in library order. Smart definitions are ignored, `track_ids` are the members. A track id missing from `library_ids` is an error naming the playlist.
- Rules: bound and present → if fingerprint unchanged, exact sync (rename, members, order); if changed in Mixxx → `Replace`: reset and warn, `Keep`: append missing only and warn. Bound but absent → recreate (`created = true`). Bound with the other kind (container switched) → keep the old object unbound, warn, create the new kind (the binding is overwritten, not added to `retired`). Unbound → adopt the single same-name non-hidden object (`created = false`; `Replace` sets members, `Keep` appends missing); with two or more same-name playlists adopt none, create a new one and warn; else create (`created = true`). Binding locators must match `^(playlist|crate):[0-9]+$` and be unique. Locked object → untouched with warning. `request.removed` source: bound, `created`, present, fingerprint unchanged, unlocked → delete members then row, report `deleted`; otherwise report `retired` and keep the object. A removed source without a previous binding is an error. `previous.identity` must be empty or `IDENTITY`. New playlist `position = max(position)+1`, `PlaylistTracks.position` 1-based, dates `CURRENT_TIMESTAMP`; new crate `INSERT INTO crates(name, locked, autodj_source)` and `crate_tracks` unique pairs.

- [ ] **Step 1: Failing tests:** `new_playlists_are_ordered_and_flattened_with_unique_names`; `crates_dedupe_members_and_ignore_order`; `smart_playlist_exports_captured_members`; `rename_and_reorder_update_the_bound_object_in_place`; `owned_edit_in_mixxx_is_reset_in_replace_and_appended_in_keep`; `deleted_owned_playlist_is_recreated`; `same_name_foreign_playlist_is_adopted_replace_vs_keep`; `two_same_name_playlists_are_not_adopted`; `removed_unchanged_owned_playlist_is_deleted_and_edited_one_is_retired`; `removed_without_binding_is_error`; `hidden_and_locked_objects_are_never_modified`; `container_switch_retires_old_and_creates_new`; `missing_track_id_is_error`; `report_bindings_match_native_state`.
- [ ] **Step 2–4:** FAIL → implement → PASS. **Step 5:** Commit `feat(mixxx): playlists, crates and ownership`.

### Task 9: Staged write orchestration

**Files:** Modify `mixxx.rs`; create `mixxx/verify.rs`.

**Interfaces:**
- Produces: `pub struct StagedMixxx { pub ownership: PlaylistOwnershipReport, pub warnings: Vec<String>, pub changed: bool }`; `pub fn write_staged_owned(stage: &Path, library: &SyncLibrary, options: &SyncOptions, request: &PlaylistOwnershipRequest) -> Result<StagedMixxx>`: open `stage`, `inspect`, one transaction running `write_tracks` → `write_cues` → `write_containers`, commit only when `total_changes()` rose, then `verify::staged(stage, library, &outcome)` re-reads the copy and fails the sync unless: foreign keys resolve, hot cue slots are unique per track, loops have length, blobs decode, row counts equal the plan, tracks outside the library are unchanged (compare a before-image hash of those rows). Adds one timing warning with the count of AAC (`.m4a`/`.mp4`/`.aac`) and WMA sources, a warning for tracks outside any `directories` entry, and for a database under `.var/app/org.mixxx.Mixxx` a warning listing the count of tracks outside `~/Music`, `/media` and `/run/media`. Also `pub fn write(path, library, options)` for tests.

- [ ] **Step 1: Failing tests:** `repeat_run_reports_no_change`; `unrelated_tracks_playlists_and_settings_rows_are_untouched`; `verify_rejects_a_corrupted_stage` (inject a duplicate slot through a hook); `aac_and_wma_sources_add_one_timing_warning`; `flatpak_library_warns_about_tracks_outside_visible_folders`; `full_run_on_schema_39_and_40`.
- [ ] **Step 2–4:** FAIL → implement → PASS. **Step 5:** Commit `feat(mixxx): staged write with readback verification`.

### Task 10: Timing rule

**Files:** Modify `crates/cuerithm-dj-sync/src/timing.rs`; create `crates/cuerithm-dj-sync/src/mixxx/lame.rs`.

**Interfaces:**
- Produces: `pub(crate) struct Mp3Start { pub info_frame: bool, pub first_frame_samples: u32, pub trim: Option<u32> }` and `pub(crate) fn mp3_start(path: &Path) -> Result<Mp3Start>`: bounded read of at most 2,400 bytes after any ID3v2 tag; find the first MPEG frame (error "MP3 start could not be determined" when none); `first_frame_samples` is 1152 for MPEG-1 and 576 for MPEG-2/2.5; the marker (`Info` or `Xing`) sits at frame offset `4 + side_info` with side info 17/32 (MPEG-1 mono/stereo) or 9/17 (MPEG-2/2.5); `info_frame` is whether a marker is present; `trim` is `Some(delay + 529)` only when the 4 bytes at `marker + 120` are `LAME`, `Lavf` or `Lavc` and the 12-bit delay `(b[0] << 4) | (b[1] >> 4)` at `marker + 141` is at most 4000, else `None` (the CRC is not checked, as FFmpeg does not check it).
- In `timing::offset_seconds`, handle `DjSoftware::Mixxx` **before** the profile is unpacked and independent of it: `.mp3` → `(trim.unwrap_or(0) as f64 − if info_frame { 0 } else { first_frame_samples as f64 }) / track.sample_rate`; `.aac`, `.m4a`, `.mp4`, `.wma` and every other extension → 0. Warnings for AAC/WMA come from Task 9, not from this function. `track.sample_rate` must be non-zero for MP3 (error otherwise).

- [ ] **Step 1: Failing tests** (synthetic MPEG frames built in the test, no encoder needed): `mp3_start_reads_info_frame_identifier_and_delay_for_mono_and_stereo_mpeg1_and_mpeg2`; `unknown_identifier_gives_no_trim_but_keeps_the_info_frame`; `no_marker_means_no_info_frame_and_first_frame_length_by_mpeg_version`; `lavf_and_lavc_identifiers_trim_like_lame`; `non_576_delay_changes_the_offset`; `crc_is_not_checked`; `implausible_delay_over_4000_is_ignored`; `unparsable_file_is_an_error`; `mixxx_offsets_for_the_three_header_facts_at_44100_and_48000` (−1152/44100, 0, 1105/44100); `lossless_ogg_opus_aac_and_wma_are_zero`; `mixxx_does_not_need_an_export_timing_profile`; `export_is_applied_exactly_once_to_cues_and_grid`.
- [ ] **Step 2–4:** FAIL → implement → PASS.
- [ ] **Step 5:** Re-run the scratch measurement (`mp3_timing_experiment.py`, `mp3_identifier_experiment.py`, kept in the task commit message as numbers) against `mp3_start` on the generated files and confirm it reports the same facts; commit `feat(mixxx): decoder-aligned timing offsets`.

### Task 11: `prepare_sync` integration

**Files:** Modify `crates/cuerithm-dj-sync/src/sync.rs`, `smart_rules.rs`, `validation.rs` (if it switches on software), `discovery.rs`.

**Interfaces:**
- Consumes: `mixxx::{resolve_database_path, write_staged_owned}`.
- Behavior: the Mixxx arm of `prepare_sync_owned_with_runtime`: reject a non-native environment ("Mixxx runs natively…"); resolve the database; capture the settings folder as a related root; `sqlite_snapshot` into `build/mixxx/mixxxdb.sqlite`; call `write_staged_owned` with the request for that database; insert ownership; extend warnings; publish the staged file only when `changed`. `smart_fallbacks` reports every smart playlist as exported by its saved members for Mixxx; `compile(.., Mixxx)` returns `Ok(None)`.

- [ ] **Step 1: Failing tests** in `crates/cuerithm-dj-sync/src/mixxx/tests.rs` (a `#[cfg(test)]` module, because `fixture` is `cfg(test)`): `preview_apply_restore_round_trip_returns_the_original_file`; `second_preview_has_no_file_changes`; `stale_preview_after_external_edit_is_rejected`; `running_mixxx_is_detected_by_the_process_guard` (use the existing guard seam if the other targets' tests have one, otherwise cover `process_matches` and the Linux scanner only); `wine_environment_is_rejected`; `journal_recovery_after_interrupted_apply_restores_prior_state`; `smart_playlists_warn_and_export_members`; `preview_lists_added_replaced_and_kept_counts`.
- [ ] **Step 2–4:** FAIL → implement → PASS. **Step 5:** Commit `feat(mixxx): wire Mixxx into prepare, apply and restore`.

### Task 12: Library service, ledger and organization

**Files:** Modify `crates/cuerithm-library/src/dj_sync_service.rs` (`canonical_target`: a directory becomes `<dir>/mixxxdb.sqlite`), `dj_sync_service/registration.rs`, `dj_sync_service/organization.rs`, `library_backup.rs` (only if it enumerates software), and add tests in `dj_sync_service/ownership_tests.rs`.

**Interfaces:** `canonical_target` maps folder and file spellings of a Mixxx library to one baseline identity; ownership registration for Mixxx uses the generic desktop-database capture (same as djay).

- [ ] **Step 1: Failing tests:** `mixxx_folder_and_file_targets_share_one_baseline`; `mixxx_playlist_ownership_survives_rename_and_acknowledges`; `modified_scope_syncs_only_changed_tracks`; `playlist_scope_adds_only_its_tracks_in_keep_mode`; `full_scope_manage_mode_replaces_matched_tracks`; `crash_between_apply_and_acknowledgement_settles_idempotently`.
- [ ] **Step 2–4:** FAIL → implement → PASS. **Step 5:** Commit `feat(mixxx): library service integration`.

### Task 13: Path repair (required for Organize)

**Files:** Modify `crates/cuerithm-dj-sync/src/path_repair.rs` (both `DjSoftware::Mixxx` arms); create `mixxx/repair.rs`; read `crates/cuerithm-library/src/organization_references.rs` (it calls `inspect_path_reference` for every known non-independent export and blocks Organize for a track when it fails, so without this a synced track cannot be moved).

**Interfaces:** `pub(crate) fn reference_id(database: &Path, source: &Path) -> Result<String>` returning the matched `track_locations.id` as text (error when absent or ambiguous); `pub(crate) fn repair_reference(database: &Path, old: &Path, new: &Path, expected_id: &str) -> Result<()>` updating `track_locations.location`, `directory`, `filename` for that row (error when the id no longer matches `old`, or when `new` already has a row); both use `location_key`.

- [ ] **Step 1: Failing tests:** `repair_moves_the_location_row_and_keeps_library_data`; `repair_rejects_missing_or_colliding_destination`; `organize_can_move_a_track_synced_to_mixxx` (through the library organization flow, asserting no blocker and the Mixxx row follows the file).
- [ ] **Step 2–4:** FAIL → implement → PASS. **Step 5:** Commit `feat(mixxx): path repair for moved tracks`.

### Task 14: Tauri and desktop UI

**Files:** Modify `apps/desktop/src-tauri/src/sync_commands.rs` (discovery inputs: home, local app data), `apps/desktop/src/sync-types.ts`, `sync-preferences.ts`, `views/Sync.tsx`; tests `apps/desktop/src/test/sync.test.tsx`, `sync-preferences.test.ts`.

**Interfaces:**
- `DjSoftware` gains `"mixxx"`; `syncTargets` gains `{id:"mixxx",name:"Mixxx",location:"Mixxx settings folder"}`; `SyncOptions.mixxx?: {existing_tracks:"keep"|"replace";container:"playlists"|"crates";replace_metadata:boolean;replace_key:boolean;replace_cues:boolean}`; `defaultSyncOptions.mixxx` = keep, playlists, true, true, true.
- UI for Mixxx only: a **Mode** select (*Add only — new tracks and playlists, existing Mixxx data untouched* / *Manage library — CueRithm replaces grids, keys and cues*), a **Playlists as** select (*Playlists* / *Crates*), three checkboxes (*Metadata*, *Key*, *Hot cues and loops*) enabled only in Manage mode, the existing *Beatgrid locks*, *Nearest colors* and *Skip beatgrids*. Hide Engine/USB-only controls and key-notation. Saved preferences sanitize the new block.

- [ ] **Step 1: Failing tests:** `selecting_mixxx_shows_mode_container_and_toggles`; `metadata_key_cue_toggles_are_disabled_in_add_only`; `preview_request_carries_the_mixxx_options`; `old_saved_preferences_load_with_mixxx_defaults`; `corrupt_mixxx_preferences_fall_back_to_defaults`.
- [ ] **Step 2:** `npm test --prefix apps/desktop -- --run`; FAIL. **Step 3:** Implement. **Step 4:** PASS plus `npm run build --prefix apps/desktop` and `cargo build -p cuerithm-desktop`. **Step 5:** Commit `feat(mixxx): desktop target and options`.

### Task 15: Documentation

**Files:** Modify `docs/user-guide/sync/index.md`, `docs/user-guide/help/not-available-yet.md`, `docs/release-readiness.md`, `crates/cuerithm-dj-sync/README.md`; create `docs/user-guide/sync/mixxx.md` if the guide index supports pages.

- [ ] **Step 1:** Document: where Mixxx keeps its library on each system (incl. Flatpak), close Mixxx first, the two modes with a before/after example, playlists vs crates, what is converted (smart → ordinary, folders flattened), what is not transferred, backup and restore, and the stated AAC and Windows/Linux runtime caveats. **Step 2:** run the user-guide validation used by the repo (`docs/site-requirements.txt`, `tools/ci`) and fix any link or build error. **Step 3:** Commit `docs(mixxx): user guide, readiness and roadmap`.

### Task 16: Installed-application acceptance

**Files:** Create `tools/mixxx-acceptance/README.md` and `tools/mixxx-acceptance/run.sh`; results in `docs/research/mixxx-sync-contract.md` (new section *Runtime acceptance*).

- [ ] **Step 1:** Launch `/Applications/Mixxx.app/Contents/MacOS/Mixxx --settings-path <scratch> --log-level debug` once so Mixxx creates its own database, then quit it (SIGTERM) and confirm `settings.mixxx.schema.version`.
- [ ] **Step 2:** With a small generated library (a click MP3 with a LAME header, a FLAC, a WAV), write a Mixxx-shaped sync through the real adapter (a `#[ignore]` integration test reading the scratch path from an env var), in both modes.
- [ ] **Step 3:** Relaunch Mixxx on the same folder loading the MP3 into deck 1 (`mixxx <file>`); capture the log and the post-quit database. Pass only when the log shows `Successfully deserialized Beats (BeatGrid-2.0)`, the key is read, hot cues load, no "Initializing track metadata … from file" line appears for adapter-written tracks, and the database still holds our grid, key and cues after Mixxx exits.
- [ ] **Step 4:** Record what passed, what could not be run (Windows, Linux, audible alignment) and any residual in the contract document; do not claim more. **Step 5:** Commit `test(mixxx): installed-application acceptance record`.

### Task 17: Whole-branch verification and review

- [ ] **Step 1:** `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`, `cargo build --workspace --release` all clean; `npm test --prefix apps/desktop -- --run` and `npm run build --prefix apps/desktop` clean. Fix and re-run until green; report exact results.
- [ ] **Step 2:** Run the code-review skill on the branch, resolve findings, repeat Step 1.
- [ ] **Step 3:** Update the spec status to implemented-with-evidence, list remaining gaps (Windows/Linux Mixxx runs, AAC alignment, libmad execution), then push `feat/mixxx-sync` and open a PR against `fix/rekordbox-sync-flow` only after confirming with the user.
