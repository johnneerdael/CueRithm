# Undo or resolve a correction

<p class="lead">Restore the cues, grid and BPM from before your last saved correction, or work out why CueRithm rejected a save. Undo takes one click; redoing a rejected save takes a minute.</p>

## Undo the last saved correction { #undo-the-last-correction }

1. Load the track you corrected and choose **Edit cues & grid**.
2. Choose **Undo last saved correction**, next to **Preview corrections**.
3. The panel confirms *Previous saved preparation restored.* The cue buttons, the beat lines and the **BPM** in the table return to what they were.

Undo restores the cues, loops, grid and the track's BPM. It works across
restarts, because CueRithm keeps the history in your library. Your music files were
never changed, so there is nothing to restore in them.

Undo reverses one thing at a time:

- **It undoes the most recent correction only.** If you saved two corrections, one undo brings back the state before the second. The first stays in place.
- **It undoes a whole batch.** If you applied a [cue template](cue-templates.md) to several tracks, Undo restores every one of them, and is refused if any one has changed since.
- **It is not a redo.** To bring a correction back, make it again.

## Read the correction history { #correction-history }

When a track has saved corrections, a line at the bottom of the panel reads *Saved
correction history · N*. Open it to see one entry per correction:

| Entry | Meaning |
|---|---|
| *Latest correction · Undo available* | The track is exactly as this correction left it |
| *Earlier correction · Newer work preserved* | Something saved after it, so Undo would overwrite newer work and is not offered |
| *Undone* | You have already undone it |

## When Undo is unavailable { #when-undo-is-unavailable }

**Undo last saved correction** stays greyed out when the track is no longer exactly
as your correction left it. CueRithm refuses rather than overwrite newer work. It
happens when:

- you saved another correction afterwards;
- you [analysed the track again](../analyze-again.md) or retried its preparation, which replaces the cues and grid;
- the track's record changed, for example you saved a tag edit, archived and restored it, or moved it;
- the track is queued or preparing in **Incoming**;
- the audio file on disk is no longer the file CueRithm analysed.

If you press Undo anyway and it fails, you see *Track changed after this
correction; Undo cannot replace newer work*.

## Go back to the automatic cues { #automatic-cues }

To discard every hand correction on a track, [analyse it again](../analyze-again.md).
The track gets a new key, BPM, energy, cues and grid from a fresh analysis, and
earlier corrections can no longer be undone. Only do this when you want the
automatic result.

## Resolve a rejected save { #resolve-a-rejected-save }

CueRithm checks a correction against the library twice, when you preview and when
you save. If the check fails, the panel shows the reason in a message, keeps your
draft on screen, and saves nothing.

| Message | What it means | What to do |
|---|---|---|
| *Library changed; refresh the correction preview* | Something else changed the library since the panel loaded, for example another track finished preparing or you saved a tag edit | Choose **Reload saved edits**, then redo the edit |
| *Track changed; refresh the correction preview* | This track changed while the panel was open | The same |
| *Source changed; prepare the track again* | The audio file is no longer the one CueRithm analysed | [Analyze the track again](../analyze-again.md), then correct the new result |
| *Preparation is active; wait before editing cues* | The track is queued or running, or needs attention | Wait until the header says *All up to date*, or see [Fix a track that needs attention](../../files/needs-attention.md) |
| *Prepare and restore this track before editing cues* | The track is in **Incoming** or **Archive** | Wait for it to finish, or [restore it from the archive](../../files/archive.md) |
| *Preparation is incomplete*, or *Prepare this track to create a saved cue/grid publication* | The track has no saved cues and grid to correct | Analyze it again |
| *This correction preview or Undo is no longer available* | The preview was already saved or replaced | Choose **Preview corrections** again |
| *Cue template is missing* | The template was deleted | Pick another template or make it again |

When you see *Library changed* or *Track changed*:

1. Write down the values you typed, or leave the panel as it is and note them.
2. Choose **Reload saved edits**. This **discards your draft** and loads the track's current saved cues and grid.
3. Enter your edits again.
4. Choose **Preview corrections**, then **Save preview**.

Plain validation messages, such as *Cue position must be within the track*, are
explained on the pages for each correction: [Move or add a cue](move-or-add-cues.md),
[Set a loop](set-loops.md) and [Fix a wrong beatgrid](fix-the-beatgrid.md).

A library-wide check means any change to the library can invalidate an open
preview. Do corrections while the header says *All up to date*, and keep each
visit to the panel short.

## Start over without saving { #discard-a-draft }

To abandon an unsaved draft, choose **Close corrections**, or load another track.
Both discard it without asking. To drop only the preview and keep the draft,
choose **Discard preview**.

## Related

- [Fix a wrong cue, loop or beatgrid](index.md)
- [Reuse one cue layout on many tracks](cue-templates.md)
- [Analyze a track again](../analyze-again.md)
- [Where CueRithm keeps your data](../../files/library/where-data-lives.md)

<!-- verified: apps/desktop/src/components/PublicationEditor.tsx (Undo last saved correction, history details labels, Reload saved edits resets draft, Discard preview, messages, canUndo = first entry with can_undo); crates/cuerithm-library/src/manual_publication.rs (expect_revision, guarded, load checks, undo_owned_in_database requires current cue revision and epoch, undo restores before-state and appends a revision, error strings); crates/cuerithm-library/tests/manual_publication.rs (undo after second apply rejected; queued preparation blocks undo; regenerated publication drops manual ownership; batch undo atomic); crates/cuerithm-library/migrations/008_maintenance_writes.sql (track epoch bumps on any track row update); legacy guides/manual-corrections.md re-checked -->
