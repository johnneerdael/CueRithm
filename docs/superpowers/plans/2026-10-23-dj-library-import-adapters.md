# DJ Library Source Adapters Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Read every agreed local source family into the lossless import contract and implement the restricted DJ.Studio experiment without overstating source-app acceptance.

**Architecture:** Each adapter implements the shared `SourceAdapter` interface over private consistent snapshots and emits bounded observation records. Reuse existing native codecs where applicable, preserving extra raw source values and reporting unsupported profiles/fields. DJ.Studio's existing-record writer is a separate test-only experiment with production capability disabled.

**Tech Stack:** Rust, rusqlite/SQLCipher, quick-xml, safe plist decoding, bounded native binary/protobuf/zlib codecs and existing dj-sync modules.

**Spec:** [Approved design](../specs/2026-10-23-dj-library-import-refresh-design.md); [shared contracts and Tasks 1–8](2026-10-23-dj-library-import.md).

## Global Constraints

- The default mode is **Store as imported**.
- Retagging and renaming must be opt-in even if enabled in global Incoming settings.
- Preserve current DSP calculations.
- Existing project instructions prohibit rekordbox XML transport.
- Native Mixed In Key Windows storage is uninspected in the pack.
- Production write capability remains disabled until its exact D5 native-app acceptance gates pass.
- Target macOS, Windows and Linux; use native Rust parsers, no installed Python or vendor runtime.
- Read only allowlisted library/schema data; filter credentials/account/settings payloads and never mutate a live source.

## Review Focus

1. Mono/multichannel Mixxx files still use stereo engine cue units; zero and end-only ranges survive (Task 11).
2. A valid library lacks referenced media or analysis sidecars: retain items/membership with precise missing-field issues (Tasks 12, 16–17).
3. Duplicate slots or equal-time memory/hot cues appear: preserve raw identity/families and require conflict handling (Tasks 10–12, 16–17).
4. A foreign-origin database opens on a different OS: native timing stays unresolved unless its originating decoder profile is qualified (Tasks 11, 14–15).
5. DJ.Studio contains a traversal key, alternate grid or project override: reject unsafe paths and preserve unowned persistence exactly (Tasks 18–18b).

---

## File map and common adapter test contract

Create reader modules under `crates/cuerithm-dj-sync/src/import/readers/`: `mod.rs`, `m3u.rs`, `apple_music.rs`, `traktor.rs`, `virtual_dj.rs`, `mixxx.rs`, `serato3.rs`, `serato4.rs`, `serato_tags.rs`, `engine_dj.rs`, `djay_pro.rs`, `djay_codec.rs`, `keyed_archive.rs`, `mixed_in_key11.rs`, `bookmark.rs`, `rekordbox.rs`, `pioneer_usb.rs`, `dj_studio.rs`, `dj_studio_records.rs`. Split further only where a format boundary warrants it. Modify `import/registry.rs` as each reader lands; do not advertise an absent reader.

Public integration tests: `crates/cuerithm-dj-sync/tests/import_<provider>.rs`, one per provider name above; `tests/import_fixture_support/mod.rs` generates synthetic snapshots independently of outbound serialization where possible. Codec unit tests may live beside private parsers. Experimental writer files are listed in Task 18b.

**Common interfaces:** Every reader implements Task 1's `SourceAdapter`; named adapter structs are `<Name>Reader`, e.g. `MixxxReader`, registered by exact `SourceProvider`. Consumes `ImportSnapshot`, `ReadOptions`, `ObservationSink`, limits and cancellation; produces `SourceProfile` and `ObservationHeader`. No adapter returns `SyncLibrary` as its lossless representation. `ReadOptions` selects history/queue, optional file-tag capture and explicitly supplied origin/path mappings; publication selection remains a library-service responsibility.

**Common required assertions for every reader test below:** all source rows are emitted or accounted as typed issues; missing paths remain visible; raw unknown safe fields retained; track/collection/membership IDs stable across repeat reads; source hashes unchanged; absent vs empty/null preserved; profile drift fails safely; cancelled reads do not claim completeness. Synthetic cases have no live app/account dependencies. A D2 parser test never sets D4/D5/D6 capability.

### Task 9: M3U/M3U8 and Apple Music exported plist

