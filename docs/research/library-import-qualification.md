# DJ library import qualification

## Current-base baseline

On the isolated, unchanged `446aab645b40dcaad82182c79d96daa7fa27f9cc` checkout, Linux cloud execution produced:

- `cargo test --workspace --release --no-fail-fast -- --test-threads=2`: **2072 passed, 48 failed, 11 ignored**, across 148 test targets. The failing target is `cuerithm-library --lib`.
- Every failed test reports **DJ backing mount has no block-device source**. The cloud workspace uses overlay/tmpfs rather than a block-device-backed DJ volume. These tests do not establish native volume identity acceptance in this environment; the production guards remain intact.
- Frontend: `pnpm --dir apps/desktop test -- --maxWorkers=1`: **59 files, 535 tests passed**.
- The first attempted baseline build lacked the bundled-runtime path in the new checkout. A local ignored symlink to the already verified sealed runtime resolved that setup issue before the recorded test run.

The 48 baseline failures are listed below so a later branch comparison can distinguish existing environmental failures from regressions. Windows/macOS-named tests here exercised Rust fixtures on Linux; they are not native-platform acceptance.

- `dj_sync_service::mixxx_tests::a_library_is_created_from_a_target_spelled_as_the_missing_database_file`
- `dj_sync_service::mixxx_tests::a_library_is_created_where_mixxx_has_none_and_restore_takes_it_away_again`
- `dj_sync_service::mixxx_tests::a_running_mixxx_blocks_the_service_preview_with_a_clear_reason`
- `dj_sync_service::mixxx_tests::add_only_keeps_existing_tracks_while_manage_library_replaces_them`
- `dj_sync_service::mixxx_tests::changed_scope_is_empty_after_a_full_sync_and_a_rename_updates_the_playlist_in_place`
- `dj_sync_service::mixxx_tests::replacing_the_whole_library_builds_a_new_one_and_restore_brings_the_old_one_back`
- `dj_sync_service::mixxx_tests::playlist_scope_adds_only_its_tracks_and_that_playlist`
- `dj_sync_service::ownership::registration_identity::tests::same_volume_same_byte_catalog_replacement_changes_registration`
- `dj_sync_service::ownership_tests::changed_ledger_or_replaced_database_blocks_publication_before_any_write`
- `dj_sync_service::ownership_tests::djay_changed_after_an_acknowledged_deletion_is_a_valid_noop`
- `dj_sync_service::ownership_tests::first_preview_and_acknowledged_restore_reject_identical_database_replacements`
- `dj_sync_service::ownership_tests::native_restore_completion_recovers_local_state_and_blocks_new_previews_until_finished`
- `dj_sync_service::ownership_tests::persisted_transition_recovery_is_idempotent_without_a_second_native_write`
- `dj_sync_service::ownership_tests::published_unacknowledged_sync_blocks_new_previews_and_recovers_same_creation_proof`
- `dj_sync_service::ownership_tests::virtual_dj_case_only_subtree_rename_applies_and_restores_exact_directory_spelling`
- `dj_sync_service::recovery_tests::interrupted_external_publication_restores_without_acknowledging_a_baseline`
- `dj_sync_service::recovery_tests::legacy_hash_baseline_cannot_authorize_native_deletion`
- `dj_sync_service::recovery_tests::owned_deletion_is_staged_acknowledged_and_restored_with_its_ledger`
- `dj_sync_service::tests::archived_track_restore_still_rejects_changed_source_identity_or_bytes`
- `dj_sync_service::tests::archived_tracks_can_restore_native_backups_and_own_tag_hashes`
- `dj_sync_service::tests::captured_windows_configuration_changes_block_apply_but_not_captured_recovery_or_restore`
- `dj_sync_service::tests::changed_metadata_is_selected_from_the_durable_target_baseline`
- `dj_sync_service::tests::discard_cannot_remove_native_applied_unacknowledged_or_restored_history`
- `dj_sync_service::tests::discarded_library_previews_release_staging_and_leave_sources_and_native_state_unchanged`
- `dj_sync_service::tests::library_revision_and_source_changes_reject_stale_work_and_record_errors`
- `dj_sync_service::tests::old_native_journal_without_environment_or_configuration_still_recovers_and_restores`
- `dj_sync_service::tests::preview_apply_changed_and_restore_use_acknowledged_baselines`
- `dj_sync_service::tests::windows_context_service_preserves_reference_and_foreign_tracks_then_noops_and_restores`
- `dj_sync_service::tests::replaced_same_spelling_library_parent_refuses_apply_and_restore`
- `dj_sync_service::tests::windows_context_service_refuses_changed_mapping_at_apply_recovery_and_restore`
- `dj_sync_service::tests::windows_installation_updates_keep_acknowledged_library_identity`
- `dj_sync_service::tests::windows_owned_playlist_sync_survives_organization_and_changed_resync`
- `library_backup::inventory_phase_tests::native_path_repair_receipts_roundtrip_and_missing_recovery_bytes_are_refused`
- `organization_references::tests::a_byte_identical_replaced_native_target_invalidates_the_frozen_reference`
- `organization_references::tests::a_retained_reference_guard_cannot_acknowledge_a_later_external_native_edit`
- `organization_references::tests::a_retained_reference_guard_cannot_retire_source_after_a_later_external_native_edit`
- `organization_references::tests::a_verified_own_receipt_can_rebase_another_frozen_track_snapshot`
- `organization_references::tests::acknowledged_target_edit_blocks_cleanup_verification_without_overwrite`
- `organization_references::tests::copy_inverse_rebinds_to_owned_original_and_preserves_later_native_edits`
- `organization_references::tests::external_edit_after_preview_blocks_before_audio_publication`
- `organization_references::tests::independent_copy_history_keeps_raw_source_paths_losslessly`
- `organization_references::tests::independent_device_copy_does_not_require_a_mounted_or_closed_target`
- `organization_references::tests::linked_reference_repairs_only_path_before_local_ack_and_preserves_remote_edits`
- `organization_references::tests::normal_sync_cannot_replace_native_receipts_during_interrupted_cleanup`
- `organization_references::tests::partial_multi_target_repair_retains_original_and_retry_does_not_republish_completed_target`
- `organization_references::tests::pending_copy_source_edit_requires_an_explicit_choice_and_preserves_native_metadata`
- `organization_references::tests::required_offline_target_is_a_visible_blocker_before_publication`
- `organization_references::tests::windows_configuration_organization_repairs_native_refs_and_legacy_membership`

