# Playlist recipes

<p class="lead">Ready-made smart playlists and generator setups for the lists DJs build most: a warm-up, a peak hour, the tracks that mix with a key, this week's arrivals. Each recipe gives the exact rules, built only from fields CueRithm has.</p>

Every recipe is one problem and one solution. Pick the one nearest to your goal, copy its rule into the rule editor, and change the numbers to suit your music. The examples use the 26-file Beatport purchase folder that runs through this guide, so you can check each result against the track counts shown.

!!! tip "New to smart playlists?"
    Do [Build a smart playlist](../build-a-smart-playlist.md) first. It teaches the rule editor, the preview and the sidebar dot that every recipe relies on.

## Pick a recipe

| Your goal | Recipe | Built with | In the purchase folder |
|---|---|---|---|
| Choose an opening set at moderate energy | [Build a warm-up playlist from BPM and energy](warm-up-playlist.md) | **BPM** is between, **Energy** is between | 7 tracks |
| Gather the highest-energy tracks | [Collect your peak-time tracks by energy](peak-time-playlist.md) | **Energy** is at least | 9 tracks |
| Find the next track from a key | [Collect the tracks that mix with a key](tracks-that-mix-with-a-key.md) | **Key** is compatible with key | 6 tracks for `7A` |
| See what is new | [Keep a playlist of the tracks you added this week](added-this-week.md) | **Date added** is in the last | all 26 on the first day |
| Spot music no set uses | [Find tracks that are not in a set yet](tracks-not-in-a-set-yet.md) | `playlist` is not in playlist folder | depends on your sets |
| Browse by tempo band | [Make a playlist for every BPM range](playlist-per-bpm-range.md) | **Generate playlists**, **BPM ranges** | 4 playlists |
| Keep the long club versions together | [Collect only the extended mixes](collect-extended-mixes.md) | **Title** ends with | 20 tracks |

The playlist **Melodic house 128+**, which keeps melodic house at 128 BPM and up, is built step by step in [Build a smart playlist](../build-a-smart-playlist.md) (`genre contains Melodic House` and `BPM is at least 128`, 10 tracks).

## Use a recipe

1. Click **+** beside **PLAYLISTS** and set **Playlist type** to **Smart playlist**.
2. Enter the name and the conditions the recipe lists.
3. Click **Preview matches** and compare the count with the recipe.
4. Click **Create**.

Then adapt it. [Write smart playlist rules](../smart-rules.md) explains all or any, OR groups, dates, keys and the few comparisons that surprise people, and [Smart playlist fields and operators](../../reference/smart-playlist-fields.md) lists every field.

## Related

- [Build a smart playlist](../build-a-smart-playlist.md)
- [Write smart playlist rules](../smart-rules.md)
- [Generate playlists from your library](../generate-playlists.md)
- [Build playlists and sets](../index.md)

<!-- verified: each linked recipe checked against apps/desktop/src/smart-rules.ts, components/SmartRuleEditor.tsx, crates/cuerithm-smartlists/src and crates/cuerithm-library/src/playlists; counts computed from _meta/the private demo-library facts sheet -->
