# Build a warm-up playlist from BPM and energy

<p class="lead">A smart playlist that gathers the steady, lower-energy tracks to open a night with, so you pick your first hour from a short list instead of the whole library.</p>

<div class="recipe-meta" markdown>
**Uses:** a smart playlist with the fields **BPM** and **Energy**.  
**Works on:** prepared tracks that have a measured BPM and energy rating.  
**Time:** About three minutes.  
**Undo:** Remove the playlist from its **⋯** menu, or click **Undo last playlist change** straight after creating it.
</div>

## The problem

You open a night and want tracks that sit in a narrow BPM band at moderate energy. Scrolling 26 tracks to find them by eye is slow, and a list sorted by BPM still mixes in the loud ones.

| Before | After |
|---|---|
| 26 tracks in **Tracks**, 126 to 160 BPM, energy 5 to 8 | **Warm-up candidates**: 7 tracks, 126 to 128 BPM, energy 5 to 6 |

The seven tracks in the Beatport purchase folder used in this guide:

| Track | BPM | Key | Energy |
|---|---|---|---|
| When The Fire Starts To Burn (Extended Mix) | 126 | 7A | 6/10 |
| Palm Of Your Hands (Extended Mix) | 127 | 11A | 6/10 |
| Desert (Original Mix) | 128 | 12A | 6/10 |
| F\*ck The Club (Extended Mix) | 128 | 12A | 6/10 |
| Murda (Extended Mix) | 128 | 1A | 6/10 |
| Shake The Block (Extended Mix) | 128 | 1A | 6/10 |
| Undercurrent (Original Mix) | 128 | 9A | 5/10 |

## The solution

<!-- figure:playlists-recipe-warm-up-editor -->
<figure markdown>
![The smart playlist editor named Warm-up candidates with BPM is between 124 and 128 and Energy is between 1 and 6, Match all conditions, and the preview reading 7 matching tracks](../../assets/screens/playlists-recipe-warm-up-editor-light.webp#only-light){ .screen loading=lazy }
![The smart playlist editor named Warm-up candidates with BPM is between 124 and 128 and Energy is between 1 and 6, Match all conditions, and the preview reading 7 matching tracks](../../assets/screens/playlists-recipe-warm-up-editor-dark.webp#only-dark){ .screen loading=lazy }
<figcaption>Two between conditions: a BPM window and an energy window.</figcaption>
</figure>
<!-- /figure -->

1. Click **+** beside **PLAYLISTS**, type `Warm-up candidates` and set **Playlist type** to **Smart playlist**.
2. Make the first condition the field **BPM**, the operator **is between**, from `124` to `128`.
3. Click **Add condition**. Set the field to **Energy**, the operator to **is between**, from `1` to `6`.
4. Leave **Match** on **all conditions**.
5. Click **Preview matches**. It should read *7 matching tracks*.
6. Click **Create**.

The exact rules:

```text
Match: all conditions
BPM      is between   124 to 128
Energy   is between   1 to 6
```

Open the playlist and click the **BPM** column header to read it from the slowest track up. Pick the ones you like and add them to your hand-ordered set with [Reorder, remove and restore tracks](../edit-playlist-tracks.md#add-tracks).

## How it works

**is between** includes both ends, so a 128 BPM track is in and a 129 BPM track is out. BPM is compared to two decimals. Energy is the whole-track rating from 1 to 10, so `1 to 6` keeps everything up to 6.

Why not **Energy is at most 6**? A track with no energy rating counts as 0 for that operator and would slip in. **is between** only matches tracks that have a value. See [Write smart playlist rules](../smart-rules.md#number-rules).

## Variations

- **A wider window.** `BPM is between 124 to 130` and `Energy is between 5 to 6` finds 13 tracks, adding *Atoma*, *Knockout*, *No Good*, *On The Ground*, *Say Less* and *Underground*.
- **Stay in key.** Add `Key is compatible with key 12A` to keep the five tracks that mix with `12A`: *Palm Of Your Hands*, *Desert*, *F\*ck The Club*, *Murda* and *Shake The Block*.
- **Freeze it.** Take a [snapshot](../combine-and-tidy-playlists.md#snapshot) to turn the list into a playlist you can reorder by hand.

## Related

- [Create a playlist and add tracks](../create-a-playlist.md)
- [Check the flow of a playlist](../check-playlist-flow.md)
- [Collect your peak-time tracks by energy](peak-time-playlist.md)
- [Write smart playlist rules](../smart-rules.md)

<!-- verified: apps/desktop/src/smart-rules.ts, components/SmartRuleEditor.tsx, views/PlaylistEditor.tsx (labels and flow); crates/cuerithm-smartlists/src/numeric.rs (NumberBetween inclusive, needs a value; missing counts as 0), snapshot.rs field labels BPM and Energy; counts and tracks computed from _meta/the private demo-library facts sheet -->
