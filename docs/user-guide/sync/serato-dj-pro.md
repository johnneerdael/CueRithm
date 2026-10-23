# Sync to Serato DJ Pro

<p class="lead">Add CueRithm's cues, loops and beatgrids to Serato DJ Pro 4 and rebuild your crates. Unlike the other destinations, Serato keeps cues and grids inside the audio files, so this sync edits your music files' tags. A sync takes a few minutes, and you need free disk space for its backup.</p>

!!! warning "This sync writes into your music files"
    Serato stores cues, loops and the beatgrid in each audio file's tags. CueRithm writes those tags and leaves other tags and the audio itself unchanged, but the files do change. Every affected file is backed up first, so the backup is as large as the tracks you sync. The preview reports the total size.

**Before you start:** Serato DJ Pro 4 has created its library (`root.sqlite` exists), Serato is closed (CueRithm looks for `Serato DJ Pro`), and you have free space for the backup.

## Sync your library { #sync-your-library }

1. Choose **Sync** in the sidebar, open **Sync to** and choose **Serato DJ Pro 4**.
2. Under **Serato Library folder**, check the location. On macOS this is `~/Library/Application Support/Serato/Library`, and on Windows `%LOCALAPPDATA%\Serato\Library`. Choose the folder that holds `root.sqlite`, not `master.sqlite`, which is Serato's own combined database.
3. If your music is on an external or network drive, enter that drive in **External or network drive root (optional)**. CueRithm then writes the drive's own Serato location database, which Serato must have created ("Serato location database is missing; initialize this drive or share in Serato first").
4. Choose **What to sync** ([Choose what to sync](choose-what-to-sync.md)) and check the options ([below](#options)).
5. Choose **Preview sync**. Read the number of files and the size, then choose **Sync to Serato DJ Pro 4**.

**What you should see:** **Sync complete. Your backup is available below.** In Serato the tracks show their cues, loops and grids, and your playlists appear as crates.

## What CueRithm writes { #what-is-written }

| Where | From CueRithm |
|---|---|
| Serato's database | Tracks with title, artist, album, genre, BPM, key and year; crates for your playlists; a refreshed aggregate library |
| The audio file's tags | A marker tag with up to eight hot cues and eight saved loops, with names and colours, and a beatgrid tag with up to 128 tempo anchors |

The audio files that can take these tags are MP3, M4A and MP4, FLAC, Ogg Vorbis, WAV and AIFF. Others, such as Opus or WMA, stop the preview: "Serato 4 native tag sync supports MP3, MP4/M4A, FLAC, Ogg Vorbis, WAV and AIFF; unsupported audio: …". The active-loop flag is not written.

Timing is adjusted for Serato's own decoder on a copy: cues and grids in MP3 files, and in some AAC files, are shifted by a few tens of milliseconds so they land correctly in Serato. Your saved CueRithm cues do not change. After the tags are written CueRithm records each file's new fingerprint, so the library stays consistent.

## Choose the Serato options { #options }

| Option | Default | What it does |
|---|---|---|
| **All smartlists to playlists** | off | Sends every smart playlist as an ordinary crate |
| **Convert keys** | **Original keys** | Writes keys as **Camelot**, **Open Key** or **Musical notation** ([table](sync-options.md#key-notation)) |
| **Use nearest supported colors** | on | Moves cue colours to the closest one Serato offers |
| **Keep existing beatgrids** | off | Writes no grid tag and leaves the grid in each file |
| **Beatgrid locks** | **Default** | **Default** locks every track that receives a grid; **Lock** and **Unlock** set all of them |
| **Field mappings** | none | Fill one destination field from another ([Set the sync options](sync-options.md#field-mappings)) |

Smart playlists become Serato smart crates when the whole rule fits.

## Restore a Serato sync { #backup-and-restore }

The backup of a Serato sync contains the native library files and every audio file whose tags changed. **Restore backup** puts the original tags back, provided the files are still what the sync wrote. A restore refuses when you have edited those files in Serato or elsewhere since ([Preview, apply and restore a sync](preview-apply-restore.md#restore)). Archiving a synced track does not stop its restore.

## What can go wrong { #what-can-go-wrong }

| What you see | Why | What to do |
|---|---|---|
| "Serato 4 root.sqlite is missing; initialize the library in Serato first" | The folder has no Serato library | Start Serato once, or choose the right folder |
| "Serato supports eight hot cues and eight saved loops" | A track has more than eight cues | [Remove a cue](../analysis/corrections/move-or-add-cues.md) in CueRithm |
| "Serato grids support at most 128 tempo anchors" | A very long grid | Keep the grid constant ([Fix a wrong beatgrid](../analysis/corrections/fix-the-beatgrid.md)) |
| "Close the target DJ software before syncing" | Serato is running | Quit it, preview again |
| The preview is large | Each tagged audio file is staged and backed up | Sync a playlist at a time ([Choose what to sync](choose-what-to-sync.md#playlist-scope)) |

CueRithm's output for Serato was checked against the library and tag formats, not run inside Serato ([Check what works with your DJ software](../help/compatibility.md)). Try a small playlist first.

## Related

- [Set the sync options](sync-options.md)
- [Choose what to sync](choose-what-to-sync.md)
- [Preview, apply and restore a sync](preview-apply-restore.md)
- [Fix a wrong cue, loop or beatgrid](../analysis/corrections/index.md): the cues and grid that Serato receives

<!-- verified: apps/desktop/src/views/Sync.tsx (Serato drive root field, Convert keys, Beatgrid locks); crates/cuerithm-dj-sync/src/serato4.rs (module doc, messages), serato4/media.rs (formats, Markers2, BeatGrid, BPMLOCK, limits), sync.rs; dj_sync_service.rs (tag gate, update_source_hashes); docs/research/dj-sync-timing.md (Serato offsets), dj-sync-playlist-ownership.md; legacy guides/sync.md re-checked -->
<!-- UNVERIFIED: that Serato displays the crates and tags as described; CueRithm's output was checked against file formats only -->
