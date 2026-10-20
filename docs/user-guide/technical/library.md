# The library database

CueRithm keeps everything it knows in a single SQLite database, with a small set of caches beside it. Your music files stay where they are.

## The engine

- SQLite (bundled with the app, via `rusqlite`), file name `library.sqlite`, in the application data folder.
- **WAL** journal mode, synchronous FULL, foreign keys on, a 5-second busy timeout.
- An application ID marks the file as a CueRithm library (a non-empty SQLite file without it is rejected).
- Schema version (`user_version`) is currently 12. Migrations are embedded SQL files applied in order, in an immediate transaction. A newer schema is refused ("Library schema is newer than this application"), and there is no downgrade. Migrations preserve existing tracks, jobs and cue ownership.

## Where things live

| Item | Location |
| --- | --- |
| Library database and its `-wal`, `-shm` and `.lock` files | The data folder |
| Embedded key model, installed once and verified | `analysis-components/` |
| Decode scratch folders | `cache/media/` (cleaned at start) |
| Extracted cover art | `cache/artwork/` |
| Waveform caches | `cache/waveforms/` |
| Original-file backups from tag writes and cleanup | `tagging/<workspace>/` |
| Your music | Wherever you keep it. The database stores absolute paths. |

On macOS the data folder is `~/Library/Application Support/com.cuerithm.app`. See [Install](../get-started/install.md#where-cuerithm-keeps-its-data).

If you move or delete a track outside CueRithm, its stored path breaks. A scan for missing files exists in the code but its screens are not finished.

## Core tables

| Table | Purpose |
| --- | --- |
| `tracks` | One row per file: path, title, artist, album, genre, search text and sort keys, the file's SHA-256, `state` (*incoming* or *ready*), `archived`, the full analysis, measured key, BPM, energy, duration, tagging and cleanup provenance |
| `jobs` | Preparation jobs: state (`queued`, `running`, `attention`, `completed`, `superseded`), stage, error, attempts, unique per track, source hash and settings revision |
| `cue_revisions` | Append-only cue history. Each completed preparation inserts a new revision. |
| `settings`, `selected_settings` | Immutable settings snapshots identified by a SHA-256, plus a pointer to the current one |
| `preparation_analysis` | The analysis cache |
| `tag_writes`, `tag_write_files` | The tag-write journal |
| `library_preferences` | Revision-checked time zone, key compatibility and opt-in automatic Smart Playlist settings |
| `automatic_playlist_nodes`, `automatic_playlist_state` | Managed playlist identities and the last reconciled input, preference and playlist revisions |
| Playlist, cleanup and maintenance tables | Playlists with members and edits, cleanup plans and batches, custom tags and categories |

A global `revision` counter increments on every change, which is how the interface knows to refresh.

Each job pins the settings revision it was queued under, so changing settings never alters a job already in flight.

## Paths across systems

Paths are stored as raw bytes with a platform prefix, so non-UTF-8 names survive: `unix:` bytes, or `windows:` UTF-16.

## One process at a time

CueRithm takes an exclusive advisory lock on `library.sqlite.lock`. A second process gets "This library is already open". A short retry tolerates a decoder child that briefly inherited the lock handle.

## Recovery at startup

- Jobs that were *running* go back to *queued*.
- Any tag-write, cleanup or other journal still in the *prepared* state is rolled back.
- Leftover decode scratch folders are deleted.
- If the application build lacks tagging support, it refuses to open a library with pending journals rather than ignoring them.

## Settings storage

Everything in the preparation settings (watch folders, cue settings, tagging, cleanup and the enable flags) is in the database. Library preferences and automatic Smart Playlist settings share a separate revision-checked database record; saving them does not change preparation settings. Display preferences (waveform style, theme, column layout, sidebar width and player height) are stored in the interface's local storage on your computer.

## Backups

Original bytes of every file that CueRithm changes are kept indefinitely under `tagging/`. There is no retention control yet. The size is visible as *Recovery backups* on the tagging settings page.
