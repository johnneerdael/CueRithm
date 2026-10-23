# Move or rename files without breaking the library

<p class="lead">CueRithm remembers each track by where its file is. If you move or rename a file yourself, the track no longer finds it. This page shows what that looks like, how to avoid it, and where to repair it. Understanding it takes two minutes.</p>

!!! note "Move tracking is coming"
    CueRithm 0.1.14 does not follow files you move yourself. Tracking moves inside a watch folder, and tools to reconnect a track, search a folder for missing tracks or replace a folder root, are planned and listed under In development on [What CueRithm does not do yet](../help/not-available-yet.md). None of it is released. After it ships, a file moved within a watch folder will be recognised as the same track. This page describes how CueRithm 0.1.14 behaves today and will be updated then.

## What happens when you move a file { #what-happens }

A track's identity in the library is the location of its file. When you move or rename a file in Finder, Explorer or your file manager, CueRithm sees two things at once: a location where a known track's file used to be, and a new file at a new location. It treats them separately.

Here is a real case. A watch folder, `~/Music/Beatport`, holds 26 prepared FLAC files. Fourteen of them are moved up one folder, from a subfolder into its parent, inside the watch folder.

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
- **Do not rearrange a watch folder by hand.** Moving files out of the folder leaves missing tracks and no new ones. Moving them within it also adds new twins, as above.
- **Put a track's file back before you do anything else** if you moved it by mistake. A track whose file returns to its recorded location works again at once.

## Find out what a move broke { #find }

1. Choose **Maintenance** in the sidebar and open the **Missing & broken** tab.
2. Select **Scan library files**.
3. Read the heading, for example *14 files need attention*, and the **Recorded location** and **Outcome** columns. A moved file shows *Missing* at the location it left.

The scan only reports. The screen says *Relocating files is not available in this maintenance screen*, so it cannot point a track at its new location. [Find missing, moved or broken files](library/find-missing-files.md) walks through the whole check and the repair choices.

## Repair it { #repair }

You have two choices.

**Put the files back.** Move each file to its recorded location, then scan again. The missing tracks are present again with everything attached. The new twins now point at locations that no longer exist, so they show as missing; archive them (see [Archive tracks you no longer play](archive.md)) or follow the repair steps in [Find missing, moved or broken files](library/find-missing-files.md).

**Keep the new locations.** The new tracks are the working copies, and the original tracks are missing. Follow [Find missing, moved or broken files](library/find-missing-files.md) to find and repair them. Afterwards, check the playlists that used to hold the original tracks, because the new tracks start with none.

!!! warning "CueRithm 0.1.14"
    The Duplicates scan in Maintenance can list a missing track beside its present twin. Do not use it to repair missing files, and never choose a missing track as the keeper.

## What can go wrong { #what-can-go-wrong }

| What you see | Why | What to do |
|---|---|---|
| Every song appears twice in Tracks | You moved files inside a watch folder | Read [What happens when you move a file](#what-happens), then [repair it](#repair) |
| Pressing play shows *File not found - move it back or run Maintenance* | The file is not at its recorded location | Put it back, or see [A track shows a dash or will not play](../help/troubleshooting/no-waveform-or-playback.md) |
| Many files report *Unavailable* instead of *Missing* | The whole drive or folder is not connected | Reconnect the drive and scan again |
| Playlists lost tracks after a repair | The new tracks were never in them | Add the new tracks to the playlists |
| The moved files were never picked up | They sit outside every watch folder | Add them with [Add tracks](add-tracks.md) |

## Related

- [I moved or renamed my files](../help/troubleshooting/files-moved.md)
- [Find missing, moved or broken files](library/find-missing-files.md)
- [Rename and sort files into folders](../rename/organize-files.md)
- [Keep a folder like Beatport in your library automatically](watch-folders.md)

<!-- verified: crates/cuerithm-library/src/jobs.rs (enqueue_one keys tracks by path: a new path creates a new track; old record stays); crates/cuerithm-library/src/prepare.rs (watch scan enqueues unseen paths); crates/cuerithm-library/src/maintenance/files.rs (file_state: Missing when the parent exists, Unavailable when it does not); apps/desktop/src/views/MaintenanceBroken.tsx (Scan library files, Recorded location, Outcome, 'Relocating files is not available in this maintenance screen', heading 'N files need attention'); crates/cuerithm-media/src/decode.rs ('Audio file is missing'); crates/cuerithm-library/src/organization/coordinator.rs (organization updates the track path); apps/desktop/src/views/OrganizationSettings.tsx (known linked libraries repaired); ia.md 'Pending product change' and coordinator update on Duplicates (0.1.13 behaviour). Worked example: 14 of 26 files moved up one folder, as supplied by the owner; counts are arithmetic on that case. Code-read, not run: playlist memberships and manual corrections staying on the original track. Does not describe unreleased behaviour as available. -->
