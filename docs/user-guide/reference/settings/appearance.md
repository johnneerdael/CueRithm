# Choose the waveform style

<p class="lead">The Appearance section has one setting: the colours used to draw waveforms in the track table and the player. It applies the moment you click and takes ten seconds.</p>

<!-- figure:existing-15-settings-appearance-reference-settings-appearance -->
<figure markdown>
![Appearance settings with six waveform style previews](../../assets/screens/15-settings-appearance.webp){ .screen loading=lazy }
<figcaption>The Appearance section of Settings.</figcaption>
</figure>
<!-- /figure -->

<!-- figure:reference-settings-appearance -->
<figure markdown>
![The Appearance page of Settings with six waveform style buttons: CueRithm, Dusk, Lagoon, Tonal, Layers and Layers, tonal, each with a sample waveform, CueRithm selected](../../assets/screens/reference-settings-appearance-light.webp#only-light){ .screen loading=lazy }
![The Appearance page of Settings with six waveform style buttons: CueRithm, Dusk, Lagoon, Tonal, Layers and Layers, tonal, each with a sample waveform, CueRithm selected](../../assets/screens/reference-settings-appearance-dark.webp#only-dark){ .screen loading=lazy }
<figcaption>Six waveform styles, each previewed on the same invented track.</figcaption>
</figure>
<!-- /figure -->

!!! tip "New to this?"
    [Choose a waveform style](../../customize/waveform-style.md) shows the six styles side by side on real tracks and helps you pick one. [Switch between light and dark](../../customize/theme.md) covers the separate colour theme.

Open **Settings → Appearance**. The section is headed "How waveforms are drawn in track previews and the player. Saved on this computer and applied immediately." There is no save button.

## The six waveform styles { #styles }

Each style appears as a button with a small sample waveform of an invented track (intro, drop, breakdown, drop). Choosing one changes every waveform on screen at once: the **PREVIEW** column of the table, the player and the samples.

| Style (button label) | Description in the app | Look | Default |
|---|---|---|---|
| **CueRithm** | Cobalt bass, coral mids, ice highs. Moments blend between them. | Blended | Yes |
| **Dusk** | Raspberry bass, amber mids, cream highs. A warm blend. | Blended | |
| **Lagoon** | Deep blue bass, teal mids, pale mint highs. A cool blend. | Blended | |
| **Tonal** | One blue throughout; lightness alone separates bass, mids and highs. | Blended | |
| **Layers** | Bass, mids and highs as stacked layers in cobalt, coral and ice. | Stacked layers | |
| **Layers, tonal** | Stacked layers in three tones of one blue. | Stacked layers | |

Bar height always follows the average loudness of each moment, scaled to the track. The style only changes how the three frequency bands (bass, mids, highs) are coloured, so switching never recomputes a waveform. The tonal styles do not rely on hue to tell the bands apart, which helps if colours are hard for you to separate. How the waveform is drawn: [How is the waveform drawn?](../../how-it-works/waveform.md).

| Control | What it does | Default | Valid values |
|---|---|---|---|
| **Waveform style** (a group of six radio buttons) | Picks the style. The chosen one is marked as selected. | **CueRithm** | The six styles above |

## Where the choice is kept { #storage }

The style is remembered by the app on this computer, not in your library. It does not travel with a [library backup](../../files/library/back-up-library.md), a preparation preset or a sync, and each computer keeps its own. If the app cannot store it, it falls back to **CueRithm** each time.

## The colour theme is not in Settings { #theme }

| You want | Use | Choices |
|---|---|---|
| Light, dark or follow the system | The **Colour theme** button in the header, next to **Add tracks**. Each click cycles to the next choice; the tooltip reads "Colour theme: *mode* (click to change)". | System (default), Light, Dark |

The waveform colours adjust themselves to stay readable on either theme. See [Switch between light and dark](../../customize/theme.md).

## Related

- [Choose a waveform style](../../customize/waveform-style.md)
- [Switch between light and dark](../../customize/theme.md)
- [Preview a track with the player](../../analysis/preview-a-track.md)
- [Look up a setting](index.md)

<!-- verified: apps/desktop/src/views/WaveformSettings.tsx (heading text, radiogroup, note); apps/desktop/src/waveform-colour.ts (palettes: ids, labels, detail text, modes, defaultStyle cuerithm); apps/desktop/src/waveform-style.ts (localStorage key, fallback); apps/desktop/src/theme.ts (system/light/dark cycle); apps/desktop/src/App.tsx (Colour theme button label and tooltip); legacy learn/appearance.md re-checked -->
