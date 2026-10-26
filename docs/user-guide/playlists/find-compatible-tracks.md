# Find tracks that mix with the one playing

<p class="lead">Pick a track and get a ranked list of prepared tracks whose key, BPM and energy sit close to it, with the reason for each match. Hear any of them in the player, then save the ones you like as a playlist. It takes a couple of minutes.</p>

**Before you start:** the source track must be prepared and not archived, with the measurements you want to match. [Read a track's key, BPM and energy](../analysis/read-key-bpm-energy.md) explains where they come from.

## Choose the source track { #source }

1. In **Tracks**, select the track you are mixing from. You can also load it in the player.
2. Under **TOOLS** in the sidebar, click **Compatible tracks**.

The selected track becomes the source. If nothing is selected, the track loaded in the player is used. To pick another, use **Search your library** (type an artist or title) and the **Source track** menu. **Load more source tracks** shows more. Under the menu you see the source's key, BPM and energy, for example `7A · 130 BPM · Energy 7/10` for *Come To Life (Original Mix)*.

## Decide what "compatible" means { #options }

<!-- figure:playlists-compatible-options -->
<figure markdown>
![Compatible tracks with Come To Life (Original Mix) as the source track showing 7A, 130 BPM and Energy 7/10, with Match compatible keys on in Harmonic mode and Match BPM on with a tolerance of 4](../assets/screens/playlists-compatible-options-light.webp#only-light){ .screen loading=lazy }
![Compatible tracks with Come To Life (Original Mix) as the source track showing 7A, 130 BPM and Energy 7/10, with Match compatible keys on in Harmonic mode and Match BPM on with a tolerance of 4](../assets/screens/playlists-compatible-options-dark.webp#only-dark){ .screen loading=lazy }
<figcaption>The starting options: harmonic keys and a BPM tolerance of 4.</figcaption>
</figure>
<!-- /figure -->

| Control | What it does | Starts as |
|---|---|---|
| **Match compatible keys** | Keeps tracks whose key mixes with the source's | On |
| **Key mode** | **Harmonic** or **Fuzzy**, for this search only | Your **Key compatibility** setting, normally **Harmonic** |
| **Match BPM** | Keeps tracks within a BPM distance of the source | On |
| **BPM tolerance (±)** | The distance, from 0 to 300; the boundary counts | `4` |
| **Include half/double BPM** | Also compares each candidate at half and at double its BPM | Off |
| **Match whole-track energy** | Keeps tracks whose energy rating is close | Off |
| **Energy tolerance (±)** | The distance in rating points, 0 to 9 | `1` when switched on |
| **Genre restriction** | Keeps only this exact genre value, ignoring case | Any genre |
| **Custom tags (optional)** | Keeps tracks that carry every tag you tick | None |

**Harmonic** accepts the same key number on either letter and the neighbouring numbers on the same letter, so `7A` accepts `6A`, `7A`, `8A` and `7B`. **Fuzzy** accepts the same or a neighbouring number on either letter. See [Understand Camelot keys](../analysis/camelot-keys.md). Changing the mode here does not change your library setting; that lives in [Library preferences](../reference/settings/library.md).

Half and double BPM lets a 64 BPM track match a 128 BPM source. The result says which comparison it used, and the track's saved BPM does not change.

## Find and read the matches { #results }

Click **Find compatible tracks**. The page shows *N compatible tracks*, how many tracks were checked, and how many were excluded because a measurement you asked for is unknown. A track with no key, BPM or energy cannot match a check that needs it.

<!-- figure:playlists-compatible-results -->
<figure markdown>
![Five compatible tracks for Come To Life with reasons such as Same key: 7A and Compatible key: 7A to 8A (harmonic), starting with When The Fire Starts To Burn, then Say Less, Dancin, 17 and Other Dimension](../assets/screens/playlists-compatible-results-light.webp#only-light){ .screen loading=lazy }
![Five compatible tracks for Come To Life with reasons such as Same key: 7A and Compatible key: 7A to 8A (harmonic), starting with When The Fire Starts To Burn, then Say Less, Dancin, 17 and Other Dimension](../assets/screens/playlists-compatible-results-dark.webp#only-dark){ .screen loading=lazy }
<figcaption>Each match lists why it was included; the same key comes first.</figcaption>
</figure>
<!-- /figure -->

Matches are ordered by exact key first, then the smaller BPM difference, then the smaller energy difference, then artist and title. Every row lists the reasons it matched, such as `Same key: 7A` or `Compatible key: 7A → 8A (harmonic)`, and its key, BPM and energy.

For *Come To Life (Original Mix)* with the starting options, the purchase folder gives five matches:

| # | Track | Key | BPM | Why it is here |
|---|---|---|---|---|
| 1 | When The Fire Starts To Burn (Extended Mix) | 7A | 126 | Same key; 4 BPM away, at the edge of ±4 |
| 2 | Say Less (Extended Mix) | 8A | 130 | Neighbouring key; same BPM |
| 3 | Dancin (Extended Mix) | 6A | 131 | Neighbouring key; 1 BPM away |
| 4 | 17 (Extended Mix) | 6A | 128 | Neighbouring key; 2 BPM away |
| 5 | Other Dimension (Extended Mix) | 6A | 128 | Neighbouring key; 2 BPM away |

The same key comes first even though its BPM is furthest away. Rows 4 and 5 tie, so artist breaks it.

Change a control to widen or narrow the search. Switching on **Match whole-track energy** at `0` keeps only *17 (Extended Mix)* and *Other Dimension (Extended Mix)*, which share the source's 7/10. Switching off **Match compatible keys** and setting the BPM tolerance to `2` gives 19 tracks, so the key is doing most of the narrowing. Any change clears the old results, so run **Find compatible tracks** again.

!!! note
    A match is a preparation aid, not a verdict. Click **Preview** on a row to play it in the player, and listen before you decide on a transition.

## Hear a match and open it { #audition }

- **Preview** loads the track in the persistent player and plays it.
- Clicking a track's title selects it, so it is already selected when you go **Back to library**.

## Save the matches as a playlist { #save }

1. Tick the tracks you want, or **Select all N matches**.
2. Type a **Playlist name** (it starts as `Compatible tracks`).
3. Click **Save selected to playlist**.

CueRithm creates an ordinary playlist at the top level of the sidebar, in the order shown, and opens it. The page confirms with *Saved N tracks to …. Use sidebar Undo to revert.* The playlist is a snapshot: it does not change when your library does. Use a [smart playlist rule](recipes/tracks-that-mix-with-a-key.md) when you want a list that stays current.

## What can go wrong

| What you see | Why | What to do |
|---|---|---|
| **Find compatible tracks** is greyed out | No source is chosen, or a tolerance is out of range | Choose a source; keep BPM 0 to 300 and energy 0 to 9 |
| *Source key is unknown. Prepare or correct the source, or disable key matching.* (or BPM, or energy) | The source lacks a measurement you asked to match | Prepare the track, or switch that check off |
| *Choose a prepared source track from Tracks* | The source is archived or still in Incoming | Choose another source |
| *No matches. Widen the tolerances or disable a restriction and search again.* | Nothing fits all enabled checks | Raise the BPM tolerance or switch a check off |
| *Library changed. Find compatible tracks again.* | A track changed while you looked | Click **Find compatible tracks** again |

## Related

- [Check the flow of a playlist](check-playlist-flow.md)
- [Collect the tracks that mix with a key](recipes/tracks-that-mix-with-a-key.md)
- [Preview a track with the player](../analysis/preview-a-track.md)
- [How is the key detected?](../how-it-works/key.md)

<!-- verified: apps/desktop/src/views/CompatibleTracks.tsx (labels, defaults, messages, save flow); App.tsx (source from selection or player track, toolSaved opens playlist); compatible-types.ts; crates/cuerithm-library/src/compatible.rs (rules, ordering, errors, reasons); crates/cuerithm-smartlists/src/keys.rs; src/test/compatible-tracks.test.tsx; example computed from _meta/the private demo-library facts sheet by re-implementing compatible.rs ordering; legacy guides/compatible-tracks.md re-checked -->
