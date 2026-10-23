# Set a loop

<p class="lead">Save a loop with a start and an end on a track, so your DJ software can offer it as a saved loop. Adding one takes about three minutes.</p>

A saved loop is a cue with an end time. CueRithm stores it with the track and sends
its start and end when you sync. The player in CueRithm does **not** play a loop
continuously: **Seek** and the cue button jump to the loop's start, and playback
then runs on past the end. Use the loop's times to check it by ear, then let your DJ
software loop it.

**Before you start:** load the track and open **Edit cues & grid**, as in [Move or add a cue](move-or-add-cues.md#open-the-panel).

## 1. Add a loop at the playhead { #add-a-loop }

1. Pause the player at the beat where the loop should start.
2. Choose **Add saved loop at playhead**. A row named *Saved loop* appears in the lowest free slot, starting at the playhead.
3. Look at **End · seconds**. CueRithm sets the end 16 beats after the start, using the grid's BPM, or at the end of the track if that comes first.
4. Give the loop a **Name** and a **Color**.

The row shows two extra controls: **End · seconds** and a **Save active-loop flag**
checkbox.

## 2. Set the loop's length { #set-the-length }

Type the end time in **End · seconds**. The end is the start plus the loop's beats
times `60 ÷ BPM`. At 128 BPM one beat lasts 0.46875 seconds:

| Loop length | Beats | Seconds at 128 BPM |
|---|---|---|
| 1 bar | 4 | 1.875 |
| 2 bars | 8 | 3.75 |
| 4 bars | 16 | 7.5 |
| 8 bars | 32 | 15 |

At 130 BPM, the BPM of *Come To Life (Original Mix)*, 16 beats last 7.385 seconds.
So a loop starting at 59.0 seconds there ends at 66.385. If you change the grid's
BPM, the end of a loop you already set does not change.

The end must come after the start and fall inside the track, or the preview is
refused with *Saved loops need an end after their start and within the track*.

## 3. Turn a cue into a loop, or back { #cue-to-loop }

Change a row's **Kind** from **Cue** to **Saved loop**. CueRithm gives it an end 16
beats after its start and an unticked active flag. Change **Kind** back to **Cue**
and the end and the flag are removed.

## 4. Decide about the active-loop flag { #active-flag }

Tick **Save active-loop flag** if the loop should be switched on when your DJ
software loads the track. CueRithm saves the flag with the loop and the next sync sends
it to rekordbox, VirtualDJ, Engine DJ and Pioneer USB. djay Pro, Serato DJ Pro 4 and
Mixxx receive the loop without the flag. Whether a DJ program then switches the loop
on depends on the program, so check the loop in your DJ software after your next sync.

The loop CueRithm adds itself, **S.O.S.**, is saved this way: a 16-beat, magenta
loop with the flag ticked. It appears only when **Emergency loop** is on in the
cue settings. See [Choose how cues are placed](../cue-rules.md).

## 5. Preview and save { #preview-and-save }

Choose **Preview corrections**, check that the loop's row in **Review before saving**
reads like `7 · Practice loop · 0:59.0–1:06.4 loop`, then choose **Save preview**.
The loop's start appears as a cue in the strip and the waveform.

## Where loops travel

- **In your library.** Loops count towards the limit of 128 cues and share the slots of cues.
- **In sync.** The loop's start and end go to targets that have saved loops, within each target's own limits. rekordbox takes hot cues and loops in slots 1 to 8. See [Preview, apply and restore a sync](../../sync/preview-apply-restore.md) and [Sync to rekordbox 6 or 7](../../sync/rekordbox-direct.md).
- **Not in your files.** Nothing is written to the audio file or its tags.

Loops can also come from a cue template, where **Kind** is **Saved loop** or **Active
saved loop** and **Loop beats** sets the length. See
[Reuse one cue layout on many tracks](cue-templates.md#rows).

## What can go wrong

| Symptom | Cause | Fix |
|---|---|---|
| The player does not loop | The player seeks to the loop's start only | Check the loop by ear, then sync it and loop it in your DJ software |
| *Saved loops need an end after their start and within the track* | The end is at or before the start, or beyond the track | Type an end time inside the track and after the start |
| The loop is missing in your DJ software | The target takes loops in limited slots, or a hot-cue slot above 8 on rekordbox | Move the loop to a slot between 1 and 8. See the sync preview |
| The active flag did not arrive | Not every target takes it | Switch the loop on in your DJ software |

## Related

- [Move or add a cue](move-or-add-cues.md)
- [Undo or resolve a correction](undo-corrections.md)
- [Reuse one cue layout on many tracks](cue-templates.md)
- [Set the sync options](../../sync/sync-options.md)

<!-- verified: the active-loop flag is read by the rekordbox writers (crates/cuerithm-rekordbox native_plan.rs and asset_staging.rs, fed from rekordbox_sync_service/assets.rs), the VirtualDJ writer (AutoTrigger), the Engine DJ writer (active_loops bit mask) and the Pioneer USB writer (isActiveLoop and the memory PCPT status); the djay Pro, Serato 4 and Mixxx writers never read it. UNVERIFIED in the target programs themselves: no round trip was run for this note -->
<!-- verified: apps/desktop/src/components/PublicationEditor.tsx (Add saved loop at playhead: 16 beats at draft BPM, End, Save active-loop flag, Kind switch, help text); crates/cuerithm-library/src/manual_publication.rs (validate loop rules, 128 cap); crates/camelot-core/src/lexicon_publication.rs (S.O.S. via ActiveLoop 16 beats); crates/cuerithm-library/src/publication.rs; crates/cuerithm-library/src/dj_sync.rs (loop end and active_loop projected); crates/cuerithm-library/src/rekordbox_sync_service/assets.rs (eight slots); apps/desktop/src/test/manual-publication.test.tsx (loop end 24 at BPM 60) -->
