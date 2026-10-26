# How are cues placed?

<p class="lead">Cues are the structural landmarks you see in the player: Start, Drop, Breakdown, a second Drop and Breakdown, the end of the track and, optionally, an emergency loop. They are found from the track's low-frequency energy, bar by bar.</p>

!!! abstract "What this means for you"
    - **Treat automatic cues as starting points.** Audition them. A track with an unusual structure can get no Breakdown, or a Drop you would place elsewhere.
    - **Cues are only as good as the grid.** They are placed on the BPM and first beat from [How are BPM and the beatgrid found?](tempo.md). Fix a wrong grid first.
    - **Three settings change the result**: **Minimum breakdown length**, **Drop at start** and **Emergency loop**. Re-analyse a track after changing them.
    - **Cues never carry an energy label.** Energy is one rating for the whole track: see [How is energy rated?](energy.md).
    - Move, add or template cues yourself with [Fix a wrong cue, loop or beatgrid](../analysis/corrections/index.md).

!!! note "Provenance"
    The cue detector is described in these docs as the "validated Lexicon-compatible DSP". Its calculations are kept exactly as validated, so this page explains what it does and what you can change, not its internal formulas. See [How CueRithm was made](../help/about/provenance.md) for the origin and limits of this work.

## What the detector needs { #inputs }

The left channel of the track at 44.1 kHz, the unrounded BPM and first beat from [How are BPM and the beatgrid found?](tempo.md), and the track's genre tag. If timing failed, there are no cues and the track cannot become ready.

## What the detector looks at { #front-end }

The detector follows the bass and kick, not the whole mix. It filters the track so that only the low end remains, then cuts the track into four-beat sections, one bar each, and measures each section's energy and how strongly each beat lands. Every cue is a position on that bar grid.

## How a cue is chosen, stage by stage { #stages }

Each stage builds on the one before.

| Order | Cue | What the detector looks for |
|---|---|---|
| 1 | **Drop** | The first section where the low-end energy rises to near its peak and stays there. Two ways of measuring are compared and reconciled, then the result is snapped to the bar grid |
| 2 | **Breakdown** | A sustained dip in energy after the Drop, in the middle part of the track. It must be at least your **Minimum breakdown length** |
| 3 | **Second drop** | Energy returning to full strength after the Breakdown, up to about four fifths of the track, with fallbacks when the principal rule finds nothing |
| 4 | (**Drop at start**) | If set to Never and the first Drop sits at the very start, a later big Drop can become the Drop, unless the track stays high in energy throughout |
| 5 | **Second breakdown** | A second dip after the second drop, with a simpler fallback |
| 6 | **Last beat** | The last sustained stretch of full-strength beats, snapped to a 16-beat grid |
| 7 | **Emergency loop** (optional) | A 16-beat loop in a steady, late block of the track |

The Start cue always exists when timing exists. The genre tag influences only tracks whose genre contains "techno" or "hardcore"; see [Choose how cues are placed](../analysis/cue-rules.md).

Using the demo library: "Atoma (Extended Mix)" at <span class="cr-bpm">130</span> BPM gets all six cues: start at 0:00, drop at 0:44, breakdown at 1:36, second drop at 2:24, second breakdown at 3:23 and last beat at 3:42.

## Names, colours and types { #published-names-colours-and-types }

The detector's raw names are published through a cue template. What the player shows is the published name and colour.

| Detector name | Published as | Colour | Type |
|---|---|---|---|
| Start | Start | yellow | cue |
| Drop | Drop | red | cue |
| Breakdown | Breakdown | blue | cue |
| Second Drop | Drop #2 | red | cue |
| Second Breakdown | Breakdown #2 | blue | cue |
| Last beat | End / Fade-out | violet | cue |
| Emergency loop | S.O.S. | magenta | active loop, 16 beats |

Slots, shown as letters in the player, follow time order, and the emergency loop takes the next slot after the others. The full list is in [Cue names, letters and colours](../reference/cue-types.md).

## When a cue is missing { #when-a-cue-is-missing }

