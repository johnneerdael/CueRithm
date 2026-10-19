# Lexicon library maintenance discovery

Inspected installed Lexicon 1.11.15 (build 1.11.15.0), using an existing private
copy. ASAR SHA-256:
`85fdbba4af97886ff1a626e6a4c96b4f9866dca3624fb2348a0332d7d6b63860`.
These findings come from static shipped-code inspection, not execution of a
cleanup against a music collection. No extracted code or app assets are included.

## Confirmed features

| Feature | Shipped behavior and source anchors |
| --- | --- |
| Duplicate tracks | `DuplicateTrackScanner`, renderer 324266; exact decoded-audio digest groups plus artist/title matching. Full library, selected playlists, or full-library results filtered to selected playlists; ignored paths and persisted ignored tracks. |
| Duplicate review and merge | `DuplicateTrack`, renderer 323969; keep a chosen copy, lock/ignore groups or tracks, copy tags/cues without archiving, or replace playlist references and archive other eligible records. Processing does not directly delete the audio files. `execute`, 324169; review confirmation, 283336. |
| Preferred copy | `prefer`, 324308; folder, most cues, highest bitrate and text-based preferences. Preselection accounts for availability, cues and quality, and avoids copy-suffixed filenames where possible. Source also contains streaming/play-count preferences; these do not expand Cuerithm's approved local-library scope or reintroduce excluded play statistics. |
| Unused files | Scanner `Mt`, renderer 258907; recursively compare a chosen folder's files with library paths. All/include/exclude extension modes, size totals, selection, text export and a separate confirmed disk-deletion action. This means disk files not referenced by the library. |
| Missing/corrupt tracks | `BrokenTrackScanner`, renderer 258204; non-streaming library records, missing-file detection and optional corrupt-audio checking. This is distinct from unreferenced files on disk. |

## Duplicate matching details

The fingerprint worker is `639.index.worker.js` (loader renderer 226040).
It invokes bundled FFmpeg with `-i PATH -vn -f md5 -`, caches the returned digest,
and uses a 30-second process timeout. MP4 video detection skips video streams
except MJPEG/ANSI. Missing files and symlink-like files are skipped. This is an
exact decoded-audio checksum path; broad perceptual audio matching is not
established by this code.

The audio-scan eligibility path uses durations strictly above 15 seconds and
below 1,500 seconds (renderer 324723). Artist/title scanning has five tolerance
modes. None uses normalized exact names plus duration groups with adjacent
members within five seconds; Medium uses broader normalization; Low/High modes
use trigram Dice scores. Low's threshold is 0.9; High uses 0.7, reduced to 0.5 for
short titles with at most six trigrams. A separate High mode strips bracketed
title text. See `findDuplicatesByArtistTitle`, renderer 324573, and title-trigram
helper `H`, 324207. Remaining normalization helpers need reference cases before
claiming full matching parity.

Exact audio groups and metadata candidates must remain distinguishable. Metadata
similarity alone is not proof of identical audio or safe automatic cue transfer.

## Unused-file boundaries

The scanner excludes known DJ/application directories, Apple metadata files and
associated sidecar patterns before comparing normalized paths. It refuses broad
system/user/volume roots in its folder-selection UI. Source path comparison folds
case and uses additional macOS Unicode handling (renderer 258968).

The results page's deletion handler (renderer 290311) confirms the selected file
count and bytes and then uses a permanent-delete operation, with a report. This
inspection did not invoke deletion. The Rust port must bind actions to reviewed
file identities and folder boundaries, preserve currently referenced files and
distinguish inaccessible/disconnected locations from genuinely missing records.
Cross-platform path and recovery behavior require their own acceptance tests.

## Port delivery boundary

The user requested these functions on 2026-10-18 after confirming manual metadata
cleanup should update files and library with undo. Add duplicate review/merge,
unused-file scanning and missing/corrupt-track maintenance to the Cuerithm scope.
Playlist-reference replacement depends on the planned playlist store. File
removal is a separately reviewed user action; scans and duplicate archiving do
not imply permission to remove source audio. Current findings establish that the
features are present, not that their Rust port or native UI is already complete.

## Matching checkpoint (2026-10-19)

The independent `cuerithm-maintenance` crate now implements the five metadata
tolerances and keeper/tag-source preselection. Its reference fixture contains
1,342 authored cases captured from the installed Electron Node 20.18.1 runtime,
including all 641 upstream transliteration characters. The installed ASAR and
private source-copy hashes agree with the identity above. The fixture records
selected-factory and capture hashes; proprietary helper code stays private.

