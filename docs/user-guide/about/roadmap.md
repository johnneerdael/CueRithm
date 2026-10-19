# Not available yet

CueRithm 0.1 is early software. This page lists what is missing or unfinished, so nothing in the guide over-promises.

## Not in the app

| Item | Status |
| --- | --- |
| **Direct rekordbox sync** | Not available. The **Rekordbox** sidebar button is disabled with the tooltip *Direct rekordbox sync is not available in this build*. The internal foundations (reading a copy of a rekordbox 7 database) have been validated on one rekordbox version, 7.2.19, read-only. rekordbox 6 has not been tested and nothing is written to rekordbox. The design is **direct database sync only**: CueRithm will not add an XML transport. |
| **Duplicate and file-maintenance screens** | Not built. Duplicate detection, missing-file and unused-file scans exist as internal code and have no interface yet. |
| **Playlist import and export** | None. Playlists live in the library. |
| **Cue editing and a cue template editor** | Cues are read-only. There are no custom anchors and no tempo-range option. |
| **Variable-tempo grids and downbeat detection** | The grid is one BPM and one first beat. |
| **Per-segment energy and energy-labelled cues** | Deliberately not offered. |
| **Plays, last played and danceability** | Deliberately excluded. |
| **Other OneTagger workflows** | Spotify, quick tag and renamer are not included. |
| **Backup retention controls** | Original-file backups are kept indefinitely. Only manual Cleanup has Undo. |
| **An audio device picker** | Playback uses the system default output. |

## Known limitations

- **Traxsource.** A live lookup has not been shown to succeed from every network (HTTP 403 on the validation machine). Beatport has one successful probe, not universal coverage.
- **Very long tracks.** Decoding stops at about 24 minutes, so long mixes do not prepare.
- **Platforms.** There is **no Intel Mac build**. Windows and Linux builds are unsigned, and their native playback and restart acceptance is not complete.
- **Database.** The library schema is version 11. Older app versions refuse a newer library, and there is no downgrade.
- **Accuracy.** Figures are agreement with reference tools on the developers' own library. Held-out accuracy, real-music energy agreement and large-library performance are not measured. See [How accurate is it?](../technical/accuracy.md).
- **Signing.** Windows and Linux packages are unsigned. There is no auto-update.

## What may come

Direct rekordbox 6 and 7 sync, the maintenance screens, a cue template editor and more complete Windows and Linux acceptance are the stated gaps in the project's release notes. There are no dates.