**Files:** Create `readers/{m3u,apple_music}.rs`, `tests/import_{m3u,apple_music}.rs`; modify registry.

**Interfaces:** Produces `M3uReader` and `AppleMusicReader` implementing the common interface. M3U identity includes source playlist binding plus normalized source entry path; occurrence identity includes entry position, not just track ID. Apple track IDs are source-library scoped; persistent playlist IDs govern hierarchy.

- [x] Write `m3u_resolves_relative_entries_and_preserves_a_b_a` asserting UTF-8/BOM/UTF-16LE/CRLF, UNC/drive/file URI, Unicode, relative playlist-directory paths, duplicate order and EXTINF as display hints only. Write `apple_plist_retains_milliseconds_and_missing_remote_records` asserting 1250 ms → 1.25 s, rating raw/scale, parent persistent IDs, exported smart membership/rules and absent DJ cues/grid; remote rows become unavailable references rather than disappearing.
- [x] Run `cargo test -p cuerithm-dj-sync --test import_m3u --test import_apple_music`; expect missing reader failure.
- [x] Implement safe parsers with bounded text/plist traversal, exact percent-decoding errors and no external entities. Apple plist is distinct from prohibited rekordbox XML; do not add native Music-database access. Media tags require selected read-only file-tag capture; neither parser triggers DSP or copying.
- [x] Rerun tests with malformed URI/plist, empty playlist and missing member; expect common assertions and format assertions pass.
- [x] Commit `feat: read M3U and Apple Music library exports`.

### Task 10: Traktor NML and VirtualDJ libraries

**Files:** Create `readers/{traktor,virtual_dj}.rs`, `tests/import_{traktor,virtual_dj}.rs`; modify registry. Use existing `virtual_dj/` format helpers only where they preserve inbound semantics.

**Interfaces:** Produces `TraktorReader` and `VirtualDjReader`. Traktor path decoding precedes playlist-key resolution. VirtualDJ snapshot includes selected home/per-volume databases and My Lists/legacy playlist artifacts.

- [x] Write `traktor_preserves_cue_classes_sparse_slots_and_unknown_xml` asserting START=1250 ms → 1.25 s, nonzero loop LEN, independent nested grid BPM, raw INFO key vs numeric key, all 24 key mappings, `/:`/volume escaping and no invented sample rate. Write `virtualdj_distinguishes_scan_and_poi_bpm` asserting Scan.Bpm=0.5 → 120 BPM, POI.Bpm=128 → 128 BPM, cue Num=1 → slot0, loop Size=8 → 4 seconds at Scan.Bpm=0.5. Preserve same-time memory/hot rows, actions/flags, My Lists and legacy folder tracks.
- [x] Run `cargo test -p cuerithm-dj-sync --test import_traktor --test import_virtual_dj`; expect missing readers.
- [x] Implement NML PROGRAM/root checks, safe recursive hierarchy, raw smart-rule/unknown XML retention and separate opt-in history. Implement VirtualDJ per-volume path authority and `.vdjfolder`/`.subfolders`/M3U parsing; unsupported filters retain materialized membership and a loss issue. Preserve source times as native until clock-qualified.
- [x] Rerun with cycles, invalid lengths, malformed XML/entity input, equal basenames on two volumes, streaming/missing records and duplicate slots; expect pass without fabricated normalization.
- [x] Commit `feat: read Traktor and VirtualDJ library sources`.

### Task 11: Mixxx relational data and versioned codecs

**Files:** Create `readers/mixxx.rs`, `tests/import_mixxx.rs`; extend existing `mixxx/{proto,beats,keys,cues,schema}.rs` only with reusable inbound helpers, preserving writer output/tests.

**Interfaces:** Produces `MixxxReader`; exact profile initially pinned source schema39/40 with required column/settings/codec fingerprints, as supported by fixtures. Older revisions have explicit diagnostic/read capability only when separately covered, never automatic migration based on Mixxx's own compatibility promise.

