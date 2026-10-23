# Keep smart playlists up to date automatically

<p class="lead">Let CueRithm create smart playlists for you by artist, BPM range, genre, key or release month, and add new ones as new music arrives. Setting it up takes about two minutes.</p>

Every smart playlist you build yourself already refreshes by itself. This page is about **Automatic Smart Playlists**, a different thing: playlists that CueRithm creates for you, and keeps creating when a new artist, genre or release month shows up in your library. They are off until you ask for them.

If you want a single playlist with your own rules, use [Build a smart playlist](build-a-smart-playlist.md). If you want a one-time batch you can then edit, use [Generate playlists from your library](generate-playlists.md).

## Turn on the groups you want { #turn-on }

1. Click **Settings** in the sidebar, then **Smart Playlists** in the list of settings pages.
2. Tick the groups you want. **By Artist**, **By BPM**, **By release date**, **By Genre** and **By Key** are all off to begin with, in new and existing libraries.
3. If you ticked **By BPM**, [add your ranges](#bpm-ranges).
4. Click **Save Smart Playlist settings**. The page confirms with *Smart Playlist settings saved.*

This page has its own save button. It does not need the **Save settings** button used by the preparation pages, and it does not re-prepare any track.

The lists appear in the sidebar under a folder called **Automatic Smart Playlists**.

## See what each group creates { #options }

<!-- figure:existing-16-settings-smart-playlists-automatic-smart-playlists -->
<figure markdown>
![Smart Playlist settings with one configured BPM range](../assets/screens/16-settings-smart-playlists.png){ .screen loading=lazy }
<figcaption>Automatic smart playlists are switched on in Settings.</figcaption>
</figure>
<!-- /figure -->

| Option | Folder it fills | One list for each… | Rule behind the list |
|---|---|---|---|
| **By Artist** | **Artists** | individual artist or remixer | **Artist or remixer credit** is the name |
| **By BPM** | **BPM** | range you configure, named like `126–130 BPM` | **BPM** is between the two numbers |
| **By release date** | **Release dates** | month, inside a folder for its year | **Release month (YYYYMM)** equals the month |
| **By Genre** | **Genres** | individual genre in your library | **Individual genre** is the name |
| **By Key** | **Keys** | Camelot key, `1A` to `12B` (24 lists) | **Key** is compatible with key |

Some details that decide what you get:

- **By Artist** splits collaboration credits on commas, semicolons, `feat.`, `ft.`, `vs`, `x` and `&`. When the remixer tag is empty, a name in a title such as *(Name Extended Remix)* counts as a remixer.
- **By release date** makes four lists in each month, for example *October, 2026*, *Warm-Up October, 2026*, *Dancefloor October, 2026* and *Anthems October, 2026*. The last three add an energy range of 1 to 5, 6 to 7 and 8 to 10. The all-tracks list also holds tracks with no energy rating.
- **By Key** lists hold every track that mixes with that key, using your [Key compatibility](../reference/settings/library.md) setting. With the default, **Harmonic**, the list `7A` holds `6A`, `7A`, `8A` and `7B` tracks. In the purchase folder used in this guide that is six tracks. The recipe [Collect the tracks that mix with a key](recipes/tracks-that-mix-with-a-key.md) builds the same list by hand.
- Grouping values come from prepared tracks that are not archived.

## Add BPM ranges { #bpm-ranges }

1. Tick **By BPM**.
2. Click **Add BPM range**. A row appears with **From** and **To**. The first suggestion is 120 to 124, the next 125 to 129, and so on.
3. Type your own numbers. Both ends are included, so `126` to `130` holds every track from 126 BPM up to 130 BPM: 18 tracks in the purchase folder.
4. Use the ✕ on a row (**Remove BPM range**) to delete it.

You can have up to ten ranges. Ranges may overlap, and a track then appears in each list it fits. Each range needs a minimum above 0 and a maximum of at most 999, with the minimum no greater than the maximum. If a range is invalid, the page says which one, for example *Range 1: enter a minimum and maximum above 0 and up to 999 BPM, with minimum no greater than maximum.*, and **Save Smart Playlist settings** stays unavailable.

## Know when new lists appear { #new-tracks }

After you save, CueRithm creates the lists for the music you already have. From then on it checks about once a second: when prepared tracks arrive or their tags change, it adds lists for any new artist, genre or release month, and every list's members refresh. You do nothing.

**By release date** needs a release year and month in the track's tags, either a year and month or a full date. A year alone is not enough. If a purchase has no release tag, the track appears in the other groups but in no month list. Fix the tag with [Edit tags and artwork](../tagging/edit-tags-by-hand.md).

## Rename, move or customise a list { #customise }

You can rename an automatic list, move it, or make it a favourite. See [Group playlists in folders and reorder them](organize-playlists.md). It keeps its identity when you do.

You cannot keep custom rules on an automatic list. While a group is on, CueRithm maintains the rules of its lists and puts them back if you change them. To customise one, choose **Duplicate** in its **⋯** menu. The copy is yours, with the same rules, and CueRithm leaves it alone.

## Turn a group off or remove a list { #turn-off }

- **Untick a group and save.** CueRithm stops creating lists for it. The lists that exist stay, and their members keep updating. Removing a BPM range also keeps its list.
- **Remove a list you do not want.** CueRithm treats that as a choice and does not create it again. Restore it from **Removed playlists** if you change your mind.
- A playlist you made yourself is never adopted because its name matches an automatic one.

## What can go wrong

| What you see | Why | What to do |
|---|---|---|
| There is no **Automatic Smart Playlists** folder | No group is ticked and saved, or no track is prepared yet | Tick a group, click **Save Smart Playlist settings**, and check **Incoming** |
| A track is in no month list | It has no release year and month in its tags | Add the tag, see [Edit tags and artwork](../tagging/edit-tags-by-hand.md) |
| **Save Smart Playlist settings** is greyed out | A BPM range is incomplete or invalid | Read the message above the options and correct that range |
| A list you edited has its old rules back | An enabled group maintains its rules | Duplicate the list and edit the copy |
| A list I removed has not come back | Removing it is treated as a choice | Restore it from **Removed playlists** |

## Related

- [Generate playlists from your library](generate-playlists.md)
- [Write smart playlist rules](smart-rules.md)
- [Smart Playlists settings](../reference/settings/smart-playlists.md)
- [Library preferences](../reference/settings/library.md)

<!-- verified: apps/desktop/src/views/AutomaticPlaylistOptions.tsx (labels, ranges, validation messages, notes); views/PlaylistPreferences.tsx (Save Smart Playlist settings, saved text); views/Settings.tsx (page list); crates/cuerithm-library/src/playlists/automatic.rs (folders, names, rules, energy bands, managed rules restored, removed = opt-out), credits.rs (artist splitting, remixer from title, release month sources), preferences.rs; src/test/automatic-playlists.test.tsx; counts computed from _meta/the private demo-library facts sheet; legacy guides/playlists.md and guides/settings.md re-checked -->
