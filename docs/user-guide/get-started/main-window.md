# Find your way around the window

<p class="lead">CueRithm is one window with four parts: a sidebar on the left, a header across the top, a main area that shows tracks or a tool, and a player that stays at the bottom. Five minutes here tells you where everything lives and which page explains it.</p>

| Part | What you do there | Learn more |
|---|---|---|
| [Sidebar](#the-sidebar) | Switch between your library, playlists and tools | [Choose your path](choose-your-path.md) |
| [Header](#the-header) | Search, see progress, switch the theme, add music | [Add tracks](../files/add-tracks.md) |
| [Track table](#the-track-table) | Read key, BPM and energy, select tracks, act on them | [Find a track](../analysis/find-tracks.md) |
| [Player](#the-player) | Play a track, read the waveform, jump between cues | [Preview a track](../analysis/preview-a-track.md) |

## Move between your library, playlists and tools { #the-sidebar }

The sidebar has three groups and a footer. Click an item to open it in the main area.

<!-- figure:existing-crop-sidebar-main-window -->
<figure markdown>
![The sidebar listing Library, Playlists, Tools and Settings](../assets/screens/crop-sidebar.webp){ .screen loading=lazy }
<figcaption>The sidebar holds your library views, your playlists and the tools.</figcaption>
</figure>
<!-- /figure -->

**LIBRARY** holds the three views of your tracks. The number beside each is live, and the **Incoming** number is highlighted while tracks are waiting.

| Item | What it shows |
|---|---|
| **Tracks** | Every prepared track that is not archived. |
| **Incoming** | Tracks that are being prepared, or that need you. See [Follow a track through Incoming](../files/follow-incoming.md). |
| **Archive** | Tracks you set aside. They keep their analysis and cues. See [Archive tracks you no longer play](../files/archive.md). |

**PLAYLISTS** lists your playlists, folders and smart playlists. The two small buttons beside the heading are **Undo last playlist change** and **Create playlist** (+). Folders have a chevron, smart playlists have a lightning-bolt icon, and favourites are pinned above the tree with a star. The **…** button on a row opens its menu (**Edit**, **Duplicate**, **Add to favorites**, **Move up**, **Move down**, **Move to folder…**, **Remove**), and removed playlists wait under **Removed playlists** until you restore them. Drag tracks from the table onto a playlist to add them. Start with [Create a playlist and add tracks](../playlists/create-a-playlist.md) and [Group playlists in folders and reorder them](../playlists/organize-playlists.md).

**TOOLS** opens whole-screen tools. There is no Tools menu; these are the sidebar buttons.

| Item | Use it to | Page |
|---|---|---|
| **Playlist tools** | Combine, split and tidy playlists, or generate them | [Combine, split and tidy playlists](../playlists/combine-and-tidy-playlists.md) |
| **Compatible tracks** | Find tracks that mix with the one you select | [Find tracks that mix with the one playing](../playlists/find-compatible-tracks.md) |
| **Tracklist matcher** | Rebuild a set from pasted text or CSV | [Rebuild a set from a tracklist](../playlists/match-a-tracklist.md) |
| **Cleanup** | Preview and apply fixes to titles, genres and artists | [Clean up messy titles, genres and artists](../tagging/cleanup-messy-titles.md) |
| **Maintenance** | Find duplicates, missing files and unused files | [Look after your library](../files/library/index.md) |
| **Library backup** | Save and restore the library's state | [Back up your library](../files/library/back-up-library.md) |
| **Organize files** | Rename and sort files by a pattern | [Rename and sort files into folders](../rename/organize-files.md) |
| **Sync** | Send playlists, cues and grids to DJ software | [Send your library to DJ software](../sync/index.md) |

The footer holds **Settings**, a **LOCAL LIBRARY** label and a version label that shows only *0.1*. See [Open Settings](#settings).

Press ++cmd+b++ / ++ctrl+b++ to collapse the sidebar to a rail of icons. The rail opens again while the pointer or keyboard focus is on it, and the button at the top of the sidebar switches between collapsed and pinned open.

## Search, check progress and add music { #the-header }

The header runs across the top of the main area.

<!-- figure:existing-33-search-main-window -->
<figure markdown>
![Search results for a single artist](../assets/screens/33-search.webp){ .screen loading=lazy }
<figcaption>The search box filters the table as you type.</figcaption>
</figure>
<!-- /figure -->

- **Search library** (++cmd+f++ / ++ctrl+f++) filters the view you are in as you type; its hint reads *Search tracks, artists, genres…*. Press the shortcut from a tool or Settings and CueRithm returns to your last library view first. [Find a track: search, sort and filter](../analysis/find-tracks.md) covers what it matches.
- **Progress** reads *Preparing 26 tracks* while work is running, *3 need attention* when something stopped, and *All up to date* when nothing is left. A track that needs attention is explained in [Fix a track that needs attention](../files/needs-attention.md).
- **Colour theme** cycles System, Light and Dark each time you click it. See [Switch between light and dark](../customize/theme.md).
- **Add tracks** opens the system file picker and sends your files to Incoming. You can also drop files or folders anywhere on the window. [Add tracks: drop, pick or watch a folder](../files/add-tracks.md) has all three ways.

## Read, select and act on tracks { #the-track-table }

Above the table, the title shows the view you are in and its track count. When a playlist is open, **Playlist flow** appears there too ([Check the flow of a playlist](../playlists/check-playlist-flow.md)). The slider button opens **Preparation settings**.

<!-- figure:existing-crop-table-head-main-window -->
<figure markdown>
![The table header and the first rows](../assets/screens/crop-table-head.webp){ .screen loading=lazy }
<figcaption>One row per track, with key, BPM, energy and a preview.</figcaption>
</figure>
<!-- /figure -->

The default columns are **Track / Artist** (the title with the artist beneath it), **Genre**, **BPM**, **Key**, **Energy**, **Duration** and **Preview**, a mini waveform. Key is a chip such as <span class="cr-key">6A</span>, BPM shows two decimals, and energy is a number out of 10 with a bar. Use the columns button at the end of the header to change them: [Show the columns you need](../analysis/choose-columns.md). The icon in the narrow column before the title reports preparation: a spinner while working, a warning triangle when a track needs you, a check when a catalog match was written. [Status icons and job states](../reference/status-icons.md) lists every one.

Click a row to select it, ++cmd++ / ++ctrl++ + click to add or remove one, ++shift++ + click for a range, and ++cmd+a++ / ++ctrl+a++ to select the whole view. Double-click a row, or press its play button, to load it into the player. [Work on many tracks at once](../analysis/select-many.md) covers more.

Selecting tracks adds a toolbar:

| Button | What it does | Page |
|---|---|---|
| **Edit metadata** | Change tags and artwork | [Edit tags and artwork](../tagging/edit-tags-by-hand.md) |
| **Review match** | Compare catalog matches for one track | [Review and fix a wrong match](../tagging/review-a-match.md) |
| **Curate** | Add categories, a rating and notes | [Group tracks with categories and custom tags](../tagging/curate-tracks.md) |
| **Organize files** | Open the file organizer for the selection | [Rename and sort files into folders](../rename/organize-files.md) |
| **Analyze again** | Prepare the tracks again | [Analyze a track again](../analysis/analyze-again.md) |
| **Archive** / **Restore** | Move tracks out of or back into your library | [Archive tracks you no longer play](../files/archive.md) |
| **Add to playlist** | Add the selection to the playlist picked in the **Choose playlist…** menu beside it (a second bar under the toolbar) | [Create a playlist and add tracks](../playlists/create-a-playlist.md) |

**Drafts and history** is always there and lists tag edits and curation changes that are not saved yet. The footer under the table counts the tracks and reminds you that analysis is offline.

## Preview a track in the player { #the-player }

The player never leaves the bottom of the window. Load a track and it shows the title, artist and the track's **BPM**, **KEY** and **ENERGY**; play, pause and back-to-start buttons plus a **CUE** button that stops and jumps to the first cue; the elapsed and remaining time; and a volume slider.

<!-- figure:existing-crop-player-panel-2-main-window -->
<figure markdown>
![The player with the waveform and cue buttons](../assets/screens/crop-player-panel.webp){ .screen loading=lazy }
<figcaption>The player stays on screen under every view.</figcaption>
</figure>
<!-- /figure -->

Under that is the detailed waveform, headed **BEATGRID & CUES**, with beat lines and cue markers. It shows 32 seconds around the playhead; click it to show the entire track and click again to return. The overview below always shows the whole track, and clicking it seeks. The strip at the bottom has one button per cue with its letter, name and time. Click a button to jump there. **Edit cues & grid** opens corrections for the loaded track.

Press ++space++ to play or pause, and ++arrow-left++ / ++arrow-right++ to jump one bar. [Preview a track with the player](../analysis/preview-a-track.md) covers everything here, [Jump to the drop with cues](../analysis/jump-to-cues.md) is the tutorial, and [Check the beatgrid at beat level](../analysis/check-the-beatgrid.md) shows how to verify the grid.

## Make the window fit your screen { #layout }

Drag the vertical divider between the sidebar and the main area to resize the sidebar (180 to 340 pixels, 224 by default), and the horizontal divider above the player to change its height (200 to 420 pixels, 270 by default). Both are remembered on this computer. The window opens at 1440 by 940 pixels and does not shrink below 980 by 680. In a window narrower than 1250 pixels, the Edit metadata and Curate panels take over the table area instead of opening beside it. See [Resize the sidebar and player](../customize/layout.md).

## Switch between light and dark { #light-and-dark }

CueRithm follows your system theme until you choose otherwise. Click the **Colour theme** button until it shows the mode you want. Key, energy and cue labels stay readable in both, because they never rely on colour alone. [Choose a waveform style](../customize/waveform-style.md) changes how waveforms are drawn.

<!-- figure:existing-92-light-player-main-window -->
<figure markdown>
![CueRithm in the light theme](../assets/screens/92-light-player.webp){ .screen loading=lazy }
<figcaption>The same window in the light theme.</figcaption>
</figure>
<!-- /figure -->

## Open Settings { #settings }

**Settings** in the sidebar footer opens nine pages: **Incoming**, **Cue generation**, **Automatic tagging**, **Cleanup**, **File organization**, **Preparation presets**, **Library preferences**, **Smart Playlists** and **Appearance**. Settings that shape preparation are saved with **Save settings**. [Find the setting you need](../customize/find-a-setting.md) maps each setting to a page, and [Settings](../reference/settings/index.md) lists every control.

## What can go wrong { #what-can-go-wrong }

| What you see | Why | What to do |
|---|---|---|
| The sidebar shows only icons | It is collapsed | Press ++cmd+b++ / ++ctrl+b++ or use the pin button |
| The table is empty, or says *No matching tracks* | A search is active, or you are in Incoming, Archive or an empty playlist | Clear the search box and choose **Tracks** |
| The header says *3 need attention* | A stage failed on those tracks | [Fix a track that needs attention](../files/needs-attention.md) |
| A row shows only a dash under **Preview** | The file is missing, moved or unreadable | [I moved or renamed my files](../help/troubleshooting/files-moved.md) |
| The player says *Nothing playing* | No track is loaded | Double-click a row |
| The waveform is a dash or the track will not play | The file cannot be read or decoded | [A track shows a dash or will not play](../help/troubleshooting/no-waveform-or-playback.md) |

## Related

- [Prepare your first tracks](first-tracks.md) puts the window to work.
- [Keyboard shortcuts](../reference/keyboard-shortcuts.md) lists every shortcut.
- [Pick the right guide for your goal](choose-your-path.md) matches what you want to do to a page.

<!-- verified: apps/desktop/src/App.tsx (header, search placeholder and Cmd+F, status texts, theme button, Add tracks, selection toolbar, Drafts and history, footer text, empty-state titles, sidebar-width 180-340 default 224, player-height 200-420 default 270, 1250px compact inspector, keyboard handler); components/Sidebar.tsx (groups LIBRARY, PLAYLISTS, TOOLS, buttons, rail, LOCAL LIBRARY, CUERITHM 0.1); components/PlaylistMenu.tsx; PlaylistMembershipControls.tsx (Add to playlist); TrackTable.tsx (default columns, status icons, row actions); PlayerPanel.tsx (transport, CUE button, BEATGRID & CUES, 32 seconds, cue strip, Edit cues & grid); views/Settings.tsx (nine pages); theme.ts (cycle); src-tauri/tauri.conf.json (1440x940, min 980x680); legacy get-started/main-window.md re-checked (its "Rekordbox is shown but disabled" is out of date: Sync is enabled and contains rekordbox 6 / 7). -->
