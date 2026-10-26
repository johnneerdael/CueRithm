# Sync to VirtualDJ

<p class="lead">Update your VirtualDJ library with CueRithm's beatgrids, keys, cues and playlists. A sync takes a few minutes, and it keeps a backup you can restore.</p>

**Before you start:** VirtualDJ 8.5 is installed and has been started once, so its `database.xml` exists. VirtualDJ is closed. CueRithm looks for it by the names `virtualdj`, `virtualdj8`, `virtualdj64` and `virtualdj_home`, and refuses to write while one is running. VirtualDJ's maker discourages editing its database from outside, which is why CueRithm writes only while VirtualDJ is closed and keeps a backup of every file it replaces.

## Sync your library { #sync-your-library }

<!-- figure:sync-virtualdj-options -->
<figure markdown>
![Sync with VirtualDJ chosen: the VirtualDJ library folder field, the Application environment menu, What to sync, and the options All smartlists to playlists, Use nearest supported colors and Keep existing beatgrids](../assets/screens/sync-virtualdj-options-light.webp#only-light){ .screen loading=lazy }
![Sync with VirtualDJ chosen: the VirtualDJ library folder field, the Application environment menu, What to sync, and the options All smartlists to playlists, Use nearest supported colors and Keep existing beatgrids](../assets/screens/sync-virtualdj-options-dark.webp#only-dark){ .screen loading=lazy }
<figcaption>VirtualDJ has the shared options and no key notation or drive controls.</figcaption>
</figure>
<!-- /figure -->

1. Choose **Sync** in the sidebar, open **Sync to** and choose **VirtualDJ**.
2. Under **VirtualDJ library folder**, check the location. CueRithm fills in the first library it finds, and if it finds several a **Found libraries** menu lets you choose. The folder button browses; the refresh button looks again. You can pick the folder or its `database.xml`.
3. Choose **What to sync** ([Choose what to sync](choose-what-to-sync.md)).
4. Check the options: **All smartlists to playlists**, **Use nearest supported colors**, **Keep existing beatgrids** and **Field mappings**. There is no key-notation option; VirtualDJ always receives the key in musical notation, such as `Gm` for a track in 6A.
5. Choose **Preview sync**. The summary lists the files to update, which are `database.xml` and the playlist files, and the folders to create.
6. Choose **Sync to VirtualDJ**.

**What you should see:** **Sync complete. Your backup is available below.** Open VirtualDJ; the tracks should show their CueRithm details, grids and cues.

## What CueRithm writes { #what-is-written }

| In VirtualDJ | From CueRithm |
|---|---|
| Song entries in `database.xml` | One for each track in the sync, found by file location. Tracks VirtualDJ has not seen are added |
| Tags | Title, artist, album, genre, label, key, comment, year, rating and track colour. For a track VirtualDJ already has, these are replaced with CueRithm's values |
| Tempo and grid | The BPM and a beat-grid marker per tempo anchor, unless **Keep existing beatgrids** is on |
| Cues and loops | Numbered cues and loops with names and colours. A loop gets its length in beats, and an active loop is set to start on load |
| Playlists | In a folder named `CueRithm` inside VirtualDJ's `MyLists`, as playlist files with their order. A VirtualDJ without `MyLists` gets the older `Playlists` and `Folders` layout |

Songs that CueRithm is not syncing are left alone. On a song it does update, the cues and loops are replaced as a set, and VirtualDJ's other point types are kept. VirtualDJ's play counts and other history are not touched. A smart playlist is sent as a native VirtualDJ smart playlist when the whole rule fits, and otherwise as an ordinary playlist ([Choose what to sync](choose-what-to-sync.md#smart-playlists)). CueRithm adjusts nothing for decoder timing in VirtualDJ.

## Libraries on other drives { #other-drives }

VirtualDJ keeps one database per drive. For a Windows library, a track on another drive is written into that drive's own `VirtualDJ/database.xml`, which must already exist. CueRithm will not create it: "VirtualDJ drive database is missing; initialize this drive in VirtualDJ first". See [Use a Windows DJ library on Linux](windows-libraries-on-linux.md#map-drives).

## What can go wrong { #what-can-go-wrong }

| What you see | Why | What to do |
|---|---|---|
| "Close the target DJ software before syncing" | VirtualDJ is running | Quit it, preview again |
| "DJ target was not found; initialize it in the DJ app first" | The folder has no `database.xml` | Start VirtualDJ once, or choose the right folder |
| "Unsupported VirtualDJ database version" | `database.xml` is not version 8.5 | Open it in a current VirtualDJ first |
| "Selected source paths collide in the Windows destination" | Two files would get the same Windows path | Rename one |
| Cues look a few milliseconds off | Decoder differences between programs | [Check the beatgrid at beat level](../analysis/check-the-beatgrid.md) in CueRithm first; fix the grid there, then sync again |

CueRithm's output for VirtualDJ was checked against the database format, not run inside VirtualDJ ([Check what works with your DJ software](../help/compatibility.md)). Try a small playlist first. To undo a sync, see [Preview, apply and restore a sync](preview-apply-restore.md#restore).

## Related

- [Set the sync options](sync-options.md)
- [Choose what to sync](choose-what-to-sync.md)
- [Preview, apply and restore a sync](preview-apply-restore.md)
- [Save a destination for next time](saved-destinations.md)

<!-- verified: apps/desktop/src/views/Sync.tsx; sync-types.ts (location label); crates/cuerithm-dj-sync/src/virtual_dj.rs (track_xml, POI cues/loops, Version 8.5 check, MyLists/CueRithm.subfolders, legacy layout, foreign POIs); discovery.rs (candidates, process names); sync.rs (per-drive databases, messages); formatting.rs call in track_xml (musical key); docs/research/dj-sync-timing.md (VirtualDJ zero offset), dj-sync-playlist-ownership.md; legacy guides/sync.md re-checked -->
<!-- UNVERIFIED: how VirtualDJ lists the CueRithm playlist folder; that the "CueRithm" folder name is shown exactly so in VirtualDJ -->