## Feature evidence

The handover manifest verifies all 22 supplied artifacts. Their schema/synthetic tests establish documentation, layout and invented fixture evidence (D0–D2) only. Shared contract tests and read-side infrastructure checks do not establish D3 copied-library, D4 source-app inbound, D5 reopened-app write, or D6 hardware acceptance. Reader-specific field/profile capability and actual feature test results will be added as each adapter lands.

## Shared import processing and file publication

The shared backend stores imported values without scheduling preparation. Key,
BPM, grid, whole-track energy, cue generation, occupied-hotcue replacement,
catalog enrichment, cleanup, selected audio-tag writes and file organization are
independent policy choices. Generated or provider results require an effects
preview and explicit apply before adoption. Missing credentials or an unmatched
catalog query block the requested stage while preserving the valid import.
Catalog provenance retains the selected typed candidate and attempted providers.

Controlled file-publication tests cover MP3, AAC/ADTS, AAC/M4A, ALAC/M4A, FLAC,
WAV, AIFF, Ogg Vorbis, Opus and WMA. They compare decoded PCM before/after a
selected title write and require exact original bytes after Undo. The invented
signal, generator and manifest are documented in
[the fixture record](../../tools/fixtures/import-publication-README.md).
These tests establish neither prediction accuracy nor DJ-app acceptance.

The import wrapper currently accepts BPM and the existing text editor fields:
title, artist(s), album, album artist, genre(s), style, label, version/mix,
remixer, producer, composer, lyricist, grouping, comment, catalog number, ISRC
and key. The native writer still validates each field/container combination;
the ten-format title test does not qualify every field in every container.
Artwork, dates, numeric track/disc fields, ratings and energy are not enabled by
this import wrapper. Unsupported representations block publication explicitly.

File writes reuse durable metadata and organization journals, staged copies,
PCM verification and original-byte backups. Restart tests cover linked writes
and interrupted Undo; later local edits, replaced files, naming collisions and
new source bindings invalidate stale authority. Naming uses final accepted
metadata even when audio-tag writing is off. Imported items retain their IDs and
remain unprepared; file publication creates no preparation jobs.

Renaming or moving an original source-linked file remains blocked until its
exact native reference-repair profile is qualified. Explicit copying preserves
the original source reference. A detached synthetic rename fixture exercises
the existing organization service only; it provides no source-repair acceptance.
Unqualified native cue/grid clocks remain ineligible for export, including when
an imported item is bound to an older prepared track. Desktop controls, source
adapters and final user documentation remain later tasks.

