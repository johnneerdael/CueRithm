# Keyboard shortcuts

<p class="lead">Every shortcut CueRithm responds to, grouped by what you are doing, with the key to press on a Mac and on Windows or Linux. There are fewer than a dozen, and none can be changed.</p>

!!! tip "New to this?"
    The shortcuts that matter most while you work through a pile of new music are search (++cmd+f++ / ++ctrl+f++), play and pause (++space++) and the bar jump (++arrow-left++ / ++arrow-right++). [Preview a track with the player](../analysis/preview-a-track.md) and [Find a track: search, sort and filter](../analysis/find-tracks.md) show them in use.

CueRithm has no menu bar commands of its own. Every shortcut below is handled inside the window. Where a row gives two forms, the first is for macOS (++cmd++) and the second for Windows and Linux (++ctrl++).

## When a shortcut works { #rules }

Shortcuts depend on where the focus is.

| Situation | What happens |
|---|---|
| You are typing in a text box, a menu list or an editable area | Letter shortcuts and arrows go to the text. Only search (++cmd+f++ / ++ctrl+f++) and the sidebar toggle (++cmd+b++ / ++ctrl+b++) still work. |
| A dialog or menu is open, or you are editing a playlist | Most shortcuts wait until it closes. Search and the sidebar toggle wait too, so the search box does not take the cursor from a dialog. ++esc++ closes the dialog or menu. |
| Focus is on a button or a link | ++space++ presses that button instead of playing or pausing |
| You are dragging tracks or a playlist | ++esc++ cancels the drag; other shortcuts wait |
| CueRithm is saving a draft before it quits | Shortcuts are ignored |

The search box shows the hint for your system: "⌘ F" on a Mac, "Ctrl+F" on Windows and Linux. Search needs exactly that key: on a Mac a bare ++ctrl+f++ stays with the text field (it moves the cursor forward), and ++shift++ or ++alt++ with the key does nothing.

## Find and select { #find }

| Shortcut | What it does | Notes |
|---|---|---|
| ++cmd+f++ / ++ctrl+f++ | Moves the cursor to **Search library** | Works from Settings and from the tools too, but not while a dialog or menu is open or you are editing a playlist: CueRithm returns to the library view you were in, closes Playlist tools and puts the cursor in the box. It searches title, artist and genre. |
| ++cmd+a++ / ++ctrl+a++ | Selects every track in the current view or search result | Works in **Tracks**, **Incoming**, **Archive** and in playlists, not while Playlist tools is open or in a tool or Settings. It selects all matches, including rows not loaded yet. |
| ++arrow-up++ / ++arrow-down++ | Moves the selection one row | With the **Track list** focused. Replaces the selection with the next or previous row and scrolls to it. With nothing selected, either key selects the first row. |
| ++enter++ | Plays the selected row | With the **Track list** focused. If several rows are selected, the first one plays. |

See [Work on many tracks at once](../analysis/select-many.md) for what to do with a selection.

## Play and navigate { #play }

| Shortcut | What it does | Notes |
|---|---|---|
| ++space++ | Plays or pauses the loaded track | Needs a track loaded in the player |
| ++arrow-left++ / ++arrow-right++ | Jumps one bar back or forward | A bar is four beats at the track's saved BPM: 240 divided by the BPM, in seconds. A track at 128 BPM jumps 1.875 s. Needs a loaded track with a BPM; it stops at the start and the end of the track. |
| ++enter++ or ++space++ with the large waveform focused | Switches between the 32-second zoom and the whole track | Focus the waveform with ++tab++. This replaces play and pause while the waveform has focus. |

The waveform's tooltip reminds you: "← / → jump one bar". More on cues and zoom: [Jump to the drop with cues](../analysis/jump-to-cues.md) and [Check the beatgrid at beat level](../analysis/check-the-beatgrid.md).

## Window { #window }

| Shortcut | What it does | Notes |
|---|---|---|
| ++cmd+b++ / ++ctrl+b++ | Collapses the sidebar to an icon rail, or pins it open again | The choice is remembered. A collapsed sidebar opens over the content while you point at it. Its tooltip reads "Collapse to icons (⌘B)" on a Mac and "Collapse to icons (Ctrl+B)" on Windows and Linux. |

The sidebar and the player can also be resized by dragging the divider: the sidebar between 180 and 340 pixels wide and the player between 200 and 420 pixels tall. See [Resize the sidebar and player](../customize/layout.md).

## Edit tags { #edit }

| Shortcut | What it does | Notes |
|---|---|---|
| ++cmd+s++ / ++ctrl+s++ | In **Edit metadata**: checks your changes, and saves them once the check passed | First press is the same as **Preview changes**; when a valid preview is on screen, it is the same as **Save changes**. Nothing happens while the editor is busy or a discard question is open. |
| ++esc++ | Closes **Edit metadata**, **Review match**, **Curate** or **Drafts and history** and returns to the track list | Equivalent to **Back to tracks**. It does nothing while a menu or dialog inside the panel is open; those take the key first. |

