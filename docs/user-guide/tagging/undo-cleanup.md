# Undo a cleanup

<p class="lead">Reverse a whole cleanup batch from the Undo history, and learn which tracks CueRithm leaves alone because they changed after the batch. It takes about a minute.</p>

Undo covers the batches you apply by hand in **Cleanup**. Automatic cleanup on [Incoming](../files/follow-incoming.md) has no entry in the history; see [What Undo does not cover](#not-covered) below.

!!! warning "Undo puts the old tags back into the files"
    Undo rewrites each file with the tags it had before the batch, using the original that CueRithm saved at apply time. Run it before you tag, edit or reorganise those tracks again, or the track is left alone and reported as a conflict.

## Undo a batch { #undo-a-batch }

1. Open **Cleanup** in the sidebar and scroll to **Undo history**. Entries are newest first. Each shows **Cleanup** or **Undo**, the date and time, the state (**complete**, or **interrupted** if CueRithm was closed mid-batch) and the number of tracks.
2. Press **Undo** next to the batch you want to reverse. The button appears only on a **Cleanup** entry that still has at least one track you can restore.
3. Wait for "Undoing cleanup…" to finish. The result reads "N applied · N unchanged · N conflicts · N failed". Here *applied* means restored, and *unchanged* counts tracks that an earlier Undo already restored.

A new **Undo** entry is added to the history, and the batch you reversed loses its button once every track is restored. The history survives a restart.

## What Undo restores { #what-undo-restores }

For every track in the batch, Undo restores two things together:

- **The file.** The original bytes from before the batch, taken from the recovery backup that Apply saved. The restored file is checked before it replaces the current one.
- **The library record.** The title, artist, album and genre, and the category choices the track had before.

It never touches analysis. Key, BPM, beatgrid, energy and cues were not changed by the cleanup, so there is nothing to put back.

## When a track is left alone { #conflicts }

Undo checks every track before it writes. If the track no longer looks like the result of the batch, CueRithm keeps what is there now and reports a **conflict** for that track; the other tracks still restore. There are two reasons:

| Message | Meaning |
|---|---|
| "Later file edit preserved" | The file is not the one Apply produced. Something re-tagged, replaced or re-saved it afterwards. |
| "Later library changes preserved" | The track's library record changed after the batch: an edit in [Edit metadata](edit-tags-by-hand.md), a category change in [Curate](curate-tracks.md), a [cue or beatgrid correction](../analysis/corrections/index.md), [Analyze again](../analysis/analyze-again.md), archiving the track, or a rename or move with [Organize files](../rename/organize-files.md). |

Open the line "N conflicts · N failed" under the entry to see each track's name and message. If you want the old tags on a conflicted track anyway, set them yourself with [Edit tags and artwork](edit-tags-by-hand.md).

A **failed** track means Undo could not finish, for example because the saved original no longer matches its checksum. That track is left as it is.

## What Undo does not cover { #not-covered }

- **Automatic cleanup.** Tracks cleaned while they were in Incoming have no history entry and no Undo button. Correct them by hand, or change the rule and run a manual cleanup. See [Run cleanup on every new track](cleanup-automatically.md).
- **Other tag changes.** Edits you saved in the metadata editor have their own history: [Undo a tag change](tag-history.md).
- **Undo itself.** An **Undo** entry has no button. To apply the rules again, scan and apply again.

## What CueRithm keeps { #retention }

Every Apply saves the original file as a recovery backup, and Undo reads from it. CueRithm does not delete these backups. There is no retention setting yet, so they accumulate; the total size is shown under **Settings → Automatic tagging → Recovery backups**. A [library backup](../files/library/back-up-library.md) saves your preparation data, not your music files, so keep your own copy of the music too.

## What can go wrong { #what-can-go-wrong }

| Symptom | Cause | Fix |
|---|---|---|
| No **Undo** button on an entry | It is an **Undo** entry, every track is already restored, or nothing was written (all conflicts or failures) | Nothing to restore; scan again if you want different results |
| Every track reports "Later library changes preserved" | You ran Analyze again, edited or reorganised the tracks after the batch | Edit the tags by hand; run Undo first next time |
| "Later file edit preserved" on one track | Another program changed the file | Decide which version you want; CueRithm will not overwrite it |
| The batch shows **interrupted** | CueRithm closed during Apply | Reopen CueRithm; interrupted writes are rolled back on start, then check the entry's conflicts and failures |

If the batch changed more than you expected, see [Cleanup changed too much](../help/troubleshooting/cleanup-changed-too-much.md).

## Related

- [Clean up messy titles, genres and artists](cleanup-messy-titles.md)
- [Write cleanup rules](cleanup-rules.md#what-apply-guarantees)
- [Run cleanup on every new track](cleanup-automatically.md)
- [Undo a tag change](tag-history.md)

<!-- verified: apps/desktop/src/views/Cleanup.tsx (history entries, Undo button condition, result line, HistoryDetails); crates/cuerithm-library/src/cleanup/mod.rs (undo_cleanup, undo_snapshot: "Later library changes preserved", "Later file edit preserved"; row_identity fields); crates/cuerithm-library/src/cleanup/journal.rs (commit_metadata restore); crates/cuerithm-library/src/cleanup/tests.rs (damaged backup, interrupted undo); docs/cleanup.md; legacy about/roadmap.md (backup retention controls unfinished); legacy learn/cleanup.md re-checked -->