DJ.Studio production writes remain disabled; native Windows MIK storage and rekordbox XML remain unsupported. Existing qualified output adapters retain their separate prior evidence.

## Shared-service checks through Task 3

The shared store passes ten focused tests; affected migration/backup/storage integration suites pass 97 tests; shared validator/snapshot controls pass 12 tests; dj-sync passes 413 tests with three explicitly ignored. Both changed Rust packages pass all-target Clippy with warnings denied. The broader library run retained the same 48 baseline filesystem limitations; its nine additional schema-fixture failures were corrected and rerun successfully. Whole-workspace final qualification and source-specific/native gates remain pending.

Task 8 verification: 90 import tests pass with the tagging feature and 74 with
the default build. Feature/default all-target Clippy and formatting pass. The
broad feature run covered 40 targets: 774 passed, 50 failed, 2 ignored. Of those
failures, 48 match the baseline above; the other two were old schema-version
assertions, updated for migration 29 and independently rerun successfully
(catalog-access target 3/3; historical tagging migration 1/1). The broad run is
reported with its actual failures rather than described as fully passing.

## M3U/M3U8 and Apple Music exported libraries

The M3U reader supports UTF-8 (optional BOM), BOM-marked UTF-16LE and CR/LF
line endings. Relative entries resolve against the original selected playlist
folder, never the private snapshot folder. Windows drive/UNC/file-URI spellings
retain their source context; equivalent Windows paths share a track while every
occurrence retains its own position and original reference. EXTINF is a display
hint only. Empty playlists remain visible. Other legacy encodings are rejected
explicitly. Absolute foreign paths require an explicit origin or root mapping;
no basename matching, media copying or analysis occurs.

The Apple reader handles exported plist XML with the library-export 1.1 shape,
not the native Music database. It retains library/track/persistent playlist IDs,
parent hierarchy, repeated membership, materialized smart playlists and their
original rules. Millisecond duration becomes fractional seconds; rating retains
its raw 0–100 scale. Missing member rows become explicit unavailable references.
Absent and empty metadata remain distinct. Safe unknown record and membership
fields survive; account/credential/settings keys are filtered. DJ cues and grids
are absent in this transport. Neither source reader triggers DSP or writes.

Media-tag capture is optional and read-only, stored as a separate observation
with raw payload blobs. The XML parser bounds bytes, nodes and nesting, rejects
internal/external entity declarations and malformed structure, and never fetches
the standard Apple DTD. Stable repeated reads and source fingerprints are tested.
These reader fixtures establish D2 only; native-app acceptance remains unrun.

Task 9 verification: all 421 dj-sync tests pass (3 ignored), including eight new reader cases; all-target Clippy `-D warnings`, formatting and diff checks pass.

## Traktor and VirtualDJ readers

The Traktor parser currently qualifies NML19 with Traktor Pro3/4 program
markers. It decodes Traktor path separators before resolving playlist keys,
retains the textual INFO key independently of the 24-value musical-key mapping,
and converts cue milliseconds to native seconds. Nested grid BPM remains
independent of track BPM; no sample rate is invented. Same-time memory/hot
cues, sparse and duplicate slots, loop ranges and unknown XML survive. Invalid
loop lengths retain their raw source and a field issue instead of an invented
end. Ordered playlist and smart-list membership/rules are preserved.

The VirtualDJ parser covers XML8.4/8.5, selected per-volume database artifacts,
My Lists `.vdjfolder` files, `.subfolders` hierarchy/order files and legacy
M3U/M3U8 lists. Scan BPM is seconds per beat (`0.5` → `120` BPM); grid POI BPM
is already BPM (`128` stays `128`). Loop size is beats. Actions and unknown
flags remain raw, rather than becoming hotcues. Unsupported dynamic filters
retain materialized membership and an explicit issue. Empty folders with a
selected `order` file remain visible. Tracks from different database artifacts
retain separate source identity; ambiguous references require review.

Both readers keep native clocks unresolved, retain unavailable/streaming media
and preserve source bytes. History collections are opt-in; an ordinary folder
named History does not become a history source. NML/playlist cycles, unsupported
profiles, malformed XML/entities and order-file traversal are rejected. Parser
fixtures establish D2 only. Live multi-volume selection must preserve every
original root authority in desktop integration; the copied-source fixture does
not establish D4 native multi-volume acceptance or D5 reference-repair support.

Task 10 verification: all 431 dj-sync tests pass (3 ignored), including ten new Traktor/VirtualDJ cases. All-target Clippy `-D warnings`, formatting and diff checks pass.

