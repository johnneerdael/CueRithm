# Navigate a track with cues

**You will:** load a track, read the beatgrid and jump between its structural cues.

## 1. Load a track

Select a row, then press the play button on the row, or double-click the row. The track decodes and appears in the player. Playback uses your system's default audio output.

<figure markdown>
  ![The player with a colour waveform, beat lines and five labelled cues](../assets/screens/40-player.webp){ .screen loading=lazy }
  <figcaption>The player with the whole track, five cue markers and a button per cue.</figcaption>
</figure>

## 2. Read the waveform

The colour waveform has three layers: bass, mids and highs. In the **Layers** style shown here, bass is cobalt, mids are coral and highs are ice-mint. You can change the palette in [Settings > Appearance](appearance.md). Thin vertical lines are beats. Cue markers are labelled with a letter.

Below the waveform is a small **overview** of the whole track. Click it to seek.

## 3. Jump to a cue

Each cue has a button with its letter, name and time.

<figure markdown>
  ![A row of cue buttons: Start, Drop, Breakdown, Drop 2 and End or Fade-out, each with a time](../assets/screens/crop-cue-chips.webp){ .crop loading=lazy }
  <figcaption>Cue buttons. Click one to seek there.</figcaption>
</figure>

A typical set looks like this:

| Cue | Meaning |
| --- | --- |
| <span class="cr-cue cr-cue--start">A · Start</span> | The first beat of the grid. |
| <span class="cr-cue cr-cue--drop">B · Drop</span> | The main drop. |
| <span class="cr-cue cr-cue--breakdown">C · Breakdown</span> | A breakdown, when the track has one that is long enough. |
| <span class="cr-cue cr-cue--drop">D · Drop #2</span> | The second drop. |
| <span class="cr-cue cr-cue--breakdown">Breakdown #2</span> | A second breakdown, when present. |
| <span class="cr-cue cr-cue--end">E · End / Fade-out</span> | The last strong beat before the outro. |

Not every track gets every cue. A track with no clear breakdown simply has fewer cues. The **CUE** button in the transport jumps to the first cue and pauses.

## 4. Zoom to the beats

The waveform defaults to a **32-second** view that follows the playhead. Beat lines become clearly visible, so you can check that the grid sits on the kicks.

<figure markdown>
  ![The waveform zoomed to a 16-second window with beat lines](../assets/screens/41-player-zoom16.webp){ .screen loading=lazy }
  <figcaption>Zoomed view, shown with the previous 16-second window. Click the waveform to switch between the default 32-second view and the whole track.</figcaption>
</figure>

Use ++arrow-left++ and ++arrow-right++ to jump a bar (four beats) at a time, and ++space++ to play or pause.

## What just happened

The cue buttons and markers are drawn from the cues saved at preparation time. The beat lines come from the saved BPM and first-beat anchor. Cues are read-only in this release: you can seek to them, but not edit them. The detection logic is described in [The cue engine](../technical/cues.md).
