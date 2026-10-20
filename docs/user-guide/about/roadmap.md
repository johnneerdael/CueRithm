# Not available yet

CueRithm 0.1 is early software. This page lists what is missing or unfinished, so nothing in the guide over-promises.

Native [DJ software Sync](../guides/sync.md) is available in the current worktree for
VirtualDJ, Engine DJ, djay Pro, Serato DJ Pro 4 and Pioneer USB. Final verification,
PR review and merge remain pending; acceptance in native DJ apps and on physical
players has not been established.

## Not in the app

| Item | Status |
| --- | --- |
| **Direct rekordbox sync** | Not available as a desktop sync target. Pioneer USB export is a separate drive workflow. The internal foundations (reading a copy of a rekordbox 7 database) have been validated on one rekordbox version, 7.2.19, read-only. rekordbox 6 has not been tested and nothing is written to rekordbox. The design is direct database sync only: CueRithm will not add an XML transport. |
| **Duplicate and file-maintenance screens** | Not built. Duplicate detection, missing-file and unused-file scans exist as internal code and have no interface yet. |
| **Standalone playlist import/export** | Native playlist export is part of [DJ software Sync](../guides/sync.md). Standalone M3U/XML export and playlist import remain unavailable. |
| **Cue editing and a cue template editor** | Cues are read-only. There are no custom anchors and no tempo-range option. |
| **Variable-tempo grids and downbeat detection** | The grid is one BPM and one first beat. |
| **Per-segment energy and energy-labelled cues** | Deliberately not offered. |
| **Plays, last played and danceability** | Deliberately excluded. |
| **Other OneTagger workflows** | Spotify, quick tag and renamer are not included. |
| **Backup retention controls** | Original-file backups are retained. Manual Cleanup has Undo, and Sync history offers guarded Restore backup and interrupted-operation recovery. General retention controls are unfinished. |
| **An audio device picker** | Playback uses the system default output. |

## Known limitations

- Traxsource lookups have not been confirmed to work from every network. Requests returned HTTP 403 on the validation machine. Beatport has one successful probe, which does not establish universal coverage.
- Decoding stops at about 24 minutes, so long mixes do not prepare.
- There is no Intel Mac build. Windows and Linux builds are unsigned, and their native playback and restart acceptance is not complete.
- The library schema is version 12. Older app versions refuse a newer library, and there is no downgrade.
- Published figures measure agreement with reference tools on my own library. Held-out accuracy, real-music energy agreement and large-library performance are not measured. See [How accurate is it?](../technical/accuracy.md).
- There is no auto-update.

## What may come

Direct rekordbox 6 and 7 sync, the maintenance screens, a cue template editor and more complete Windows and Linux acceptance are the gaps I list in the release notes. There are no dates.
