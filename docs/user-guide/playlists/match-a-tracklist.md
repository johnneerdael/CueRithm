# Rebuild a set from a tracklist

<p class="lead">Paste a list of artists and titles, or load a text or CSV file, and CueRithm finds the matching tracks you own. You listen, pick the right version of each, and save a playlist in the order of the list. It takes about five minutes.</p>

Matching reads only your local library. It does not connect to a streaming service or download anything, and it matches prepared, unarchived tracks only, so finish [following new tracks through Incoming](../files/follow-incoming.md) first.

## Enter the tracklist { #enter }

1. Under **TOOLS** in the sidebar, click **Tracklist matcher**.
2. Paste your list into **Reference tracklist**, or click **Choose text or CSV file** and pick a `.txt` or `.csv` file.

For plain text, write one track per line as `Artist - Title`, with spaces around the hyphen. CueRithm splits each line at the first ` - `. A line without one is searched by title alone. Blank lines are skipped. For example:

```text
Adriatique - Palm Of Your Hands (Extended Mix)
Cassian, AR/CO - Come To Life (Original Mix)
Jast - Dancin (Extended Mix)
Anyma (ofc) - Atoma (Extended Mix)
Someone Else - Unreleased Edit
```

A file you choose is loaded into the box, and a `.csv` file switches **Input format** to CSV for you. For CSV, **Input format** reads **CSV: artist and title columns**. Tick **CSV has artist/title headers** when the first row names the columns `artist` and `title` (one of each). Without headers, the first column is the artist and the second is the title. If your list is written as `Title - Artist`, tick **Swap artist and title**.

The limits are 1 MiB per list, 1,000 entries and 4,096 bytes per field.

## Choose how strict matching is { #matching }

**Matching** has two settings:

| Setting | An entry matches a track when |
|---|---|
| **Same tags** (the start) | the title, and the artist if you gave one, are equal once capitals, punctuation and accents are ignored |
| **Same or similar tags** | they are equal, or close enough in spelling that both the title and the artist pass a similarity check |

The version name is part of the title. A line that says `Atoma` does not match the library title *Atoma (Extended Mix)* under **Same tags**, and **Same or similar tags** is meant for spelling differences, not for missing words. Put the version name in your list. <!-- UNVERIFIED: trigram similarity of 'atoma' against 'atoma extended mix' computed by hand at about 0.48, below the 0.72 title threshold in tracklist.rs; not run in the app -->

Click **Find library candidates**.

## Review and choose { #review }

The review shows *Review N entries* and a count such as *0 selected · 5 skipped*. Nothing is chosen for you, even when an entry has exactly one exact match. Similar tags can describe a different recording, so you decide.

For each entry:

1. Read the candidates, up to ten per entry, best match first. Each shows the title, artist and album, **Same tags** or **Similar tags** with a percentage, and the length.
2. Click **Preview** beside a candidate to play it in the persistent player. See [Preview a track with the player](../analysis/preview-a-track.md).
3. Select the candidate you want. **Skip this entry** is selected until you choose.

With **Same tags** and the five lines above, the purchase folder gives one candidate each for the first four entries and none for *Someone Else - Unreleased Edit*, which says *No library candidates. This entry will be skipped.* If an entry has more than ten candidates, a note says *Showing 10 of N candidates* and suggests adding an artist or a version name.

Open the skipped-entries line at the bottom (it starts with the number of skipped entries) to see every skipped entry as text you can select and copy. Use it as your shopping list for the tracks you do not own.

## Save the playlist { #save }

1. Type a **Playlist name** (it starts as `Reference tracklist`).
2. Click **Save selected to playlist**.

CueRithm creates an [ordinary playlist](organize-playlists.md#item-types) at the top level of the sidebar, in the order of your list, and opens it. If you chose the same library track for two entries, it appears once, at the first of those places. Entries you left on **Skip this entry** are left out. The page confirms with *Saved N tracks to “…”.*

Here the four choices give a playlist of *Palm Of Your Hands* (127 BPM, `11A`), *Come To Life* (130 BPM, `7A`), *Dancin* (131 BPM, `6A`) and *Atoma* (130 BPM, `2A`), in that order. Open it and use [Check the flow of a playlist](check-playlist-flow.md) to see how it moves.

To take the playlist back, click **Undo playlist creation** on this page, or the sidebar's **Undo last playlist change** ([how Undo works](organize-playlists.md#undo)).

## What can go wrong

| What you see | Why | What to do |
|---|---|---|
| **Find library candidates** is greyed out | The list is empty or over 1 MiB | Paste a list, or split a long one |
| *Choose a .txt or .csv file.* | The file has another extension | Export your list as text or CSV |
| *CSV needs one artist column and one title column; check the header setting* | The header row lacks exactly one `artist` and one `title` | Rename the headers, or untick **CSV has artist/title headers** |
| *Tracklist exceeds 1,000 entries* | The list is too long | Match it in parts |
| *Refresh matches before saving. Your current review is retained below.* | The library changed after you matched | Click **Find library candidates** again; your choices stay on screen |
| **Save selected to playlist** is greyed out | Nothing is chosen, the name is empty, or the review is out of date | Choose a candidate and check the name |

## Related

- [Check the flow of a playlist](check-playlist-flow.md)
- [Reorder, remove and restore tracks](edit-playlist-tracks.md)
- [Add tracks: drop, pick or watch a folder](../files/add-tracks.md)
- [Find tracks that mix with the one playing](find-compatible-tracks.md)

<!-- verified: apps/desktop/src/views/TracklistMatcher.tsx (labels, defaults, messages, review, save, undo); tracklist-types.ts; App.tsx (toolSaved opens playlist); crates/cuerithm-library/src/tracklist.rs (parsing, limits, normalisation, thresholds, candidate cap 10, prepared tracks only, save via save_prepared_playlist: top level, de-duplicated, first position); src/test/tracklist-matcher.test.tsx; example computed from _meta/library-facts.md; legacy guides/tracklist-matcher.md re-checked -->
