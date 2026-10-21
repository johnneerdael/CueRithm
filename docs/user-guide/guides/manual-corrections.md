# Correct cues, loops and the beatgrid

Load a prepared track, then choose **Edit cues & grid** in the player. The player
stays available while the correction panel is open.

![Cue and beatgrid correction panel with the player visible](../assets/screens/manual-cues-editor.png)

Change **BPM** or use **½** and **×2** to correct a half-time or double-time result.
Change **First beat** in seconds, or seek to a beat and choose **Set at playhead**.
This is a constant beatgrid. The first beat must be inside the track; negative
anchors and variable-tempo grid editing are not supported.

Each cue has a numbered slot, name, start time and color. Choose **Saved loop** to
add an end time after its start. **Add cue at playhead** and **Add saved loop at
playhead** use the player's current position. **Seek** lets you check a position.
Slots must be unique. All cue and loop times must stay within the track.

Choose **Preview corrections**, review the resulting cues and grid, then choose
**Save preview**. A preview does not change the saved track. Saved publication is
used by the player, the track's BPM summary and synchronization. Key, whole-track
energy and raw measured analysis remain unchanged. Corrections do not write tags
into your music files.

Local preview seeks to a saved loop's start. Continuous, sample-accurate looping
is not available in the local player. The active-loop flag is saved in the
library; current sync adapters do not transfer that flag to DJ targets. Saved
loop starts and ends are included in the existing sync projection, subject to
each target's own supported cue and loop limits.

## Reuse a cue template

![Reusable cue template editor](../assets/screens/manual-cue-template.png)

Choose **New cue template**, give it a name and choose the measured marker, name,
kind and color for each row. Offset rows place a cue before or after a structural
marker by a chosen number of beats. Loop rows use a length in beats. You can use
a uniquely named saved cue as an anchor instead of a measured structural marker.
A missing or ambiguous named anchor prevents the preview.

**Replace existing cue slots** starts off. With it off, existing cues remain and
the template fills free slots. **Preserve manual corrections** also starts on:
manual cue edits, removed slots and corrected grid values stay protected. Turn
these controls off only when you intend to replace that saved work. Templates
use the current saved BPM and grid; they do not rerun cue detection.

Choose a saved template and **Preview template** before **Save preview**. With
multiple tracks selected in Tracks, enable **Apply to selected tracks** to review
and save the template as one batch. All selected tracks must be prepared and
unchanged before the batch is saved.

## Undo and conflicts

**Undo last saved correction** restores the publication from before the last
available correction, including a template batch. Undo history survives restart.
Undo refuses to overwrite newer preparation or edits on any affected track.

If the library, source file or track changes while a preview is open, saving is
rejected. Your draft stays visible. **Reload saved edits** discards the draft and
loads the current saved preparation so you can make a fresh preview. Close the
panel to discard unsaved edits.