- **No timing, silence or empty audio:** no cues at all.
- **No Drop:** no Breakdown, second drop, second breakdown or loop either. The Last beat can still exist.
- **No Breakdown:** the dip was shorter than your minimum, very early in the track, or in its last half minute. "Prophecy (Extended Mix)" has a Drop at 0:59 and a second drop at 2:57 but no Breakdown between them. "Brackish (Original Mix)" has only a start, a drop at 0:31 and a last beat at 2:45.
- **Second drop and Last beat** may not be found.

To add a missing cue by hand, see [Move or add a cue](../analysis/corrections/move-or-add-cues.md).

## What happens to old cues { #revisions }

Each preparation stores a new cue revision, and earlier revisions are kept. Cues are recomputed when the effective genre changes after tagging or cleanup, with key, BPM and energy preserved. Cues are never written to file tags. A failed preparation keeps the previous cues.

## Settings that matter { #settings-that-matter }

<!-- figure:existing-11-settings-cue-generation-how-it-works-cues -->
<figure markdown>
![Cue generation settings](../assets/screens/11-settings-cue-generation.webp){ .screen loading=lazy }
<figcaption>The settings that change where cues land.</figcaption>
</figure>
<!-- /figure -->

| Setting (**Settings → Cue generation**) | Default | Effect |
|---|---|---|
| **Minimum breakdown length** | 64 beats (1 to 4,096) | A breakdown's length is rounded up to a multiple of 16 beats, then compared with this. Shorter breakdowns are ignored |
| **Drop at start** | Never (or Automatic) | Whether a Drop can sit at the very start of the track |
| **Emergency loop** | Off | Adds the S.O.S. loop when the track has a suitable section |

Automatic preparation uses these settings and the standard cue template. [Manual corrections and reusable templates](../analysis/corrections/cue-templates.md) are separate, and a tempo-range option is not available. Settings are explained in [Cue generation settings](../reference/settings/cue-generation.md).

## Where you see it in the app { #where-you-see-it }

<!-- figure:how-player-cues -->
<figure markdown>
![The player showing Atoma (Extended Mix) with cue markers at 0:00, 0:44, 1:36, 2:24, 3:23 and 3:42 on the waveform](../assets/screens/how-player-cues-light.webp#only-light){ .screen loading=lazy }
![The player showing Atoma (Extended Mix) with cue markers at 0:00, 0:44, 1:36, 2:24, 3:23 and 3:42 on the waveform](../assets/screens/how-player-cues-dark.webp#only-dark){ .screen loading=lazy }
<figcaption>Each cue is a labelled position on the bar grid; the track needs at least a Start to become ready.</figcaption>
</figure>
<!-- /figure -->

| Place | What shows |
|---|---|
| The player | Cue markers with letters on the waveform and the beat grid. See [Jump to the drop with cues](../analysis/jump-to-cues.md) |
| **Preview** column in Tracks | Small cue ticks on each track's waveform, one for every saved cue and loop. Hovering lists them by their saved names, the same as in the player, so the column follows a correction |
| **Settings → Cue generation** | The three settings above, and a note that existing cues are replaced after a successful analysis |

## Related

- [Jump to the drop with cues](../analysis/jump-to-cues.md)
- [Choose how cues are placed](../analysis/cue-rules.md)
- [Fix a wrong cue, loop or beatgrid](../analysis/corrections/index.md)
- [How accurate is it?](accuracy.md)

<!-- verified: crates/camelot-core/src/lexicon_cues.rs (stage order in generate_structural_cues; LexiconConfig; breakdown span rounded up to 16 beats against min_cue_beats; classify_genre techno or hardcore; Start always created), structural_cues.rs (1..=4096 beats), lexicon_publication.rs (default template names, labels, colours), publication.rs (S.O.S. active loop magenta, 16 beats, Last beat template row), jobs.rs (cue revisions), apps/desktop/src/views/Settings.tsx (labels, defaults), src-tauri builtin.rs, RowPreview.tsx (cues in the preview), published-analysis.ts, docs/release-readiness.md (factory publication rows); demo data from the private demo-library facts sheet (raw detector names used there). Described by effect only: thresholds and formulas of lexicon_cues.rs are deliberately not reproduced -->
