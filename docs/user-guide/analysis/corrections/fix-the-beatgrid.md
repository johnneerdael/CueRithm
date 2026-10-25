# Fix a wrong beatgrid

<p class="lead">Halve or double the BPM, retype it, or move the first beat so the beat lines sit on the kicks. The correction takes a minute; checking it takes a few more.</p>

**Before you start**

- Find out which fault you have with [Check the beatgrid at beat level](../check-the-beatgrid.md#read-what-you-see).
- Load the track and open **Edit cues & grid**, as in [Move or add a cue](move-or-add-cues.md#open-the-panel).

## What you can change { #what-you-can-change }

The **Constant beatgrid** box at the top of the panel holds the whole grid:

<!-- figure:tracks-corrections-grid-fields -->
<figure markdown>
![The Constant beatgrid box with the BPM field, the half and double buttons, First beat in seconds and Set at playhead](../../assets/screens/tracks-corrections-grid-fields-light.webp#only-light){ .screen loading=lazy }
![The Constant beatgrid box with the BPM field, the half and double buttons, First beat in seconds and Set at playhead](../../assets/screens/tracks-corrections-grid-fields-dark.webp#only-dark){ .screen loading=lazy }
<figcaption>The whole grid is a BPM and a first beat.</figcaption>
</figure>
<!-- /figure -->

| Control | What it does |
|---|---|
| **BPM** | The tempo, from 0.01 to 1000 in steps of 0.01 |
| **½** | Divides the BPM by two |
| **×2** | Multiplies the BPM by two |
| **First beat · seconds** | The time of beat one, from 0 to the track's length |
| **Set at playhead** | Copies the player's position into the first beat |

That is everything. The grid is one BPM and one first beat. CueRithm has no
variable-tempo grid and no way to mark which beat starts a bar. See
[What CueRithm does not do yet](../../help/not-available-yet.md).

## 1. Halve or double the BPM { #halve-or-double-the-bpm }

Use this when the lines sit on every second kick, or twice as often as the kicks.

1. Choose **×2** if the grid is in half-time, or **½** if it is in double-time. The **BPM** field shows the new value.
2. Continue to step 4.

For example, *Play With Fire (Original Mix)* is measured at 140 BPM. Some DJs
count dubstep at half that speed. Choose **½** and the field reads 70.00; the
lines now fall on every second beat of the old grid. The first beat does not
move.

CueRithm's tempo detector reports tempos between 98 and 195 BPM, so a track that
you count outside that range can appear at double or half your count.

## 2. Correct the BPM { #correct-the-bpm }

Use this when the lines match the kicks at the start and slip away by the end.

1. Look at the end of the track in the 32-second window. If the lines come **earlier** than the kicks, the BPM is too high. If they come **later**, it is too low.
2. Type a slightly different value in **BPM**, for example 127.95 instead of 128.00.
3. Continue to step 4, check the result, and repeat in smaller steps.

??? note "Work out the BPM from a measured offset"
    If after N beats the lines are early by d seconds, the true beat length is the
    grid's beat length plus d ÷ N, and the true BPM is 60 divided by that. At 128
    BPM, lines 47 milliseconds early after 256 beats (two minutes) give
    60 ÷ (0.46875 + 0.047 ÷ 256), which is 127.95 BPM.

## 3. Move the first beat { #move-the-first-beat }

Use this when every line is early or late by the same amount.

1. Play the track and pause just after the first strong kick of a bar.
2. Choose **Set at playhead**. **First beat · seconds** takes the playhead's time.
3. Fine-tune the value in thousandths of a second if you need to.
4. Continue to step 4.

Choose the first beat of a bar, the "one" of the first full bar. CueRithm sends
this time to your DJ software as beat 1 of the grid. The first beat cannot be
negative: if a cropped intro starts mid-beat, use 0.

A pause by ear is only approximate. You judge the grid by eye and by ear after you
save, so expect to adjust once or twice.

## 4. Preview, save and check { #preview-and-save }

<!-- figure:tracks-corrections-grid-preview -->
<figure markdown>
![Review before saving for Play With Fire (Original Mix) showing 140.00 to 70.00 BPM after choosing the half button](../../assets/screens/tracks-corrections-grid-preview-light.webp#only-light){ .screen loading=lazy }
![Review before saving for Play With Fire (Original Mix) showing 140.00 to 70.00 BPM after choosing the half button](../../assets/screens/tracks-corrections-grid-preview-dark.webp#only-dark){ .screen loading=lazy }
<figcaption>Preview shows the BPM before and after; saving is a separate step.</figcaption>
</figure>
<!-- /figure -->

1. Choose **Preview corrections**. **Review before saving** lists the track, the BPM before and after (for example `140.00 → 70.00 BPM`), the first beat before and after, and the number of cues.
2. Choose **Save preview**.
3. Look at the waveform. The beat lines and the **BPM** in the table now use the saved grid.

The waveform draws the saved grid, not your draft, so you only see the effect after
you save. If the result is worse, choose **Undo last saved correction** and try
again. See [Undo or resolve a correction](undo-corrections.md).

## What a grid correction leaves alone { #what-it-leaves-alone }

- **Cues and loops** stay at the same seconds. They do not follow a new first beat. Move them with [Move or add a cue](move-or-add-cues.md), or apply a [cue template](cue-templates.md) whose **Start cue** is *At corrected first beat*.
- **Key and energy** do not change.
- **Your files** are not written. The **Embedded BPM** tag keeps the old value.
- **Sync.** The next sync sends the corrected BPM and first beat. The sync option **Keep existing beatgrids** leaves the grid in your DJ software alone, so untick it to send yours, and **Beatgrid locks** applies to Engine DJ and Serato DJ Pro. See [Set the sync options](../../sync/sync-options.md).

## What can go wrong

| Message or symptom | Cause | Fix |
|---|---|---|
| *First beat must be within the track; negative anchors are not supported* | The first beat is below 0 or beyond the track's length | Type a time from 0 to the track's length |
| *BPM must be greater than zero and at most 1000* | The BPM field is empty, zero or too large | Type a BPM up to 1000 |
| The lines match in one place and not in another | The tempo changes in the track | A constant grid cannot follow. Align it where you mix |
| The grid went wrong again after you analysed the track again | A new analysis replaces the corrected grid | Correct it again. See [Analyze a track again](../analyze-again.md) |
| *Library changed; refresh the correction preview* | Something else changed the library while the panel was open | **Reload saved edits**, then redo the edit. See [Undo or resolve a correction](undo-corrections.md#resolve-a-rejected-save) |

## Related

- [Check the beatgrid at beat level](../check-the-beatgrid.md)
- [Move or add a cue](move-or-add-cues.md)
- [Undo or resolve a correction](undo-corrections.md)
- [How are BPM and the beatgrid found?](../../how-it-works/tempo.md)

<!-- verified: apps/desktop/src/components/PublicationEditor.tsx (Constant beatgrid fieldset: BPM 0.01-1000, half, double, First beat, Set at playhead, help text, preview text, Save preview); apps/desktop/src/published-analysis.ts (player draws the saved grid); crates/cuerithm-library/src/manual_publication.rs (validate messages; write_publication sets tracks.bpm; raw analysis unchanged); crates/cuerithm-library/src/dj_sync.rs (beatgrid sent as one beat 1 anchor, bpm and start_time); crates/cuerithm-dj-sync/src/model.rs (SyncBeat at a downbeat); apps/desktop/src/views/Sync.tsx (Keep existing beatgrids, Beatgrid locks); crates/camelot-core/src/lexicon_timing/mod.rs (98 to 195); apps/desktop/src/components/CueTemplateEditor.tsx (Start cue options); legacy guides/manual-corrections.md and about/roadmap.md re-checked -->