- [x] Write `mixxx_cues_use_two_engine_channels_at_every_source_channel_count` for 44.1/48/96 kHz and 1/2/6 channels, fractional position, -1 start sentinel, zero time, negative pre-roll and end-only range. Assert `1764000 / (2*44100) == 20.0`; beat protobuf frame22050/44100 == 0.5 seconds. Add `mixxx_preserves_all_keys_variable_beats_and_occurrences`: all 24 keys, sparse/duplicate slots, A/B/A ordered playlist and unordered crates.
- [x] Run `cargo test -p cuerithm-dj-sync --test import_mixxx`; expect missing reader.
- [x] Detect settings key `mixxx.schema.version`, explicit optional projections and LEFT JOIN broken locations. Read hidden/deleted/media-missing flags separately. Decode `BeatGrid-1.0/2.0`, `BeatMap-1.0`, `KeyMap-1.0`, proto2 presence/signed int32, unknown bytes and qualified native-endian legacy 16-byte grid; failed grid stays failed, no scalar zero-offset fallback. Retain rating/color epochs, key changes and tuning. Compare originating decoder profile before timing inversion.
- [x] Rerun with schema manifest reconstruction/independent vectors, corrupt protobuf, unknown version, invalid rate, historical corrupt black zero cue and duplicate hot slots; expect raw preservation plus diagnostics, no silent query-order resolution. Run existing Mixxx writer tests for regression separately.
- [x] Commit `feat: import Mixxx libraries with native timing provenance`.

### Task 12: Serato 3/Lite, Serato 4/Lite and file performance tags

**Files:** Create `readers/{serato3,serato4,serato_tags}.rs`, `tests/import_{serato3,serato4,serato_tags}.rs`; reuse existing `serato4.rs` discovery/schema facts; modify registry.

**Interfaces:** Produces `Serato3Reader`, `Serato4Reader`; helper `decode_serato_performance(tags: &FileTagObservation, limits: &ImportLimits) -> Result<SeratoPerformanceObservation>`. Result keeps Markers2/grid/autotags distinct from ordinary metadata and raw legacy Markers_.

- [x] Write `serato3_chunks_are_parent_bounded_and_volume_scoped`, `serato4_optional_tables_do_not_drop_crates`, `serato_markers2_vectors_preserve_slots_and_loops` asserting cue1250 ms=1.25 seconds, loop4000–8000 ms=4–8 seconds, grid markers1.25/5.25 seconds and computed120 BPM. Assert each container wrapper (ID3 GEOB, FLAC, MP4, Ogg), loop-only records, unknown entries, sparse slots and retained duplicate occurrences. Legacy Markers_-only input emits `unsupported_legacy_serato_markers` and no false empty hotcue replacement.
- [x] Run `cargo test -p cuerithm-dj-sync --test import_serato3 --test import_serato4 --test import_serato_tags`; expect missing readers/codecs.
- [x] Implement legacy database V2/crate/smartcrate/neworder readers with UTF-16BE bounds and per-volume roots. Implement v4 space/asset/location/container joins, optional keep-alive/rating tables and raw smart rules. Native database tags have precedence over current file metadata except explicitly file-owned performance observations; missing media retains tracks and reports unavailable cues/grids. Preserve streaming local cache references without account ingestion. Lite differences require exact profile evidence.
- [x] Rerun with truncated lengths/base64, count overflows, missing volumes, optional-table absence, unknown smart operators, root pseudo-containers and partial media-tag failures; expect common assertions and zero file writes. No Serato5 or legacy Markers_ decoding claim.
- [x] Commit `feat: import Serato libraries and Markers2 performance data`.

### Task 13: Engine DJ relational variants and mixed-endian blobs

**Files:** Create `readers/engine_dj.rs`, `tests/import_engine_dj.rs`; reuse `engine_dj.rs` native facts/helpers without invoking its writable paths; modify registry.

**Interfaces:** Produces `EngineDjReader`; identity is origin DB UUID plus origin track ID, with local row ID retained separately. Snapshot manifests participating `m.db`, selected `stm.db`/`hm.db` and required sidecars coherently.

