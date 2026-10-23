# Undo a merge or restore removed files

<p class="lead">Undo a duplicate merge, restore files you removed as unused, and retry anything that was interrupted. All of it happens in History &amp; recovery, and the history survives restarts.</p>

**Before you start:** you need a merge from [Find and merge duplicate tracks](find-duplicates.md) or a removal from [Find and remove unused files](clean-up-unused-files.md) to undo. Scans change nothing, and **Missing & broken** has no action, so neither has anything to undo.

## Open History & recovery { #open }

Choose **Maintenance** under TOOLS in the sidebar and open the **History & recovery** tab. The note at the top reads *History is saved across restarts. Undo and Restore check current files and library state before changing anything.* Select **Refresh history** after another view changed something.

The tab has four sections: **Recovery**, **Duplicate merges**, **Unused-file removals and retained originals** and **Backup disposal history**. When something needs you, a button on the other Maintenance tabs reads, for example, *1 recovery item needs attention* and opens this tab.

## Undo a duplicate merge { #undo-merge }

Under **Duplicate merges**, find the entry **Duplicate merge** with its time and counts, such as *1 archived · 0 restored · 0 updated · 1 playlists*. Select **Undo merge**. The message reads *Merge undone. Original files and library state restored.*

Undo puts back what the merge changed:

- The archived tracks return to **Tracks**.
- Every playlist the merge touched has its old order again.
- If you copied tags and cues from a donor, the keeper's file gets its original bytes back and the library gets its old tags, cues and grid. CueRithm kept those bytes for this reason.

The entry then reads **Undone by** followed by an ID, and it cannot be undone twice. The undo itself is listed as **Merge undo**, which has no button of its own. To merge again, scan again.

### When Undo is refused { #refused }

Undo checks that nothing relevant has changed since the merge, because putting old data over newer work would lose it. Edits to unrelated tracks, new tracks arriving in Incoming and smart playlists refreshing do not block it.

| Message | Why | What to do |
|---|---|---|
| *Later relevant edits prevent Undo* or *Later track edits prevent Undo* | You edited, re-tagged or archived one of the merged tracks after the merge | Undo is no longer possible for that merge. Fix the tracks by hand |
| *Later playlist edits prevent Undo* | A playlist the merge changed was edited afterwards | Put the playlist back as it was after the merge, or fix it by hand |
| *Source changed; Undo needs review* | The keeper's file was modified by another app or by CueRithm after the merge | Undo is blocked to protect your newer file |
| *Original backup changed* | The retained original of the keeper's file is damaged or gone | Undo cannot restore the file bytes |
| *Duplicate operation is not available for Undo* | It was already undone | Nothing to do |

An older merge recorded by an earlier build, which only archived tracks, uses a stricter rule: *Later library or playlist changes prevent Undo* appears as soon as anything in the library changed.

## Restore files you removed { #restore-files }

Under **Unused-file removals and retained originals**, each removal reads **File removal · committed** with its file count and total size. Expand **Reviewed paths** to see every file, with its role, and every folder that was removed. Select **Restore files**. The message reads *Files restored to their original locations.*

Restore recreates the missing folders and copies the retained originals back to their original paths. It stops, and overwrites nothing, when:

- a newer file already sits at an original path, because restore refuses an occupied path;
- a retained original is missing or changed;
- the originals were [disposed of](#dispose).

The entry becomes **File restore** and reads **Restored by** an ID. A restored file is a normal file again: if no library track uses it, it shows up as unused in the next scan.

## Recover an interrupted operation { #recover }

If CueRithm quits or loses power in the middle of a merge or a removal, it rolls the operation back the next time it starts. A file you changed in the meantime is never overwritten. It is kept, the operation is marked as a conflict with its backups retained, and the entry appears under **Recovery** or in the removals list with the error shown.

Fix the cause first: free the path, reconnect the drive or put the newer file where you want it. Then select the retry button on the entry:

| Button | Where |
|---|---|
| **Retry merge recovery** | **Recovery** |
| **Retry file recovery** | A removal whose state is not *committed*, or that has an error or *Cleanup pending* |
| **Retry disposal recovery** | **Backup disposal history** |

Retry never overwrites an unknown later version of a file. It reports the error again if the cause is still there.

## Free disk space by disposing of retained originals { #dispose }

Every removal keeps verified copies of its files in CueRithm's data folder so that **Restore files** works. They stay until you dispose of them. See [Where CueRithm keeps your data](where-data-lives.md#what-is-in-the-folder) for the location.

1. Tick **Select retained originals** on each removal whose originals you no longer need.
2. Select **Review backup disposal**. The preview **Dispose of retained originals** lists every backup file with its size, and a warning names the removals that will lose Restore.
3. Tick **I understand retained originals will be permanently disposed.**
4. Select **Permanently dispose reviewed backups**, or **Keep retained originals** to back out.

!!! danger "Disposal cannot be undone"
    After disposal the removed files cannot be restored. Only the retained originals of unused-file removals can be disposed of here. The size shown is the reviewed amount. Your disk may free less, for example while cleanup is pending or when the volume keeps snapshots.

The original-file backups made by tagging and cleanup are not managed here. **Settings → Automatic tagging** shows their size under **Recovery backups**, and CueRithm keeps them without a limit; see [Where CueRithm keeps your data](where-data-lives.md#space).

## Undo things outside Maintenance { #elsewhere }

| You want to undo | Go to |
|---|---|
| A cleanup | [Undo a cleanup](../../tagging/undo-cleanup.md) |
| A tag or artwork edit | [Undo a tag change](../../tagging/tag-history.md) |
| Renamed or moved files | [Recover or undo a reorganisation](../../rename/undo-organize.md) |
| A sync to DJ software | [Preview, apply and restore a sync](../../sync/preview-apply-restore.md) |
| An archived track | Select it in **Archive** and choose **Restore**, once its file is in place. See [Archive tracks you no longer play](../archive.md) |
| The whole library state | [Restore your library from a backup](restore-library.md) |

## What can go wrong { #problems }

| What you see | Why | What to do |
|---|---|---|
| **Undo merge** is missing from an entry | It is a **Merge undo** entry, or already undone | Scan and merge again if needed |
| *Cleanup pending* beside a removal | Temporary files could not be deleted yet | Select **Retry file recovery** |
| *Another maintenance operation is still running* | Only one operation runs at a time | Wait, or select **Cancel operation** |
| History shows an error banner for one section | That section could not be read | Select **Refresh history** |

## Related

- [Find and merge duplicate tracks](find-duplicates.md)
- [Find and remove unused files](clean-up-unused-files.md)
- [Restore your library from a backup](restore-library.md)
- [Where CueRithm keeps your data](where-data-lives.md)

<!-- verified: apps/desktop/src/views/MaintenanceHistory.tsx, Maintenance.tsx, MaintenanceShared.tsx; apps/desktop/src/test/maintenance.test.tsx; crates/cuerithm-library/src/maintenance/donors.rs (undo checks and messages), merge.rs (version 1 journal rule), removal.rs (restore conditions), removal/retention.rs; docs/development/maintenance-api.md; docs/development/desktop.md (tag and cleanup originals have no retention control); legacy guides/maintenance.md re-checked. -->
