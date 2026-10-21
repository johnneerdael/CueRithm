# Player, waveform and cues

The player is a panel at the bottom of the window. It is always visible and plays through your system's default audio output.

<figure markdown>
  ![The player: track details, BPM, key, energy, transport, waveform, overview and cue buttons](../assets/screens/crop-player-panel.webp){ .crop loading=lazy }
</figure>

## Header

The header shows artwork (CueRithm's mark if the file has none), title, artist, **BPM**, **KEY** (a chip) and **ENERGY**. Before you load a track, it says *Nothing playing* and *Double-click a track to load and play*.

## Transport

| Control | What it does |
| --- | --- |
| **Back to start** | Seek to 0:00. |
| **Play / Pause** | Toggle playback (++space++). |
| **CUE** | Pause and jump to the first cue, or to the start if there is none. |
| Time | Elapsed time `m:ss.s` and, below it, the remaining time. |
| **Volume** | Slider from 0 to 1, default 0.65. |

Playback decodes the track into one cached 44.1 kHz stereo copy, so cue positions and the waveform line up exactly with what you hear for every format.

## The waveform

The label *BEATGRID & CUES* (or *PREVIEW* before analysis) sits above the waveform with the file codec. At right it says either *32 seconds* or *Entire track*, plus "click waveform to switch".

- **32 seconds** is the default: a window that follows the playhead, with beat lines clearly spaced.
- **Entire track** shows everything.

Click the waveform, or focus it and press ++enter++ or ++space++, to toggle between them.

<figure markdown>
  ![Zoomed waveform shown with the previous 16-second window](../assets/screens/41-player-zoom16.webp){ .screen loading=lazy }
</figure>

Beat lines come from the saved BPM and first-beat anchor. Cue lines carry a letter. Colours follow the [waveform style](../learn/appearance.md) you chose.

## The overview

The thin strip under the waveform is the **Track overview**. It always shows the full track. Click it to seek. ++arrow-left++ and ++arrow-right++ jump one bar (four beats at the saved BPM) when a track with a BPM is loaded.

## Cue buttons

Each cue has one button showing its slot letter, name, time and colour. Click a button to seek to that cue. The cue names are:

| Name | Notes |
| --- | --- |
| Start | The first beat of the grid. |
| Drop | The main drop. |
| Breakdown | Present when a long enough breakdown exists. |
| Drop #2 | A second drop. |
| Breakdown #2 | A second breakdown. |
| End / Fade-out | The last strong beat before the outro. |
| S.O.S. | An emergency loop. Only when enabled in [Settings > Cue generation](settings.md#cue-generation). |

Until preparation finishes the area says *Cues appear here after preparation finishes.*

Choose **Edit cues & grid** to change cue positions, names, colours and slots, save loops, or correct the constant grid. Preview before saving; use Undo to restore the previous publication. Reusable templates are in the same panel. See [Manual corrections](manual-corrections.md).

## The light theme

<figure markdown>
  ![The player in the light theme](../assets/screens/92-light-player.webp){ .screen loading=lazy }
</figure>
