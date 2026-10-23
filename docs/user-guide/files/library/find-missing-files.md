# Find missing, moved or broken files

<p class="lead">Scan your library to see which tracks no longer have a readable file where CueRithm expects it, work out why, and repair the library with the tools 0.1.14 has. A scan reads every file once, so allow a few minutes for a large library. It changes nothing.</p>

!!! warning "CueRithm 0.1.14"
    The results of **Missing & broken** are a report. They offer no action: there is no way to point a track at a new file location, no tool that finds moved files, and no way to remove a track from the library. The screen says so itself: *Relocating files is not available in this maintenance screen.* The repairs below use only what 0.1.14 has: moving files back and archiving tracks. Following moves inside a watch folder, relocating tracks and removing stale tracks are listed under *In development* in [What CueRithm does not do yet](../../help/not-available-yet.md). Version 0.1.14 can also list a track with a missing file inside a duplicate group, next to its available twin; never choose that track as keeper, as [Find and merge duplicate tracks](find-duplicates.md) explains.

## Scan the library files { #scan }

Choose **Maintenance** under TOOLS in the sidebar and open the **Missing & broken** tab. Select **Scan library files**. The progress line reads *Checking library files…* with a count.

The result heading reads, for example, **14 files need attention**, and the line below it gives how many tracks were checked and how: *40 checked · Availability only*. The table has three columns: **Recorded location**, **Outcome** and **Details**. Present files are hidden unless you tick **Show present files**. The scan checks every track that is not in Archive.

Even in the quick mode CueRithm reads each present file to compare its checksum with the one stored when the track was prepared. That is how it notices a file edited elsewhere.

## Read the outcome of each file { #outcomes }