See [Edit tags and artwork](../tagging/edit-tags-by-hand.md).

## Curate { #curate }

These work while the **Curate** panel has focus and you are not typing in a box or managing categories.

| Shortcut | What it does |
|---|---|
| ++1++ to ++9++ | Turns the first nine category choices on or off for the selected tracks, in the order shown in the panel. A number appears on each choice that has a shortcut. |
| <kbd>[</kbd> and <kbd>]</kbd> | Moves to the previous or next track |

The panel notes: "While Curate is focused: 1–9 toggle the first choices. [ / ] moves between tracks. Typing and playback shortcuts keep their own controls." See [Group tracks with categories and custom tags](../tagging/curate-tracks.md).

## Menus and dialogs { #menus }

| Shortcut | Where | What it does |
|---|---|---|
| ++esc++ | Row **Actions for** menu, playlist **Actions for** menu, **Category filters** pop-over, **Move** dialog, **Discard unsaved changes?** dialog | Closes it. In the discard dialog, ++esc++ means **Keep editing**. |
| ++arrow-up++ / ++arrow-down++, ++home++, ++end++ | Playlist **Actions for** menu | Moves between the menu items |
| ++tab++ | Playlist **Actions for** menu | Closes the menu |
| ++tab++ and ++shift+tab++ | **Discard unsaved changes?** and **Move** dialogs | Cycles through the buttons without leaving the dialog |
| ++esc++ | While dragging tracks or a playlist | Cancels the drag |

## Mouse and trackpad { #mouse }

| Gesture | What it does |
|---|---|
| Click a row | Selects it and deselects the others |
| ++cmd++ / ++ctrl++ and click | Adds the row to the selection, or removes it |
| ++shift++ and click | Selects the rows from the first selected row to the one you click |
| Double-click a row, or press its play button | Loads the track in the player and plays it |
| Click the large waveform | Switches between the 32-second zoom and the whole track |
| Click the small overview waveform | Moves the playhead there |
| Click a cue button in the cue strip | Moves the playhead to that cue |
| Press **CUE** in the player | Pauses and goes to the first cue, or to the start if there is none |
| Press **Back to start** | Goes to the start of the track |
| Drag the selected tracks onto a playlist in the sidebar | Adds them to that ordinary playlist. See [Create a playlist and add tracks](../playlists/create-a-playlist.md). |
| Drag a playlist in the sidebar | Reorders it or moves it into a folder. Drop on **Move to top level** to take it out of a folder. See [Group playlists in folders and reorder them](../playlists/organize-playlists.md). |
| Drop audio files onto the window | Starts preparing them. A "Drop to prepare" overlay appears. See [Add tracks: drop, pick or watch a folder](../files/add-tracks.md). |

## Related

- [Preview a track with the player](../analysis/preview-a-track.md)
- [Work on many tracks at once](../analysis/select-many.md)
- [Find the setting you need](../customize/find-a-setting.md)
- [Find your way around the window](../get-started/main-window.md)

<!-- verified: apps/desktop/src/App.tsx (keydown handler: Mod+F after the dialog, menu and playlist-editor guard, exact platform modifier and no Shift or Alt; Mod+B, Mod+A, arrows, Space and their guards; header kbd hint; jumpBar = 240/bpm; resize clamps 180-340 and 200-420; onSelect click/Cmd/Shift); apps/desktop/src/components/TrackTable.tsx (ArrowUp/Down/Enter, double-click, Escape for row menu); apps/desktop/src/components/PlayerPanel.tsx (waveform Enter/Space zoom, tooltip, CUE and Back to start, cue buttons, overview click); apps/desktop/src/components/MetadataEditor.tsx (Mod+S check/save; discard dialog Escape/Tab trap; button labels Preview changes and Save changes); apps/desktop/src/components/CuratePanel.tsx (1-9 and [ ]); apps/desktop/src/components/InspectorFrame.tsx (Escape closes panel); apps/desktop/src/components/PlaylistMenu.tsx; PlaylistMoveDialog.tsx; CurationFilters.tsx; apps/desktop/src/use-playlist-drag.ts (Escape cancels drag); apps/desktop/src/components/Sidebar.tsx (title with the keyboard hint); apps/desktop/src/platform.ts (hint wording by platform; the handlers accept Cmd or Ctrl); apps/desktop/src-tauri/src/lib.rs and tauri.conf.json (no menu or accelerator defined, checked by search); apps/desktop/src/test/workspace.test.tsx and brand-ui.test.tsx; legacy get-started/shortcuts.md re-checked. UNVERIFIED: the standard operating-system shortcuts a Tauri window may add (for example quit or copy and paste) are not defined by CueRithm and are not listed. -->
