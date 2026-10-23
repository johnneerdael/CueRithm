# Set the time zone and key matching for smart playlists

<p class="lead">Library preferences hold two choices that smart playlists depend on: which time zone calendar rules use, and how strictly "compatible key" is read. Both save with their own button and take under a minute to change.</p>

<!-- figure:existing-14-settings-library-reference-settings-library -->
<figure markdown>
![Library preferences settings](../../assets/screens/14-settings-library.webp){ .screen loading=lazy }
<figcaption>The Library preferences section of Settings.</figcaption>
</figure>
<!-- /figure -->

!!! tip "New to this?"
    Smart playlists work without touching these. They matter when a date rule such as "date added in the last 7 days" crosses midnight in the wrong city, or when you want a wider set of mixable keys. [Build a smart playlist](../../playlists/build-a-smart-playlist.md) shows both in use.

Open **Settings → Library preferences**. The section is headed "Calendar and key-matching rules for smart playlists." It has its own button, **Save library preferences**, and does not use **Save settings**.

## The two preferences { #controls }

| Control | What it does | Default | Valid values | Task page |
|---|---|---|---|---|
| **Time zone** | The zone used to decide which calendar day a date falls on, for every date rule in a smart playlist. Leave it blank to follow your computer's time zone. The box shows "System time zone" when blank. | Blank (system time zone) | An IANA zone name such as `Europe/Amsterdam` or `America/Los_Angeles`, up to 128 characters. A name CueRithm does not recognise is refused on save. | [Write smart playlist rules](../../playlists/smart-rules.md) |
| **Key compatibility** | How the smart rule **is compatible with key** compares two keys, and which tracks the automatic **By Key** lists contain. | **Harmonic** | **Harmonic**; **Fuzzy** | [Keep smart playlists up to date automatically](../../playlists/automatic-smart-playlists.md) |

The box for **Time zone** is announced by screen readers as "Library time zone", and the menu as "Key compatibility mode".

### What Harmonic and Fuzzy mean { #key-modes }

Keys are read as Camelot keys: a number from 1 to 12 and a letter, `A` for minor and `B` for major. The numbers wrap around, so 12 is next to 1. See [Understand Camelot keys](../../analysis/camelot-keys.md).

| Mode | A key matches when it is… | Matches for 8A | Matches for 12A |
|---|---|---|---|
| **Harmonic** | the same key, one step up or down with the same letter, or the same number with the other letter (relative major or minor) | 7A, 8A, 9A, 8B | 11A, 12A, 1A, 12B |
| **Fuzzy** | the same number, or one step up or down, with either letter | 7A, 7B, 8A, 8B, 9A, 9B | 11A, 11B, 12A, 12B, 1A, 1B |

The demo playlist **Mixes with 7A** is the Harmonic set for 7A: keys 6A, 7A, 8A and 7B. In Harmonic mode, 8A and 9B do not match; in Fuzzy mode they do.

The key you compare against, and the key on each track, are accepted as Camelot (`8A`), Open Key (`1m`, `2d`) or musical names (`Am`, `A minor`, `C major`, `Db`). The Compatible tracks tool has its own **Key mode** choice for a single search ([Find tracks that mix with the one playing](../../playlists/find-compatible-tracks.md)).

## Save, reload and what changes { #saving }

| Control or message | Meaning |
|---|---|
| **Save library preferences** | Enabled once something differs from what is saved. Reads "Saving…" while it works. |
| "Library preferences saved." | The change is stored. Every ready smart playlist is marked out of date and re-evaluates with the new zone or mode. |
| "Loading library preferences…" | The section is still reading the saved values |
| **Reload library preferences** | Appears after an error; reads the saved values again |
| "Library preferences changed; refresh before saving" | Another window saved first. Press **Reload library preferences** and repeat your change. |

Changing the time zone changes which tracks fall inside a "last 7 days" or "is on" rule, because those rules compare calendar days in this zone. Nothing in your files changes.

## Related

- [Write smart playlist rules](../../playlists/smart-rules.md)
- [Smart playlist fields and operators](../smart-playlist-fields.md), which describes how dates and keys are compared
- [Smart Playlists settings](smart-playlists.md)
- [Look up a setting](index.md)

<!-- verified: apps/desktop/src/views/PlaylistPreferences.tsx (labels, placeholder, maxLength 128, buttons and messages); apps/desktop/src/playlist-types.ts; crates/cuerithm-library/src/playlists/preferences.rs (IANA zone parse, 128 chars, revision check, marks ready lists stale); crates/cuerithm-smartlists/src/keys.rs (Harmonic/Fuzzy distance rules, accepted notations); crates/cuerithm-smartlists/tests/contract.rs (8A vs 9B harmonic false, fuzzy true); apps/desktop/src/views/CompatibleTracks.tsx (own Key mode); apps/desktop/src/test/playlist-preferences.test.tsx; legacy guides/settings.md re-checked -->