- [x] Write `engine_reads_legacy_and_performance_view_without_migration` and `engine_vectors_keep_48k_sample_units` asserting grid native1.25/5.25 seconds, beat indices0/8, BPM120, quickcue1.25, loop4–8 and activeOnLoad bit1. Assert both grid lists/raw flags retained, UTF-8 cue name lengths use bytes, linked-list hierarchy/membership order and disconnected nodes accounted.
- [x] Run `cargo test -p cuerithm-dj-sync --test import_engine_dj`; expect missing reader.
- [x] Query legacy Track columns versus PerformanceData table/view explicitly; never normalize/drop/migrate live schema. Decode four-byte BE declared-size + bounded zlib, mixed endianness, sample rate, sentinel/negative cues and loop flags. Invalid sample rate yields unresolved timing rather than assumed44100. Retain direct versus inherited folder membership and raw key/rating mappings.
- [x] Rerun with declaration mismatch, inflated bombs, invalid/cyclic next links, missing source UUID, streaming URI and unsupported schema; expect pass and byte-identical source artifacts.
- [x] Commit `feat: read Engine DJ sources without schema mutation`.

### Task 14: djay Pro keyed archives and typed streams

**Files:** Create `readers/{djay_pro,djay_codec,keyed_archive}.rs`, `tests/import_djay_pro.rs`; preserve existing `djay_pro.rs` writer; modify registry. Shared archive decoder is reused by MIK in Task 15.

**Interfaces:** Produces `DjayProReader`; `decode_keyed_archive(bytes: &[u8], limits: &ImportLimits) -> Result<ArchivedValue>` and `decode_djay_stream(bytes: &[u8], limits: &ImportLimits) -> Result<ArchivedValue>`. `ArchivedValue` retains scalars, dictionaries, ordered arrays, UID references and opaque unknown class/block bytes; never instantiates vendor classes.

- [x] Write `djay_typed_stream_alignment_and_pointers_are_bounded`, `djay_joins_collections_without_dropping_missing_locations` and `djay_manual_tempo_and_key_remain_distinct`. Assert 20-byte header/class marker, LE alignment, null/control/URI/block handling, bounded pointer arrays, analyzed/manual key and signed tempo exponents. Epoch0 converts to 2001-01-01 UTC, independent of host timezone. Foreign-origin compressed timing stays unresolved.
- [x] Run `cargo test -p cuerithm-dj-sync --test import_djay_pro`; expect missing reader/codecs.
- [x] Decode database2 collection/key rows, bplist/NSKeyedArchiver and typed-stream routes with raw unknown preservation. Resolve ordered childUUID hierarchy and parent recovery; preserve cue numbers/end times, alternate location candidates, straight-grid/manual shift/exponent evidence and smart filter losses. Gate exact v5+ shapes rather than trusting marketing version alone.
- [x] Rerun malformed alignment/count/pointer/UID cycles, alternate grids, orphan artists/items, repeated members and unknown archive classes; expect pass and explicit issues without placeholder values becoming tags.
- [x] Commit `feat: read djay Pro library archives and typed records`.

### Task 15: Mixed In Key 11 macOS-origin artifacts

**Files:** Create `readers/{mixed_in_key11,bookmark}.rs`, `tests/import_mixed_in_key11.rs`; reuse Task 14 archive decoder; modify registry.

**Interfaces:** Produces `MixedInKey11Reader`; `decode_embedded_bookmark_path(bytes: &[u8], limits: &ImportLimits) -> Result<BookmarkPathEvidence>`. This is historical embedded-path evidence, not security authority or portable alias resolution.

- [x] Write `mik_fixture_preserves_analysis_cache_markers_and_beats`: native key8A vs cached key retained, beat times `[0.0,0.5,1.0]`, archived tempo120, two generic markers at `[-0.005,32.125]`, no pad slots/downbeat inference, embedded path `/DemoMusic/Example.flac`. Assert sparse collection order and source energy/key segments remain provenance, not product per-cue energy. Test Windows/Linux explicit path mapping for this macOS-origin artifact.
- [x] Run `cargo test -p cuerithm-dj-sync --test import_mixed_in_key11`; expect missing reader.
- [x] Validate Core Data store/model metadata and schema/entity mappings from fixtures; read all songs independent of membership, collections, joins and cue rows. Decode bounded bookmark TOCs/records/chains and MIKBeats UID arrays including direct numeric objects; preserve raw analyzed/default/tag values and Apple epoch. Unknown bookmark leaves unresolved item; no Core Data migrations, security-scope reuse, source media decoding or optional waveform deserialization.
- [x] Rerun unknown model, null/default key0, UID/TOC cycles/truncation, duplicate cue time, unordered cue ties, malformed bookmark, orphan/cyclic parents and unavailable external volume; expect pass and native Windows profile explicitly unsupported.
- [x] Commit `feat: import supported Mixed In Key 11 snapshots`.

