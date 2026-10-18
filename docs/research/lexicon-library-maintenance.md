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
