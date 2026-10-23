# Recover or undo a reorganisation

<p class="lead">Put files back after a rename, move or copy; finish or cancel an interrupted batch; and understand the hidden recovery files CueRithm leaves behind. Undo takes a minute, recovery a few.</p>

!!! warning "Undo moves files on disk again"
    Undo is another reorganisation, run in reverse. It re-checks every file first and stops for any track that changed, so run it before you edit, move or re-tag those files in other programs. Nothing is deleted: the file that Undo sets aside stays on disk as a hidden recovery copy.

Everything here happens in **History and recovery** at the bottom of **Organize files** (sidebar, under **TOOLS**). It lists every apply and every undo, newest first, and it survives a restart.

## Read the history { #history }

Each entry shows **Organization** or **Undo**, its state, the date and "N of M completed".

<!-- figure:existing-88-organization-history-undo-organize -->
<figure markdown>
![Organization history with a recoverable interruption and a guarded Undo](../assets/screens/88-organization-history.jpg){ .screen loading=lazy }
<figcaption>Each batch is listed with its result and Undo.</figcaption>
</figure>
<!-- /figure -->

| State | Meaning |
|---|---|
| **applying** | Still running. You can leave the page; it continues. |
| **complete** | Every eligible track finished. |
| **partial** | Some tracks finished and others failed, were blocked or were skipped. |
| **cancelled** | You cancelled; the files that finished stay where they are. |
| **recovery required** | Something was interrupted after a file was published. See [Finish or cancel an interrupted batch](#recover). |
| **undone** | The batch was reversed. |

Press **Details** for the counts "N completed · N unchanged · N skipped · N blocked · N failed · N need recovery · N cancelled / N total" and, 50 to a page, each track's old path, new path, outcome and reason. A track that sits in a linked DJ library also lists that reference and its state.

## Undo a batch { #undo }

1. Find the entry and press **Undo**. It appears when the batch has finished tracks that can still be reversed. A batch that a track in Incoming still owns offers **Retry in Incoming** instead.
2. CueRithm shows "Checking and undoing…". For each finished track it checks that:
    - the file at the new path is still the file CueRithm wrote,
    - the original path is still free, and for a **Copy to folder** batch that the original file is unchanged,
    - any linked DJ library can be repaired back,
    - there is room on the disk.
3. Tracks that pass are returned to their original path. The file that was at the new path is set aside as a hidden recovery copy. Tracks that fail a check are reported as blocked, for example "Undo blocked because the track changed", and stay as they are.
4. A new **Undo** entry appears. When all finished tracks are back, the original entry reads **undone**.

Undo handles the finished tracks of the batch only. Tracks that were blocked, skipped or failed during Apply were never moved, so there is nothing to reverse.

## Finish or cancel an interrupted batch { #recover }

If CueRithm closes, loses the disk or hits an error in the middle of a batch, it picks up from its journal and shows the batch as **recovery required** with the reason. Nothing is guessed:

- A file that was not yet published stays at its original place and nothing changed on disk for that track.
- A file that was already published keeps both copies until its path in the library has been switched and any linked DJ library repaired.
- If something unexpected is found, such as a changed folder, a file you added or a volume that is not mounted, CueRithm keeps every file, shows the error and does not mark the track finished.

Choose one of these buttons:

| Button | What it does |
|---|---|
| **Retry recovery** | Runs the unfinished steps again from the journal. Use it after you fix the cause, for example by reconnecting the drive. |
| **Retry in Incoming** | Appears instead when the batch belongs to a track still in Incoming. It resumes that track's preparation with the rules it was started with. |
| **Cancel pending files** | Stops the tracks that have not yet been published. Tracks that finished stay finished. Files that are staged or published keep their recovery evidence and may still need **Retry recovery** before Undo. |
| **Cancel operation** | Shown while a batch is applying. It stops at the next safe point and the completed tracks are kept. |

## Resolve a changed original { #changed-originals }

After a **Copy to folder** batch CueRithm remembers the exact bytes of each original so a watch folder does not import them again. If an original is later edited or replaced, the page shows **Changed original files** with its path and two choices:

| Choice | Result |
|---|---|
| **Keep managed copy** | The library keeps using the copy. CueRithm ignores this version of the original, but will ask again if the original changes again. |
| **Add changed original as Incoming** | The changed file enters Incoming as a new track and is analysed from scratch. |

## Hidden recovery files { #hidden-originals }

**Move to folder** and **Rename in place** do not delete the old file. After the track is switched to its new path, the old file is renamed in its old folder to something like `.cuerithm-3f2a…-0.original`. The same happens to the file that Undo sets aside. They start with a dot, so Finder, Explorer and most file managers hide them.

CueRithm never removes them, and there is no clean-up button for them yet. They are your safety copies: keep them until you have checked the result, and know that deleting one by hand removes the safety copy for that track. **Estimated additional space** in the plan counts them. Your [library backup](../files/library/back-up-library.md) does not contain music files.

## What can go wrong { #what-can-go-wrong }

| Symptom | Cause | Fix |
|---|---|---|
| No **Undo** button | The batch is still applying, nothing finished, it is already undone, or an Incoming track owns it | Wait, or use **Retry in Incoming** |
| "Undo blocked because the track changed" | The file or its library record changed after Apply | Decide which version you want; CueRithm will not overwrite later edits |
| "Destination folder changed or is unavailable; choose the folder again" | The destination folder was renamed, unmounted or replaced | Reconnect it, or choose it again with **Choose destination** |
| "Organization preview expired; create a new preview" or "Library changed. Preview the complete plan again before applying." | The library changed between preview and Apply | Press **Preview complete plan** again |
| An Apply stops with "Organization requires N bytes …" | Not enough space for staging and the recovery copies | Narrow the scope, free space or choose another destination |
| Tracks show as missing after you moved files in Finder | The files were moved outside CueRithm | [I moved or renamed my files](../help/troubleshooting/files-moved.md) and [Move or rename files without breaking the library](../files/move-files.md) |

## Related

- [Rename and sort files into folders](organize-files.md)
- [Build a naming pattern](naming-patterns.md)
- [Undo a cleanup](../tagging/undo-cleanup.md)
- [Find missing, moved or broken files](../files/library/find-missing-files.md)

<!-- verified: apps/desktop/src/views/Organize.tsx (history, Details, Undo, Retry recovery, Retry in Incoming, Cancel pending files, Cancel operation, Changed original files); apps/desktop/src/organization-types.ts (states); apps/desktop/src/test/organization.test.tsx; crates/cuerithm-library/src/organization/service/batches.rs (undo checks, cancel, recover, messages); crates/cuerithm-library/src/organization/coordinator.rs (copy undo, quarantine names); crates/cuerithm-library/src/organization/README.md (restart table, never deletes quarantines); service/roots.rs and planning.rs (messages); legacy guides/file-organization.md#history-recovery-and-undo and about/roadmap.md (backup retention controls unfinished) re-checked -->
