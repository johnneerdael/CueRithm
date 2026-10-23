# Restore your library from a backup

<p class="lead">Replace your library's current playlists, cues, analysis and settings with a saved backup. The restore runs the next time CueRithm starts, and your current library is kept first. Plan about five minutes plus a restart.</p>

**Before you start:** you need a backup made by [Back up your library](back-up-library.md) on this computer, with the same CueRithm version. Restore replaces the library's records only. It does not touch music files, undo tag edits written into them, or change anything in DJ software.

!!! warning "Gotcha"
    Everything you did in CueRithm after the backup is replaced: tracks added, playlists changed, settings and presets edited. CueRithm saves your current library as a pre-restore backup first, so you can go back; see [Go back to before the restore](#go-back).

## Check the backup { #inspect }

1. Choose **Library backup** under TOOLS in the sidebar.
2. Select **Inspect backup folder**.
3. In the picker (*Select a CueRithm backup folder*), choose the dated backup folder itself, the one named `CueRithm-` followed by a time stamp. Do not choose the folder that contains it.

**Restore preview** appears with the backup's date, a track count such as *26 tracks*, one of two messages, and the backup's path:

- *Music locations and retained recovery files match this backup.* Every music file is where and what the backup expects.
- *14 source files were unavailable when saved and remain unavailable.* Those files were already missing when the backup was made, and they must still be missing.

Inspecting runs every check a restore runs. If it fails, an error message names the reason and no restore button appears. See [When a restore is blocked](#blocked).

## Schedule the restore { #schedule }

1. Read the line *Your current library is kept as a pre-restore backup. This restore is applied before the library opens on your next launch.*
2. Tick *Replace my current preparation state with this backup on the next launch.*
3. Select **Restore on next launch**.

The panel changes to **Restore scheduled**. While a restore is scheduled, **Create backup** and **Inspect backup folder** are off. To change your mind, select **Cancel scheduled restore** at any time before you restart. It removes the copy CueRithm staged.

## Apply it { #apply }

Finish anything that is running, quit CueRithm completely and open it again. During start-up, before the library opens, CueRithm checks the files again, saves your current library as a pre-restore backup, and swaps in the backup's records. Then it opens as normal with the restored library.

If any check fails at that point, CueRithm opens your current library unchanged and keeps the request on screen. Nothing is retried on its own.

## When a restore is blocked { #blocked }

A blocked request shows **Restore blocked**, the reason, and *Your current library was left unchanged. Fix the reported file consistency problem and retry, or cancel this restore.* Choose **Retry on next launch** once you have fixed the cause, or **Cancel scheduled restore** to carry on with your current library.

Most reasons come down to one rule: **every music file and recovery file the backup recorded must be exactly as it was when the backup was made.** That keeps a restore from putting back records that describe files that no longer exist.

| Message | What changed since the backup | What to do |
|---|---|---|
| *Backup source locations have changed. Music files and tag edits are not restored* | A recorded music file was moved, replaced or deleted, or a file that was missing has come back | Put the files back as they were, or make a newer backup. See [Find missing, moved or broken files](find-missing-files.md) |
| *A source file changed since its saved preparation. Reprepare it before using this backup* | A recorded music file was edited, for example re-tagged | Use a newer backup |
| *Retained recovery resources or models have changed; this database-only backup cannot be restored safely* or *Organization or metadata recovery resources, retained originals or native identities changed; this backup cannot restore them* | CueRithm wrote tags, ran cleanup, reorganised files, removed unused files or synced since the backup | Use a newer backup. Work done after a backup usually rules it out |
| *This backup requires the same CueRithm version, platform and library installation* | You updated CueRithm, or the backup came from another computer or library | Make a new backup with the current version |
| *Backup library schema differs from the current library* | The library format changed | Make a new backup with the current version |
| *Backup contains unexpected entries*, *Backup database hash does not match its manifest* or *Backup SQLite integrity check failed* | The backup folder was edited, renamed or damaged | Use another copy of the backup |
| *A library restore is already scheduled* | An earlier request is still waiting | Cancel it first |

If a restore stops in the middle of the database update, CueRithm will not open the library again until its pre-restore backup can be validated. This is rare. Report it through [Report a problem](../../help/report-a-problem.md) and keep the data folder untouched.

## Go back to before the restore { #go-back }

Each restore keeps the library it replaced as a normal backup in CueRithm's data folder, under `library-backups/pre-restore-…`. Inside that folder is a `CueRithm-…` backup folder. To return to it, select **Inspect backup folder**, choose that inner folder and follow the same steps. CueRithm does not delete pre-restore backups. See [Where CueRithm keeps your data](where-data-lives.md#what-is-in-the-folder) for the location on each system.

## Related

- [Back up your library](back-up-library.md)
- [Where CueRithm keeps your data](where-data-lives.md)
- [Undo a merge or restore removed files](undo-maintenance.md)
- [Find missing, moved or broken files](find-missing-files.md)

<!-- verified: apps/desktop/src/views/LibraryBackup.tsx; apps/desktop/src/test/library-backup.test.tsx (labels, restore blocked, retry, cancel); crates/cuerithm-library/src/library_backup.rs (inspect checks and messages, stage_library_restore, cancel, apply_pending_library_restore, pre-restore-<id>-<uuid> folder, blocked-before-mutation, mutation_started); crates/cuerithm-library/tests/library_backup.rs (restore replaces settings; changed source blocks; returned file blocks); apps/desktop/src-tauri/src/lib.rs (restore applied before Host::open); legacy guides/library-backup.md re-checked. The 14-file message is shown with an illustrative count -->
