# Cue names, letters and colours

<p class="lead">CueRithm places up to six structural cues on a track and, if you ask for it, one emergency loop. This page lists each cue's name, its letter, its colour and what it marks, exactly as the player and your DJ software show them. Look-up time: one minute.</p>

!!! tip "New to this?"
    [Jump to the drop with cues](../analysis/jump-to-cues.md) teaches you to read and use them in the player. [Choose how cues are placed](../analysis/cue-rules.md) explains the settings that shape them.

A cue is a labelled position in a track. CueRithm's cues are structural markers placed by analysing the shape of the track. They carry no energy value: a track has one whole-track energy rating, and cues never show or change it. Cues live in your library and travel to DJ software when you sync; CueRithm never writes them into the audio file's tags.

## The seven cues { #names }

The analysis finds six roles. When a track is prepared, CueRithm publishes them with a standard layout, which is what the player, the table preview and your DJ software show. The **Emergency loop** setting adds a seventh.

| Cue in the player | What it marks | Name in the analysis | Kind | Colour name | Colour on screen |
|---|---|---|---|---|---|
| <span class="cr-cue cr-cue--start">Start</span> | The first beat of the track, where the beatgrid begins | Start | Hot cue | yellow | `#f5f50c` |
| <span class="cr-cue cr-cue--drop">Drop</span> | Where the full-energy section first begins after the intro | Drop | Hot cue | red | `#e60e0e` |
| <span class="cr-cue cr-cue--breakdown">Breakdown</span> | Where energy falls away after the drop, for at least your **Minimum breakdown length** | Breakdown | Hot cue | blue | `#1058de` |
| <span class="cr-cue cr-cue--drop">Drop #2</span> | Where the energy comes back after the breakdown | Second Drop | Hot cue | red | `#e60e0e` |
| <span class="cr-cue cr-cue--breakdown">Breakdown #2</span> | A second drop in energy after the second drop | Second Breakdown | Hot cue | blue | `#1058de` |
| <span class="cr-cue cr-cue--end">End / Fade-out</span> | Where the steady beat ends, the last good place to leave the track | Last beat | Hot cue | violet | `#aa3bff` |
| S.O.S. | A 16-beat loop in a steady late section, for when a transition goes wrong. Added only when **Emergency loop** is on. | Emergency loop | Active loop, 16 beats | magenta | `#e60ede` |

"Name in the analysis" is the label the detector gives the marker and the one you see in technical output and in the research notes. The player and your DJ software show the left-hand name. The settings behind these cues are on [Cue generation settings](settings/cue-generation.md); how they are found is on [How are cues placed?](../how-it-works/cues.md).

### What the demo library looks like { #examples }

| Track | Cues in the player, with slot letter and time |
|---|---|
| 17 (Extended Mix), Miss Monique and others | A Start 0:00, B Drop 1:15, C Breakdown 2:15, D Drop #2 3:00, E End / Fade-out 4:30 |
| Atoma (Extended Mix), Anyma (ofc) | A Start 0:00, B Drop 0:44, C Breakdown 1:36, D Drop #2 2:24, E Breakdown #2 3:23, F End / Fade-out 3:42 |
| Brackish (Original Mix), Tape B and Of The Trees | A Start 0:01, B Drop 0:31, C End / Fade-out 2:45 |
| Prophecy (Extended Mix), Baset and others | A Start 0:00, B Drop 0:59, C Drop #2 2:57, D End / Fade-out 4:26 |

## Letters: the slot of each cue { #letters }

Each cue sits in a numbered slot. The player shows slots as letters, **A** for the first, **B** for the second and so on up to **Z**. A slot after the 26th shows its number, so the 27th slot reads `27`.

- Slots follow the order of the cues in time, so the letter depends on which cues were found. On *Atoma* **C** is the Breakdown; on *Brackish*, which has no breakdown, **C** is End / Fade-out.
- The S.O.S. loop takes the slot after the highest slot used by the other cues.
- Letters appear in the cue strip below the waveform (for example "C Breakdown 1:36.0"), on the waveform in the zoomed view, and in the tooltip of the table's **PREVIEW** column ("C · Breakdown"). Selecting a cue button moves the playhead to that cue.
- The slot number travels with the cue when you sync, so the cue keeps its place in your DJ software. What each program does with it is on the page for that destination ([Send your library to DJ software](../sync/index.md)).

