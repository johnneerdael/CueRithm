# Resize the sidebar and player

<p class="lead">Give the playlists more room, shrink the sidebar to icons, or make the waveform taller. Each change takes a drag or a keystroke, and CueRithm remembers it.</p>

The window has four regions (see [Find your way around the window](../get-started/main-window.md)): the sidebar on the left, the header on top, the main area, and the player at the bottom. Two thin dividers and one shortcut change how they share the space.

## Make the sidebar wider or narrower { #sidebar-width }

Point at the thin vertical divider between the sidebar and the main area. The pointer turns into a column-resize arrow and the divider lights up in the accent colour. Drag it left or right. A long playlist name that was cut off with an ellipsis fits once you widen the sidebar.

The sidebar is 224 px wide at first. You can drag it between 180 px and 340 px, and it stops at both ends. The divider is named **Resize sidebar** for screen readers, and it responds to a mouse, trackpad or pen only. It does not respond to the keyboard.

## Collapse the sidebar to icons { #collapse }

Press ++cmd+b++ / ++ctrl+b++, or select the **Collapse sidebar** button at the top of the sidebar next to the CueRithm name. The sidebar shrinks to a 60 px rail of icons, and the main area gets the space. Sections shrink to a divider line. Counts disappear, except that **Incoming** keeps a small dot while tracks are waiting there.

<!-- figure:customize-layout-collapsed -->
<figure markdown>
![CueRithm with the sidebar collapsed to a narrow rail of icons and the track table using the freed space](../assets/screens/customize-layout-collapsed-light.webp#only-light){ .screen loading=lazy }
![CueRithm with the sidebar collapsed to a narrow rail of icons and the track table using the freed space](../assets/screens/customize-layout-collapsed-dark.webp#only-dark){ .screen loading=lazy }
<figcaption>A collapsed sidebar is a 60 px rail of icons; hover it to open it over the table.</figcaption>
</figure>
<!-- /figure -->

A collapsed sidebar still works:

- Rest the pointer on the rail, or move keyboard focus into it, and after a moment it opens over the main area. It closes again when you move away.
- It stays open while a playlist menu or a dialog from it is showing.
- The sidebar divider is switched off while the sidebar is collapsed.

Press the same keys, or the **Pin sidebar open** button on the opened rail, to bring it back. Both the width and the collapsed state are remembered.

## Make the player taller or shorter { #player-height }

The player sits at the bottom, under a thin horizontal divider. Drag that divider, named **Resize player**, up to make the player taller and the track area shorter, or down to do the opposite. A taller player gives the waveform and cue buttons more room; see [Preview a track with the player](../analysis/preview-a-track.md).

The player is 270 px high at first. You can drag it between 200 px and 420 px. Like the sidebar divider, it responds to the pointer only.

## What happens in a narrow window { #window }

CueRithm opens at 1440 × 940 and cannot be made smaller than 980 × 680. In a window 1250 px wide or narrower, the editors that open beside the track table, **Edit metadata**, **Review match**, **Curate** and **Drafts and history**, take the whole main area instead. The track table comes back when you close the editor. To keep both on screen, make the window wider or collapse the sidebar.

## Go back to the defaults { #reset }

There is no reset button. Drag each divider back towards 224 px and 270 px, or press ++cmd+b++ / ++ctrl+b++ to expand the sidebar again. The sizes belong to this computer's copy of CueRithm. They are not in the library, so a [library backup](../files/library/back-up-library.md) does not contain them.

To change which columns the track table shows, see [Show the columns you need](../analysis/choose-columns.md).

## Related

- [Switch between light and dark](theme.md)
- [Choose a waveform style](waveform-style.md)
- [Keyboard shortcuts](../reference/keyboard-shortcuts.md)
- [Find the setting you need](find-a-setting.md)

<!-- verified: apps/desktop/src/App.tsx (preference() ranges: sidebar-width 180-340 default 224, player-height 200-420 default 270, sidebar-collapsed; resize handlers use pointer events only; separators "Resize sidebar" and "Resize player"; Cmd/Ctrl+B handler; compactInspector at max-width 1250), components/Sidebar.tsx (Collapse sidebar / Pin sidebar open, title text, 140 ms peek, pinned open while menu or dialog), styles.css (60 px rail, sidebar-resize hidden when collapsed, hover colour var(--cr-primary), @media 1250 inspector rule), src-tauri/tauri.conf.json (1440x940, min 980x680); legacy get-started/main-window.md re-checked. -->