Coverage includes exact versus broad artist aliases, remix/version normalization,
short-title Dice thresholds, greedy matching/group ordering, five-second adjacent
duration chains, Unicode case/whitespace behavior, copy-suffixed filenames,
availability and cue-count/bitrate keeper preferences. References use controlled
file-presence, path and extension helpers; they do not establish actual filesystem,
decode, directory traversal or application-wide scan parity.

The normalizer uses the separately downloaded MIT slugify 1.6.6 character map,
with provenance/notices retained. The Rust port pins ICU root collation for stable
cross-platform artist ordering; the captured English-locale cases agree, while
parity across every possible reference application locale is not established.
Legacy JavaScript case folding and whitespace differ from Rust regex defaults;
explicit fixtures cover those differences.

Invalid/duplicate candidate IDs and nonfinite measurements are rejected. Fuzzy
matching has a work limit and returns an error without partial groups when that
limit is exhausted. Metadata candidates do not suggest automatic cue transfer;
only an explicitly exact-audio group may suggest a separate donor. Suggestions
perform no file or database writes.

This is the pure matching checkpoint. Library fingerprint orchestration, persisted
ignores/review, playlist replacement, durable merge/undo, unused-file and
missing/corrupt scanners, and native delivery remain incomplete. The full port
plan is `docs/superpowers/plans/2026-10-19-library-maintenance.md`.

## Native fingerprint boundary (2026-10-19)

`MediaRuntime::fingerprint` now provides the separate native-rate/channel
signed-16 PCM MD5 path needed for exact duplicate candidates. It hashes a
verified private source snapshot, prioritizes default audio disposition and
then channel count (first on ties), and retains the native rate/layout. A generated
two-stream MOV verifies this choice against the original binary. It does not reuse
the resampled stereo playback cache. Returned data pins the encoded source
SHA-256, decoder identity, algorithm, PCM format, rate, channels, selected audio
stream ordinal and audio-stream count. Empty decoded audio is not a candidate.

The minimal FFmpeg 9.0.2 build now enables its MD5 muxer and signed-16 PCM
encoder. Sealing checks these capabilities as well as existing playback,
artwork, decoder and system-linkage requirements. The recipe identity is
`cuerithm-swr44100-stereo-f32-native-md5-v2`. Existing runtime-cache keys include
the changed recipe/scripts; no Homebrew decoder is used by the application.

Four generated WAV cases, spanning 8/44.1/48/96 kHz and mono/stereo, agree with
digests produced by the installed Lexicon's FFmpeg 7.0. Across the repository's
14 generated common-format fixtures, the new runtime produces valid digests for
all 14 and matches that reference decoder for 12. AAC/ADTS and AAC/M4A differ
between the two decoder builds. Therefore a fingerprint cache must include the
decoder identity; this checkpoint does not establish cross-version AAC digest
parity. Lossless containers with identical playable PCM produce the same digest.
The authored WAV reference/provenance is under `crates/cuerithm-media/tests/`.

Cancellation is checked during hashing, copying, probing and decoding. Probe and
fingerprint processes each have the inspected worker's 30-second limit and bounded
output; child processes are killed and reaped on cancellation or limits. The
private snapshot is removed on success or error. Final validation rejects source
or snapshot changes, including a path replaced by a symlink to identical bytes.

Actual video streams remain separate from audio and embedded pictures. The
check applies to stream contents, even under a misleading extension, and accepts
attached artwork. Those are explicit reliability policies beyond the inspected
worker's MP4-extension-only video check. Strict decoding reports failures instead
of retaining the source worker's swallowed command errors. No scan mutates an
audio file or library record.

This implements the media boundary. Library scan orchestration, persisted cache,
scopes, unused/missing/corrupt results, review and merge/undo remain pending.

## Read-only duplicate scan orchestration (2026-10-19)

The library now captures coherent duplicate inputs and evaluates selected smart
playlist scopes freshly. Full library excludes archived rows; explicit ordinary
playlist scope retains its archived members; full-library filtering retains
outside copies in a group that intersects selected playlists. These scans do not
publish smart membership or change metadata, cues, audio, preparation jobs or
ordinary playlist references. Owned fingerprint-cache and scan-report persistence
are separate operational state under schema7.

Source batching uses ascending imported track IDs (renderer322120). Cuerithm
uses stable SQLite insertion order, preserving this policy across its UUID IDs
and avoiding title-sort changes affecting greedy matching. Fingerprint groups
precede metadata candidates; later groups lose members used by earlier groups.
Metadata candidates retain unavailable copies for review when another member is
present, and only available copies can be suggested as keepers. Match methods
remain explicit. Audio fingerprint eligibility keeps the inspected strict
15-to-1500-second duration bounds; unknown durations skip that pass.

