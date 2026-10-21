# Not available yet

CueRithm 0.1 is early software. This page lists what is missing or unfinished, so nothing in the guide over-promises.

Native [DJ software Sync](../guides/sync.md) is available in the current worktree for
VirtualDJ, Engine DJ, djay Pro, Serato DJ Pro 4 and Pioneer USB. Acceptance in native DJ apps and on physical players remains separate from
implementation and fixture tests.

## Not in the app

| Item | Status |
| --- | --- |
| **Direct rekordbox sync** | Not available as a desktop sync target. Pioneer USB export is a separate drive workflow. The internal foundations (reading a copy of a rekordbox 7 database) have been validated on one rekordbox version, 7.2.19, read-only. rekordbox 6 has not been tested and nothing is written to rekordbox. The design is direct database sync only: CueRithm will not add an XML transport. |
| **Standalone playlist import/export** | Native playlist export is part of [DJ software Sync](../guides/sync.md). Standalone M3U/XML export and playlist import remain unavailable. |
| **Variable-tempo grids and downbeat detection** | The grid is one BPM and one first beat. |
| **Per-segment energy and energy-labelled cues** | Deliberately not offered. |
| **Plays, last played and danceability** | Deliberately excluded. |
| **Other OneTagger workflows** | Spotify, quick tag and renamer are not included. |
| **Backup retention controls** | Original-file backups are retained. Manual Cleanup has Undo, and Sync history offers guarded Restore backup and interrupted-operation recovery. General retention controls are unfinished. |
| **An audio device picker** | Playback uses the system default output. |

Manual corrections, maintenance, compatible-track search, reference-tracklist matching, playlist flow and local library backup have dedicated [guides](../guides/index.md). Their implementation and fixture checks do not establish acceptance on every native platform.

## Known limitations

- Traxsource may require a separately hosted [FlareSolverr-compatible solver](../guides/catalog-solver.md). Live fetching and parsing were verified with Trawl; universal catalog coverage and packaged-app acceptance remain unverified. Beatport has one successful direct API probe.
- Decoding stops at about 24 minutes, so long mixes do not prepare.
- There is no Intel Mac build. Windows and Linux builds are unsigned, and their native playback and restart acceptance is not complete.
- The library schema is version 22. Older app versions refuse a newer library, and there is no downgrade.
- Published figures measure agreement with reference tools on my own library. Held-out accuracy, real-music energy agreement and large-library performance are not measured. See [How accurate is it?](../technical/accuracy.md).
- There is no auto-update.

## What may come

Direct rekordbox 6 and 7 sync and more complete native application, device, Windows and Linux acceptance remain open. Additional automation is planned separately; no dates are promised.
