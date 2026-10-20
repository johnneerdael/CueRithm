# Choose a theme and waveform style

Switch between light and dark, then choose a waveform palette.

## Theme

The **Colour theme** button in the header cycles **System**, **Light** and **Dark**. System follows your operating system. CueRithm remembers your choice on this computer.

<div class="two-up" markdown>

<figure markdown>
  ![Dark theme](../assets/screens/40-player.webp){ .screen loading=lazy }
  <figcaption>Dark</figcaption>
</figure>

<figure markdown>
  ![Light theme](../assets/screens/92-light-player.webp){ .screen loading=lazy }
  <figcaption>Light</figcaption>
</figure>

</div>

## Waveform style

Open **Settings > Appearance**. Each of the six styles has a sample waveform preview. The change applies immediately and is saved on this computer.

<figure markdown>
  ![Appearance settings with six waveform style previews](../assets/screens/15-settings-appearance.webp){ .screen loading=lazy }
  <figcaption>CueRithm, Dusk, Lagoon, Tonal, Layers and Layers, tonal.</figcaption>
</figure>

| Style | Look |
| --- | --- |
| **CueRithm** | Cobalt bass, coral mids, ice highs. Moments blend between them. |
| **Dusk** | Raspberry bass, amber mids, cream highs. A warm blend. |
| **Lagoon** | Deep blue bass, teal mids, pale mint highs. A cool blend. |
| **Tonal** | One blue throughout. Lightness alone separates bass, mids and highs. |
| **Layers** | Bass, mids and highs as stacked layers in cobalt, coral and ice. |
| **Layers, tonal** | Stacked layers in three tones of one blue. |

<div class="two-up" markdown>

<figure markdown>
  ![Dusk waveform style](../assets/screens/93-style-dusk.webp){ .screen loading=lazy }
  <figcaption>Dusk</figcaption>
</figure>

<figure markdown>
  ![Lagoon waveform style](../assets/screens/94-style-lagoon.webp){ .screen loading=lazy }
  <figcaption>Lagoon</figcaption>
</figure>

</div>

Bar height always follows the average loudness of each moment, scaled per track. The tonal styles never rely on hue to separate frequency bands, which helps if colour is hard for you to tell apart.

The waveform is computed once per track from the decoded audio, as three frequency bands, and cached. The style only changes how those bands are coloured. Switching styles does not recompute the waveform. See [Waveforms](../technical/waveform.md).
