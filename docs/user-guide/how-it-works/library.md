# How is the library stored?

<p class="lead">CueRithm keeps its library in one SQLite database, with caches and recovery files beside it. Your music stays where it is; the database stores each file's location and everything CueRithm learned about it.</p>

!!! abstract "What this means for you"
    - **The library holds locations, not audio.** Moving or renaming a music file outside CueRithm can leave its local binding unresolved. **Missing & broken** reports it; reviewed **Relocate** reconnects it while keeping its track identity. See [Find missing, moved or broken files](../files/library/find-missing-files.md).
    - **Originals are kept, without a limit.** Every file CueRithm rewrites leaves its original bytes in the data folder. General retention controls are unfinished, so these backups grow until you remove them. See [Where CueRithm keeps your data](../files/library/where-data-lives.md).
    - **Back the library up with Library backup**, and your music with your usual backup. A library backup does not contain music. See [Back up your library](../files/library/back-up-library.md).
    - **The schema is version 29.** An older CueRithm refuses a newer library, and there is no downgrade.

## What kind of database it is { #the-engine }

- SQLite, bundled with the app, in a file named `library.sqlite` in the application data folder.
- **WAL** journal mode, synchronous FULL, foreign keys on and a 5-second busy timeout.
- An application ID marks the file as a CueRithm library. A non-empty SQLite file without it is rejected.
- The schema version (`user_version`) is currently 29. Migrations are embedded SQL files applied in order, each in an immediate transaction. A newer schema is refused with "Library schema is newer than this application". Migrations preserve existing tracks, jobs and cue ownership.

## Where things live { #where-things-live }

| Item | Location in the data folder |
|---|---|
| Library database and its `-wal`, `-shm` and `.lock` files | The legacy top-level location, or `libraries/<generation-id>/` |
| Active-library pointer and activation journals | `active-library.json` and manager-owned files in the data folder |
| Embedded key model, installed once and verified | `analysis-components/` |
| Decode scratch folders (cleaned at start), extracted cover art, waveform caches | `cache/media/`, `cache/artwork/`, `cache/waveforms/` |
| Original-file backups from tag writes, cleanup and manual edits | `tagging/`, `cleanup/`, `metadata-edits/` |
| Recovery records for file organisation, maintenance and sync | `organization-references/`, `maintenance/`, `sync/` |
| Library backups and scheduled restore requests | `library-backups/` |
| Your music | Wherever you keep it. Portable source locators and optional local bindings refer to it |

The data folder's location on each system, and which folders are safe to leave alone, are on [Where CueRithm keeps your data](../files/library/where-data-lives.md).

## What is stored { #core-tables }

| You see | Stored in | Notes |
|---|---|---|
| A track | `tracks` | One stable track identity: optional local path, title, artist, album, genre, search text and sort keys, an optional known file SHA-256, `state` (*incoming* or *ready*), `archived`, the full analysis, key, BPM, energy, duration, and tagging and cleanup provenance |
| A stage in Incoming | `jobs` | State (`queued`, `running`, `attention`, `completed`, `superseded`), stage, error, attempts, the source hash and the settings revision. Unique per track, source hash and settings revision |
| Cues | `cue_revisions`, `accepted_musical_revisions` | Native cue publication and independently accepted imported/manual musical data, with explicit selected revisions |
| Portable history and source observations | `portable_domain_records`, `portable_library_meta` | Validated logical data and provenance retained across migrations; operational jobs and write permissions are excluded |
| Corrections and templates | `manual_publication_operations`, `manual_publication_state`, `cue_templates` | Previews, history and named templates |
| Settings | `settings`, `selected_settings` | Immutable snapshots identified by a SHA-256, plus a pointer to the current one |
| Cached analysis | `preparation_analysis` | Reused when source, settings, engine and decoder match |
| Tag-write journal | `tag_writes`, `tag_write_files` | What was written, and how to roll it back |
| Preferences | `library_preferences` | Time zone, key compatibility mode and the automatic smart playlist options, in their own revision-checked record |
| Playlists, cleanup, Curate, maintenance, organisation | Many tables | Members and edits, cleanup plans and batches, custom tags and categories, file plans |
| DJ software | `native_records`, `native_track_links`, `sync_links` | Stored copies of the records read from a DJ library, kept as the baseline for conflict detection |

