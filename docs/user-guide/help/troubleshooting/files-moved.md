# I moved or renamed my files

<p class="lead">Reconnect a moved file to the track that holds your playlists, cues and categories. Start with the missing-file report, then review relocation matches before changing the library.</p>

<!-- figure:existing-help-preview-dash-light-help-troubleshooting-files-moved -->
<figure markdown>
![The Tracks table after files were moved: each moved track appears twice, once with a dash instead of a preview](../../assets/screens/help-preview-dash-light.webp#only-light){ .screen loading=lazy }
![The Tracks table after files were moved: each moved track appears twice, once with a dash instead of a preview](../../assets/screens/help-preview-dash-dark.webp#only-dark){ .screen loading=lazy }
<figcaption>An old offline track can appear beside a newly discovered track until you review the reconnection.</figcaption>
</figure>
<!-- /figure -->

## What you see { #what-you-see }

The example here comes from a real library: a purchase folder, `~/Music/Beatport`, watched by CueRithm and holding 26 FLAC files. Fourteen of the files were moved up one folder inside the watch folder. Afterwards:

| What you see | What it means |
|---|---|
| **Tracks** shows 40 tracks for 26 files | The watch folder found the 14 moved files at their new paths and prepared them as 14 new tracks. |
| Some titles appear twice | One row is the old track, the other is the new track for the same file. |
| The old row shows a dash in **PREVIEW**, and double-clicking it leaves the message `Audio file is missing` | The old track still points at the old path, where there is no file any more. See [A track shows a dash or will not play](no-waveform-or-playback.md). |
| **Maintenance** → **Missing & broken** lists 14 files with the outcome **Missing** | Those are the old tracks. The **Recorded location** column shows the path CueRithm expects. |

## Why it happens { #why }

A track has a stable library identity and a file reference. A move outside CueRithm can leave the reference pointing at the old location. A [watch folder](../../files/watch-folders.md) may discover the new location as an additional track; it does not choose which record’s playlists or corrections should survive.

Use [Relocate](../../files/library/find-missing-files.md#relocate) to make that decision explicitly. Moves made through [Organize files](../../rename/organize-files.md) update CueRithm’s references as part of their own reviewed operation.

## Fix it { #fix }

1. Choose **Maintenance** in the sidebar, open **Missing & broken** and select **Scan library files**. Read **Recorded location** and **Outcome**.
2. Reconnect an unplugged drive first if many tracks are **Unavailable**. See [When a whole drive is missing](../../files/library/find-missing-files.md#drive).
3. In **Tracks**, select the original tracks and choose **Relocate**. Choose one replacement file, or a folder to search for several tracks. For a whole folder or drive change, use **Find Lost Tracks** in the missing-file tab.
4. [Review candidates and occupied destinations](../../files/library/find-missing-files.md#survivor), choose the complete survivor, and keep the portable preflight backup enabled. A filename match alone does not prove identical audio.
5. Confirm and reconnect only the reviewed entries. This changes references without moving or tagging the music.

If you moved the files by mistake, putting them back at their exact recorded locations remains an alternative. Do not archive either copy until you have chosen whose work to retain. [Undo relocation](../../files/library/find-missing-files.md#undo-relocation) restores guarded library references; it does not move files back.

## Check it worked { #check }

- **Scan library files** shows **0 files need attention** and *No files need attention in this report.*
- **Tracks** counts the files you really have, and each row shows a waveform in **PREVIEW**.
- Double-clicking a track plays it.

## Prevent it { #prevent }

- Reorganise with [Rename and sort files into folders](../../rename/organize-files.md) instead of Finder or Explorer. CueRithm keeps its library in step with its own moves, and [Recover or undo a reorganisation](../../rename/undo-organize.md) can reverse them.
- Use reviewed reconnection after an external move. [Move or rename files without breaking the library](../../files/move-files.md#repair) explains the alternatives.
- [Back up your library](../../files/library/back-up-library.md) before you rearrange a large folder.

## Still stuck? { #still-stuck }

If the scan lists files you did not move, or the files are back in place and still report **Missing**, [report the problem](../report-a-problem.md) with the outcome shown in the **Outcome** and **Details** columns. Leave out your real folder names.

## Related

- [Find missing, moved or broken files](../../files/library/find-missing-files.md)
- [Undo a merge or restore removed files](../../files/library/undo-maintenance.md)
- [Keep a folder like Beatport in your library](../../files/watch-folders.md)
- [What CueRithm does not do yet: missing and moved files](../not-available-yet.md#missing-and-moved-files)

<!-- verified: apps/desktop/src/views/Relocation.tsx; MaintenanceBroken.tsx; crates/cuerithm-library/src/maintenance/relocate.rs; portable.rs; tests/relocation.rs. The upstream demo-library screenshot and 14-file discovery example remain historical examples; no app was launched or real library touched for this update. -->
