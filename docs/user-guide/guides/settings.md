# Settings reference

Open **Settings** at the bottom of the sidebar, or the sliders button at the top right of the library. Settings are saved into the library. **Save settings** appears at the top (it reads *Saving…* then *Settings saved*); the Library preferences and Appearance pages save themselves.

## Incoming { #incoming }

| Control | Default | Meaning |
| --- | --- | --- |
| **Add watch folder** | `Music/Cuerithm/Incoming` on a new library | Files that appear in these folders are prepared automatically. Remove one with the **×** beside it. |

The note on the page: *Completed tracks move into Tracks automatically. Original audio files stay in place.* The file-stability wait (2 seconds) is not adjustable in the interface.

## Cue generation { #cue-generation }

<figure markdown>
  ![Cue generation settings](../assets/screens/11-settings-cue-generation.webp){ .screen loading=lazy }
</figure>

| Control | Default | Meaning |
| --- | --- | --- |
| **Existing cue points** | Replace | Fixed. Cues are replaced after a successful analysis. A failed preparation keeps the previous cues. |
| **Minimum breakdown length** | 64 | In beats (1 to 4096). Shorter breakdowns are not used. |
| **Drop at start** | Never | **Never** or **Automatic**. Whether a drop may be placed at the very start of a track. |
| **Emergency loop** | off | Adds a loop cue (labelled S.O.S.) when the track has a suitable late section. |

## Automatic tagging { #automatic-tagging }

See [Automatic tagging settings](tagging-settings.md).

## Cleanup { #cleanup }

<figure markdown>
  ![Cleanup settings](../assets/screens/13-settings-cleanup.webp){ .screen loading=lazy }
</figure>

| Control | Default | Meaning |
| --- | --- | --- |
| **Clean incoming tracks automatically** | off | Apply the saved rules to each new track after tagging and before it enters Tracks. |
| Smart Fixes, Genres, Artists | empty | The same editors as the Cleanup tool. See [Cleanup rules](cleanup-rules.md). |

## Library preferences { #library-preferences }

<figure markdown>
  ![Library preferences settings](../assets/screens/14-settings-library.webp){ .screen loading=lazy }
</figure>

| Control | Default | Meaning |
| --- | --- | --- |
| **Library time zone** | system | An IANA name such as `Europe/Amsterdam`. Calendar rules in smart playlists use it. |
| **Key compatibility mode** | Harmonic | **Harmonic** or **Fuzzy**. Used by *is compatible with key*. Press **Save library preferences**. |

## Appearance { #appearance }

Waveform style: six palettes. See [Choose a theme and waveform style](../learn/appearance.md). The light and dark theme is a separate button in the header.

## What is not a setting

There is no model selector, no tempo-range control, no cue template editor and no audio device picker in this release. Playback uses the system default output.