A global `revision` counter increases on every change, which is how the interface knows to refresh. Each job pins the settings revision it was queued under, so changing Settings never alters a job already in flight.

The DJ-software copies can be large. With a big rekordbox collection they can make the database several times bigger than the track table alone. <!-- Measured once on a maintainer library: the DJ-software copies were about half of a 253 MB database; not a published benchmark -->

## Paths across systems { #paths-across-systems }

Machine-local paths use native bytes (`unix:` bytes or `windows:` UTF-16). They are optional bindings, not the track’s identity. Portable locators retain the source path flavour and exact representation, plus a source root and relative components when they can be represented safely.

A foreign path never becomes a local binding just because it looks familiar. [Portable restore](../files/library/restore-library.md#connect-folders) combines an explicitly chosen local folder with safe relative components and verifies each file. Unresolved or unrepresentable names keep their metadata and playlist memberships.

## One process at a time { #one-process-at-a-time }

CueRithm takes an exclusive lock on `library.sqlite.lock`. A second copy gets "This library is already open". A short retry tolerates a decoder child process that briefly inherited the lock handle.

## Recovery at startup { #recovery-at-startup }

- Jobs that were *running* go back to *queued*.
- A tag write, cleanup, duplicate merge, unused-file removal, file organisation or metadata edit that was interrupted before it finished is rolled back or completed from its journal.
- Leftover decode scratch folders are deleted.

## Where your settings are stored { #settings-storage }

Everything in the preparation settings (watch folders, cue settings, tagging, cleanup and the enable flags) is in the database. Library preferences and automatic smart playlist settings share a separate revision-checked record, so saving them does not change preparation settings. Display preferences (waveform style, theme, column layout, sidebar width and player height) are stored in the interface's local storage on your computer, outside the library.

## Backups and retention { #backups }

CueRithm keeps the original bytes of every file it changes, under `tagging/`, `cleanup/` and `metadata-edits/`. A write that changes nothing keeps no backup. The total is visible as **Recovery backups** on **Settings → Automatic tagging**. There is no retention control yet; the only explicit disposal is for the backups of unused-file removals, in [Undo a merge or restore removed files](../files/library/undo-maintenance.md).

[Portable library backup](../files/library/back-up-library.md#create) writes logical `.cuerithm` records from a coherent SQLite snapshot, including committed WAL data. It excludes music, caches, runnable jobs, executable Undo journals, credentials and machine bindings.

Restore builds and validates a separate library generation. The app restarts so every old handle closes before activation. The prior library is retained; failed validation or activation leaves a validated previous library available. [Retained-library recovery](../files/library/restore-library.md#go-back) selects a complete generation, without merging later edits. Restored libraries do not resume old preparation or enable external writes.

The separate [same-installation snapshot](../files/library/back-up-library.md#installation-snapshots) keeps the older database-and-consistency-manifest workflow, with strict local resource checks. Neither format copies music or reverses tag edits or DJ-software changes.

## Where you see it in the app { #where-you-see-it }

| Place | What shows |
|---|---|
| **Maintenance → Missing & broken** | Tracks whose stored path no longer points at a readable file; from here you can remove their records or reconnect them |
| **Library backup** (sidebar) | Create a backup, and restore one |
| **Settings → Library preferences** | Time zone and key compatibility mode, saved in the library |
| **Settings → Automatic tagging → Recovery backups** | Size and file count of the retained originals |

## Related

- [Where CueRithm keeps your data](../files/library/where-data-lives.md)
- [Back up your library](../files/library/back-up-library.md)
- [I moved or renamed my files](../help/troubleshooting/files-moved.md)
- [How are tags written safely?](tag-writing.md)

<!-- verified: crates/cuerithm-library/src/store.rs; migrations/029_portable_identity.sql; portable.rs; musical.rs; generations.rs; backup/spool.rs; library_backup.rs; apps/desktop/src-tauri/src/host.rs; portable_commands.rs; tests/library_generations.rs; tests/portable_backup.rs. Existing upstream analysis/recovery descriptions retained; portable bindings, generation storage and schema updated from current source. -->