## Mixxx reader

The read-only Mixxx reader qualifies exact music-table, column, index and
foreign-key fingerprints for source schema39/40 and the settings key
`mixxx.schema.version`. Fixtures are independently regenerated from the pinned
upstream migration history; `PRAGMA user_version` cannot select a profile.
Broken location references, deleted records and filesystem-missing flags remain
separate observations. Native rating, RGB color, source tuning and cached key
fields remain available without fabricating absent metadata.

Cue positions use `position/(2*samplerate)` for mono, stereo and multichannel
sources at 44.1/48/96 kHz. Fractional positions, negative pre-roll, zero and
end-only ranges are preserved. BeatGrid1/2 and BeatMap1 use audio frames divided
by the source sample rate, independently of cue engine units and scalar BPM.
Malformed grids never become invented zero-offset grids. Legacy 16-byte grids
require an explicitly established source byte order; the host supplies none.
The pinned source dispatches both BeatGrid1 and BeatGrid2 to the same legacy
fallback. This permits byte decoding only, not canonical timing qualification.

KeyMap1 preserves all24 musical-key values, proto2 presence, signed key-change
positions, tuning and unknown fields, with raw blob references. Conflicting
blob/text/cache evidence produces a diagnostic. Unknown codecs and invalid
sample rates preserve the record and raw bytes with field issues. Duplicate hot
slots and historical black zero-time cues remain visible. Ordered playlists
preserve A/B/A occurrences; equal-position ties use native row IDs and report
ambiguity. Crates remain unordered. Auto DJ queues and history are opt-in.
All imported timings remain native/unresolved; no decoder origin is inferred
and no file tags, source database or existing outbound writer is changed.

Task11 verification: all439 dj-sync tests pass (3 ignored), including eight new
Mixxx cases. A separate existing Mixxx regression run passes134 tests (2 ignored).
All-target Clippy `-D warnings`, formatting and diff checks pass. These results
establish D2 fixture behavior; D4/D5/D6 native-app acceptance remains unrun.

## Serato readers and file-owned performance data

Serato3 reads parent-bounded legacy databaseV2/crate chunks, UTF-16BE text,
per-volume identities, nested `%%` crate names, repeated ordered occurrences,
empty crates and `neworder.pref`. Unknown fields and smart predicates remain
raw with explicit unsupported-rule diagnostics. Large track sets stage on
private disk instead of accumulating the whole library in memory. Truncated
children cannot consume a following track; configured hierarchy limits and
BOM-marked ordering text are covered.

Serato4 currently qualifies the aggregate `master.sqlite` schema12 music-table
projection, with explicit optional columns/tables. A missing keep-alive table
preserves crates; when present, retained child crates preserve their parents.
Database metadata remains authoritative. Unmounted location roots, dangling
assets and null membership references remain visible. A/B/A occurrences keep
native row identities and order. Equal-position ties retain a diagnostic.
Top-level Apple Music/Streaming Tracks pseudo-containers are excluded; native
smart rules are preserved without claiming equivalent live predicates.

The shared read-only performance decoder handles the independent handover
Markers2/BeatGrid vectors through actual ID3 GEOB, FLAC comment, MP4 freeform
and Ogg comment envelopes. Cue1250ms becomes native1.25seconds; saved loops
retain sparse indices, lock and4–8second ranges. BeatGrid intermediate BPM is
computed from native seconds/beat counts, independently of scalar BPM.
Autotags retain BPM and both gain values separately. Track color and grid lock
remain distinct native values. Unknown entries, duplicate slots and original
source blobs survive. Ambiguous duplicate tags never select a replacement by
query order. Malformed performance fields leave other decoded fields intact.

Legacy Markers_ has no decoded replacement cues and emits
`unsupported_legacy_serato_markers`. Serato5/CUELOOP, service metadata XML,
product-specific Lite differences and other aggregate schema revisions have no
qualified parser/app acceptance claim. Remote/native cache path evidence can
remain visible without reading account tables or enabling online services.
The named Serato3/4 storage fixtures establish D2 only, including optional-table
variants; no Serato application was launched. D4/D5/D6 remain unrun.

Serato performance observations require reading the referenced audio's tags,
independently of optional general tag capture. They stay separate from native
database metadata and never run DSP or write music/source databases. Unavailable
audio has an explicit performance-unavailable issue. All imported timing stays
native/unresolved until an exact originating decoder profile is qualified.
Live multi-volume source selection still requires the guarded native entry
points assigned to Task19; coherent copied-root fixtures do not grant authority
over unselected volumes.

