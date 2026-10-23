# Sync to Engine DJ

<p class="lead">Update an Engine DJ library, or fill an Engine drive for a standalone player, with CueRithm's tracks, grids, cues and playlists. A sync takes a few minutes plus the time to copy audio when you ask for copies.</p>

**Before you start:** the Engine DJ library or drive has been initialised by Engine DJ (its `Database2/m.db` exists and uses the current schema, major version 3), and Engine DJ is closed. CueRithm refuses to write while a process called `Engine DJ` is running.

## Sync to an Engine library { #sync-to-engine }

1. Choose **Sync** in the sidebar, open **Sync to** and choose **Engine DJ**.
2. Under **Engine Library folder or target drive**, check the location. CueRithm finds `Music/Engine Library` and any mounted drive that holds an `Engine Library` folder; the folder button browses, and the refresh button looks again. You can choose the `Engine Library` folder, its `Database2` folder, the drive, or `m.db` itself.
3. Choose **What to sync** ([Choose what to sync](choose-what-to-sync.md)).
4. Check the options ([below](#options)).
5. Choose **Preview sync**, read the warnings, then choose **Sync to Engine DJ**.

**What you should see:** **Sync complete. Your backup is available below.**

## What CueRithm writes { #what-is-written }

| In Engine DJ | From CueRithm |
|---|---|
| Track rows | Title, artist, album, genre, label, comment, BPM, year, key and rating (as stars). Tracks Engine does not have are added; for a track it has, these fields are replaced |
| Performance data | The beat grid (unless **Keep existing beatgrids** is on), up to eight quick cues and up to eight loops with names and colours, and the loops that start on load |
| Playlists | Engine playlists with their order, and smart playlists when the whole rule fits |
| Cover art | Only with **Add album art** |
| Audio | Not copied unless the library is on a drive or **Copy audio to the target drive** is on |

Unrelated tracks and playlists in the library stay. CueRithm adjusts nothing for decoder timing in Engine DJ. A ninth cue or loop stops the preview ("Engine DJ supports eight cue slots", "Engine DJ supports eight loop slots").

## Choose the Engine DJ options { #options }

| Option | Default | What it does |
|---|---|---|
| **All smartlists to playlists** | off | Sends every smart playlist as an ordinary playlist |
| **Copy audio to the target drive** | off | Copies each audio file onto the Engine drive and refers to the copy. See below |
| **Use nearest supported colors** | on | Moves cue colours to the closest of Engine's eight; with it off the exact colour is written |
| **Keep existing beatgrids** | off | Leaves Engine's grids as they are |
| **Beatgrid locks** | **Default** | **Default** keeps each track's current lock and locks new tracks that receive a CueRithm grid. **Lock** and **Unlock** set every track |
| **Add album art** | off | Writes each track's cover, normalised to fit Engine's limits; a cover over 10 MB is refused |
| **Field mappings** | none | Fill one destination field from another ([Set the sync options](sync-options.md#field-mappings)) |

### When audio is copied { #copy-audio }

If the Engine Library sits on a mounted drive, CueRithm copies the audio for you and writes references relative to the drive. That covers folders under `/Volumes`, `/media`, `/run/media` and `/mnt`, and `E:\Engine Library`-style folders on Windows. For any other folder, such as `~/Music/Engine Library`, tracks stay where they are and Engine refers to them in place unless you switch **Copy audio to the target drive** on. A smart playlist rule that depends on file location is sent as its saved tracks when audio is copied, because the copies have new locations.

CueRithm refuses a copy that would replace one of your own library files: "Device export would replace a library source audio file; choose another destination or keep the source copy unchanged".

## What can go wrong { #what-can-go-wrong }

| What you see | Why | What to do |
|---|---|---|
| "Select an initialized Engine DJ library folder, drive or m.db" | No `m.db` was found there | Start Engine DJ on that library or drive once |
| "Engine DJ requires the current schema major 3; found 2" | An older Engine library | Update the library in Engine DJ first |
| "Close the target DJ software before syncing" | Engine DJ is running | Quit it, preview again |
| "Engine DJ cover image exceeds 10 MB" | A track's cover is too large | [Replace the artwork](../tagging/edit-tags-by-hand.md) with a smaller image |

CueRithm's output for Engine DJ was checked against the database format, not run inside Engine DJ or on a player ([Check what works with your DJ software](../help/compatibility.md)). Try a small playlist first. To undo a sync, see [Preview, apply and restore a sync](preview-apply-restore.md#restore).

## Related

- [Set the sync options](sync-options.md)
- [Choose what to sync](choose-what-to-sync.md)
- [Use a Windows DJ library on Linux](windows-libraries-on-linux.md)
- [Save a destination for next time](saved-destinations.md)

<!-- verified: apps/desktop/src/views/Sync.tsx; sync-types.ts; crates/cuerithm-dj-sync/src/engine_dj.rs (resolve_database_path, schema major 3, track update, performance data, slot limits, lock default, colour palette, cover limits), sync.rs (engine_device_library, copies_media, validate_device_source_replacements), discovery.rs (candidates, process name "Engine DJ"); docs/research/dj-sync-timing.md (Engine zero offset); legacy guides/sync.md re-checked -->
