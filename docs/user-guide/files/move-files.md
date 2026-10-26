# Move or rename files without breaking the library

<p class="lead">CueRithm remembers each track by where its file is. If you move or rename a file yourself, the track no longer finds it. This page shows what that looks like, how to avoid it, and where to repair it. Understanding it takes two minutes.</p>

Use [Organize files](../rename/organize-files.md) when CueRithm should move the audio itself. If you move files yourself inside a [watch folder](watch-folders.md), CueRithm follows them and keeps the same track. If you have moved them elsewhere, [Find moved files](library/find-missing-files.md#find-moved) or [Relocate](library/find-missing-files.md#relocate) reconnects the original record without moving audio again.

!!! note "Which version"
    Following moves inside a watch folder, **Find moved files…** and **Remove from library…** need CueRithm 0.1.15 or later. Earlier versions treat every move as described under [What happens when you move a file](#what-happens). **Relocate** and **Find Lost Tracks** need CueRithm 0.1.16 or later.

## What happens when you move a file { #what-happens }

A track has a stable library identity and a local file reference. When you move or rename a file in Finder, Explorer or your file manager, CueRithm sees two things at once: a location where a known track's file used to be, and a new file at a new location. It treats them separately.

CueRithm follows an exact, unambiguous move inside one watch folder while keeping the same track: when a new file has exactly the content of one missing, prepared, unarchived track from the same watch folder, that track points at the new location and nothing new is added. This also works for files you moved while CueRithm was closed, the next time it scans the folder. The cases that are not followed are a file that could belong to more than one track (true duplicates), a file that was edited or re-encoded, a move out of the watch folder, and a track that is archived or still being prepared. They behave as described next.

Here is what a move that is not followed does, using a real case from before CueRithm followed moves. A watch folder, `~/Music/Beatport`, holds 26 prepared FLAC files. Fourteen of them are moved up one folder, from a subfolder into its parent, inside the watch folder.

| | Before | After |
|---|---|---|
| Files on disk | 26 | 26 |
| Tracks in CueRithm | 26 | **40** |
| Tracks whose file is missing | 0 | **14** |
| New tracks with identical audio | 0 | **14** |
| Tracks that still work normally | 26 | 12 |

The watch folder noticed 14 files it had never seen at those locations and prepared them as new tracks. The 14 original tracks still point at the old locations, where nothing exists any more. That is 40 tracks for 26 files.

| | The track whose file is missing | Its new twin |
|---|---|---|
| Key, BPM, energy, cues | Still shown from the earlier analysis | Measured again |
| Playlists, categories, ratings, notes | Still attached | None |
| Cues you corrected by hand | Kept | Not copied; the new track has fresh cues |
| Playing it | Fails with *File not found - move it back or run Maintenance* | Works |
| Missing & broken scan | Listed as *Missing* | Present |

Renaming a file in place does the same thing: the old name becomes a missing track and the new name becomes a new track. [I moved or renamed my files](../help/troubleshooting/files-moved.md) is the short symptom version of this page.

## Avoid the problem { #avoid }

- **Rename and move through CueRithm.** Select the tracks and choose **Organize files**, or set a rule under Settings → File organization for tracks that are still coming in. CueRithm previews the changes, updates its own records, repairs known paths in linked DJ libraries and keeps a history you can undo. See [Rename and sort files into folders](../rename/organize-files.md) and [Recover or undo a reorganisation](../rename/undo-organize.md).
- **Keep moves inside a watch folder.** Exact, unambiguous moves within one watch folder keep the original track. For other moves use [Find moved files](library/find-missing-files.md#find-moved) or [Relocate](library/find-missing-files.md#relocate); edited or ambiguous files can be admitted as new tracks.
- **Put a track's file back before you do anything else** if you moved it by mistake. A track whose file returns to its recorded location works again at once.

## Find out what a move broke { #find }

<!-- figure:prepare-missing-broken-moved -->
<figure markdown>
![Maintenance, Missing & broken tab after a scan: the heading reads 14 files need attention and the table lists recorded locations (blurred) with the outcome Unavailable](../assets/screens/prepare-missing-broken-moved-light.webp#only-light){ .screen loading=lazy }
![Maintenance, Missing & broken tab after a scan: the heading reads 14 files need attention and the table lists recorded locations (blurred) with the outcome Unavailable](../assets/screens/prepare-missing-broken-moved-dark.webp#only-dark){ .screen loading=lazy }
<figcaption>After a move, the scan lists each track whose file is gone, here as Unavailable because the emptied subfolders were deleted too.</figcaption>
</figure>
<!-- /figure -->

1. Choose **Maintenance** in the sidebar and open the **Missing & broken** tab.
2. Select **Scan library files**.
3. Read the heading, for example *14 files need attention*, and the **Recorded location** and **Outcome** columns. A moved file shows *Missing* at the location it left.

The scan reports availability. In the same tab, select the missing tracks and choose **Find moved files…** to search a folder for their exact content, **Find Lost Tracks** reviews root replacement, and selected tracks in **Tracks** also have **Relocate**. [Find missing, moved or broken files](library/find-missing-files.md#repair) explains each, with candidate review, backups and survivor choices.

## Repair it { #repair }

To keep the new locations and the original tracks’ work, select the originals in **Missing & broken** and choose **Find moved files…** when the files are unchanged, or select them in **Tracks** and choose **Relocate**. Review candidates and, if a new track already owns the destination, choose the complete survivor deliberately. Ordinary playlist links are rewired to that survivor. [Reconnect selected tracks](library/find-missing-files.md#relocate) gives the steps and [Undo relocation](library/find-missing-files.md#undo-relocation) explains guarded recovery.

If a missing track's file is gone for good, select it in **Missing & broken** and choose **Remove from library…**: only the library record leaves, never an audio file, and **Undo removal** in History & recovery brings it back ([Remove the records of files that are gone](library/find-missing-files.md#remove)). You can instead put the files back at their recorded locations and scan again. [Archive](archive.md) unwanted records only after deciding which track’s metadata and corrections you want to preserve. For a new computer, [restore a portable archive and map its music folders](library/restore-library.md#connect-folders).

## What can go wrong { #what-can-go-wrong }

| What you see | Why | What to do |
|---|---|---|
| Every song appears twice in Tracks | You moved files in CueRithm 0.1.14 or earlier, or the moved files were edited, ambiguous or left the watch folder | Read [What happens when you move a file](#what-happens), then [repair it](#repair) |
| Pressing play shows *File not found - move it back or run Maintenance* | The file is not at its recorded location | Put it back, or see [A track shows a dash or will not play](../help/troubleshooting/no-waveform-or-playback.md) |
| Many files report *Unavailable* instead of *Missing* | The whole drive or folder is not connected | Reconnect the drive and scan again |
| A new track has none of the old playlists or corrections | Discovery created a separate identity | Review relocation and its complete survivor before archiving either track |
| The moved files were never picked up | They sit outside every watch folder | Add them with [Add tracks](add-tracks.md), or use [Find moved files](library/find-missing-files.md#find-moved) |

## Related

- [I moved or renamed my files](../help/troubleshooting/files-moved.md)
- [Find missing, moved or broken files](library/find-missing-files.md)
- [Rename and sort files into folders](../rename/organize-files.md)
- [Keep a folder like Beatport in your library automatically](watch-folders.md)

<!-- verified: apps/desktop/src/views/Relocation.tsx, MaintenanceBroken.tsx; crates/cuerithm-library/src/watch_moves.rs (relink_moved_tracks: ready, unarchived, same watch folder, exactly one missing track and one new file, settle wait), prepare.rs (RELINKED_STAGE), maintenance/missing.rs, relocate.rs; jobs.rs; portable.rs; tests/watch_moves.rs, relocation.rs, missing_repair.rs. Existing demo-library example retained as an example of the case that is not followed. -->