| **Outcome** | What it means | What to do |
|---|---|---|
| **Missing** | The file is not at the recorded location, but its folder exists. | Find the file, or [repair the library](#repair). |
| **Unavailable** | The folder itself is gone or cannot be read: an unplugged drive, a renamed or deleted folder, or a permissions problem. **Details** gives the system's message. | Reconnect the drive first. See [When a whole drive is missing](#drive). |
| **Symbolic link** | The recorded path is a symbolic link. CueRithm does not follow links. | Replace the link with the real file at that location. |
| **Not a regular file** | The recorded path is a folder or another kind of item. | Restore the file. |
| **Changed** | The file changed while the scan was reading it. | Scan again. |
| **Unsupported**, **Timed out**, **Decode error**, **Video**, **No audio** | Only with **Inspect audio decoding** on. The decoder could not use the file. **Details** gives its message. | See [Check that present files can be decoded](#inspect). |

A present file can still carry the flag **· Source changed** in **Details**. It means the file's bytes differ from what CueRithm recorded when it prepared the track, for example because another app edited the tags. The track keeps working, but a [library backup](back-up-library.md) refuses to run until you [analyse it again](../../analysis/analyze-again.md).

## Why moved files look like a missing track and a duplicate { #moved-files }

CueRithm knows a track by the location and contents of its file. If you move a file in Finder or Explorer, the old track still points at the old location. The watch folder then meets a file it has never seen, at the new location, and prepares it as a new track.

The example library shows it. 14 files were moved up one level inside `~/Music/Beatport`, out of a subfolder named after the release. Afterwards the library holds 40 tracks for 26 files: 12 untouched, 14 old tracks whose file is gone, and 14 new tracks for the same files.

| | Old track | New track |
|---|---|---|
| File | Gone from the recorded location | At the new location |
| **Missing & broken** | Listed as **Missing**, or **Unavailable** once you also deleted the emptied subfolder | Not listed |
| Playlists, hand-corrected cues, categories | Still there | None. Prepared from scratch |

You can recognise the situation from three signs:

1. **Tracks** counts more tracks than you have music files.
2. The scan lists a block of files that all share one folder, the old one.
3. In **Tracks**, with the **Location** column shown (see [Show the columns you need](../../analysis/choose-columns.md)) and sorted by **TRACK**, every affected title appears twice, once with the old path and once with the new.

Files that CueRithm moves itself with [Organize files](../../rename/organize-files.md) keep their records. Only moves made outside CueRithm break the location. To avoid the problem in future, read [Move or rename files without breaking the library](../move-files.md).

## Repair the library today { #repair }

Do not use [Duplicates](find-duplicates.md) for this repair. It can list an old track next to its new twin, but the old track can never be keeper or donor, so merging would archive it without carrying over its cues or tags. You have two options. Choose by what the old tracks hold.

| You want | Choose |
|---|---|
| To keep the old tracks' playlists, hand-corrected cues and categories | [A. Put the files back](#put-back) |
| To keep the new folder layout | [B. Archive the old tracks](#archive-old) |

### A. Put the files back { #put-back }

1. In the scan result, read **Recorded location** for each missing file. That is the exact folder and file name to restore.
2. Move each file back to its recorded location, with the same name.
3. Select **Scan library files** again. The old tracks no longer appear in the list, because their files are present. The new tracks, whose files you just moved away, now read **Missing**. CueRithm recognises the restored files by location and contents, so it does not prepare them again. If a file's contents changed in between, for instance because preparing the new track rewrote its tags, the old track shows **· Source changed** and is prepared again.
4. [Archive](#what-archive-does) the new tracks. In **Tracks**, sort by **TRACK** and hold ++cmd++ / ++ctrl++ while you select every track whose **Location** is the new path. Then choose **Archive** in the toolbar, or **Archive track** in the row's **⋯** menu.

### B. Archive the old tracks { #archive-old }

1. In **Tracks**, show the **Location** column and sort by **TRACK**. Hold ++cmd++ / ++ctrl++ and select every old track, the one whose **Location** is the old folder.
2. Choose **Archive** in the toolbar, or **Archive track** in the row's **⋯** menu.
3. Rebuild what lived on the old tracks. Select the new tracks and use **Add to playlist** in the toolbar (see [Reorder, remove and restore tracks in a playlist](../../playlists/edit-playlist-tracks.md)); smart playlists pick the new tracks up by themselves. Redo hand-made [cue corrections](../../analysis/corrections/index.md) and [categories](../../tagging/curate-tracks.md) on the new tracks.
4. Scan again. The archived tracks are no longer checked, so the report should read **0 files need attention**.

### What Archive does { #what-archive-does }

- The track leaves **Tracks**, the **Missing & broken** scan and full-library duplicate scans. It stays in **Archive** with its analysis and cues. Nothing is deleted, and no music file is touched.
- Ordinary playlists keep the membership but hide the archived track. Smart playlists leave archived tracks out unless a rule asks for them.
- A DJ-software [sync](../../sync/index.md) sends only tracks that are in **Tracks**, so archived tracks are left out.
- To undo it, select the track in **Archive** and choose **Restore**. Restore checks the file again. A track whose file is still missing lands in **Incoming** with the error *Source unavailable*, so restore only after the file is back.

## When a whole drive is missing { #drive }

If hundreds of tracks read **Unavailable** and their locations start with the same drive or network path, the storage is probably offline. Reconnect it, wait for it to mount, and scan again. Do not archive these tracks. CueRithm cannot tell a disconnected drive from a deleted folder, which is why the outcome is **Unavailable** rather than **Missing** when a folder is absent.

## Check that present files can be decoded { #inspect }

Tick **Inspect audio decoding** before you scan to decode every present file with the bundled decoder. The note beside the box reads *Decode audio to distinguish unsupported formats, timeouts and decoding errors.* The result then names the decoder, and a file that exists but cannot be read shows **Decode error**, **Unsupported** or **Timed out**. Without the tick the note reads *Check file availability. A present file has not been checked for decoding errors.* A damaged file you cannot replace will not play; see [A track shows a dash or will not play](../../help/troubleshooting/no-waveform-or-playback.md).

## What can go wrong { #problems }

| What you see | Why | What to do |
|---|---|---|
| **0 files need attention** but a track will not play | The quick scan checks availability only | Tick **Inspect audio decoding** and scan again |
| A banner reads *Library or maintenance inputs changed. Reload or rescan.* | A track, file or playlist changed after the scan | Select **Scan library files** again |
| *Another maintenance operation is still running* | Only one scan or apply runs at a time | Wait, or select **Cancel operation** |
| The old tracks still read **Missing** after you moved the files back | A name differs, even by one character, or the folder is different | Compare with **Recorded location** and correct it |
| Many tracks show **· Source changed** | Another app edited the tags after CueRithm prepared the files | Select them and choose [Analyze again](../../analysis/analyze-again.md) |

## Related

- [Find and merge duplicate tracks](find-duplicates.md)
- [Move or rename files without breaking the library](../move-files.md)
- [I moved or renamed my files](../../help/troubleshooting/files-moved.md)
- [Archive tracks you no longer play](../archive.md)
- [Undo a merge or restore removed files](undo-maintenance.md)

<!-- verified: ia.md Pending product changes; apps/desktop/src/views/MaintenanceBroken.tsx, MaintenanceShared.tsx (fileStates, Stale), Maintenance.tsx; crates/cuerithm-library/src/maintenance/broken.rs (non-archived tracks, hash of present files, inspect via fingerprint), files.rs (Missing vs Unavailable by parent folder), scan.rs; crates/cuerithm-library/src/jobs.rs enqueue_one (path+hash recognition, archived ignored), store.rs set_archived (Restore re-checks file, Source unavailable), playlists/queries.rs (archived hidden from playlist views), dj_sync.rs (ready and not archived only), organization/coordinator.rs (Organize updates track path); apps/desktop/src/components/TrackTable.tsx row menu "Archive track", App.tsx selection toolbar Archive/Restore and Ctrl/Cmd multi-select, columns.ts Location; prepare/watch-folders.md. UNVERIFIED: effect of archiving on entries already written to DJ software. The 14 moved files / 40 tracks / 26 files figures come from the owner's description of the real library. legacy guides/maintenance.md re-checked -->
