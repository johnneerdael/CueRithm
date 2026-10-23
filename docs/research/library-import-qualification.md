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
