# Back up your library

<p class="lead">Save a dated copy of your playlists, cues, analysis and settings before a big change. A backup takes a minute or two on a small library and longer on a large one. It does not include your music.</p>

**Before you start:** let [Incoming](../follow-incoming.md) finish, so the header reads *All up to date*. A backup refuses to run while work is in progress.

## What a backup holds { #what-it-holds }

| In the backup | Not in the backup |
|---|---|
| Every track record: its stored analysis (key, BPM, beatgrid, energy), cues and file location | Your music files. Keep a separate backup of them |
| Playlists, folders and smart playlists | Tag and artwork changes written into music files |
| Preparation settings and presets, including watch folders | Changes made in DJ software |
| Tag, cleanup, Curate and maintenance history | Provider sign-ins and keys. They live in your system's credential store |
| | Theme, waveform style, sidebar and player sizes and table columns. See [Where CueRithm keeps your data](where-data-lives.md#outside) |
| | The original-file backups CueRithm made when it wrote tags. They stay in the data folder |

The backup records where each music file is and a checksum of it. It uses these records later to check that restoring is safe. Files that were already missing are noted as unavailable and stay unavailable.

## Create a backup { #create }

<!-- figure:existing-library-backup-back-up-library -->
<figure markdown>
![Library backup with a restore preview and the player still visible](../../assets/screens/library-backup.png){ .screen loading=lazy }
<figcaption>Create a backup, and review a restore before it is scheduled.</figcaption>
</figure>
<!-- /figure -->

1. Choose **Library backup** under TOOLS in the sidebar.
2. Select **Create backup**.
3. In the folder picker (*Choose where to save the backup*), pick a folder on a disk you trust, not inside your music library.
4. Wait while *Checking library and file consistency…* shows. CueRithm reads every music file to check its checksum.

When it finishes, **Backup saved** shows the time, a track count such as *26 tracks*, and the path of the new backup. The count is the number of distinct music file locations the backup covers. Inside the folder you chose, CueRithm created a folder named `CueRithm-` followed by a time stamp and an ID, containing two files, `library.sqlite` and `manifest.json`. The note reads *The private location manifest stays inside this folder. Keep both files together.*

A backup does not change your library. If it fails, your library is as it was.

## Why a backup can be refused { #blocked }

CueRithm refuses to back up a library that is half-way through something, because the backup would capture an inconsistent state.

| Message | Why | What to do |
|---|---|---|
| *Finish preparation and resolve tagging, cleanup or maintenance recovery before using a library backup* | Tracks are being prepared, or a tag, cleanup or maintenance write is unfinished | Wait for Incoming to settle and check [History & recovery](undo-maintenance.md#recover) |
| *Finish metadata saves, accepted preparation reviews and auxiliary artifact cleanup before using a library backup* | A metadata save or review is pending | Open **Drafts and history** in Tracks and finish or discard it |
| *Finish organization and resolve its preparation, Copy-source or native-reference recovery before using a library backup* | A file reorganisation is unfinished | Finish it in [Organize files](../../rename/organize-files.md) |
| *Finish or recover native rekordbox sync before using a library backup* or *Resolve or discard pending DJ sync previews before using a library backup* | A sync is unfinished | Apply, restore or discard it in [Preview, apply and restore a sync](../../sync/preview-apply-restore.md) |
| *A source file changed since its saved preparation. Reprepare it before using this backup* | A music file was edited outside CueRithm after it was prepared | Find it with [Missing & broken](find-missing-files.md#outcomes) (**· Source changed**) and choose [Analyze again](../../analysis/analyze-again.md) |
| *A retained recovery backup is missing or changed* | A file in the data folder that Undo depends on was altered | Do not touch the data folder. See [Where CueRithm keeps your data](where-data-lives.md) |
| *Backup exceeds the supported size limit* or *Too many library backup dependencies* | The database is over 1 GiB or the library tracks more than 50,000 files and recovery items | Contact support through [Report a problem](../../help/report-a-problem.md) |

## Keep the backup safe { #keep }

- Keep `library.sqlite` and `manifest.json` together, unedited and in their folder. A backup with a renamed, missing or extra file is rejected.
- Copy the backup folder to a second disk if the first holds your music.
- Make a fresh backup after a large batch of changes. Restoring needs every music file listed in the backup to be exactly as it was, so an older backup stops being usable once those files are re-tagged or moved. [Restore your library from a backup](restore-library.md#blocked) explains.

A backup works only with the library installation, CueRithm version and operating system that made it. It does not move your library to another computer, and CueRithm does not delete old backups for you. A portable backup is listed under *In development* in [What CueRithm does not do yet](../../help/not-available-yet.md).

## Related

- [Restore your library from a backup](restore-library.md)
- [Where CueRithm keeps your data](where-data-lives.md)
- [Undo a merge or restore removed files](undo-maintenance.md)
- [Find missing, moved or broken files](find-missing-files.md)

<!-- verified: apps/desktop/src/views/LibraryBackup.tsx, LibraryBackup.css; apps/desktop/src/backup-api.ts, backup-types.ts; apps/desktop/src/test/library-backup.test.tsx; apps/desktop/src-tauri/src/backup_commands.rs; crates/cuerithm-library/src/library_backup.rs (folder name CueRithm-<created>-<uuid>, library.sqlite and manifest.json, quiet_database and settled error messages, location/missing handling, 1 GiB / 50,000 limits, sources/models/resources/organization inventory); crates/cuerithm-library/tests/library_backup.rs; crates/cuerithm-credentials/src/native.rs (system credential stores); ia.md Pending product changes; legacy guides/library-backup.md re-checked (legacy named Curate history and presets; both are in the database). The 26 tracks are illustrative -->
