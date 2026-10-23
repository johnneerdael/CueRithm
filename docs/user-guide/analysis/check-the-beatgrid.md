# Check the beatgrid at beat level

<p class="lead">In about five minutes you will check whether CueRithm's beat lines sit on your track's kicks, spot the three usual faults, and know whether to correct the grid.</p>

<!-- figure:existing-41-player-zoom16-check-the-beatgrid -->
<figure markdown>
![The zoomed waveform showing beat lines](../assets/screens/41-player-zoom16.webp){ .screen loading=lazy }
<figcaption>Zoom in until you can see the beat lines against the kicks.</figcaption>
</figure>
<!-- /figure -->

**Before you start:** load a prepared track in the player. See [Preview a track with the player](preview-a-track.md).

## What the beatgrid is { #what-the-grid-is }

CueRithm saves two numbers for the grid: one BPM and the time of one first beat.
It draws a thin vertical line at the first beat and then every `60 ÷ BPM` seconds
to the end of the track. Those are the lines you see in the waveform. They show the
saved grid, so they include any [correction you made](corrections/fix-the-beatgrid.md).

Two things follow from that:

- **The grid is constant.** CueRithm does not follow tempo changes, so a track with live drums or a tempo shift can only match for part of its length.
- **No line is special.** CueRithm does not detect which beat starts a bar, so every line looks the same. The bar jumps with ++arrow-left++ and ++arrow-right++ count four beats from wherever the playhead is.

Neither can be changed with a setting. Variable-tempo grids and bar detection are
listed under [What CueRithm does not do yet](../help/not-available-yet.md).

## 1. Check the first beat { #check-the-first-beat }

1. Press **Back to start** in the transport.
2. Start the track and watch the 32-second window. At 128 BPM it holds about 68 beat lines.
3. Look at the first strong kick, the bass bump at the start of a bar. A beat line should run through the start of the bump, not just before or after it.

If the track opens with a pad or silence, wait for the first kick.

*What you should see:* the line and the bump start together. If every line is a
little early or late by the same amount, the first beat is off.

## 2. Look for drift { #look-for-drift }

A small BPM error does not show at the start. It adds up. Check three places:

1. The start, as above.
2. The middle. Click a cue such as **Breakdown** or **Drop #2**, then look at the next kicks.
3. The end. Click **End / Fade-out** and look again.

If the lines sit on the kicks at the start and slide away by the end, the BPM is
slightly off. As a guide, a BPM that is 0.05 too high at 128 BPM puts the lines about
90 milliseconds out after four minutes, almost a fifth of a beat.

Click the waveform to switch to **Entire track** and see the whole track at once.
The beat lines are too dense to judge alignment there, so use the 32-second window
for the check and the whole-track view to see where each cue falls.

## 3. Listen with bar jumps { #listen-with-bar-jumps }

Your ears are the final check.

1. Press **CUE** to pause at the first cue, or click a cue you trust.
2. Press ++arrow-right++ once. The playhead moves four beats, one bar.
3. Press play and listen for the kick landing on the beat.
4. Repeat from a different cue.

*Why:* each jump moves the playhead by `240 ÷ BPM` seconds. If the saved BPM is
right, you land on the same place in the groove each time. If you land a little
earlier or later in the bar than before, the BPM or the first beat is off.

## 4. Read what you see { #read-what-you-see }

| What you see | What it means | What to do |
|---|---|---|
| Every line is early or late by the same amount, all the way along | The first beat is off | [Move the first beat](corrections/fix-the-beatgrid.md#move-the-first-beat) |
| The lines match the kicks at the start and slip further apart by the end | The BPM is slightly off | [Correct the BPM](corrections/fix-the-beatgrid.md#correct-the-bpm) |
| Lines sit on every second kick, so there are half as many lines as kicks | The grid is in half-time | [Double the BPM with ×2](corrections/fix-the-beatgrid.md#halve-or-double-the-bpm) |
| Lines sit on every kick and between them, so there are twice as many lines as kicks | The grid is in double-time | [Halve the BPM with ½](corrections/fix-the-beatgrid.md#halve-or-double-the-bpm) |
| The lines match in one part and not in another | The track changes tempo or has a live drummer | The constant grid cannot follow. Align it for the part you mix, or leave it |
| The lines are right but a cue is in the wrong place | The cue is off, not the grid | [Move or add a cue](corrections/move-or-add-cues.md) |

CueRithm's tempo detector reports tempos between 98 and 195 BPM, so a slow track you
count at 70 BPM can appear at 140. That is a common cause of a half-time or
double-time grid.

If the grid is right, you are done. If you correct it, the corrected BPM shows in the
table and is what sync sends to your DJ software.

## Related

- [Fix a wrong beatgrid](corrections/fix-the-beatgrid.md)
- [Read a track's key, BPM and energy](read-key-bpm-energy.md)
- [How are BPM and the beatgrid found?](../how-it-works/tempo.md)
- [My key or BPM differs from another app](../help/troubleshooting/key-or-bpm-differs.md)

<!-- verified: apps/desktop/src/published-analysis.ts (playbackBeats: first beat plus n x 60/bpm); components/PlayerPanel.tsx (beat lines, 32 s window, cue strip, transport); App.tsx (jumpBar = 240/bpm, saved publication tempo); apps/desktop/src/styles.css (.beat-line, one style); apps/desktop/src/test/player.test.tsx and workspace.test.tsx (bar jump uses publication tempo); crates/camelot-core/src/lexicon_timing/mod.rs (min_tempo 98, max_tempo 195); legacy technical/tempo.md, learn/navigate-with-cues.md and about/roadmap.md re-checked; 0.05 BPM drift arithmetic: 512 beats x (60/128 - 60/128.05) = 0.094 s -->
