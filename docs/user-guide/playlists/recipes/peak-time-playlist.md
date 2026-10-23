# Collect your peak-time tracks by energy

<p class="lead">A smart playlist called <strong>Energy 7 and up</strong> that holds your highest-energy tracks and grows by itself as you add music.</p>

<div class="recipe-meta" markdown>
**Uses:** a smart playlist with the field **Energy**.  
**Works on:** prepared tracks that have an energy rating.  
**Time:** About two minutes.  
**Undo:** Remove the playlist from its **⋯** menu, or click **Undo last playlist change** straight after creating it.
</div>

## The problem

CueRithm rates every track once for its whole length, from 1 to 10. In the Beatport purchase folder used in this guide that is 15 tracks at 6/10, 8 at 7/10, 2 at 5/10 and 1 at 8/10, so a rating of 7 or more isolates the top of the library.

| Before | After |
|---|---|
| 26 tracks, energy 5 to 8 | **Energy 7 and up**: 9 tracks |

The nine tracks:

| Track | Key | BPM | Energy |
|---|---|---|---|
| 17 (Extended Mix) | 6A | 128 | 7/10 |
| Come To Life (Original Mix) | 7A | 130 | 7/10 |
| Here In My Arms (Enjoy The Silence) (Extended Mix) | 5A | 132 | 7/10 |
| Man Like Me (Extended Mix) | 4A | 128 | 7/10 |
| Nivk, Rabteu - PLV (Creeds remix - Extended) | 2A | 160 | 7/10 |
| Other Dimension (Extended Mix) | 6A | 128 | 7/10 |
| Play With Fire (Original Mix) | 12A | 140 | 8/10 |
| Prophecy (Extended Mix) | 4A | 130 | 7/10 |
| Vai Não Pare (Take My Body) (Extended Mix) | 2A | 132 | 7/10 |

## The solution

1. Click **+** beside **PLAYLISTS**, type `Energy 7 and up` and set **Playlist type** to **Smart playlist** ([the editor step by step](../build-a-smart-playlist.md#start)).
2. Set the first condition to the field **Energy**, the operator **is at least**, the value `7`.
3. Click **Preview matches**. It should read *9 matching tracks*.
4. Click **Create**.

The exact rule:

```text
Match: all conditions
Energy   is at least   7
```

## How it works

**Energy** is CueRithm's own whole-track rating from 1 to 10, measured when the track was prepared. There is one rating per track; it is not a rating of a section or of a cue. See [How is energy rated?](../../how-it-works/energy.md). The rule is checked again whenever a track is prepared or changes, so a new 8/10 track joins the list without any action. Tracks with no rating have a value of 0 and never reach 7; see [how numbers compare](../smart-rules.md#number-rules). To see a track's rating, read [Read a track's key, BPM and energy](../../analysis/read-key-bpm-energy.md).

## Variations

- **Keep the BPM sensible.** Add `BPM is between 126 to 135`. The list drops to seven tracks, leaving out the 160 BPM *PLV* and the 140 BPM *Play With Fire*.
- **Only the very top.** Change the value to `8`: one track, *Play With Fire (Original Mix)*.
- **Only one genre.** Add `genre contains Melodic House` for the melodic part of the peak.
- **A hand-ordered peak hour.** Use the list to choose from, then build an ordinary playlist as in [Create a playlist and add tracks](../create-a-playlist.md).

## Related

- [Build a warm-up playlist from BPM and energy](warm-up-playlist.md)
- [Write smart playlist rules](../smart-rules.md)
- [Read a track's key, BPM and energy](../../analysis/read-key-bpm-energy.md)
- [Check the flow of a playlist](../check-playlist-flow.md)

<!-- verified: apps/desktop/src/smart-rules.ts, components/SmartRuleEditor.tsx; crates/cuerithm-library/src/playlists/snapshot.rs (energy from analysis level, label Energy), crates/cuerithm-smartlists/src/numeric.rs (missing counts as 0); counts computed from _meta/the private demo-library facts sheet (energy 15 at 6, 8 at 7, 2 at 5, 1 at 8) -->
