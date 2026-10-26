# Make a playlist for every BPM range

<p class="lead">Let the playlist generator cut your library into 10-BPM bands in one go: a smart playlist for each tempo band you actually own, in a folder of its own.</p>

<div class="recipe-meta" markdown>
**Uses:** **Generate playlists** with the **BPM ranges** dimension.  
**Works on:** prepared, unarchived tracks that have a measured BPM.  
**Time:** About three minutes.  
**Undo:** Click **Undo last playlist change** (the arrow beside **+**) to remove the whole batch in one step.
</div>

## The problem

You want to browse by tempo: everything around 128, everything around 140. Sorting **Tracks** by BPM works one search at a time, and nothing keeps a band as its own list.

| Before | After |
|---|---|
| 26 tracks in one list | A folder **BPM bands** with four smart playlists |

The generator makes a list only for bands that hold a track. In the Beatport purchase folder used in this guide:

| Playlist | Tracks |
|---|---|
| `120 - 129.99` | 11 |
| `130 - 139.99` | 12 |
| `140 - 149.99` | 2 |
| `160 - 169.99` | 1 |

There is no `150 - 159.99` list because no track sits there.

## The solution

<!-- figure:playlists-generator-bpm -->
<figure markdown>
![Generate playlists with the New folder named BPM bands, Dimension 1 set to BPM ranges, and a preview of four smart playlists: 120 - 129.99 with 11 tracks, 130 - 139.99 with 12, 140 - 149.99 with 2 and 160 - 169.99 with 1](../../assets/screens/playlists-generator-bpm-light.webp#only-light){ .screen loading=lazy }
![Generate playlists with the New folder named BPM bands, Dimension 1 set to BPM ranges, and a preview of four smart playlists: 120 - 129.99 with 11 tracks, 130 - 139.99 with 12, 140 - 149.99 with 2 and 160 - 169.99 with 1](../../assets/screens/playlists-generator-bpm-dark.webp#only-dark){ .screen loading=lazy }
<figcaption>BPM ranges previews one smart playlist for every 10-BPM band that holds a track.</figcaption>
</figure>
<!-- /figure -->

1. Under **TOOLS**, click **Playlist tools**, then **Generate playlists** at the top right ([open the generator](../generate-playlists.md#open-generator)).
2. Leave **Target folder** on **New folder**, and type `BPM bands` in **New folder name**.
3. Open the menu for **Dimension 1** and choose **BPM ranges**.
4. Click **Preview playlists**. It should read *4 smart playlists* and *5 new nodes · 0 existing playlists reused*: four lists and the new folder.
5. Open **Conditions** on a row to see its rule, for example `BPM is between 120 – 129.99`.
6. Click **Apply preview**.

The generator settings:

```text
Target folder:      New folder, named BPM bands
Dimension 1:        BPM ranges
Combine dimensions: off
```

## How it works

Each list is an ordinary [smart playlist](../build-a-smart-playlist.md) with the rule **BPM** **is between** the band's start and end. Bands end in `.99` because BPM is [compared to two decimals](../smart-rules.md#number-rules), so a 129.99 BPM track belongs to the lower band and a 130 BPM track to the next. Every prepared track lands in exactly one band. The lists refresh like any smart playlist, so a new 128 BPM track appears in `120 - 129.99` without any action.

## Variations

- **Tempo and energy.** Add **Energy** as a second dimension and tick **Combine dimensions**. You get names such as `120 - 129.99 06 Energy`, one per pairing. Some pairings hold no track and still appear with `0 tracks` in the preview, so check the counts before you apply. <!-- UNVERIFIED: empty pairings are listed; read from generator.rs (no filter after the Cartesian product), not run in the app -->
- **One group per dimension.** Leave **Combine dimensions** off with two dimensions and each gets its own sub-folder, such as `BPM ranges` and `Energy`.
- **Run it again after new music.** A new band, for example `150 - 159.99`, gets a list when you generate again with **BPM bands** as the **Target folder**. Existing lists are reused. See [Generate playlists from your library](../generate-playlists.md#rerun).
- **Fixed bands of your choosing.** Use the **By BPM** option in **Settings**, **Smart Playlists**, which keeps adding lists for ranges you type, such as 126 to 130. See [Keep smart playlists up to date automatically](../automatic-smart-playlists.md#bpm-ranges).

## Related

- [Generate playlists from your library](../generate-playlists.md)
- [Keep smart playlists up to date automatically](../automatic-smart-playlists.md)
- [Write smart playlist rules](../smart-rules.md)
- [How are BPM and the beatgrid found?](../../how-it-works/tempo.md)

<!-- verified: apps/desktop/src/views/PlaylistGenerator.tsx (labels, preview texts, defaults); crates/cuerithm-library/src/playlists/generator.rs (observed values, bands only where tracks exist, separate sub-folders, combine product, new_nodes counts the folder), generator/templates.rs (ranges: floor to 10, end = start+10-0.01, rule NumberBetween), tests/playlist_generation.rs (fractional_bpm_boundaries); counts computed from _meta/the private demo-library facts sheet -->
