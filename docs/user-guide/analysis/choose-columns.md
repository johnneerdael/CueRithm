# Show the columns you need

<p class="lead">Add the album, label or bitrate to the track table, hide what you never read, and put the columns in the order you scan them. It takes about two minutes, and CueRithm remembers your choice.</p>

<!-- figure:existing-34-columns-dialog-choose-columns -->
<figure markdown>
![The Table columns dialog with checkboxes for the columns and a Find metadata box](../assets/screens/34-columns-dialog.webp){ .screen loading=lazy }
<figcaption>Choose which columns the table shows.</figcaption>
</figure>
<!-- /figure -->

## Open the column list { #open-the-column-list }

<!-- figure:tracks-columns-panel -->
<figure markdown>
![The Table columns panel open beside the track table, with the Find metadata box and a checkbox for each column](../assets/screens/tracks-columns-panel-light.webp#only-light){ .screen loading=lazy }
![The Table columns panel open beside the track table, with the Find metadata box and a checkbox for each column](../assets/screens/tracks-columns-panel-dark.webp#only-dark){ .screen loading=lazy }
<figcaption>The Columns button at the end of the heading opens the list of columns with an arrow pair for each.</figcaption>
</figure>
<!-- /figure -->

1. Open **Tracks** (or **Incoming**, **Archive** or a playlist).
2. Click the **Columns** button at the right end of the table heading. Its tooltip reads *Choose and order columns*.
3. The **Table columns** panel opens. Type in *Find metadata…* to narrow the list; the list holds one row per column, each with a checkbox and two arrow buttons.

The first time you open the panel, CueRithm may read the embedded tags of some
files to learn which fields your library has. *Reading embedded metadata…* shows
while it works.

Click the **Columns** button again, or **Close columns** (the × in the panel), when
you are done. The panel stays open until you do, so you can tick, untick and
reorder in one visit.

## Show or hide a column { #show-or-hide }

Tick a column to show it; untick it to hide it. A new column appears at the right
end of the table. A hidden column keeps its place in the order, so ticking it
again puts it back where it was.

The title column is special. It always combines the artwork, the title, a file
badge such as `FLAC` and the artist. Tick **Album art** and the artwork moves into
a column of its own.

## Put columns in your order { #reorder-columns }

Each column has two arrow buttons. **Move … left** moves the column one place
towards the start of the table; **Move … right** moves it one place towards the
end. A column you have never ticked cannot move yet.

!!! tip "If a column seems stuck"
    Hidden columns keep their place in the order, so a visible column may need
    two presses to pass a hidden neighbour. Press the arrow again.

The table scrolls sideways when the columns are wider than the window.

## Choose from the available columns { #available-columns }

The default columns are **TRACK / ARTIST**, **GENRE**, **BPM**, **KEY**, **ENERGY**,
**DURATION** and **PREVIEW**. These are what else you can show.

| Group | Columns | What they show |
|---|---|---|
| Track | **Artist**, **Album**, **Album art**, **Filetype**, **Status** | Library values. **Filetype** is the file format name, and **Status** is the track's state in the library, such as `ready` |
| Where and when | **Location**, **Date added**, **Date modified** | **Location** is the full path of the file on your computer. Dates use your computer's date format |
| Your own data | **User rating**, **Curation notes**, **Categories** | What you set with [Curate](../tagging/curate-tracks.md). **User rating** also sorts |
| Technical | **Codec**, **Container**, **Sample rate**, **Channels**, **Bit depth**, **Bitrate**, **Size**, **Encoded duration** | Facts about the audio file. **Bitrate** is in kbps and **Size** in MB. **Encoded duration** is the duration stored in the file, as a plain number |
| Tags in your files | **Album artist**, **Comment**, **Year**, **Release date**, **Track No.**, **Disc No.**, **Grouping**, **Remixer**, **Mix**, **Label**, **Publisher**, **Composer**, **Producer**, **Rating**, **ISRC**, **Catalog number** and more | The value stored in the file's tags. A tag column appears in the list only when files in your library have that tag |
| Embedded copies | **Embedded key**, **Embedded BPM**, **Embedded title**, **Embedded artist**, **Embedded album**, **Embedded genre** | The tag in the file, not CueRithm's own value |

A tag name CueRithm does not know gets a plain label made from the tag name, for
example `tag.custom:mood` becomes **Custom:mood**. An empty value shows an em dash
(—).

**KEY** and **BPM** show what CueRithm measured. **Embedded key** and **Embedded
BPM** show what a file's tags say, which is useful when you want to compare the
two. See [Read a track's key, BPM and energy](read-key-bpm-energy.md#where-the-numbers-come-from).

## Go back to the default columns { #default-columns }

There is no reset button. Tick **Title**, **Genre**, **BPM**, **Key**, **Energy**,
**Duration** and **Preview**, untick everything else, and use the arrows to restore
the order above.

CueRithm saves your columns on this computer, not in the library. Another
computer starts with the defaults.

## What can go wrong

| Symptom | Cause | Fix |
|---|---|---|
| A tag you expect, such as **Label**, is not in the list | No file in your library has that tag, or CueRithm has not read it yet | Open the panel again. If the tag is missing from the files, [edit tags and artwork](../tagging/edit-tags-by-hand.md) first |
| **Embedded key** disagrees with **KEY** | The tag was written by another program, or by tagging with *Catalog metadata* as the source | **KEY** is CueRithm's own measurement. See [My key or BPM differs from another app](../help/troubleshooting/key-or-bpm-differs.md) |
| The **Location** column shows a long path | That is the full path of the file | Hide the column before you share your screen |
| Columns are cut off at the right | Their combined width is wider than the window | Scroll the table sideways, or hide a column |

## Related

- [Find a track: search, sort and filter](find-tracks.md)
- [Work on many tracks at once](select-many.md)
- [Preview a track with the player](preview-a-track.md)
- [Status icons and job states](../reference/status-icons.md)

<!-- verified: apps/desktop/src/components/TrackTable.tsx (columns panel, defaults, localStorage track-columns, arrow buttons, hidden vs order, sort buttons); apps/desktop/src/columns.ts (labels, metadataText formats); apps/desktop/src/test/workspace.test.tsx (custom columns, move left); apps/desktop/src/test/columns.test.ts; apps/desktop/src-tauri/src/host.rs metadata_fields; apps/desktop/src/views/TaggingSettings.tsx (Key and BPM written to files); legacy guides/track-table.md re-checked -->
