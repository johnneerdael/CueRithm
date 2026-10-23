# Read, preview and correct your tracks

<p class="lead">Find a track, read what CueRithm measured, hear it in the player and correct a cue or the beatgrid when one is off. Reading a track takes a few minutes; a correction takes about five.</p>

<!-- figure:existing-40-player-index -->
<figure markdown>
![The CueRithm window with the track table above the player and its labelled cues](../assets/screens/40-player.webp){ .screen loading=lazy }
<figcaption>Track table, player, beatgrid and cues in one window.</figcaption>
</figure>
<!-- /figure -->

## Start here

<div class="grid cards" markdown>

-   **Read what CueRithm measured**

    ---

    Find the key, BPM and energy of any track and use the three numbers to
    pick what plays next.

    [:octicons-arrow-right-24: Read a track's key, BPM and energy](read-key-bpm-energy.md)

-   **Jump straight to the drop**

    ---

    Load a track, read the waveform and use the cue buttons to move between
    the start, the drop, the breakdown and the end.

    [:octicons-arrow-right-24: Jump to the drop with cues](jump-to-cues.md)

-   **Make the beat lines match the kicks**

    ---

    Check the grid, then correct the BPM or the first beat when the lines
    drift or sit on every second kick.

    [:octicons-arrow-right-24: Check the beatgrid at beat level](check-the-beatgrid.md)

    [:octicons-arrow-right-24: Fix a wrong beatgrid](corrections/fix-the-beatgrid.md)

-   **Find a track in a big library**

    ---

    Search by title, artist or genre, sort by BPM or energy and choose the
    columns you read.

    [:octicons-arrow-right-24: Find a track: search, sort and filter](find-tracks.md)

</div>

## What CueRithm measures for each track

CueRithm analyses every track offline, from the audio. It never reads its key,
BPM, energy or cues from your file tags or filenames.

| What | What you see | Where to read about it |
|---|---|---|
| Key | One Camelot code for the whole track, such as <span class="cr-key">7A</span> | [Understand Camelot keys](camelot-keys.md) |
| BPM | One constant tempo, shown with two decimals, such as <span class="cr-bpm">130</span> BPM | [Read a track's key, BPM and energy](read-key-bpm-energy.md) |
| Beatgrid | Beat lines drawn from the BPM and one first-beat time | [Check the beatgrid at beat level](check-the-beatgrid.md) |
| Energy | One rating from 1 to 10 for the whole track, such as <span class="cr-energy">7/10</span> | [Read a track's key, BPM and energy](read-key-bpm-energy.md) |
| Cues | Labelled positions: Start, Drop, Breakdown, Drop #2, Breakdown #2 and End / Fade-out | [Jump to the drop with cues](jump-to-cues.md) |
| Waveform | A colour waveform of the whole track | [Preview a track with the player](preview-a-track.md) |

Energy is one number per track. CueRithm shows no energy per section and puts
no energy on a cue.

## Your goal → Page

### Find and choose tracks

| Your goal | Page |
|---|---|
| Find a track by title, artist or genre, or sort by BPM or energy | [Find a track: search, sort and filter](find-tracks.md) |
| Show the album, label, bitrate or any other column, and put columns in your order | [Show the columns you need](choose-columns.md) |
| Select many tracks, then edit, analyse, archive or add them to a playlist | [Work on many tracks at once](select-many.md) |

### Read what CueRithm measured

| Your goal | Page |
|---|---|
| Understand the key, BPM and energy columns, and choose the next track | [Read a track's key, BPM and energy](read-key-bpm-energy.md) |
| Know what 7A or 12B means and which keys mix | [Understand Camelot keys](camelot-keys.md) |

### Listen and navigate

| Your goal | Page |
|---|---|
| Play a track, change the waveform zoom, seek and set the volume | [Preview a track with the player](preview-a-track.md) |
| Move between the start, the drop, the breakdown and the end | [Jump to the drop with cues](jump-to-cues.md) |
| See whether the beat lines sit on the kicks | [Check the beatgrid at beat level](check-the-beatgrid.md) |

### Control how tracks are analysed

| Your goal | Page |
|---|---|
| Change where Drop, Breakdown and the other cues land | [Choose how CueRithm places cues](cue-rules.md) |
| Measure a finished track again after changing a setting | [Analyze a track again](analyze-again.md) |

### Correct what is wrong

| Your goal | Page |
|---|---|
| See what you can correct and how a correction is saved | [Fix a wrong cue, loop or beatgrid](corrections/index.md) |
| Move a cue, rename it, recolour it or add a new one | [Move or add a cue](corrections/move-or-add-cues.md) |
| Add a loop with a start and an end | [Set a loop](corrections/set-loops.md) |
| Halve or double the BPM, or move the first beat | [Fix a wrong beatgrid](corrections/fix-the-beatgrid.md) |
| Give many tracks the same cue layout | [Reuse one cue layout on many tracks](corrections/cue-templates.md) |
| Go back to the cues and grid from before a correction, or fix a save that was rejected | [Undo or resolve a correction](corrections/undo-corrections.md) |

## Not what you wanted?

- To bring new music in or analyse a track again, go to
  [Manage files](../files/index.md) or
  [Analyze a track again](analyze-again.md).
- To change how cues are placed on future tracks, see
  [Choose how cues are placed](cue-rules.md).
- To fix a title, artist, genre or cover, go to
  [Edit tags and artwork](../tagging/edit-tags-by-hand.md).
- To find tracks that mix with the one playing, use
  [Find tracks that mix with the one playing](../playlists/find-compatible-tracks.md).
- To send corrected cues and grids to rekordbox or another DJ program, see
  [Send your library to DJ software](../sync/index.md).
- If a waveform shows a dash or a track will not play, see
  [A track shows a dash or will not play](../help/troubleshooting/no-waveform-or-playback.md).

## Related

- [Find your way around the window](../get-started/main-window.md)
- [How does CueRithm analyse a track?](../how-it-works/pipeline.md)
- [Status icons and job states](../reference/status-icons.md)
- [Keyboard shortcuts](../reference/keyboard-shortcuts.md)

<!-- verified: apps/desktop/src/components/TrackTable.tsx; PlayerPanel.tsx; PublicationEditor.tsx; display.ts; published-analysis.ts; crates/cuerithm-library/src/publication.rs (published cue names); docs/user-guide/_meta/ia.md page plan; legacy guides/manual-corrections.md, guides/track-table.md, guides/player.md re-checked -->