Cache reuse requires a fresh complete encoded SHA-256, decoder/runtime identity
and fingerprint implementation identity. A same-size edit with restored mtime
invalidates the cache. Source content and availability are checked again before
report persistence and when loading a saved report. Relevant library, playlist,
preference or ignore changes mark reports stale. Cancellation or matcher work
limit exhaustion cannot publish a complete report.

Ignored folders resolve existing filesystem aliases before component-wise
comparison. This handles macOS /var aliases and avoids treating a sibling with a
shared text prefix as an ignored child. It intentionally preserves actual
filesystem case semantics instead of applying Lexicon's universal lowercasing.
A missing leaf under an accessible existing parent is reported missing; when its
parent is absent or inaccessible, availability stays unknown. This conservative
policy avoids asserting that disconnected music was deleted.

Generated integration fixtures exercise these boundaries, including restart,
changed file identities and fresh smart rules without cached members. This is
backend scanner evidence. Review/edit/merge/undo, unused/missing/corrupt tools,
native commands and their UI acceptance remain incomplete; no user collection
was scanned or modified by these tests.

## Unused-file and missing/audio-validation backends (2026-10-19)

The library now provides persistent, read-only unused-file reports for an
explicit folder, with all/include/exclude extension modes, sizes and path-list
export. Active and archived library references are protected. Native filesystem
identity checks handle case aliases and hard links while retaining distinct files
on case-sensitive volumes. Directory symlinks are skipped, including symlink
roots, and own application storage, inspected DJ directories, Apple metadata and
associated Serato/VirtualDJ stems are excluded. Trash, recycle-bin, AppleDouble,
Spotlight and file-event directories are pruned, while file-only metadata names
do not suppress ordinary music directories. Broad system/home roots require
a narrower selection. Source stem suffix/prefix rules are reproduced; universal
case folding of every pathname is intentionally replaced with filesystem checks.

Reports retain directory-entry snapshots and complete SHA-256 proofs for their
candidates. Changed contents, new or removed entries, changed references and
library edits invalidate the reviewed snapshot. Discovery/read errors produce
explicit partial reports and issues. Cancellation does not persist a completed
report. Physical reference checks open only short-lived handles, keeping the
scan from exhausting descriptors on larger libraries. No files are removed.

The separate missing-track scan checks non-archived local records. Optional full
audio validation uses strict native fingerprint decoding and records the decoder
identity. Availability-only results mean present, not proven decodable. Missing,
unavailable, symlink, nonregular, changed, video, no-audio, unsupported codec/layout,
timeout and decode failure remain distinct outcomes. Supported audio failures
are reported as decode errors rather than asserting a particular cause of
corruption. Capabilities come from the actual bundled FFmpeg codec registry,
shared lazily across runtime clones; cancelled capability probes do not populate
the cache. Unsupported results also verify the captured source identity.

Generated fixtures cover archived references, recursive filters, stem sidecars,
symlinks, physical aliases, own-storage exclusion, same-size/restored-mtime edits,
disappearing candidates, cancellation, missing/reappearing records and valid,
malformed and unsupported audio. Native UI wiring, reviewed file removal,
duplicate review/merge, playlist replacement and durable undo remain unfinished.
These backends have not been accepted against the user's live music collection.


## Durable review and native command checkpoint (2026-10-19)

The independent review engine now implements separate keeper/tag-source choices,
lock/unlock/copy-only transitions, track ignores, and folder, most-cues, highest-
bitrate, title and Extra1 preferences. Optional best-cue donor selection runs only
when the chosen keeper changes, following the inspected generator. Overrides
can unlock a matching locked group even when it already has the chosen keeper.
No play-count or streaming preference was added.

An additional 164 authored cases were executed against the shipped
`DuplicateTrack` methods (renderer323969 onward) and scanner `prefer` generator
(renderer324308 onward), using installed Electron Node20.18.1. Their complete
selection/state outputs match Rust. `lexicon-review-v1.json` records the source,
function and capture hashes and the controlled presence, ID and normalized-POSIX-
path adapters. These fixtures do not exercise the original execute/mutation path.

Library reviews persist the captured group inputs, source/scope proofs and an
independent edit revision. Restart retains choices. Stale scans, source changes,
library/scope edits and concurrent review edits are rejected. Explicit ignore
changes update the persistent ignore set atomically with that review; its own
ignore revision is adopted while other reviews become stale. Ignored or
unavailable manual keepers/donors are rejected. If an existing selection becomes
ignored, it stays visible but is not actionable. A separate stamped ignore list
allows restoring ignored tracks after the original review becomes stale.

