# Native direct rekordbox adapter

Status: implementing the direct-sync scope in the approved library-workspace
specification. This plan preserves the complete 6/7 requirement. The encrypted
reader checkpoint is not a completed sync implementation. No collection XML
transport or fallback is part of this adapter.

Release decision, 2026-10-19: pause feature expansion to stabilize and release
the integrated application. Existing backend checkpoints remain tested but the
desktop Sync action stays disabled. The remaining steps below are deferred,
not completed; see [release scope and outstanding gaps](../../release-readiness.md).

## Evidence boundary

Use `docs/research/rekordbox-direct-contract.md` and the retained private
Lexicon 1.11.15 extraction. Installed7.2.19.0342 has a coherent inspected copied
database schema; no rekordbox 6 installation is available on this host. Publish schema
identities and authored fixtures, not personal rows, keys or proprietary code.
Keep live libraries, application bundles and analysis assets intact during probes.

## Work sequence

1. Implement bundled encrypted read-only access and coherent encrypted snapshots.
   Verify actual copied rekordbox 7 schema against the original reader, generated WAL-only
   commits, invalid keys/plain inputs, read-only mutation refusal, nonzero foreign
   key reports, cancellation/ownership and the supported compiler floor. Keep the
   shared SQLite dependency compatible with the existing library. This stage is
   implemented; current-head regression/platform verification is ongoing.
2. Add target/version/location discovery and a runtime key provider independent
   of Lexicon, Node or Python. Keep key material redacted and out of serialized
   settings/diagnostics. Report Linux local-target unavailability accurately.
   Main-process absence alone is insufficient: the background agent can retain
   and mutate the WAL.
3. Import typed metadata, identities and source/asset relationships. Scope links
   by database identity and preserve target UUIDs/master-song IDs. Add structured
   per-field conflict baselines to Cuerithm; never infer ownership from names or
   modification timestamps alone. Validate actual schema profiles before writes.
4. Implement native binary analysis readers/writers for cues, loops, constant or
   variable grids and artwork references. Preserve opaque/unowned chunks and use
   the same playable clock as preview. Compare original helper results and
   authored binary fixtures before target-app acceptance.
5. Add a concrete change preview and staged writer for metadata/foreign tables,
   cue/content-cue relationships, analysis references,32-bit IDs and verified
   local-update semantics. Coordinate encrypted DB and asset backups/publication
   through recoverable journals. Reject stale sources/targets and preserve
   unowned rows/assets and subsequent edits. Skip unchanged syncs completely.
   Never kill an application or delete its WAL/SHM files as a shortcut.
6. Add playlist hierarchy/order, MyTag categories/links and exact native smart-rule
   capabilities. Keep unsupported rules explicit, with an approved ordinary-list
   materialization option; preserve the full Cuerithm rule. Intelligent-rule
   payloads inside direct records do not add collection XML transport.
7. Expose native settings/preflight/preview/apply/history/conflict/recovery APIs
   for the user-owned branded UI. Preparation queues pending sync; it does not
   continuously write into an open target. Exercise closed-target/reopen tests
   for actual rekordbox 6 and 7, idempotence, metadata, MyTags, playlist order/native rules,
   cue/loop/grid playback, artwork, conflicts, rollback and restore. A successful
   SQL commit, generated fixture or read-only schema match is not that proof.

## Current checkpoint

The Rust adapter and research inspector can open an encrypted copy and validate
an owned standalone encrypted snapshot without exposing key material or changing
the input. No target writer, live-write schema allowlist or desktop Sync command
is enabled. Continue through the remaining sequence; do not redefine completion
around the reader or substitute XML, plaintext fixtures or a scripted external
runtime for the requested native direct implementation.
