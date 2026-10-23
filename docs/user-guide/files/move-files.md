# Move or rename files without breaking the library

<p class="lead">CueRithm remembers each track by where its file is. If you move or rename a file yourself, the track no longer finds it. This page shows what that looks like, how to avoid it, and where to repair it. Understanding it takes two minutes.</p>

Use [Organize files](../rename/organize-files.md) when CueRithm should move the audio itself. If you have already moved it elsewhere, [Relocate](library/find-missing-files.md#relocate) reconnects the original record without moving audio again. Automatic discovery of external moves is a separate capability; review any new track discovered at the moved path.

## What happens when you move a file { #what-happens }

A track has a stable library identity and a local file reference. When you move or rename a file in Finder, Explorer or your file manager, CueRithm sees two things at once: a location where a known track's file used to be, and a new file at a new location. It treats them separately.

The following example describes earlier versions, before watch-folder move tracking. Current CueRithm follows an exact, unambiguous move within one watch folder while keeping the same track. Moves outside that folder, edited files and ambiguous matches still need review.

Here is the earlier case. A watch folder, `~/Music/Beatport`, holds 26 prepared FLAC files. Fourteen of them are moved up one folder, from a subfolder into its parent, inside the watch folder.

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
- **Review moves outside a watch folder.** Exact, unambiguous moves within one watch folder keep the original track. Other moves may need Relocate; edited or ambiguous files can be admitted as new tracks.
- **Put a track's file back before you do anything else** if you moved it by mistake. A track whose file returns to its recorded location works again at once.

## Find out what a move broke { #find }

1. Choose **Maintenance** in the sidebar and open the **Missing & broken** tab.
2. Select **Scan library files**.
3. Read the heading, for example *14 files need attention*, and the **Recorded location** and **Outcome** columns. A moved file shows *Missing* at the location it left.

The scan reports availability. In the same tab, **Find Lost Tracks** reviews root replacement; selected tracks also have **Relocate**. [Find missing, moved or broken files](library/find-missing-files.md#relocate) explains candidate review, backups and survivor choices.

## Repair it { #repair }

To keep the new locations and the original tracks’ work, select the originals and choose **Relocate**. Review candidates and, if a new track already owns the destination, choose the complete survivor deliberately. Ordinary playlist links are rewired to that survivor. [Reconnect selected tracks](library/find-missing-files.md#relocate) gives the steps and [Undo relocation](library/find-missing-files.md#undo-relocation) explains guarded recovery.

You can instead put the files back at their recorded locations and scan again. [Archive](archive.md) unwanted records only after deciding which track’s metadata and corrections you want to preserve. For a new computer, [restore a portable archive and map its music folders](library/restore-library.md#connect-folders).

## What can go wrong { #what-can-go-wrong }

| What you see | Why | What to do |
|---|---|---|
| Every song appears twice in Tracks | You moved files inside a watch folder | Read [What happens when you move a file](#what-happens), then [repair it](#repair) |
| Pressing play shows *File not found - move it back or run Maintenance* | The file is not at its recorded location | Put it back, or see [A track shows a dash or will not play](../help/troubleshooting/no-waveform-or-playback.md) |
| Many files report *Unavailable* instead of *Missing* | The whole drive or folder is not connected | Reconnect the drive and scan again |
| A new track has none of the old playlists or corrections | Discovery created a separate identity | Review relocation and its complete survivor before archiving either track |
| The moved files were never picked up | They sit outside every watch folder | Add them with [Add tracks](add-tracks.md) |

## Related

- [I moved or renamed my files](../help/troubleshooting/files-moved.md)
- [Find missing, moved or broken files](library/find-missing-files.md)
- [Rename and sort files into folders](../rename/organize-files.md)
- [Keep a folder like Beatport in your library automatically](watch-folders.md)

<!-- verified: apps/desktop/src/views/Relocation.tsx; crates/cuerithm-library/src/maintenance/relocate.rs; jobs.rs; portable.rs; tests/relocation.rs. Existing demo-library example retained as an example of discovery before reviewed reconnection. -->
