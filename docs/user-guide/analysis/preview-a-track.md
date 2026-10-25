# Preview a track with the player

<p class="lead">Load any prepared track, play it, move around in it and read its waveform, beat lines and cues. You are listening in under a minute.</p>

<!-- figure:existing-40-player-2-preview-a-track -->
<figure markdown>
![The player under the track table with the waveform, beatgrid and cue buttons](../assets/screens/40-player.webp){ .screen loading=lazy }
<figcaption>The player shows the whole track and its cues.</figcaption>
</figure>
<!-- /figure -->

**Before you start:** the track must be prepared, which means it is in **Tracks**. CueRithm plays through your system's default audio output and has no device picker, so choose the output in your operating system first.

## Load a track { #load-a-track }

Do one of these:

- Double-click the row.
- Press the small play button at the left of the row.
- Select the row, focus the table and press ++enter++.

The player shows *Loading track…* while CueRithm decodes the file into a cached
copy at 44.1 kHz stereo. That is why every supported format plays the same way.
The track starts playing as soon as it loads. Press ++space++ to pause it.

A track longer than about 24 minutes is not prepared, so it never reaches
**Tracks**. See [Check that your files are supported](../files/supported-formats.md).

## Read the player header { #header }

The header shows the artwork (CueRithm's mark when the file has none), the title,
the artist, and the track's **BPM** to two decimals, its **KEY** and its
**ENERGY** rating. The numbers are the saved ones. If you
[correct the grid](corrections/fix-the-beatgrid.md), the header shows the corrected BPM.

<!-- figure:tracks-player-come-to-life -->
<figure markdown>
![The player with Come To Life (Original Mix) loaded: BPM 130.00, key 7A, energy 7, the waveform, the overview and five cue buttons](../assets/screens/tracks-player-come-to-life-light.webp#only-light){ .screen loading=lazy }
![The player with Come To Life (Original Mix) loaded: BPM 130.00, key 7A, energy 7, the waveform, the overview and five cue buttons](../assets/screens/tracks-player-come-to-life-dark.webp#only-dark){ .screen loading=lazy }
<figcaption>The player: header, transport, waveform, overview and cue buttons.</figcaption>
</figure>
<!-- /figure -->

Before you load anything, the header says *Nothing playing* and *Double-click a
track to load and play*, and the waveform area reads *Your next track starts here*.

## Play, pause and seek { #transport }

| Control | What it does |
|---|---|
| **Back to start** | Seeks to 0:00 |
| **Play** / **Pause** | Starts and stops playback. ++space++ does the same |
| **CUE** | Pauses and jumps to the first cue, or to 0:00 if the track has none |
| Time readout | Elapsed time as `m:ss.s`, and under it the time remaining |
| **Volume** | A slider for the output level |

## Read the waveform { #the-waveform }

Above the big waveform, a label reads **BEATGRID & CUES** and names the file codec,
such as `FLAC`. If a track has no analysis yet, the label reads **PREVIEW**. At the
right it says *32 seconds* or *Entire track*, followed by *click waveform to
switch*.

<!-- figure:tracks-player-entire-track -->
<figure markdown>
![The player showing the entire track of Come To Life (Original Mix) with the five cue lines labelled A to E](../assets/screens/tracks-player-entire-track-light.webp#only-light){ .screen loading=lazy }
![The player showing the entire track of Come To Life (Original Mix) with the five cue lines labelled A to E](../assets/screens/tracks-player-entire-track-dark.webp#only-dark){ .screen loading=lazy }
<figcaption>Click the waveform to switch between 32 seconds and the entire track.</figcaption>
</figure>
<!-- /figure -->

- **32 seconds** is the default. The window follows the playhead, which sits about a third of the way across. Near the start or the end of the track the window stops moving and the playhead travels instead.
- **Entire track** shows everything at once.

Click the waveform, or focus it and press ++enter++ or ++space++, to switch between
the two. The time axis under the waveform shows where the window starts and ends.

What you see in the waveform:

- **Colour.** Bass, mids and highs are drawn in different colours. Choose the palette in [Choose a waveform style](../customize/waveform-style.md).
- **Thin vertical lines** are beats. All beat lines look the same; CueRithm does not mark the first beat of a bar. See [Check the beatgrid at beat level](check-the-beatgrid.md).
- **Coloured lines with a letter** are cues. A letter is the cue's slot: `A` is slot 1, `B` slot 2, and so on.
- **The playhead** is a single bright line.

Clicking the big waveform changes the zoom; it does not move the playhead. To
seek, use the overview.

## Seek with the overview { #the-overview }

The thin strip under the waveform is the **Track overview**. It always shows the
whole track, with every cue marked. Click anywhere on it to seek there.

## Jump with the cue buttons { #cue-buttons }

Each cue has a button showing its slot letter, its name and its time. Click a
button to seek to that cue. The strip is empty until a track is loaded, and a loaded
track shows *Cues appear here after preparation finishes.* if it has no cues yet.
[Jump to the drop with cues](jump-to-cues.md) walks through the buttons.

At the right end of the strip, **Edit cues & grid** opens the
[correction panel](corrections/index.md). While it is open the button reads **Close
corrections**.

## Use the keyboard { #player-keys }

| Key | What it does |
|---|---|
| ++space++ | Play or pause the loaded track |
| ++arrow-left++ / ++arrow-right++ | Jump one bar back or forward: four beats at the saved BPM |
| ++enter++ or ++space++ on the focused waveform | Switch between 32 seconds and the entire track |

The keys do nothing while you type in a text box, such as the search box or the
correction fields. The bar jump needs a loaded track with a BPM. The full list is
in [Keyboard shortcuts](../reference/keyboard-shortcuts.md).

## Which cues and grid does the player show?

The player always draws the saved cues and the saved grid, including any
[corrections you made](corrections/index.md). The small waveform in the **PREVIEW**
column of the table marks the same saved cues, so it follows a correction. Its faint
phrase lines (every 16 bars) are still drawn from the detector's grid, so after a
[grid correction](corrections/fix-the-beatgrid.md) trust the player.

## What can go wrong

| Symptom | Cause | Fix |
|---|---|---|
| The waveform is a dash or the track will not play | The file is missing, moved or unreadable | See [A track shows a dash or will not play](../help/troubleshooting/no-waveform-or-playback.md) and [I moved or renamed my files](../help/troubleshooting/files-moved.md) |
| You hear nothing | Playback goes to the system default output | Choose the right output in your operating system, then press **Play** again |
| A message appears at the right of the cue strip | The player reported an error | Load the track again. If it repeats, see [Report a problem](../help/report-a-problem.md) |
| The arrow keys do nothing | Focus is in a text box, or the track has no BPM | Click outside the text box, then press the arrow |
| The label above the waveform says **PREVIEW** and no cues appear | The track has no saved analysis yet | Wait for preparation to finish, or see [Fix a track that needs attention](../files/needs-attention.md) |

## Related

- [Jump to the drop with cues](jump-to-cues.md)
- [Check the beatgrid at beat level](check-the-beatgrid.md)
- [Choose a waveform style](../customize/waveform-style.md)
- [How is the waveform drawn?](../how-it-works/waveform.md)

<!-- verified: apps/desktop/src/components/PlayerPanel.tsx (header, transport, CUE, zoom 32 s with 0.35 playhead position, overview, cue strip, Edit cues & grid, empty states); apps/desktop/src/components/RowPreview.tsx (PREVIEW column marks the saved cues through playbackCues; phrase lines from the detector grid); apps/desktop/src/published-analysis.ts; apps/desktop/src/App.tsx (load starts playback, key handlers); apps/desktop/src/test/player.test.tsx; apps/desktop/src/test/workspace.test.tsx (bar jump uses saved tempo); crates/cuerithm-media/src/decode.rs (44100 stereo cache, 64M-sample limit); crates/cuerithm-library/src/playback.rs (default output device); legacy guides/player.md and about/roadmap.md re-checked -->