## Colours { #colours }

Colour helps you find a cue on a crowded waveform, but never carries meaning alone: every cue also has its name and letter.

| Colour name | Used for | On screen |
|---|---|---|
| yellow | Start | `#f5f50c` |
| red | Drop, Drop #2 | `#e60e0e` |
| blue | Breakdown, Breakdown #2 | `#1058de` |
| violet | End / Fade-out | `#aa3bff` |
| magenta | S.O.S. loop | `#e60ede` |

When you add or edit a cue by hand, or build a cue template, the colour menu offers: none, yellow, red, blue, green, orange, violet, magenta, cyan and white. On screen, green is `#7fe80e`, orange `#e88915` and white `#ffffff`. A cue with **cyan** or an unrecognised colour is drawn teal (`#00d2bf`), and a cue with no colour grey (`#899692`). Cues that arrive from other DJ libraries can use further named colours, such as `red_dark` or `aqua`; the player draws each as the closest listed shade.

DJ software limits the colours it can show. When you sync, CueRithm can map to the nearest colour a target supports; see [Set the sync options](../sync/sync-options.md).

## When a cue is missing { #missing }

Not every track gets every cue. This is normal, not a failure.

| Missing cue | Why |
|---|---|
| Everything except Start | The detector found no clear drop. Breakdown, Drop #2, Breakdown #2 and S.O.S. all depend on a drop. |
| Breakdown | No dip long enough: shorter than your **Minimum breakdown length**, earlier than a quarter of the track, or too close to the end |
| Drop #2, Breakdown #2 | The track does not repeat its structure in a way the detector can see |
| End / Fade-out | No steady run of beats at the end could be found |
| S.O.S. | **Emergency loop** is off, or no suitable late section exists |

If a cue you expected is missing or wrong, [move or add a cue](../analysis/corrections/move-or-add-cues.md) or [reuse one layout on many tracks](../analysis/corrections/cue-templates.md). A hand-made or hand-moved cue keeps your name, colour and position until the track is analysed again.

## Related

- [Jump to the drop with cues](../analysis/jump-to-cues.md)
- [Choose how cues are placed](../analysis/cue-rules.md)
- [Fix a wrong cue, loop or beatgrid](../analysis/corrections/index.md)
- [How are cues placed?](../how-it-works/cues.md)
- [Cue generation settings](settings/cue-generation.md)

<!-- verified: crates/cuerithm-library/src/publication.rs (analysis names mapped to published rows; S.O.S. row); crates/camelot-core/src/lexicon_publication.rs (CueTemplate::default rows: names, colours; slot assignment by index; emergency loop slot; PublishedType loop for active loop); crates/camelot-core/src/lexicon_cues.rs (detector names and raw colours); apps/desktop/src/display.ts (cueColor, publishedColors); apps/desktop/src/components/PlayerPanel.tsx (cueSlot letters, cue strip); apps/desktop/src/components/RowPreview.tsx (tooltip letters); apps/desktop/src/components/CueTemplateEditor.tsx (cuePalette); apps/desktop/src/published-analysis.ts (player uses the published cues when present); apps/desktop/src/test/player.test.tsx ("F End / Fade-out 2:39.8", #aa3bff); apps/desktop/src-tauri/src/builtin.rs (lexicon_timing forced on, so every prepared track has a publication); legacy technical/cues.md re-checked. Facts-sheet discrepancy: the private demo-library facts sheet lists the detector names (Second Drop, Last beat); the player shows the published names (Drop #2, End / Fade-out). Demo rows convert the facts-sheet names. UNVERIFIED: the one-line descriptions of what each cue marks are paraphrased from the detector logic and legacy technical/cues.md, not checked by listening; the breakdown exclusions (before 25% of the track, within 30 seconds of the end) are the constants BD_MIN_POS and BD_MIN_GAP_SEC. -->
