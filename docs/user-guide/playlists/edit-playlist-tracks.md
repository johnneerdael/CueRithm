# Reorder, remove and restore tracks

<p class="lead">Add tracks to a playlist, put them in the order you will play them, take them out again, and get back a track that disappeared. Most changes take one click or one drag.</p>

**Before you start:** you need an ordinary playlist. [Create a playlist and add tracks](create-a-playlist.md) shows how. Only ordinary playlists accept tracks by hand; see [what each kind allows](#read-only).

## Add tracks to a playlist { #add-tracks }

Select the tracks first (see [Work on many tracks at once](../analysis/select-many.md)), then use either way below.

<!-- figure:playlists-add-bar -->
<figure markdown>
![The Tracks view with four tracks selected and the playlist bar above the table showing the Choose playlist menu set to Club nights / Warm-up set next to the Add to playlist button](../assets/screens/playlists-add-bar-light.webp#only-light){ .screen loading=lazy }
![The Tracks view with four tracks selected and the playlist bar above the table showing the Choose playlist menu set to Club nights / Warm-up set next to the Add to playlist button](../assets/screens/playlists-add-bar-dark.webp#only-dark){ .screen loading=lazy }
<figcaption>Pick the destination under Choose playlist and click Add to playlist.</figcaption>
</figure>
<!-- /figure -->

**With the playlist bar.**

1. A bar of playlist controls appears above the track table as soon as something is selected.
2. Open the menu that reads **Choose playlist…**. It lists ordinary playlists only, with their folder, for example **Club nights / Warm-up set**.
3. Choose one and click **Add to playlist**.

**By dragging.**

1. Press on a selected track and move the pointer a few pixels. A label such as *3 tracks* follows it.
2. Drop it on an ordinary playlist in the sidebar. Expand the folder first if the playlist is inside one.
3. Press ++esc++ before you let go to cancel.

Folders and smart playlists do not highlight when you drag over them, because they take no tracks by hand.

Good to know:

- New tracks go to the end, in the order you selected them. A track is in a playlist at most once, so adding one that is already there changes nothing.
- ++cmd+a++ / ++ctrl+a++ selects every track in the current view, including rows you have not scrolled to.
- A track that is still in Incoming can join a playlist, but the playlist does not show it until preparation finishes. See [Follow a track through Incoming](../files/follow-incoming.md).

## Put the tracks in order { #reorder-tracks }

An ordinary playlist opens in **manual order**, the order you built. In that state you can reorder it two ways.

<!-- figure:playlists-order-arrows -->
<figure markdown>
![The Warm-up set in manual order with Palm Of Your Hands selected and the playlist bar showing Remove from playlist and the Order up and down arrows](../assets/screens/playlists-order-arrows-light.webp#only-light){ .screen loading=lazy }
![The Warm-up set in manual order with Palm Of Your Hands selected and the playlist bar showing Remove from playlist and the Order up and down arrows](../assets/screens/playlists-order-arrows-dark.webp#only-dark){ .screen loading=lazy }
<figcaption>With a track selected, the Order arrows move it one place at a time.</figcaption>
</figure>
<!-- /figure -->

1. **With the arrows.** Select one or more tracks. Next to **Order**, click **Move selected tracks up** or **Move selected tracks down**. Each click moves every selected track one place and keeps their order relative to each other.
2. **By dragging.** Drag the selected tracks onto another row. They land immediately before the row you dropped them on.

If you sorted the table by a column, the arrows are gone and a **Manual order** button takes their place. Click it to return to your order. Sorting is a view only; it never rewrites the playlist.

!!! note
    With a search typed in the search box, reordering moves the selected tracks among the tracks you can see. Members hidden by the search keep their places.

## Take tracks out of a playlist { #remove-tracks }

1. Open the playlist and select the tracks. Use ++cmd+a++ / ++ctrl+a++ to select all of them.
2. Click **Remove from playlist**.

The selection clears. The tracks stay in your library and in every other playlist, and the audio files are not touched. Removing a track from a playlist is not the same as [archiving it](../files/archive.md).

## Bring tracks back { #restore-tracks }

| What happened | How to get the track back |
|---|---|
| You removed tracks a moment ago | Click **Undo last playlist change** (the arrow beside **+**). Each click reverses one change; there is no redo. |
| You removed them earlier | Select them in **Tracks** and add them again. They land at the end. |
| The track vanished after you archived it | Open **Archive**, select the track and click **Restore**. A playlist keeps an archived track's place but hides it while it is archived, so it reappears where it was. |

## Know what each kind of item allows { #read-only }

| | Add tracks | Remove tracks | Reorder |
|---|---|---|---|
| **Playlist** | Yes | Yes | Yes, in manual order |
| **Folder** | No | No | No; it shows the tracks of the playlists inside it, once each |
| **Smart playlist** | No | No | No; sort the table by any column instead |

To pick tracks for a smart playlist, change its [rules](smart-rules.md). To freeze one into a playlist you can edit by hand, take a [snapshot](combine-and-tidy-playlists.md#snapshot).

## What can go wrong

| What you see | Why | What to do |
|---|---|---|
| **Add to playlist** is greyed out | No destination is chosen, or there is no ordinary playlist yet | Choose one under **Choose playlist…**, or create a playlist first |
| The playlist shows fewer tracks than you added | Some are archived or still in Incoming | Check **Archive** and **Incoming**; see [Archive tracks you no longer play](../files/archive.md) |
| The **Order** arrows are missing | Nothing is selected, or the table is sorted by a column | Select a track, and click **Manual order** if it is shown |
| *Track list changed. Select tracks and try again.* | The library changed between your selection and the edit | Select the tracks again |
| *Playlist changed; refresh before editing* | Another change landed first | Repeat the action |

## Related

- [Group playlists in folders and reorder them](organize-playlists.md)
- [Check the flow of a playlist](check-playlist-flow.md)
- [Work on many tracks at once](../analysis/select-many.md)
- [Fix a problem](../help/troubleshooting/index.md)

<!-- verified: apps/desktop/src/components/PlaylistMembershipControls.tsx; App.tsx (reorderSelected, canDropInternal, dropInternal, selectAll, archiveSelected, onSelect order); playlist-order.ts; use-playlist-drag.ts; crates/cuerithm-library/src/playlists/edits.rs (AddMembers, RemoveMembers, ReorderMembers), playlists/queries.rs (condition: archived and incoming members hidden for ordinary playlists), history.rs; src/test/playlist-membership.test.tsx, playlist-track-drag.test.tsx, playlist-status.test.tsx; legacy guides/playlists.md re-checked -->