Task12 verification:453 dj-sync tests pass (3 ignored), including14 new Serato
cases across the three reader/codec targets. The existing Serato4 writer suite
separately passes32 tests. All-target Clippy `-D warnings`, formatting and diff
checks pass. Container-envelope tests use invented metadata and the committed
independent handover vectors; the file integration uses the existing synthetic
silence FLAC fixture, never user music.

## Engine DJ reader

Engine Database2 imports use coherent selected `m.db`, `stm.db` and optional
`hm.db` snapshots. Source profiles check the Information UUID and version plus
known music-column projections. The decoder currently recognizes the2.20.2,
2.20.3,2.21.0 and3.0.0 structural profiles. Independent legacy Track-column,
PerformanceData-view and PerformanceData-table fixtures cover the projections;
other source revisions fail explicitly. No writable source helper, migration,
normalization, index change or checkpoint runs. A legacy performance view is
not executed; its known Track columns are projected directly. Other music
objects must be ordinary tables, preventing a view from reading unrelated data.

Track identity uses origin database UUID plus origin track ID, retaining local
row IDs independently. Default library identity comes from `m.db`, even when a
streaming sidecar was selected first. Conflicting copies of one origin record
require a narrower source selection; no database-order priority is invented.
Remote and unavailable media retain their metadata and playlist references.
Portable paths resolve against their captured original library directory,
independently of an unqualified decoder-origin hint. Foreign absolute paths
still require an appropriate origin or explicit folder mapping.

Beat and quickcue envelopes enforce the BE declared size and bounded zlib
inflation. Both mixed-endian beat lists, sample length bytes, flags and the
witnessed one-byte footer remain preserved. At48kHz, the independent grid
vectors retain1.25/5.25seconds and beat indices0/8, yielding120BPM; scalar
`bpmAnalyzed` stays separate. Quickcues preserve UTF-8 byte lengths, sparse slots,
empty sentinels and negative pre-roll. Loops retain original indices, raw flags,
4–8second ranges and the active-on-load bit. Invalid rates leave native sample
evidence unresolved; no44.1kHz or scalar zero-anchor grid is guessed. Malformed
performance blobs retain raw bytes and field issues without dropping tracks.

Playlist/entity chains determine order rather than row IDs. Disconnected nodes
remain represented with ordering warnings; cycles fail safely. Inherited folder
membership is preserved in raw pages and is never recreated as explicit direct
membership. Native smart predicates and parent-path evidence survive, with
unsupported live predicates and unresolved mixed smart/ordinary ordering marked.
Unqualified history schemas are opt-in and produce individually counted typed
issues and partial completeness, never a false complete baseline.

These fixtures establish D2 parser behavior only. No Engine application/device
was run; D4/D5/D6 remain unrun, and all imported timing remains native/unresolved.

Task13 verification:466 dj-sync tests pass (3 ignored), including13 Engine
reader cases and shared relative-path regressions. A separate existing Engine
writer run passes21 tests. All-target Clippy `-D warnings`, formatting and diff
checks pass. Source bytes and repeated observation identity are checked by the
independent synthetic fixtures; no native-app acceptance is implied.

## djay Pro and shared Apple archive graphs

The read-only djay reader covers the known Yap `database2` music collections,
current TSAF3.0/3.0 records and bplist00/NSKeyedArchiver shapes described for v5+
storage. It does not infer compatibility from a marketing version or inspect
account/settings collections or derived search indexes. Source blobs and optional
record metadata stay separate from decoded fields; lookup/playlist auxiliary
records are paged on private disk. Missing files, artists, parent/child references
and orphan items remain visible without diagnostic text becoming song tags.

The shared native archive decoder keeps binary-plist transport references and
NS UID references distinct, preserving ordered arrays, dictionaries, numbers,
unknown classes and binary blocks. Counts, offsets, lengths, payloads, expansion
and reference depth are bounded. Classes are never instantiated. Supported
Foundation containers normalize as data; unresolved cycles remain explicit
reference evidence and cannot become descriptive text fields. Credentials/account
key names are filtered from normalized metadata. The plist crate is used only as
an independent fixture encoder in tests, not a product parser/runtime.

TSAF decoding checks its20-byte header/class marker, aligns numeric fields to the
original blob start, bounds string/numeric pointer tables and retains URI/control
and binary-block evidence. Apple epoch0 becomes2001-01-01T00:00:00Z independently
of the host timezone. Native manual key and signed tempo exponents take precedence
while analyzed values remain separate cached observations. Scalar BPM remains
independent of variable grids; explicit alternate grids are retained with an
unsupported-field issue, rather than manufactured straight beats.

