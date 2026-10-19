# Library maintenance port

Status: implementing the maintenance scope added to the approved library
workspace specification on 2026-10-18. This plan does not mark that scope
delivered. UI branding remains user-owned.

## Required behavior

Port the requested local-library features found in Lexicon 1.11.15:

- Duplicate discovery using exact decoded-audio digests and five artist/title
  tolerance modes. Label the match method; metadata similarity is not audio
  identity. Support library, selected-playlist, and library-results-filtered-to-
  selected-playlist scopes.
- Review groups with a chosen keeper, independent tag/cue donor, locks and
  persisted ignores. Offer folder, cue-count, bitrate and text preferences.
  Preserve the explicit exclusion of streaming, Plays and Last Played.
- Copy tags/cues without archiving, or replace ordinary-playlist references
  before archiving eligible other records. Keep smart definitions and evaluated
  memberships coherent. Do not delete music when merging duplicate records.
- Find unreferenced disk files recursively within an explicitly chosen folder;
  provide all/include/exclude extension filters, sizes and export. Keep this
  distinct from missing library records.
- Find missing records and optionally corrupt audio. Disconnected, inaccessible,
  changed and unsupported sources need separate states; a failed access check
  does not prove corruption or absence.
- Separate scanning, review and apply. Bind reviewed changes to identities,
  current references, settings and relevant revisions, rejecting stale plans.
  Coordinate file metadata changes with the existing recoverable publication
  journal, and provide durable undo for library/archive/reference changes.

## Architecture and boundaries

Use a pure `cuerithm-maintenance` crate for normalization, matching and review
selection. Inputs contain explicit metadata and availability verdicts; this
crate opens no files and performs no mutations. It does not reuse a key/BPM
estimate as a duplicate fingerprint. Use the existing bundled media runtime for
the separate decoded-audio pass.

The library owns scan snapshots, current file observations, fingerprint cache,
persisted ignores and guarded plans. Never hold the SQLite mutex during decode
or directory traversal. Support cancellation and bounded comparisons; cancellation
or an exhausted work budget must not publish an apparently complete scan.

Decoded-audio digest identity includes the source content hash, decoder/runtime
identity and decoding parameters. Do not label a perceptual similarity score as
the exact digest match found in the inspected source. A cached fingerprint whose
source changed is unusable, even when size and timestamp remain unchanged.

All filesystem discovery is read-only. Do not traverse directory symlinks or
assume case folding is a correct file-identity operation on every filesystem.
An unused-file removal plan rechecks root containment, current file identity,
library references and exclusions immediately before applying. Removal remains
an explicit reviewed operation with a recovery strategy; a scan or duplicate
merge never authorizes it implicitly.

Native commands expose typed snapshots and plan tokens. UI integration follows
the library backend, preserving the user's branding work and existing player,
selection, import and preparation behavior.

## Execution

### 1. Matching reference and pure engine

- Capture authored inputs/outputs from selected helpers in the actual shipped
  runtime; publish only test metadata, results, hashes and dependency provenance.
- Cover all five tolerances, duration chains, aliases, version/bracket handling,
  multilingual normalization, greedy group order and keeper/donor selection.
- Add failing Rust reference and invalid-input tests before implementation.
- Preserve exact helper behavior and record intentional policies such as input
  validation, bounded work and deterministic cross-platform collation separately.
- Run debug, release, Clippy and the supported Rust floor; verify pinned notices.

### 2. Read-only scanners and scope

- Add exact decoded-audio fingerprints, per-file status, cache invalidation,
  progress and cancellation. Test generated audio with different tags/containers,
  silence, short/long inputs, non-audio files, video and source changes.
- Port metadata/exact-group precedence and all three playlist scopes. Preserve
  unavailable members for review without selecting them as a keeper.
- Add unused and missing/corrupt scanning with generated directory fixtures,
  sidecars, symlinks, unreadable locations, disconnected roots and path variants.
- Require unchanged source hashes and no changes to audio, metadata, analysis,
  cues, playlist membership or preparation jobs from scans. Persisting owned
  scan/cache state is allowed and must not invalidate unrelated library inputs.

### 3. Durable review and publication

- Persist ignores and coherent review snapshots. Guard manual keeper/donor,
  lock, ignore and preference operations against stale group identities.
- Test duplicate merge/copy and playlist-reference replacement across manual,
  archived and smart references. Preserve order and avoid duplicate membership.
- Bind file metadata work to the existing staging/journal/PCM-proof boundary.
- Test restart, later edits, source A/B/A, transaction failure and durable undo.

### 4. Native delivery and acceptance

- Expose complete typed backend operations without copying Lexicon assets or
  proprietary implementation code. Hand contracts to the user-owned UI work.
- Exercise the complete review/apply/undo flow in an isolated generated library.
- Run current-head desktop builds on macOS, Windows and Linux and inspect any
  failed checks. Test cleanup alongside playback, incoming work and playlists.
- Commit/push separately; consolidate only reviewed changes into the shared
  baseline. Do not claim live-library acceptance from pure fixtures.

## Evidence

See `docs/research/lexicon-library-maintenance.md`. Initial source reference
coverage is tracked in `crates/cuerithm-maintenance/tests/fixtures/` and the
private execution ledger. The rest of the application's direct rekordbox sync,
live Traxsource validation and real-music scalar energy validation remain tracked
requirements; completion of this plan does not substitute for those deliveries.
