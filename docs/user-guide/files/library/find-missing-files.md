# Find missing, moved or broken files

<p class="lead">Find tracks whose files are missing or unreadable, then reconnect them without losing their library identity. A scan changes nothing; review relocation matches before applying them.</p>

Choose [Relocate](#relocate) for selected tracks or [Find Lost Tracks](#find-lost-tracks) for a folder or drive change. Reconnecting changes library references, not music files. For a move you want CueRithm to perform itself, use [Organize files](../../rename/organize-files.md).

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

A present file can still carry the flag **· Source changed** in **Details**. It means the file's bytes differ from what CueRithm recorded when it prepared the track, for example because another app edited the tags. Review saved timing before using a replacement. A [portable library backup](back-up-library.md#create) can preserve the current records without reading the audio; a same-installation recovery snapshot still has stricter file-consistency checks. [Analyze again](../../analysis/analyze-again.md) is a separate action that can replace analysis and cues.

## Why moved files look like a missing track and a duplicate { #moved-files }

A track keeps a library identity and a file reference. A move in Finder or Explorer can leave that reference pointing at the old location. A watch folder may discover the new location as another track before you reconnect the original. [Reviewed relocation](#relocate) repairs the reference while preserving the chosen track’s identity.

An earlier example from the demo library shows this discovery behaviour. 14 files were moved up one level inside `~/Music/Beatport`, out of a subfolder named after the release. Afterwards the library holds 40 tracks for 26 files: 12 untouched, 14 old tracks whose file is gone, and 14 new tracks for the same files.

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

Reconnect the existing track when you want to keep its playlists, corrections and categories. Do not use [duplicate merging](find-duplicates.md#keeper) as a substitute for reviewing which moved track should survive.

### Reconnect selected tracks { #relocate }

1. In **Tracks**, select the affected tracks and choose **Relocate** from the toolbar or track menu.
2. For one track, choose its replacement audio file. For several, choose a folder to search recursively. Present tracks are skipped in this selection workflow.
3. Select **Review matches** and read **Recorded location**, **Candidate and evidence** and **Decision**. Ambiguous matches remain unselected; choose a candidate or leave **Skip this entry**.
4. For a different recording or format, tick **Accept replacement; check saved cue timing** only after reviewing it. A filename suggestion does not prove identical audio; matching known encoded-file hashes are stronger evidence.
5. Keep **Create a portable preflight backup (recommended)** enabled and select **Choose backup folder**. A failed backup blocks relocation. Turning it off is an explicit choice.
6. Confirm the reviewed changes and select **Reconnect … entries**.

The chosen track keeps its identity, metadata, cues and playlist links. Reconnection does not move audio, write tags, prepare the track again or sync a DJ library. Changed content can make saved analysis stale; listen and check its timing before choosing [Analyze again](../../analysis/analyze-again.md).

### Replace a folder or drive root { #find-lost-tracks }

In **Maintenance → Missing & broken**, open **Find Lost Tracks**. Choose a **Recorded music root**, or enter the **Recorded source root**, then choose the **New music root**. The change applies to a complete leading folder path, not matching text elsewhere in a name.

**Missing only** is the default. Choose **All tracks, including archived entries** only when you intend to review available tracks too. **Include unavailable storage references** is separate: reconnect an offline drive first when possible. **Replace extension (optional)** is an explicit replacement, not a claim that the recording is unchanged.

Select **Review matches**, then use the same candidate, backup and confirmation steps above. **Search incomplete** identifies unsearched locations. If the search pauses at its safety limit, choose **Continue search** before treating the results as complete.

### Choose one complete survivor { #survivor }

A destination may already belong to another track. Choose **Keep source entry**, **Keep existing entry** or **Skip collision**. For a shared destination group, choose one **Group survivor** for the whole group.

CueRithm keeps that survivor’s complete metadata and cues, archives the other track records and rewires their ordinary playlist links. It does not combine selected fields from different tracks silently. Review every affected entry before confirming.

### Undo a relocation { #undo-relocation }

Select **Undo relocation** after applying, or reopen this workspace and use **Previous relocations → Undo saved relocation** after a restart. Undo checks the affected library state and file proofs again; later relevant changes can block it.

Undo restores the old library references and survivor changes. It does not move audio back, so an old reference can become unresolved again. [Retained library recovery](restore-library.md#go-back) is a separate whole-library operation.

### A. Put the files back { #put-back }

You can instead move files back to their **Recorded location** yourself, with the original names, then scan again. This changes the filesystem rather than accepting a new canonical location in CueRithm. Review any newly discovered copies before archiving them.

### B. Archive the old tracks { #archive-old }

If you no longer need an old track, select it in **Tracks** and choose **Archive**, or **Archive track** in its row menu. Archiving is not a reconnection: work attached only to that track stays with it. To preserve that work on a moved file, review [Relocate](#relocate) first.

### What Archive does { #what-archive-does }

Archive hides a track from Tracks and normal playlist views without deleting its music. The record, metadata and cues remain in Archive. [Archive tracks you no longer play](../archive.md) explains restoring one; a file reference can still need reconnection afterwards.

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
| Many tracks show **· Source changed** | The encoded file differs from the saved source | Review the recording and timing first; [Analyze again](../../analysis/analyze-again.md) can replace saved analysis and cues |

## Related

- [Find and merge duplicate tracks](find-duplicates.md)
- [Move or rename files without breaking the library](../move-files.md)
- [I moved or renamed my files](../../help/troubleshooting/files-moved.md)
- [Archive tracks you no longer play](../archive.md)
- [Undo a merge or restore removed files](undo-maintenance.md)

<!-- verified: apps/desktop/src/views/Relocation.tsx; MaintenanceBroken.tsx; App.tsx; components/TrackTable.tsx; apps/desktop/src-tauri/src/relocation_commands.rs; crates/cuerithm-library/src/maintenance/relocate.rs; tests/relocation.rs; maintenance/broken.rs. Historical 14-file example retained from the upstream demo-library guide; no new track data invented. -->
