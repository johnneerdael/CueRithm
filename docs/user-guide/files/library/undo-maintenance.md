# Undo a merge or restore removed files

<p class="lead">Undo a duplicate merge, put back library records you removed, restore files you removed as unused, and retry anything that was interrupted. All of it happens in History &amp; recovery, and the history survives restarts.</p>

**Before you start:** you need a merge from [Find and merge duplicate tracks](find-duplicates.md), a repair from [Find missing, moved or broken files](find-missing-files.md) or a removal from [Find and remove unused files](clean-up-unused-files.md) to undo. Scans change nothing. A [relocation](find-missing-files.md#undo-relocation) has its own **Previous relocations** list and Undo controls in the reconnection workspace. A [portable restore](restore-library.md#go-back) uses retained-library recovery instead.

!!! note "Which version"
    **Missing-file repairs** and their Undo need CueRithm 0.1.15 or later. Merges that delete files, and one shared limit of 10 removals across both kinds, need CueRithm 0.1.16 or later. In 0.1.15 the limit of 10 counts Missing & broken removals only.

## Open History & recovery { #open }

Choose **Maintenance** under TOOLS in the sidebar and open the **History & recovery** tab. The note at the top reads *History is saved across restarts. Undo and Restore check current files and library state before changing anything.* Select **Refresh history** after another view changed something.

The tab has five sections: **Recovery**, **Duplicate merges**, **Missing-file repairs**, **Unused-file removals and retained originals** and **Backup disposal history**. When something needs you, a button on the other Maintenance tabs reads, for example, *1 recovery item needs attention* and opens this tab.

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

### Undo a merge that deleted files { #undo-delete }

A merge that used **Delete from disk** has an entry that also reads, for example, *1 removed from the library · 1 file moved to the Trash*, plus *· 1 permanently deleted* or *· 1 not deleted* when that applies. A file that was not deleted is listed with its reason. Select **Undo merge**. The message reads *Merge undone. The removed library records are back. Files moved to the Trash stay there until you restore them.*

Undo puts back the library records, their cues, tags, links and history, their places in playlists and the keeper's file bytes when a donor changed them. It cannot take a file out of the Trash, and a permanently deleted file is gone. Restore the files from the Trash first or afterwards: if you put a file back where it was, it must be the very same file. A sync or DJ-program link that changed after the removal, for example because the sync linked the same record to the kept track, is left as it is. The message then adds, for example, *2 sync or DJ-program links were left as they are because they changed after the removal.*

Undo is refused, and nothing is changed, when:

| Message | Why | What to do |
|---|---|---|
| *Cannot undo: a different file is now at …* | Another file sits where a removed track's file was | Move it away, or put the original file back from the Trash |
| *Cannot undo: the identity or file location of … is already used by a track in the library* | The file was restored and added again as a new track | Remove that new track first, or keep the current library |
| *Undo is no longer available: the removed library records were kept only for the last 10 removals …* | The removal fell out of the Undo window | The removal is final; restore the files from the Trash by hand if you still need them |

The entry of a removal that is final reads *The removal is final.* and has no button.

## Your last 10 removals { #window }

CueRithm keeps the saved records of your **10 most recent removals of library records**, counting duplicate merges that delete files and [Missing & broken removals](find-missing-files.md#remove) together. When another removal makes it eleven, the oldest one stays in the history without its details and can no longer be undone. Undoing a removal gives its place back. The cover art of removed music is kept for as long as its removal can still be undone, and deleted with the saved records.

## Undo a removal or a relink { #undo-repairs }

Under **Missing-file repairs**, **Removed from library** entries come from **Remove from library…** and **Moved files found** entries come from **Find moved files…**. An entry shows the time and counts, such as *3 tracks · 5 playlist memberships · 1 sync links · 1 rekordbox links unlinked*, and **Affected tracks** lists each track with its location. Select **Undo removal** or **Undo relink**.

- **Undo removal** reads *Removal undone. The records, playlist positions and metadata are back.* Each track returns with its cues, tags, links and history, in its old place in every playlist; a playlist you edited since gets the track back at its old position or at the end. A rekordbox link or a sync link that something changed after the removal, for example because rekordbox was scanned again, is left as it is and listed as *changed after the removal … and were left as they are*.
- **Undo relink** reads *Relink undone. The tracks point at their old locations again.* If the file is still inside a watch folder, a note says CueRithm will follow it again the next time that folder is scanned (for example when the app starts); move it back as well to keep the old location.

Undo refuses, and changes nothing, when a different file now sits at the old location (*Cannot undo: a different file is now at …*), when another track uses the identity or the location, when the audio of a relinked track changed after it was relinked, or when the track has unfinished work. Undo checks the old location before it locks the library, so a refusal is quick.

To drop the saved records of a removal early, select **Discard saved records…**, tick **I understand this removal can no longer be undone.** and select **Discard saved records**. The history line stays, reading *Saved records discarded; this removal can no longer be undone.*, and the cover art kept for it is deleted.

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
| **Undo merge** is missing from an entry | It is a **Merge undo** entry, already undone, or a removal that is now final | Scan and merge again if needed |
| **Undo removal** is missing from an entry | It is an undo entry, already undone, or its saved records were discarded or pushed out of the last 10 | Nothing to restore. Scan Missing & broken again if the file is back |
| *Cleanup pending* beside a removal | Temporary files could not be deleted yet | Select **Retry file recovery** |
| *Another maintenance operation is still running* | Only one operation runs at a time | Wait, or select **Cancel operation** |
| History shows an error banner for one section | That section could not be read | Select **Refresh history** |

## Related

- [Find and merge duplicate tracks](find-duplicates.md)
- [Find missing, moved or broken files](find-missing-files.md)
- [Find and remove unused files](clean-up-unused-files.md)
- [Restore your library from a backup](restore-library.md)
- [Where CueRithm keeps your data](where-data-lives.md)

<!-- verified: apps/desktop/src/views/MaintenanceHistory.tsx (five sections, Missing-file repairs labels and messages, discard, undo messages), Maintenance.tsx, MaintenanceShared.tsx (undoLimitNote, skipReasons); apps/desktop/src/test/maintenance.test.tsx, missing-repair.test.tsx; crates/cuerithm-library/src/maintenance/donors.rs (undo checks and messages, links_not_restored), track_removal.rs (undo refusals, observe before lock), missing.rs (undo, discard, notes, covers), undo_window.rs (shared limit 10), purge.rs (covers), merge.rs (version 1 journal rule), removal.rs (restore conditions), removal/retention.rs; tests/removal_engine.rs, maintenance_deletion.rs, missing_repair.rs; docs/development/maintenance-api.md; docs/development/desktop.md (tag and cleanup originals have no retention control) -->
