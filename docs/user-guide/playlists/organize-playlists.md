# Group playlists in folders and reorder them

<p class="lead">Keep the sidebar tidy: nest playlists in folders, put them in the order you use them, pin the ones you open every night, and bring back one you removed. Each change takes a few seconds.</p>

Everything on this page changes only how playlists are arranged in CueRithm. Your audio files are never touched, and removing a playlist never deletes music.

## Know the three kinds of item { #item-types }

| Kind | Sidebar icon | Who chooses the tracks | Can hold |
|---|---|---|---|
| **Playlist** | music note | You, by hand ([add tracks](edit-playlist-tracks.md#add-tracks)) | Tracks, in an order you control |
| **Folder** | folder | The playlists inside it | Playlists, smart playlists and other folders |
| **Smart playlist** | lightning bolt | Rules ([build one](build-a-smart-playlist.md)) | Nothing by hand; it follows its rules |

The type is fixed when you create an item. To turn a smart playlist into a playlist you can edit by hand, take a [snapshot](combine-and-tidy-playlists.md#snapshot).

A small dot at the right of a smart playlist's name means it is not up to date: amber while it is waiting or refreshing, red when its last refresh failed. See [Keep smart playlists up to date](build-a-smart-playlist.md#it-stays-current) for what CueRithm does about it.

## Open the playlist menu { #playlist-menu }

Every row in the sidebar has a **⋯** button on the right (named **Playlist actions for** the playlist's name). It opens this menu:

<!-- figure:existing-58-playlist-menu-organize-playlists -->
<figure markdown>
![The playlist menu with Edit, Duplicate, Add to favorites, Move up, Move down, Move to folder and Remove](../assets/screens/58-playlist-menu.webp){ .screen loading=lazy }
<figcaption>The ⋯ menu beside each playlist.</figcaption>
</figure>
<!-- /figure -->

<!-- figure:playlists-menu -->
<figure markdown>
![The playlist actions menu open beside Warm-up set listing Edit, Duplicate, Add to favorites, Move up, Move down, Move to folder and Remove](../assets/screens/playlists-menu-light.webp#only-light){ .screen loading=lazy }
![The playlist actions menu open beside Warm-up set listing Edit, Duplicate, Add to favorites, Move up, Move down, Move to folder and Remove](../assets/screens/playlists-menu-dark.webp#only-dark){ .screen loading=lazy }
<figcaption>Every row has a ... menu with the same actions.</figcaption>
</figure>
<!-- /figure -->

| Item | What it does |
|---|---|
| **Edit** | Change the name or the parent folder. For a smart playlist it also opens the rules. The type cannot change. |
| **Duplicate** | Make a copy with the same name at the end of the same folder, and open it. A folder is copied with everything inside it. |
| **Add to favorites** / **Remove from favorites** | Pin or unpin the item. |
| **Move favorite up** / **Move favorite down** | Reorder your favourites. Only shown for a favourite. |
| **Move up** / **Move down** | Move one place among its siblings. Greyed out at the ends. |
| **Move to folder…** | Pick a **Destination folder** and move it there. |
| **Remove** | Hide it, and everything inside it. See [remove and restore](#remove-restore). |

Names can be 1 to 255 characters. They do not have to be unique, but give each item its own name so you can tell them apart.

## Put a playlist in a folder { #move-to-folder }

You have two ways to do it.

<!-- figure:playlists-move-dialog -->
<figure markdown>
![The Move Peak hour dialog with its Destination folder menu set to Club nights, and the Cancel and Move buttons](../assets/screens/playlists-move-dialog-light.webp#only-light){ .screen loading=lazy }
![The Move Peak hour dialog with its Destination folder menu set to Club nights, and the Cancel and Move buttons](../assets/screens/playlists-move-dialog-dark.webp#only-dark){ .screen loading=lazy }
<figcaption>Destination folder lists every folder that is not the item or inside it.</figcaption>
</figure>
<!-- /figure -->

**With the menu.**

1. Open **⋯** on the playlist and choose **Move to folder…**.
2. In **Destination folder**, choose the folder. **Your playlists** means the top level. The list never offers the item itself or anything inside it.
3. Click **Move**. The playlist becomes the last item in that folder.

**By dragging.**

1. Press on a playlist row and move the pointer a few pixels. A label with its name follows the pointer.
2. Drop it on a folder to make it that folder's last item, or on another playlist to place it immediately before that playlist.
3. To bring it back to the top level, drop it on the **PLAYLISTS** heading or on **Move to top level**, which appears while you drag.

Press ++esc++ to cancel a drag. A folder cannot be dropped into itself or into anything inside it.

## Reorder playlists in the sidebar { #reorder }

Use **Move up** and **Move down** in the **⋯** menu to move an item one place among the items in the same folder. Drag it onto another playlist in the same folder to place it before that one. To sort a whole folder by name or by size at once, use [Sort playlists](combine-and-tidy-playlists.md#sort).

## Rename a playlist or change its folder { #rename }

1. Open **⋯** and choose **Edit**.
2. Change **Playlist name** or **Parent folder**.
3. Click **Save changes**.

For a smart playlist, CueRithm asks you to click **Preview matches** before **Save changes** is available, even when you only rename it. That keeps the saved result in step with its rules.

## Pin the playlists you open most { #favorites }

Choose **Add to favorites** in the **⋯** menu. The playlist also appears in a short list above the tree, marked with a star, so you can open it without expanding folders. **Move favorite up** and **Move favorite down** reorder that list. Choose **Remove from favorites** to unpin it; the playlist stays where it is in the tree.

## Make a copy to change a version { #duplicate }

Choose **Duplicate**. The copy has the same name and sits at the end of the same folder, and CueRithm opens it. A copy of a playlist holds the same tracks. A copy of a smart playlist has the same rules and is yours to change, which is how you customise a list from [Automatic Smart Playlists](automatic-smart-playlists.md#customise). Favourite marks are not copied.

## Remove a playlist and bring it back { #remove-restore }

1. Open **⋯** and choose **Remove**. The item and everything inside it disappears from the tree. If it was open, CueRithm returns to **Tracks**.
2. To get it back, open **Removed playlists** near the bottom of the playlist list. The number beside it counts the items you can restore.
3. Click **Restore** beside the name.

The note in the list says it: *Restore playlists and their contents. Audio files stay in your library.* Restoring brings back everything that was removed together, in its old folder when that folder still exists and at the top level when it does not.

!!! warning "Gotcha"
    A smart playlist whose rules use a removed playlist shows an error until you restore that playlist or change its rules. It keeps showing its last good result in the meantime.

## Undo a playlist change { #undo }

The arrow beside **+** is **Undo last playlist change**. It reverses the most recent change, then the one before it, and so on. That includes moves, renames, removals, tracks added or removed, and everything a [playlist tool](combine-and-tidy-playlists.md) or the [generator](generate-playlists.md) applied in one step. Undo is saved with your library, so it still works after you restart CueRithm. There is no redo.

If a newer change touches the same items, Undo refuses and shows the reason, which starts *Newer playlist edits prevent Undo*. Nothing is lost; make the correction by hand.

## Send playlists to your DJ software { #send-to-dj-software }

Playlists live in your CueRithm library. To use them in DJ software, send them with [Sync](../sync/index.md), which writes to seven programs, including rekordbox and [Mixxx](../sync/mixxx.md). [Choose what to sync](../sync/choose-what-to-sync.md) shows how to send one playlist or a whole folder. Supported smart playlists stay dynamic. Where a program has no smart playlists, as in Mixxx, a smart playlist arrives with its saved tracks as an ordinary playlist or crate. CueRithm has no standalone playlist import and no M3U or XML export; see [what CueRithm does not do yet](../help/not-available-yet.md).

## What can go wrong

| What you see | Why | What to do |
|---|---|---|
| **Move to folder…** does not list a folder | You cannot move an item into itself or into something inside it | Move the inner item out first |
| The ⋯ button and **+** are greyed out | CueRithm is finishing another playlist change | Wait a moment and try again |
| *Playlists changed. Reopen Move before continuing.* | Another change landed while the dialog was open | Close it and open **Move to folder…** again |
| A smart playlist shows a dot | It is refreshing or its last refresh failed | Open it; the banner above the table gives the reason. See [Fix a problem](../help/troubleshooting/index.md) |

## Related

- [Create a playlist and add tracks](create-a-playlist.md)
- [Reorder, remove and restore tracks](edit-playlist-tracks.md)
- [Combine, split and tidy playlists](combine-and-tidy-playlists.md)
- [Status icons and job states](../reference/status-icons.md)

<!-- verified: apps/desktop/src/components/Sidebar.tsx; components/PlaylistMenu.tsx; components/PlaylistMoveDialog.tsx; views/PlaylistEditor.tsx; App.tsx (canDropInternal, dropInternal, undoPlaylist, playlistAction); use-playlist-drag.ts (6 px threshold, Escape); crates/cuerithm-library/src/playlists/edits.rs (Duplicate keeps name, Remove/Restore groups), history.rs (Undo conflicts, no redo), evaluate.rs (unavailable playlist reference); src/test/playlist-actions.test.tsx; Sync.tsx note on smart playlists; legacy guides/sync-mixxx.md and about/roadmap.md re-checked; legacy guides/playlists.md re-checked -->