Cues retain original one-based numbers, native seconds, optional end times and
slot roles; no lossy near-loop deduplication runs. Playlist child arrays determine
hierarchy/order, with parent recovery when roots are empty; missing references
retain empty-name placeholders and raw identity. Smart filters stay opaque with
explicit loss information. All performance timing remains native/unresolved.
These synthetic fixtures establish D2 only; real djay v5/current app acceptance
and cross-platform native writes remain unrun (D4/D5/D6).

Task14 verification:479 dj-sync tests pass (3 ignored), including13 djay/archive
cases. A separate existing djay writer run passes47 tests. All-target Clippy
`-D warnings`, formatting and diff checks pass. Binary-plist fixtures use an
independent dev-only encoder; TSAF fixtures include hand-positioned absolute
alignment witnesses. No native app, music decoding or source writes occur.

## Mixed In Key 11 reader

The reader covers the supplied macOS-origin DataModel 11 structure associated
with MIK 11.2.6.7446. It checks all ten tables' column types and primary keys,
Core Data entity mappings, store UUID/type and declared model identifiers.
The handover does not contain the actual native per-entity model hashes, so
hashes are checked for structure and retained in the profile fingerprint; this
is explicitly a **structural D2 profile**, with an import diagnostic. It does
not establish a native hash allowlist, copied-library acceptance or support for
native Windows MIK storage.

Every song is imported independently of membership. Native key/BPM/whole-track
energy, cached tags, source flags, dates, descriptive metadata and raw fields
remain separate. Model-default key `0` and zero tempo/energy do not become
successful analysis. Source key and energy segments stay private provenance;
markers have no inferred pad slots, energy labels or hot/memory classification.
Duplicate and negative marker times retain their identities. Beat archives keep
the explicit ordered numeric timestamps and archive tempo separately from the
song tempo, with no inferred meter, downbeats or canonical PCM origin.

The bounded bookmark decoder recovers historical components only, with no alias
resolution, mounting, security-scope reuse or source updates. Unsupported paths,
external-volume joins and malformed archives retain the song plus diagnostics.
Foreign platform paths require explicit folder mappings, including Mac-origin
bookmarks on Linux and Windows. Synthetic mapping tests read a selected local
folder; they are not native Windows/macOS acceptance. Optional waveform objects
and the Core Data model cache are never deserialized. Import uses a coherent
read-only SQLite snapshot and performs no source migrations, DSP or file writes.

Controlled cases cover malformed/truncated bookmark offsets and TOC cycles,
unsafe components, bad/cyclic archive UIDs, unknown classes/model/entity/column
profiles, null/default values, ordered comment caches, sparse membership,
duplicate markers, orphan/cyclic folders and cancellation. Repeated reads and
source guards verify deterministic observations and unchanged source artifacts.

Task15 verification:492 dj-sync tests pass (3 ignored), including13 MIK cases.
All-target Clippy with warnings denied and formatting pass. Existing outbound
adapters, including the prior bookmark parser, retain their regression coverage.

## Direct rekordbox reader

Shared import reuses the existing runtime key provider and SQLCipher 4 direct
read boundary. Keys stay in memory; they are absent from requests, source rows
and durable operations. The generic source guard still captures the original
database/WAL identities before and after the coherent encrypted backup. Runtime
provisioning supports refresh; reopening requires local access to be provisioned
again. No XML transport, native migration or source write is introduced.

A bounded cursor visits the existing fourteen music tables and only the
`localUpdateCount` registry row. Ordinary tables and known database headers are
required. A private indexed disk spool replaces whole-library DTO expansion.
These controls establish a D2 read projection; they neither add an accepted
native write schema nor turn a structural fixture into source-app acceptance.
The existing exact application/schema write-capability checks remain unchanged.

Observations retain relational content ID, MasterSongID, UUID and native database
identity separately. Missing lookup/media/analysis records keep the track with
issues. Native/cached contentCue/file tags remain separate. All sixteen numbered
hotcue slots coexist with memory cues at the same time; Kind4 memory loops retain
their active state, unknown kinds stay raw, and the -1 loop-end sentinel is not a
zero endpoint. Full PQTZ beat rows retain their original numbers, tempos and
millisecond positions, with unresolved canonical clocks. Original ANLZ bytes,
including unknown or malformed sections, remain private blobs. Artifact labels
alone cannot associate a different captured path with a source analysis reference.

