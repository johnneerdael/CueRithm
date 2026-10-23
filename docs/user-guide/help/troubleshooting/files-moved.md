# I moved or renamed my files

<p class="lead">If you move or rename music files outside CueRithm, the library keeps pointing at the old locations, and a watch folder adds the files again as new tracks. This page shows what that looks like in CueRithm 0.1.14 and how to put the library right, in about ten minutes.</p>

<!-- figure:existing-help-preview-dash-light-help-troubleshooting-files-moved -->
<figure markdown>
![The Tracks table after files were moved: each moved track appears twice, once with a dash instead of a preview](../../assets/screens/help-preview-dash-light.webp#only-light){ .screen loading=lazy }
![The Tracks table after files were moved: each moved track appears twice, once with a dash instead of a preview](../../assets/screens/help-preview-dash-dark.webp#only-dark){ .screen loading=lazy }
<figcaption>After a move the old track is missing and the moved file is added again as a new track.</figcaption>
</figure>
<!-- /figure -->

## What you see { #what-you-see }

The example here comes from a real library: a purchase folder, `~/Music/Beatport`, watched by CueRithm and holding 26 FLAC files. Fourteen of the files were moved up one folder inside the watch folder. Afterwards:

| What you see | What it means |
|---|---|
| **Tracks** shows 40 tracks for 26 files | The watch folder found the 14 moved files at their new paths and prepared them as 14 new tracks. |
| Some titles appear twice | One row is the old track, the other is the new track for the same file. |
| The old row shows a dash in **PREVIEW**, and double-clicking it leaves the message `File not found - move it back or run Maintenance` | The old track still points at the old path, where there is no file any more. See [A track shows a dash or will not play](no-waveform-or-playback.md). |
| **Maintenance** → **Missing & broken** lists 14 files with the outcome **Missing** | Those are the old tracks. The **Recorded location** column shows the path CueRithm expects. |

## Why it happens { #why }

CueRithm identifies a track by the path to its file. It has no way to recognise the same file at a new path. A moved or renamed file therefore leaves its old track behind, with its playlists, corrections and categories, pointing at nothing. A [watch folder](../../files/watch-folders.md) checks for files it has not seen every few seconds, and a file at a new path counts as a new file: it is decoded, analysed and added to the library again as a new track.

Moves that CueRithm makes itself with [Rename and sort files into folders](../../rename/organize-files.md) update the library at the same time, so they do not cause this.

!!! note "In development"
    Following file moves inside a watch folder, reconnecting moved music and removing stale tracks from Missing & broken are in development. They are not part of the released app, and this page describes CueRithm 0.1.14 as it is. The list of planned work is in [What CueRithm does not do yet](../not-available-yet.md#in-development).

## Fix it { #fix }

Either put the files back, or accept the new locations and tidy the old records. Do not move more files until you have chosen.

**If you can move the files back (the cleanest result):**

1. Choose **Maintenance** in the sidebar, open **Missing & broken** and choose **Scan library files**. The report lists each missing file under **Recorded location**.
2. Move each file back to that exact folder with its original file name. Moving the whole folder back in Finder, Explorer or your file manager is quicker than moving files one by one.
3. Choose **Scan library files** again. The old tracks now point at files that exist. The report now lists the tracks that were created for the new locations, because their files are gone.
4. In **Tracks**, select those tracks (their **Location** matches the new paths in the report) and choose **Archive** in the toolbar. Archiving removes a track from Tracks and from ordinary playlist views without deleting anything, and an archived track no longer appears in the Missing & broken report.

**If you want to keep the new locations:**

CueRithm 0.1.14 cannot repoint an old track at a new path. The Missing & broken screen says so: *Relocating files is not available in this maintenance screen.* There is also no command that removes a missing track from the library. The new tracks work and have fresh analysis, but anything stored only on the old track (playlist membership, manual corrections, categories) stays with it. [Find missing, moved or broken files](../../files/library/find-missing-files.md) owns the repair steps for this case, including what you can safely do with the old tracks. Archiving them takes them out of Tracks and out of later Missing & broken reports.

!!! warning "CueRithm 0.1.14"
    The **Duplicates** scan in Maintenance can list a missing track beside its present twin, because both have the same artist, title and length. Do not use Duplicates to repair missing files, and never choose a missing track as the keeper. The keeper menus show unavailable tracks as unavailable, but pairing them is a known problem in this version, and a merge changes your library. A fix is in development and is not in the released app. [Find missing, moved or broken files](../../files/library/find-missing-files.md) gives the repair steps that 0.1.14 does support.

## Check it worked { #check }

- **Scan library files** shows **0 files need attention** and *No files need attention in this report.*
- **Tracks** counts the files you really have, and each row shows a waveform in **PREVIEW**.
- Double-clicking a track plays it.

## Prevent it { #prevent }

- Reorganise with [Rename and sort files into folders](../../rename/organize-files.md) instead of Finder or Explorer. CueRithm keeps its library in step with its own moves, and [Recover or undo a reorganisation](../../rename/undo-organize.md) can reverse them.
- Keep your music in the folder where CueRithm first found it. [Move or rename files without breaking the library](../../files/move-files.md) explains what is safe in 0.1.14.
- [Back up your library](../../files/library/back-up-library.md) before you rearrange a large folder.

## Still stuck? { #still-stuck }

If the scan lists files you did not move, or the files are back in place and still report **Missing**, [report the problem](../report-a-problem.md) with the outcome shown in the **Outcome** and **Details** columns. Leave out your real folder names.

## Related

- [Find missing, moved or broken files](../../files/library/find-missing-files.md)
- [Undo a merge or restore removed files](../../files/library/undo-maintenance.md)
- [Keep a folder like Beatport in your library](../../files/watch-folders.md)
- [What CueRithm does not do yet: missing and moved files](../not-available-yet.md#missing-and-moved-files)

<!-- verified: crates/cuerithm-library/src/jobs.rs (enqueue_sources_using/enqueue_one: track identity is tracks.path; discovered file at a new path becomes a new track; same path+hash returns existing job); crates/cuerithm-library/src/watch.rs (Discovery.scan, 5 s scan in prepare.rs); apps/desktop/src/views/MaintenanceBroken.tsx ("Relocating files is not available in this maintenance screen", Recorded location/Outcome/Details, "0 files need attention", fileStates Missing); store.rs set_archived (flag only, no file access) and maintenance/broken.rs (WHERE archived=0); App.tsx toolbar Archive/Restore; TrackTable.tsx row menu has no remove-from-library; crates/cuerithm-maintenance review.rs/MaintenanceDuplicates.tsx (unavailable tracks disabled as keeper/donor; scan.rs groups a missing track with a present twin by artist/title); coordinator notes on 0.1.13 duplicates bug and move tracking; legacy technical/library.md re-checked -->
