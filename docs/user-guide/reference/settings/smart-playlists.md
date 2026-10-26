# Choose which smart playlists CueRithm builds for you

<p class="lead">The Smart Playlists section switches on five families of lists that CueRithm creates and keeps current: by artist, BPM range, release month, genre and key. Every option is off until you choose it. About three minutes.</p>

<!-- figure:existing-16-settings-smart-playlists-reference-settings-smart-playlists -->
<figure markdown>
![Smart Playlist settings with one configured BPM range](../../assets/screens/16-settings-smart-playlists.png){ .screen loading=lazy }
<figcaption>The Smart Playlists section of Settings.</figcaption>
</figure>
<!-- /figure -->

<!-- figure:reference-settings-smart-playlists -->
<figure markdown>
![The Smart Playlists page of Settings with By BPM switched on and one range from 126 to 130, and the other four options By Artist, By release date, By Genre and By Key](../../assets/screens/reference-settings-smart-playlists-light.webp#only-light){ .screen loading=lazy }
![The Smart Playlists page of Settings with By BPM switched on and one range from 126 to 130, and the other four options By Artist, By release date, By Genre and By Key](../../assets/screens/reference-settings-smart-playlists-dark.webp#only-dark){ .screen loading=lazy }
<figcaption>Each family of automatic lists has its own switch; BPM ranges are yours to define.</figcaption>
</figure>
<!-- /figure -->

!!! tip "New to this?"
    These are ready-made smart playlists. To write your own rules, start with [Build a smart playlist](../../playlists/build-a-smart-playlist.md). To see what the automatic lists look like in your library, read [Keep smart playlists up to date automatically](../../playlists/automatic-smart-playlists.md).

Open **Settings → Smart Playlists**. The section is headed "Automatically build and maintain lists from your library." It saves with **Save Smart Playlist settings**, not **Save settings**. When it saves, "Smart Playlist settings saved." appears.

The lists appear in the sidebar under a folder named **Automatic Smart Playlists**. Only prepared tracks that are not archived supply the artists, genres and release months. Audio files and tags are never changed.

## The five options { #options }

| Control | Creates | Default |
|---|---|---|
| **By Artist** | A folder **Artists** with one list per individual artist or remixer. Credits are split at commas, semicolons, new lines and the words and marks `featuring`, `feat.`, `feat`, `ft.`, `ft`, `vs.`, `vs`, `x` and `&`. A remix named in the title, such as "(Name Remix)", counts when the Remixer tag is empty. Spelling that differs only in capitals becomes one list. Each list's rule is **Artist or remixer credit** *is* the name. | Off |
| **By BPM** | A folder **BPM** with one list per range you define, named like `126–130 BPM`. Each range includes both ends. Ranges can overlap, so one track can be in several lists. Each list's rule is **BPM** *is between* the two numbers. | Off |
| **By release date** | A folder **Release dates** holding a folder per year, holding a folder per month (January to December). Each month has four lists: **Month, Year** (every track), **Warm-Up Month, Year** (energy 1 to 5), **Dancefloor Month, Year** (energy 6 to 7) and **Anthems Month, Year** (energy 8 to 10). A track with no energy rating appears only in the all-tracks list. | Off |
| **By Genre** | A folder **Genres** with one list per individual genre. A genre tag holding several genres separated by `;` counts for each. Each list's rule is **Individual genre** *is* the name. | Off |
| **By Key** | A folder **Keys** with all 24 lists, `1A` to `12B`. Each list's rule is **Key** *is compatible with* that key, so a list holds more than the one key, according to [Key compatibility](library.md#key-modes) in Library preferences (Harmonic by default). | Off |

For example, in the demo library the genre tag `Hard Dance / Hardcore / Neo Rave;Neo Rave` on "Nivk, Rabteu - PLV (Creeds remix - Extended)" puts that track in two lists, **Hard Dance / Hardcore / Neo Rave** and **Neo Rave**. The artist tag `Miss Monique, Henri Bergmann, Mario Eighta` on "17 (Extended Mix)" puts it in three artist lists.

### Set BPM ranges { #bpm-ranges }

Switching on **By BPM** reveals the range editor.

| Control | What it does | Default | Valid values |
|---|---|---|---|
| **Add BPM range** | Adds a range. Each new range starts 5 BPM higher than the last: the first is 120 to 124, then 125 to 129. | No ranges | Up to 10 ranges. The button stops working at 10. |
| **From** (*Range n minimum BPM*) | Lowest BPM in the range | 120 for the first | Above 0 and up to 999, in steps of 0.01 |
| **To** (*Range n maximum BPM*) | Highest BPM in the range | 124 for the first | Above 0 and up to 999; not lower than **From** |
| **×** (*Remove BPM range n*) | Removes the range from the settings. The list it already created stays. | n/a | n/a |

If a range is invalid, **Save Smart Playlist settings** is unavailable and the section says why: "Configure at most 10 BPM ranges.", "Each BPM range must be different.", or "Range n: enter a minimum and maximum above 0 and up to 999 BPM, with minimum no greater than maximum." If you switch **By BPM** off, unfinished ranges are dropped when you save.

## What happens as your library changes { #maintenance }

- New tracks and tag changes add new artists, genres and release months to the lists automatically. New years and months get their folders.
- A release month needs a complete `YYYY-MM` or `YYYY-MM-DD` date in a release-date tag (`RELEASE_TIME`, `DATE` and similar tags are checked in turn). A year on its own cannot place a track in a month.
- **Turning an option off** stops new lists being created. Lists that already exist stay, and their contents keep updating.
- **Removing a BPM range** keeps its list for the same reason.
- **Renaming or moving** a generated list keeps it managed by its option. **Deleting** one tells CueRithm not to create it again.
- **Editing the rules** of a generated list: while its option is on, CueRithm restores the rules it maintains. To make a variant, duplicate the list and edit the copy ([Write smart playlist rules](../../playlists/smart-rules.md)).
- A manual list is never taken over because its name matches a generated one.

## What these are not { #not }

The **Added this week** list in the demo library is an ordinary smart playlist whose rule is **Date added** *is in the last* 7 *days*; it is not one of the five automatic families. Build rules like it in [Write smart playlist rules](../../playlists/smart-rules.md), with the fields on [Smart playlist fields and operators](../smart-playlist-fields.md).

## Related

- [Keep smart playlists up to date automatically](../../playlists/automatic-smart-playlists.md)
- [Build a smart playlist](../../playlists/build-a-smart-playlist.md)
- [Library preferences](library.md) for the key mode and time zone
- [How do smart playlists stay current?](../../how-it-works/smart-engine.md)
- [Look up a setting](index.md)

<!-- verified: apps/desktop/src/views/AutomaticPlaylistOptions.tsx (labels, ranges, error messages, step 0.01, max 999, 10 ranges, new range 120+5n); apps/desktop/src/views/PlaylistPreferences.tsx (button, messages); crates/cuerithm-library/src/playlists/automatic.rs (folder names, rules, energy bands, naming "Warm-Up "/"Dancefloor "/"Anthems ", BPM "start–end BPM", 24 key lists, opt-out on delete, rule restoration); crates/cuerithm-library/src/playlists/credits.rs (split markers, remixer from title, release_month tag keys and formats; test named_remixers_support_unicode_and_explicit_tags_take_priority); apps/desktop/src/test/automatic-playlists.test.tsx; legacy guides/settings.md re-checked -->