Source order, repeated playlist occurrences, smart rules, My Tags, deleted flags,
unknown fields and raw record pages remain provenance. Unlinked cue/cache/tag
rows have individual accounting issues. The desktop integration must expose
source deletion states explicitly and must not infer deletions from omissions.

An exact existing native link can identify a local track only when database,
device, content ID, UUID and MasterSongID agree. Shared import stores the legacy
reference while leaving native owner/baseline/observation state unchanged. Initial
local conflicts retain the local choice by default. All-off imports create
unprepared imported items and no Incoming preparation jobs.

Controlled tests cover all sixteen pads plus coincident memory cues, negative
positions, native loops, explicit beat arrays, cloud/original paths, missing
lookups/assets, corrupt and mislabeled analysis, committed WAL, raw/comment/cache
separation, repeated reads, row/cancellation limits, source immutability, runtime
refresh and preservation of existing native state. Native app acceptance remains
separate from these fixture results.

Task16 verification:502 dj-sync tests pass (3 ignored); the existing direct
rekordbox crate passes136 tests (1 ignored). Library import suites pass77 tests
in the default build and93 with tagging enabled, including the three direct
import/bridge/refresh cases. All three changed Rust packages pass all-target
Clippy with warnings denied; formatting and diff checks pass. The known broader
library mount limitations listed above remain outside these focused runs.

## Pioneer USB and OneLibrary reader

The device reader prefers a selected SQLCipher4 `exportLibrary.db` with
`property.dbVersion=1000` over `export.pdb`. It preserves both catalog
fingerprints and never merges conflicting catalog fields by filename. An
unsupported SQL version fails explicitly rather than silently using a stale PDB.
The existing device interoperability-key policy is reused; no account or desktop
key discovery is involved. Acquisition backs up a read-only source into a private
keyed database without invoking the outbound schema/migration helper.

PDB input streams linked pages and bounded row heaps into a disk spool. Directory,
page identity/type, declared last page, cycles, bitmaps, heap/offset overlap and
visible row boundaries are checked. Inactive offsets still bound visible rows.
Short strings, long ASCII, UTF-16LE, artist/album pointer variants, unknown fields
and opaque records retain their original bytes. Unknown packed strings produce
field issues rather than empty values. Source history is opt-in; repeated
memberships retain distinct physical/native occurrences and sparse positions.

Device-relative paths resolve under the selected device root or explicit folder
mappings. Missing media remains visible with metadata; import does not copy audio,
write tags, decode PCM or edit a device reference. DAT/EXT/2EX bytes remain private
provenance. Full PQTZ rows retain milliseconds and raw beat numbers. Extended PCO2
cues take precedence over legacy PCOB when SQL supplies no cues; SQL cue rows keep
their own authority and report the richer sidecar layer separately. Coincident
memory/hot markers are preserved; one-based ANLZ hot numbers map to zero-based
slots. Missing/corrupt performance layers retain unresolved timing and unknown
hotcue-occupancy evidence.

SQL cue times use microseconds. Normalized SQL kinds follow the existing native
writer's contract: zero is memory and positive N is hot slot N-1. This differs
from the handover's inspected Lexicon helper; raw kinds/active flags remain
available and alternate producer/device acceptance is unrun. No active-loop flag
is invented for ANLZ formats without a qualified corresponding field. All native
cue/grid clocks remain unresolved; these fixtures do not qualify D4 source-app
or D6 hardware behavior.

Task17 verification:512 dj-sync tests pass (3 ignored), including10 device-reader
cases and the unchanged native USB writer regressions. All-target Clippy with
warnings denied, formatting and diff checks pass. Controls include1024/4096-byte
pages, head pages, multiple row groups, inactive rows, pointer/string variants,
heap overflow/cycles, both catalogs, malformed/unknown fields, missing files,
cue precedence and black RGB, history selection, repeated occurrences,
cancellation and source immutability. Hardware acceptance remains explicitly
unrun.

## DJ.Studio Explorer and imported records

The reader observes the native Explorer SQL index and imported JSON/structure
records separately. The SQL profile checks the pinned column, null/default,
primary-key, unique/index and foreign-key manifest, all ten migration names and
version PRAGMAs. Unknown SQL profiles keep individually accounted raw music-table
rows; no migration or version rewrite runs. Recognized JSON records can still be
inspected. Native/renderer last-writer acceptance remains unverified and all
source write capabilities stay disabled.