### Task 16: Existing accepted direct rekordbox profiles

**Files:** Create `readers/rekordbox.rs`, `tests/import_rekordbox.rs`; narrowly expose reusable read helpers from `native_rekordbox/source.rs` and the existing rekordbox crate; modify registry and library legacy bridge only where Task 3 requires it.

**Interfaces:** Produces `RekordboxReader`; consumes existing approved source-profile/key provisioning and inspection APIs. Retains both relational content ID and MasterSongID and exact target/library identity; bridges existing native links without changing their owner/baseline state.

- [x] Write `rekordbox_general_import_never_enqueues_incoming`, `rekordbox_preserves_memory_hot_same_time_and_sixteen_slots` and `rekordbox_profile_acceptance_is_not_widened`. Assert Kind0/4 memory, Kinds1–3 slots0–2 and5–17 slots3–15, millisecond conversion, sentinel OutMsec=-1, active-loop flags, full PQTZ beat rows and playlist order. Missing ANLZ/media retains metadata with issues.
- [x] Run `cargo test -p cuerithm-dj-sync --test import_rekordbox` and the library import bridge test; expect new API failure.
- [x] Implement source-neutral projection from accepted direct read snapshots, explicit LEFT JOINs/deletion completeness, native/cached/file tag separation, My Tags and smart-rule provenance. Preserve accepted encryption/version constraints. No XML route, live migrations or new automatic jobs. Existing legacy import commands retain compatibility; shared UI uses the new import service.
- [x] Rerun with source moves/cloud recorded paths, raw unknown cue kinds, corrupt ANLZ section lengths, initial local conflicts and existing native state; expect pass and existing direct sync tests unchanged.
- [x] Commit `feat: connect accepted direct rekordbox sources to shared import`.

### Task 17: Pioneer/AlphaTheta USB PDB and OneLibrary

**Files:** Create `readers/pioneer_usb.rs`, `tests/import_pioneer_usb.rs`; reuse/extend `pioneer_usb/{pdb,database,analysis}.rs` inbound helpers; modify registry.

**Interfaces:** Produces `PioneerUsbReader`, preferring accepted `exportLibrary.db` profile over `export.pdb` while preserving both artifact fingerprints. Imported audio remains on device unless separate explicitly previewed copy/organization policy applies.

- [ ] Write `usb_reads_pdb_rows_and_onelibrary_without_copying_audio` and `usb_preserves_cue_classes_anlz_and_membership_order`. Assert SQL microseconds/1e6 versus ANLZ milliseconds/1000, BPMx100/100, one-based ANLZ hotCue to zero-based slot, coincident memory/hot families, raw kind/active-loop and exact missing-sidecar diagnostics. PDB page/row/string variants and native analysis buffers have independently constructed controls.
- [ ] Run `cargo test -p cuerithm-dj-sync --test import_pioneer_usb`; expect missing reader.
- [ ] Reuse approved SQLCipher4/format-key policy and bounded linked-page/row-bitmaps/packed strings. Extend inbound lookup/track/hierarchy/history fields without lossy roundtrip through writer tables. Read DAT/EXT PQTZ/PCOB/PCO2 separately and record precedence/losses. No basename auto-merge, implicit audio copy or source reference mutation.
- [ ] Rerun overflow/heap overlap/page cycle/unknown packed string variants, missing files, both catalog representations, device-relative roots and duplicate entries; expect pass. Keep hardware D6 explicitly unrun unless an actual device test is available.
- [ ] Commit `feat: read Pioneer device libraries and native performance assets`.

### Task 18: DJ.Studio SQL and imported-record reader

**Files:** Create `readers/{dj_studio,dj_studio_records}.rs`, `tests/import_dj_studio.rs`; modify registry.

**Interfaces:** Produces `DjStudioReader`; consumes exact schema/migration/record fingerprints, native+renderer identities when known, and coherent SQL+JSON snapshot. SQL integer ID, JSON record key, structure key and project identities remain separate.

