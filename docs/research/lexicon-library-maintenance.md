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