SQL IDs, processed audio record keys/hashes, structure keys and project references
remain distinct. Only a unique normalized recorded path joins the two music
surfaces. Duplicate/ambiguous paths retain separate records and issues. Conflicting
SQL/JSON values are withheld from active normalization, with both immutable field
candidates retained; no unqualified metadata precedence is silently chosen.
JSON tags, original/effective key fields, analysis state, external mappings and
unknown properties remain separate provenance. Key12 maps to8A/Am; all24 numeric
keys and unknown-1/-2 controls preserve source values. Fractional SQL BPM is not
truncated despite INTEGER affinity.

Filesystem record sharding sums JavaScript UTF-16 code units modulo256 and uses
unpadded lowercase hex. Nested structure keys are retained while traversal,
absolute/NUL/unsafe keys, mismatched shards and source symlink escape are refused.
Strict bounded JSON rejects duplicate object keys, trailing junk, nonfinite
numbers and excessive depth/nodes. Malformed records remain raw blobs with
individual issues. Provenance/reference pages and private disk indexes bound
working sets rather than collecting the imported library in memory.

Active structure selection retains alternate references and explicit beat arrays.
Valid integer cue beats resolve against the selected array entry, never a guessed
BPM formula. Negative/fractional/out-of-range beats, conflicting beat/time values
and unqualified loop ends stay unresolved. Loop length is a beat-coordinate span,
not seconds. System/hot/memory arrays keep their order, holes, sentinels, removed
source types and cueKey fields; no cue RGB is invented. Array positions are marked
as unqualified external pad mappings. Native/canonical clock alignment remains
unqualified, including for processed-audio identities.

Native playlists retain duplicate membership IDs and deterministic(sort,id)
ordering explicitly labeled unverified against the app UI. Bookmarked Folders
remain filesystem references and do not become nested native playlists or trigger
a directory scan. Projects stay untouched references; derived caches, Chromium
storage, music, images, stems and authentication configuration are not mutated.
Read acquisition performs no renderer/IPC invocation, audio decoding or analysis.

Task18 verification:526 dj-sync tests pass (3 ignored), including13 DJ.Studio
integration cases and a deterministic source-JSON mutation-during-SQL-backup
control. All-target Clippy with warnings denied, formatting and diff checks pass.
Controls cover all24 keys and unknown sentinels, SQL/JSON divergence, UTF-16
surrogate sharding, alternate structures, loop beat units, array holes/removed
cues, malformed/duplicate/trailing JSON, unknown migrations/renderer, duplicate
paths, missing structures, paged invalid/project provenance, symlink refusal and
unchanged source/cache bytes. These are D2 controls; app reopen/write acceptance
remains unrun and production DJ.Studio writes remain disabled.

## Disposable DJ.Studio existing-record experiment

The optional `experimental-djstudio-write` Cargo feature exposes a laboratory
only. It is disabled by default and is not enabled by the desktop dependency.
Its root capability can only be minted by an internal generator creating a new
empty temporary directory; it cannot adopt a selected user library. Generated
evidence is retained for inspection. There is no production writer constructor
or Tauri command, and every receipt reports production writes disabled.

The experiment owns existing title/artist/comment metadata and the SQL key plus
JSON effective Camelot key only. It preserves analyzed state, original/note/MIK
keys, tag-cache keys, beatgrids, cue arrays/keys, external identities and project
records. It never inserts an analyzed audio identity. The SQLite schema and
partial records are authored public fixtures, not an installable app export.

An exclusive lease and native process check require the fixture to be closed.
Root/control identities, native/renderer marker and the entire generated file
inventory are guarded. Preparation backs up every fixture file, stages an
isolated SQLite transaction with integrity/foreign-key checks and stages exact
JSON patches. Journals and replacement temporaries stay outside recursively
scanned record tables. Publication replaces JSON and SQLite, invalidates only
the affected audio aggregate cache and verifies canonical SQL/JSON readback.
Guarded rollback restores byte-exact original persistence and refuses later
external edits. Idempotent retry and interrupted publication/recovery are
exercised with the original runtime capability; this does not establish
cross-process user-root recovery or native application acceptance.

Native DJ.Studio reopen (D5) and arbitrary cue/grid writes remain unrun and
unavailable. Compiling the experiment does not promote any source capability.

Task18b verification: eight experiment tests pass, including eleven publication
fault boundaries, interrupted rollback, later external edits, exclusive lease,
stale before-hashes, changed root, cancellation and native/renderer mismatch.
The default dj-sync suite remains526 passed,0 failed,3 ignored. All-target
Clippy with warnings denied passes with the experiment enabled. The desktop
feature tree does not enable the experiment; formatting and diff checks pass.
