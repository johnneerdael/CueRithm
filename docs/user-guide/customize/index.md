# Make CueRithm fit your setup

<p class="lead">Switch between light and dark, pick the waveform colours you read best, and give the part of the window you use most more room. Each change takes under a minute, and CueRithm remembers it on this computer.</p>

<!-- figure:customize-waveform-styles -->
<figure markdown>
![Settings on the Appearance page showing six waveform style previews: CueRithm, Dusk, Lagoon, Tonal, Layers and Layers, tonal](../assets/screens/customize-waveform-styles-light.webp#only-light){ .screen loading=lazy }
![Settings on the Appearance page showing six waveform style previews: CueRithm, Dusk, Lagoon, Tonal, Layers and Layers, tonal](../assets/screens/customize-waveform-styles-dark.webp#only-dark){ .screen loading=lazy }
<figcaption>The Appearance page previews all six waveform styles on the same sample track.</figcaption>
</figure>
<!-- /figure -->

## Start here

Begin with [Switch between light and dark](theme.md). It is one button, and it shows how every choice in this tab works: you change it, the window updates at once, and there is nothing to save.

<div class="grid cards" markdown>

-   **Light or dark**

    ---

    One button in the header cycles System, Light and Dark.

    [:octicons-arrow-right-24: Switch between light and dark](theme.md)

-   **Waveform colours**

    ---

    Six palettes, from the default cobalt and coral to single-blue styles that never depend on hue.

    [:octicons-arrow-right-24: Choose a waveform style](waveform-style.md)

-   **Window layout**

    ---

    Resize the sidebar and the player, or collapse the sidebar to icons.

    [:octicons-arrow-right-24: Resize the sidebar and player](layout.md)

</div>

## Your goal → page

| Your goal | Page |
|---|---|
| Use CueRithm in a dark room, or follow my computer's appearance | [Switch between light and dark](theme.md) |
| Make waveforms easier to read, or readable with colour blindness | [Choose a waveform style](waveform-style.md#contrast) |
| See longer playlist names | [Resize the sidebar and player](layout.md#sidebar-width) |
| Get more space for the track table | [Resize the sidebar and player](layout.md#collapse) |
| Show a bigger waveform in the player | [Resize the sidebar and player](layout.md#player-height) |
| Find where a setting lives | [Find the setting you need](find-a-setting.md) |
| Understand which choices are saved where | [Find the setting you need](find-a-setting.md#save) |

## What is saved where

Theme, waveform style, the sidebar and player sizes and your table columns are display preferences. CueRithm keeps them in its own storage on this computer, separate from the library. They are not in a [library backup](../files/library/back-up-library.md), and a second computer starts with the defaults. Preparation settings, such as watch folders and cue rules, live in the library and are saved with a button; [Find the setting you need](find-a-setting.md#save) explains the difference.

## Not what you wanted?

- To change how tracks are prepared, tagged or cleaned, see [Look up a setting](../reference/settings/index.md).
- To change which columns the track table shows, see [Show the columns you need](../analysis/choose-columns.md).
- To learn the window before you change it, see [Find your way around the window](../get-started/main-window.md).

## Related

- [Keyboard shortcuts](../reference/keyboard-shortcuts.md)
- [How is the waveform drawn?](../how-it-works/waveform.md)
- [Where CueRithm keeps your data](../files/library/where-data-lives.md)

<!-- verified: apps/desktop/src/theme.ts, waveform-style.ts, waveform-colour.ts, views/WaveformSettings.tsx, views/Settings.tsx, App.tsx (theme button, sidebar and player dividers, localStorage preferences), components/TrackTable.tsx (track-columns in localStorage); docs/user-guide/_meta/ia.md -->
