# Roadmap and compatibility

CueRithm already analyses, tags, cleans and organises music, manages playlists, and
syncs prepared libraries to seven DJ destinations. This page separates remaining
limits from work already delivered and work in development. Check a download’s
release notes for the features included in that build.

## Available now

[DJ software Sync](../guides/sync.md) writes to **rekordbox 6 / 7, VirtualDJ,
Engine DJ, djay Pro, Serato DJ Pro 4, Pioneer USB and Mixxx**. Windows library paths
and explicit Wine / Proton setups on Linux are supported, including drive mappings
and saved configurations. Mixxx uses its native library on macOS, Windows and Linux.

Direct rekordbox writes use database and asset compatibility checks on macOS,
Windows and Wine / Proton. Compatible rekordbox 6 / 7 patch builds can sync; unrecognized formats remain
read-only.

| Application checks completed | Recorded result |
| --- | --- |
| **rekordbox 6.8.7.0303 and 7.2.19.0342 on macOS** | Both apps reopened the synced libraries and displayed metadata, artwork, grids, cues and loops. Created FLAC tracks played and looped. Native readback, repeat-sync, restore and conflict checks also passed. |
| **Mixxx 2.5.6 on macOS** | 240 checks passed across fresh, existing, replaced, newly created and additional-format libraries. Mixxx loaded the libraries, grids and keys and preserved the synced rows. |
| **Debian Mixxx 2.5.0 on Linux, in Docker** | 105 checks passed for MP3, WAV, FLAC, AAC, ALAC, AIFF, Ogg Vorbis and Opus. The real Mixxx loaded the library, grids and keys and preserved the synced rows; MP3 timing was measured through libmad. |

The Linux Mixxx run used the sync harness on macOS with Linux timing, then opened
the result in the real Linux Mixxx. It establishes functional Linux Mixxx acceptance;
CueRithm’s desktop UI, discovery and process checks on Linux were outside that run.
See [Sync to Mixxx](../guides/sync-mixxx.md#what-has-been-tested) for the exact scope.
Windows/Wine DJ-app playback and physical-player acceptance remain separate checks.

[File organization](../guides/file-organization.md) already includes a **renamer**,
metadata-based naming patterns, move/copy operations, linked-library path repair,
previews, recovery and Undo. Manual corrections, maintenance, compatible-track
search, reference-tracklist matching, playlist flow and local library backup also
have dedicated [guides](../guides/index.md).

## Not in the app

| Item | Status |
| --- | --- |
| **Unrecognized rekordbox formats** | Compatible 6 / 7 builds can write on macOS, Windows and Wine / Proton; unknown database or required asset formats remain blocked. Pioneer USB export is a separate drive workflow. Collection XML transport is intentionally excluded. |
| **Traktor and legacy Serato sync** | Separate destinations that are not yet available. Serato DJ Pro 4 sync is included. |
| **Standalone playlist import/export** | Native playlist export is part of [DJ software Sync](../guides/sync.md). Standalone M3U/XML export and playlist import remain unavailable. |
| **Variable-tempo grids and downbeat detection** | The grid is one BPM and one first beat. |
| **Per-segment energy and energy-labelled cues** | Deliberately not offered. |
| **Plays, last played and danceability** | Deliberately excluded. |
| **Other OneTagger workflows** | Spotify and quick tag are not included. File renaming is available through [File organization](../guides/file-organization.md). |
| **Backup retention controls** | Original-file backups are retained. Manual Cleanup has Undo, and Sync history offers guarded Restore backup and interrupted-operation recovery. General retention controls are unfinished. |
| **An audio device picker** | Playback uses the system default output. |

## Known limitations

- Traxsource may require a separately hosted [FlareSolverr-compatible solver](../guides/catalog-solver.md). Live fetching and parsing were verified with Trawl; universal catalog coverage and packaged-app acceptance remain unverified. Beatport has one successful direct API probe.
- Decoding stops at about 24 minutes, so long mixes do not prepare.
- There is no Intel Mac build. Windows and Linux builds are unsigned, and their native playback and restart acceptance is not complete.
- The library schema is version 28. Older app versions refuse a newer library, and there is no downgrade.
- Published figures measure agreement with reference tools on my own library. Held-out accuracy, real-music energy agreement and large-library performance are not measured. See [How accurate is it?](../technical/accuracy.md).
- Mixxx has not been run on Windows. Linux acceptance covers Debian’s 2.5.0 package; other packages and FAAD-based AAC decoding remain unverified. The tested macOS and Debian Mixxx builds cannot play WMA.
- There is no auto-update.

## In development

### Bring an existing DJ library into CueRithm

Full DJ-library import is being developed so an existing managed database can become
the starting point for managing music in CueRithm. The intended workflow covers the
[import mechanisms offered by Lexicon](https://www.lexicondj.com/manual/import-options):

- Import a whole library or selected playlists and their tracks.
- Merge libraries from different DJ applications, updating existing entries and adding new ones.
- Replace a library when starting over is the intended action.
- Choose which fields to import, including cues, with BPM and beatgrids selectable separately.
- Back up the current library before importing.

Coverage is planned across the DJ software destinations and **Mixxx**.
**DJ.Studio is a potential additional integration**, still under consideration.
This import work extends the current sync from CueRithm to DJ software.

### Relocation, portable backup and Lexicon migration acceptance

Reviewed track relocation, `.cuerithm` logical export, root mapping, retained
library replacement and the Lexicon v26 database-backup parser are implemented.
See [Library backup](../guides/library-backup.md) and
[Maintenance](../guides/maintenance.md). Music is copied separately. Replacing
independent libraries is supported; merging them remains separate planned work.

Acceptance across real macOS, Windows and Linux computers, the original private
Lexicon backup and large libraries is still being validated. Synthetic profile
and path-flavor fixtures do not establish those runtime results.

Broader DJ-application, physical-device and platform acceptance also continues.
No delivery dates are promised.
