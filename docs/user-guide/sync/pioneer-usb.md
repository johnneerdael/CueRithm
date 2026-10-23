# Prepare a Pioneer USB stick

<p class="lead">Fill a USB stick with your tracks, playlists, cues and beatgrids so a CDJ or XDJ player can load it, without opening rekordbox. Preparing a stick takes a few minutes plus the time to copy the audio.</p>

<!-- figure:existing-86-sync-pioneer-usb-pioneer-usb -->
<figure markdown>
![Pioneer USB options with key notation, cue destination and CDJ compatibility controls](../assets/screens/86-sync-pioneer-usb.webp){ .screen loading=lazy }
<figcaption>Pioneer USB has its own options.</figcaption>
</figure>
<!-- /figure -->

**Before you start:** the stick is mounted on this computer, rekordbox is closed (CueRithm checks for it), and the tracks you want are prepared and in **Tracks** ([Choose what to sync](choose-what-to-sync.md#eligible-tracks)).

!!! warning "Test the stick before a gig"
    CueRithm's stick output was checked against the file formats, not on a physical CDJ or XDJ. Load a small playlist on your own player before you rely on the stick.

## Fill the stick { #fill-the-stick }

1. Choose **Sync** in the sidebar and open **Sync to**, then choose **Pioneer USB**.
2. Under **Target USB drive**, choose the stick. CueRithm lists mounted drives that already hold a `PIONEER` folder, so a stick that rekordbox exported to appears by itself. A blank stick does not: use the folder button or type its location. The refresh button looks again.
3. Open **What to sync**. For a stick, **Playlist or folder** is usual; choose **Warm-up set** to put that playlist on the stick.
4. Check the options ([below](#options)). The defaults give hot cues and keep your audio as it is.
5. Choose **Preview sync**. The summary reads like `8 tracks · 2 playlists · 120 files to update · 2 folders to create · 410.0 MB`; the size is what will be written to the stick. Compare it with the stick's free space.
6. Read the amber lines. They name tracks with a compatibility issue, cues that do not fit, or files that cleanup would remove.
7. Choose **Sync to Pioneer USB**. Copying audio takes the longest.
8. Eject the stick from your computer's file manager before you pull it out.

**What you should see:** **Sync complete. Your backup is available below.** and a new entry in **Sync history**.

A stick that already holds a rekordbox export keeps its other tracks and playlists; CueRithm adds and updates its own and leaves unrelated catalog content alone.

## What ends up on the stick { #what-is-on-the-stick }

| On the stick | What it is |
|---|---|
| `PIONEER/rekordbox/exportLibrary.db` and `export.pdb` | The track and playlist catalog, in both formats rekordbox export uses. A stick that uses a hidden `.PIONEER` folder keeps it |
| `PIONEER/USBANLZ/...` | An analysis file set for every track: beat grid, cues and loops, and waveforms |
| `PIONEER/Artwork/...` | Cover images, when **Add album art** is on |
| `Contents/CueRithm/xx/....` | A copy of every audio file, named by a short hash. File names on the stick are not readable; the player shows titles from the catalog |

Your original music files are not changed.

## Choose the stick options { #options }

| Option | Default | What it does |
|---|---|---|
| **Cue destination** | **Hot cues** | **Hot cues**, **Memory cues** or **Hot and memory cues**. Hot cues use pad slots and allow eight per track; the preview stops at a ninth ("USB supports eight distinct hotcue slots") |
| **CDJ compatibility** | **No detection** | **Detect incompatible audio** adds a warning for each track with a problem. **Convert incompatible audio** also writes a stereo PCM WAV copy on the stick for each track CueRithm can fix; the source file is untouched |
| **Clean up unused files** | off | Removes files under `Contents`, `USBANLZ` and `Artwork` that the catalog no longer references. Tracks in your CueRithm library are protected; inspect the removals in the preview |
| **Extra cue markers** | off | Draws a marker for every cue on the track's colour overview waveform |
| **Add album art** | off | Writes each track's cover to the stick |
| **Convert keys** | **Original keys** | Writes keys as **Camelot**, **Open Key**, **Musical notation** or as they are ([table](sync-options.md#key-notation)) |
| **Use nearest supported colors** | on | Moves each cue colour to the closest of the eight the stick supports. With it off, a colour that is not exactly one of the eight is dropped |
| **Field mappings** | none | Fill one destination field from another ([Set the sync options](sync-options.md#field-mappings)) |

Pioneer USB always sends smart playlists as ordinary playlists of their saved tracks, so **All smartlists to playlists** changes nothing here. The stick needs a beat grid, so **Keep existing beatgrids** is not offered.

## Which audio can go on the stick { #audio-formats }

| Format | Result |
|---|---|
| MP3, M4A, WAV, AIFF | Copied as they are |
| FLAC | Copied. **Detect incompatible audio** warns that FLAC and Apple Lossless playback depends on the player model; convert is not applied to it |
| Ogg Vorbis, Opus, WMA, other formats | The preview stops: "Pioneer USB requires MP3, M4A, FLAC, WAV or AIFF audio". Choose **Convert incompatible audio** to put a WAV copy on the stick |
| DRM-protected M4P | Never exported |

With **Detect** or **Convert**, CueRithm also flags a sample rate above 48 kHz or below 32 kHz, more than two channels, floating-point or extensible-header WAV, compressed AIFF and some MP3 header problems. **Convert** fixes those by writing a stereo PCM WAV copy. See [Check that your files are supported](../files/supported-formats.md).

## If the sync is interrupted

Reconnect the stick at its original mount location or drive letter, then open **Sync history** and use **Finish sync** or **Restore backup** ([Preview, apply and restore a sync](preview-apply-restore.md#recover)). A conversion backs up and restores the affected copies on the stick. If the file system cannot give the stick a stable identity, sync stops before it publishes anything.

## What can go wrong { #what-can-go-wrong }

| What you see | Why | What to do |
|---|---|---|
| "Close the target DJ software before syncing" | rekordbox or its Agent is running | Quit both, preview again |
| "This drive contains two Pioneer library catalogs; disable unused file cleanup to preserve both" | The stick has both `PIONEER` and `.PIONEER` catalogs | Switch **Clean up unused files** off |
| "Pioneer USB requires MP3, M4A, FLAC, WAV or AIFF audio" | A track is in another format | Choose **Convert incompatible audio** or leave the track out |
| "USB track metadata exceeds PDB page; shorten title/comment or path" | A title or comment is too long for the catalog | [Edit the tags](../tagging/edit-tags-by-hand.md) and preview again |
| "Device export would replace a library source audio file; choose another destination or keep the source copy unchanged" | The stick path points at your own music folder | Choose the stick itself |
| The stick is not listed | It has no `PIONEER` folder yet | Enter its location by hand |

## Related

- [Set the sync options](sync-options.md)
- [Choose what to sync](choose-what-to-sync.md)
- [Preview, apply and restore a sync](preview-apply-restore.md)
- [Build a playlist](../playlists/create-a-playlist.md): the playlists that go on the stick

<!-- verified: apps/desktop/src/views/Sync.tsx (labels, options, placeholder); crates/cuerithm-dj-sync/src/pioneer_usb.rs (write_staged_owned: file types, DRM, paths, copy, catalogs), pioneer_usb/analysis.rs (cues, extra markers), pioneer_usb/compatibility.rs (issues), pioneer_usb/paths.rs, pioneer_usb/database.rs (PDB page message), sync.rs (usb_cleanup_candidates, validate_device_source_replacements), discovery.rs (PIONEER/.PIONEER on volumes); docs/research/windows-linux-dj-sync-contract.md; legacy guides/sync.md#pioneer-usb re-checked -->
<!-- UNVERIFIED: which player models read exportLibrary.db versus export.pdb; the sample counts in step 5 are placeholders for what the app prints -->
