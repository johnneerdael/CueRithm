# Find tracks that are not in a set yet

<p class="lead">A smart playlist that shows every track you have not placed in any playlist of a folder, so nothing you bought gets forgotten. It stays current as you fill your sets.</p>

<div class="recipe-meta" markdown>
**Uses:** a smart playlist with the field `playlist` and the operator **is not in playlist folder**.  
**Works on:** prepared tracks and a folder of ordinary playlists, such as **Club nights**.  
**Time:** About three minutes.  
**Undo:** Remove the playlist from its **⋯** menu, or click **Undo last playlist change** straight after creating it.
</div>

## The problem

Your sets live in the **Club nights** folder, in playlists such as **Warm-up set** and **Peak hour**. Months later you cannot tell which purchases never made it into any of them.

| Before | After |
|---|---|
| 26 tracks in **Tracks**, no way to see which ones no set uses | **Not in a set yet**: only the tracks that no playlist in **Club nights** holds |

As you add a track to a set, it leaves the list. As you add new music, it joins the list.

## The solution

1. Click **+** beside **PLAYLISTS**, type `Not in a set yet` and set **Playlist type** to **Smart playlist**. Leave **Parent folder** on **Your playlists**, the top level.
2. Set the first condition to the field `playlist` and the operator **is not in playlist folder**.
3. In the menu that appears, choose **Club nights**.
4. Click **Preview matches**, then **Create**.

The exact rule:

```text
Match: all conditions
playlist   is not in playlist folder   Club nights
```

## How it works

A folder rule counts every track in all the playlists and smart playlists inside the folder, once each. **is not in playlist folder** keeps the tracks outside that group. Because it is a smart playlist, CueRithm re-checks it whenever you change a playlist.

!!! warning "Gotcha"
    Keep the new playlist outside the folder it refers to. A smart playlist that depends on a folder containing itself is refused with *Smart playlist dependency cycle*.

## Variations

- **One playlist only.** Choose **is not in playlist** and pick **Warm-up set**. With the eight-track *Warm-up set* built in [Create a playlist and add tracks](../create-a-playlist.md), 18 of the 26 purchase-folder tracks remain.
- **Two playlists.** Use **all conditions** with one **is not in playlist** condition for each.
- **Only the good ones.** Add `Energy is at least 7` to list unplaced high-energy tracks.
- **A fixed list.** The tool **Combine playlists** with **Active library tracks absent from all selected playlists** makes the same list once, as an ordinary playlist. See [Combine, split and tidy playlists](../combine-and-tidy-playlists.md#combine).
- **Overlooked for a while.** Add `Date added is not in the last 30 days` to see only the unplaced tracks that have been in your library for over a month. See [Keep a playlist of the tracks you added this week](added-this-week.md) for how date rules work.

## Related

- [Write smart playlist rules](../smart-rules.md#playlist-rules)
- [Group playlists in folders and reorder them](../organize-playlists.md)
- [Combine, split and tidy playlists](../combine-and-tidy-playlists.md)
- [Keep a playlist of the tracks you added this week](added-this-week.md)

<!-- verified: apps/desktop/src/smart-rules.ts (membership operator labels, field playlist), components/SmartRuleEditor.tsx (playlist menu); crates/cuerithm-smartlists/src/membership.rs, lib.rs (reference_kind); crates/cuerithm-library/src/playlists/evaluate.rs (membership_context unions folder descendants), tree.rs (dependency cycle), tools.rs (AbsentFromAll uses prepared tracks); example count computed from _meta/library-facts.md and the Warm-up set order in create-a-playlist.md -->
