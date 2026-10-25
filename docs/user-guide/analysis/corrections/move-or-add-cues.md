# Move or add a cue

<p class="lead">Move a cue to the exact beat you hear, rename or recolour it, add a cue of your own, or delete one you do not want. A single cue takes about three minutes.</p>

**Before you start**

- Load the track in the player. See [Preview a track with the player](../preview-a-track.md).
- The track must be prepared, in **Tracks**, and not preparing. Other limits are listed in [Fix a wrong cue, loop or beatgrid](index.md#before-you-correct).

## 1. Open the correction panel { #open-the-panel }

At the right end of the cue buttons, choose **Edit cues & grid**. The **Correct
cues & beatgrid** panel opens above the player, and the player stays usable. The
button now reads **Close corrections**.

<!-- figure:tracks-corrections-cue-rows -->
<figure markdown>
![The Cues & saved loops box with one row per cue showing Slot, Name, Start in seconds, Kind, Color, Seek and Remove](../../assets/screens/tracks-corrections-cue-rows-light.webp#only-light){ .screen loading=lazy }
![The Cues & saved loops box with one row per cue showing Slot, Name, Start in seconds, Kind, Color, Seek and Remove](../../assets/screens/tracks-corrections-cue-rows-dark.webp#only-dark){ .screen loading=lazy }
<figcaption>Each cue is one row: slot, name, start in seconds, kind and colour.</figcaption>
</figure>
<!-- /figure -->

The panel has a **Constant beatgrid** box and, under **Cues & saved loops**, one row
per cue. Each row has these fields:

| Field | What it holds |
|---|---|
| **Slot** | The cue's slot number, 1 to 128. Slot 1 shows as the letter `A` in the player |
| **Name** | The label on the cue button |
| **Start · seconds** | Where the cue sits, in seconds, to a thousandth |
| **Kind** | **Cue**, or **Saved loop** (see [Set a loop](set-loops.md)) |
| **Color** | None, yellow, red, blue, green, orange, violet, magenta, cyan or white |
| **Seek** | Moves the player to this row's start time |
| **Remove** | Deletes the row |

## 2. Move a cue { #move-a-cue }

1. Find the row by its **Name**.
2. Pause the player on the spot where the cue belongs. The time readout shows `m:ss.s`.
3. Convert it to seconds: minutes times 60, plus seconds. A readout of 1:51.4 is `111.4`.
4. Type that value in **Start · seconds**.
5. Choose **Seek** on the row to jump to your new time and check it by ear.

For example, suppose you hear the drop of *Come To Life (Original Mix)* half a
second after its **Drop** cue, which sits at about 0:59. Pause there, read 0:59.5,
and type `59.5`.

The panel keeps your edit as a draft. The waveform and the cue buttons change only
after you save in step 5.

## 3. Add a cue { #add-a-cue }

1. Pause the player where the new cue should sit.
2. Choose **Add cue at playhead**. A row named *New cue* appears at that time, in the lowest free slot, coloured cyan.
3. Type a **Name** and choose a **Color**.

You can hold up to 128 cues; the button is unavailable once you reach that.

To place the playhead, click the overview, press a cue button or use the bar-jump
keys, then fine-tune the time in **Start · seconds**.

## 4. Rename, recolour, renumber or remove { #rename-recolour-remove }

- **Rename.** Edit **Name**. The player shows it on the cue button.
- **Recolour.** Choose a **Color**. None leaves the cue uncoloured.
- **Renumber.** Change **Slot**. Two cues cannot share a slot, and the slot also sets the letter in the player.
- **Remove.** Choose **Remove**. The cue is deleted when you save.

New cues appear last in the cue strip. The slot number, not the position in the
strip, decides the letter.

!!! note "Hot cue slots in rekordbox"
    rekordbox direct sync accepts hot cues in slots 1 to 8 only. A hot cue in slot 9
    or above makes the sync report that the position exceeds rekordbox's eight
    slots. Keep your own cues in 1 to 8 if you sync there. See
    [Sync to rekordbox 6 or 7](../../sync/rekordbox-direct.md).

## 5. Preview and save { #preview-and-save }

<!-- figure:tracks-corrections-cue-preview -->
<figure markdown>
![The Review before saving box listing Come To Life (Original Mix) with its five cues and a button for each cue](../../assets/screens/tracks-corrections-cue-preview-light.webp#only-light){ .screen loading=lazy }
![The Review before saving box listing Come To Life (Original Mix) with its five cues and a button for each cue](../../assets/screens/tracks-corrections-cue-preview-dark.webp#only-dark){ .screen loading=lazy }
<figcaption>Preview shows the result and the cues; nothing is saved until Save preview.</figcaption>
</figure>
<!-- /figure -->

1. Choose **Preview corrections**. The panel shows **Review before saving**: the track, the BPM and first beat before and after, and the number of cues before and after. A button for each cue seeks to its time.
2. Choose **Save preview**. The panel confirms *Cues and beatgrid saved.*

You should see the cue buttons and the coloured lines in the waveform update
straight away. If you change anything after previewing, the preview disappears
and you preview again. **Discard preview** drops it without saving.

Closing the panel, or loading another track, discards an unsaved draft without
asking.

## What can go wrong

| Message or symptom | Cause | Fix |
|---|---|---|
| *Cue position must be within the track* | A start time is below 0 or beyond the track's length | Type a time between 0 and the track's length in seconds |
| *Cue slots must be unique and between 1 and 128* | Two rows share a slot, or a slot is out of range | Give each row its own slot |
| *At most 128 saved cue slots are supported* | You have more than 128 rows | Remove rows |
| *Library changed; refresh the correction preview*, or *Track changed; refresh the correction preview* | Something else changed the library or this track while the panel was open | Choose **Reload saved edits**, then redo the edit. See [Undo or resolve a correction](undo-corrections.md#resolve-a-rejected-save) |
| *Preparation is active; wait before editing cues* | The track is queued or running in **Incoming** | Wait, or see [Fix a track that needs attention](../../files/needs-attention.md) |
| Your edit vanished | You closed the panel or loaded another track before saving | Redo it and save |
| A cue is correct but the beat lines are off | The grid is wrong, not the cue | [Fix a wrong beatgrid](fix-the-beatgrid.md) |

A cue you moved stays where you put it if you later change the grid. Cues do not
follow the first beat.

## Related

- [Set a loop](set-loops.md)
- [Reuse one cue layout on many tracks](cue-templates.md)
- [Undo or resolve a correction](undo-corrections.md)
- [Cue names, letters and colours](../../reference/cue-types.md)

<!-- verified: apps/desktop/src/components/PublicationEditor.tsx (row fields, Add cue at playhead defaults, Seek, Remove, 128 cap, Preview corrections, Review before saving, Save preview, Discard preview, messages); components/CueTemplateEditor.tsx (cuePalette); components/PlayerPanel.tsx (cueSlot letters, Edit cues & grid, remount on track change); crates/cuerithm-library/src/manual_publication.rs (validate messages, guard messages); crates/cuerithm-library/src/rekordbox_sync_service/assets.rs (eight hot-cue slots); apps/desktop/src/test/manual-publication.test.tsx; legacy guides/manual-corrections.md re-checked -->