Native commands and a typed TypeScript bridge now expose duplicate/unused/broken
scans, report reload/export, duplicate review and persistent ignore management.
Long operations run on blocking workers with one maintenance slot, progress,
operation-specific cancellation and source-proof rechecking. Cancellation retains
the slot until the worker exits. Shutdown cancels active work and waits for it to
finish so owned decoder children can be reaped. Saved-report reads and unused
exports accept the caller's cancellation token as well as initial scans.
See `docs/development/maintenance-api.md` for the UI handoff contract.

This checkpoint does not expose duplicate apply, file removal or undo. Review
choices and ignores do not change audio tags, cues, archives or playlist members.
Generated library tests cover persistence, stale/concurrent edits, cancellation,
ignore restoration and unchanged content. Branded maintenance screens, merge/
copy publication, playlist-reference replacement, durable undo, reviewed disk
removal and native end-to-end acceptance remain required deliveries.

A packaged macOS test app with a separate identity, generated four-track library
and unreferenced WAV exercised the real Tauri invoke/event path: exact duplicate
scan, persisted keeper/donor edits, lock/copy-only transition, ignore/restore,
stale-review reload, unused scan/export, strict validation and acknowledged
cancellation. It returned the expected states and retained all generated audio.
This is isolated native command acceptance, not branded maintenance-screen or
live-library acceptance. Windows/Linux validation of these new commands remains
separate from earlier desktop builds.


## Merge planning and archive/reference transaction stage (2026-10-19)

The independent planner now expands selected unlocked/copy-only groups into
keeper, archive, unarchive and donor-copy actions. It preserves ignored members,
keeps copy-only groups free of explicit archive actions, and rejects locked,
unavailable, unknown or overlapping selections. Forty authored cases execute the
original `DuplicateTrack.execute` method with persistence recorders and agree on
those actions. The fixture records the actual method/capture/source hashes.
It does not execute the original metadata import or file writer.

An additional 485 authored cases execute the shipped `replaceTrackIds` method
(renderer320164 onward) with a controlled query/persistence adapter. In an
affected playlist the old keeper entry is removed and the first removed-copy
slot becomes the keeper; other removed-copy entries disappear. Unaffected
playlists keep their keeper position. The adapter supplies rows in playlist
position order. The original query has no explicit row order, so these references
do not establish its real SQLite index-dependent ordering in every library.
Cuerithm explicitly uses playlist position and compacts the resulting positions.

The library can persist a merge preview showing all donor-copy tasks and ordinary
playlist changes. Its archive/reference-only transaction stage commits reference
replacement, archive transitions, preview consumption and a durable undo record
in one SQLite transaction. Generated tests inject failures during apply and undo,
verify rollback/retry and preserve all source hashes, metadata, analysis and cues.
Archived selected-playlist keepers can be restored while ignored copies retain
their records and memberships. Smart definitions stay unchanged, cached results
become stale, and refresh evaluates the new archive state after apply and undo.

Undo history is discoverable after restart. A supported versioned journal retains
its implementation identity for provenance without requiring an identical newer
executable. Unsupported journal versions are rejected. Current undo guards use
the complete captured input stamp and file proofs; later changes, including an
archive A/B/A cycle, prevent restoration. This is conservative: unrelated library
input changes also prevent undo and need a narrower relevant-entity policy before
claiming the complete concurrent-ingestion workflow accepted.

Donor-copy plans remain blocked before any mutation until coordinated native-file
publication is connected. They never silently degrade to library-only tag/cue
copies. No native apply/undo command is registered at this checkpoint; the new
native endpoints expose preview and history only. This stage is a prerequisite
for the complete merge/copy workflow, not its delivery claim.

Static `Track.importTags` inspection (renderer311581 onward) also shows separate
nonempty-list gates for cues, tempo markers and custom tags. Empty donor lists
retain the destination lists. BaseTrack import exclusions preserve identity,
location and technical stream properties. The storage whitelist includes additional
application state, so the full persisted import policy and native field mapping
must be validated before connecting donor publication. Play statistics and other
user-excluded audio-feature fields remain outside Cuerithm's scope.


The earlier desktop run exposed a Windows unused-scan failure: an absolute
`AppData` ancestor excluded the entire selected temporary folder and yielded an
apparently complete empty report. The scanner now applies AppData/Application
Support name pruning within the explicitly chosen tree, while retaining absolute
own-storage/system exclusions and Serato ancestor protection. A protected selected
root returns an error. A native macOS reproduction of the same ancestor pattern
failed before the fix and passed afterward; all fourteen file-maintenance tests
pass locally. Current-head Windows verification is still required.