- [ ] Write `djstudio_keeps_sql_and_json_divergence_and_unknown_fields`, `djstudio_shards_utf16_and_preserves_cue_arrays`, and `djstudio_rejects_unsafe_keys`. Assert key12→8A/Am, -1/-2 unknown values, fractional BPM despite INTEGER affinity, duplicate PlaylistTracks, deterministic `(sort,id)` explicitly unverified order, beat2 resolves to selected structure entry2.time, time=-1 distinct from0 and no cue RGB field invented. Test Unicode surrogate sharding, alternate grids, integer/negative/fractional beat cues and system/hot/memory array holes/order.
- [ ] Run `cargo test -p cuerithm-dj-sync --test import_dj_studio`; expect missing reader.
- [ ] Read Explorer Tracks/Playlists/memberships/bookmarked Folders, separate audio-library and referenced track structures. Join by unique normalized path conservatively, retaining both surfaces and conflicts. Preserve processed file identity/source provenance, external maps, analysis state, alternate structures and projects as untouched references. Refuse traversal/absolute/NUL keys and symlink escape. Unresolved/fractional beat timing has no invented canonical endpoint; BPM-only never regenerates grid.
- [ ] Rerun malformed/trailing JSON, duplicate path, missing target record, unknown migrations/renderer and source mutation during capture; expect accounted issues and no writes/cache invalidation.
- [ ] Commit `feat: read DJ.Studio native and imported library surfaces`.

### Task 18b: Restricted DJ.Studio existing-record write experiment

**Files:** Create `crates/cuerithm-dj-sync/src/import/experimental/{mod,dj_studio,journal}.rs`, `tests/import_dj_studio_experiment.rs`; add Cargo feature `experimental-djstudio-write` disabled by default and never enabled by the desktop dependency; add test-only independent read-back helper.

**Interfaces:** Feature-gated `preview_djstudio_experiment(root: &DisposableExperimentRoot, patch: ExistingRecordPatch, cancel: &CancellationToken) -> Result<DjStudioExperimentPreview>`, `apply_djstudio_experiment(preview: &DjStudioExperimentPreview, cancel: &CancellationToken) -> Result<ExperimentReceipt>`, `recover_djstudio_experiment(root: &DisposableExperimentRoot, cancel: &CancellationToken) -> Result<ExperimentRecovery>`. `DisposableExperimentRoot` can be created only by the fixture generator in an empty temporary directory. No production source-root constructor or Tauri write command.

- [ ] Write `existing_record_patch_preserves_unowned_persistence` asserting owned metadata/key updates while analyzed/original key, grids, external maps, stable cueKey/array structure and project hashes remain unchanged. Write `failure_at_every_replace_restores_original_hashes` for DB/JSON/cache/journal transitions and `experiment_cannot_address_live_or_unknown_root`. Test stale before-hash, native/renderer mismatch, concurrent process/root modification, interrupted recovery and idempotent retry.
- [ ] Run `cargo test -p cuerithm-dj-sync --features experimental-djstudio-write --test import_dj_studio_experiment`; expect missing experiment API.
- [ ] Implement the handover's nine-step offline protocol: closed disposable root/exclusive lease, complete backups, exact patch manifest, staged standalone SQLite with FK/integrity checks, per-file JSON fsync/replace, journal outside recursively scanned tables, affected aggregate caches only, independent canonical-record read-back and guarded rollback. Restrict to existing generated records; never insert new analyzed identity, propagate project edits or claim arbitrary cue/grid write support. Preserve all unknown properties and their types. Production capability remains disabled, independent of feature compilation.
- [ ] Rerun fault matrix and default-feature dependency/registry tests; expect byte-exact restoration and no production writer exposure. Native reopen D5 remains an explicit separate acceptance item; never install partial synthetic records into a real app root.
- [ ] Commit `test: qualify disposable DJ.Studio existing-record write protocol`.

## Self-review

All 13 agreed provider families are assigned; Serato3/4 and USB/desktop rekordbox remain separate profiles. Rekordbox5 XML and native Windows MIK are explicit unsupported boundaries, not missing reader tasks. Common limits, provenance, cancellation and source-integrity tests apply to every task. Each reader is useful/testable through the shared Rust inspection API before desktop work; none of these commits alone certifies source-app/hardware acceptance. Continue with [desktop and qualification](2026-10-23-dj-library-import-desktop.md).
