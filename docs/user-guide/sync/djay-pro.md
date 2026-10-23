# Sync to djay Pro

<p class="lead">Update your djay Pro library with CueRithm's grids, keys, cues and playlists. A sync takes a few minutes, and it keeps a backup you can restore.</p>

**Before you start:** djay Pro has created its media library (the library is a `MediaLibrary.db` file), and djay Pro is closed. CueRithm looks for the programs `djay`, `djay Pro` and `djayApp` and refuses to write while one runs. CueRithm cannot create a djay Pro library; it only updates an existing one.

## Sync your library { #sync-your-library }

1. Choose **Sync** in the sidebar, open **Sync to** and choose **djay Pro**.
2. Under **djay Media Library folder**, check the location. CueRithm finds libraries under `Music/djay`; if it finds several, a **Found libraries** menu lets you choose. You can pick the folder that holds the library or the `MediaLibrary.db` file.
3. Choose **What to sync** ([Choose what to sync](choose-what-to-sync.md)).
4. Check the options: **All smartlists to playlists**, **Use nearest supported colors**, **Keep existing beatgrids** and **Field mappings** ([Set the sync options](sync-options.md)).
5. Choose **Preview sync**, read the warnings, then choose **Sync to djay Pro**.

**What you should see:** **Sync complete. Your backup is available below.** The first time you start djay Pro afterwards it rebuilds its own search and sort views from the library, so it may take a little longer to open.

## What CueRithm writes { #what-is-written }

| In djay Pro | From CueRithm |
|---|---|
| Media items | Title, artist, album, genre, label, comment, BPM, key, rating, track colour and year, plus bitrate and track number where CueRithm has them. Items are matched by the audio file's location; equivalent spellings of one location (different slashes or encodings) reuse the same item |
| Beat grid | CueRithm's BPM and first beat, unless **Keep existing beatgrids** is on |
| Cues and loops | Up to eight numbered cue or loop slots, with names and colours |
| Playlists | Playlists and folders with their order, and smart playlists when the whole rule fits |

djay Pro's play counts are preserved. The active-loop flag is not written to djay Pro. Anything CueRithm does not sync, including unrelated playlists and items, is left in place. CueRithm adjusts nothing for decoder timing in djay Pro.

A ninth cue stops the preview ("djay Pro supports eight numbered cue/loop slots"). If two of the files in the sync resolve to one djay item, the preview refuses to guess ("Multiple selected djay paths identify the same physical source").

## What can go wrong { #what-can-go-wrong }

| What you see | Why | What to do |
|---|---|---|
| "Close the target DJ software before syncing" | djay Pro is running | Quit it, preview again |
| "DJ target was not found; initialize it in the DJ app first" | There is no library at that location | Start djay Pro once, or choose the right folder |
| A track appears twice in djay Pro | djay already knew the file under a location CueRithm could not match | Remove the duplicate in djay Pro; keep each file in one place |

CueRithm's output for djay Pro was checked against the library format, not run inside djay Pro ([Check what works with your DJ software](../help/compatibility.md)). Try a small playlist first. To undo a sync, see [Preview, apply and restore a sync](preview-apply-restore.md#restore). On Linux with a Windows djay, see [Use a Windows DJ library on Linux](windows-libraries-on-linux.md).

## Related

- [Set the sync options](sync-options.md)
- [Choose what to sync](choose-what-to-sync.md)
- [Preview, apply and restore a sync](preview-apply-restore.md)
- [Save a destination for next time](saved-destinations.md)

<!-- verified: apps/desktop/src/views/Sync.tsx; sync-types.ts; crates/cuerithm-dj-sync/src/djay_pro.rs (module doc, slot limit, identity messages, beat grid, index invalidation), discovery.rs (candidates, process names), canonical_target in dj_sync_service.rs (folder or MediaLibrary.db); docs/research/windows-linux-dj-sync-contract.md (bitrate, track number, play counts preserved, URI identity), dj-sync-timing.md (djay zero offset) -->
<!-- UNVERIFIED: that djay Pro opens slower the first time after a sync; the code invalidates its derived index versions so djay rebuilds them, the duration is not measured -->
