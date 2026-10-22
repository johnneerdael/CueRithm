# Sync to Mixxx

[Mixxx](https://mixxx.org) is free, open-source DJ software that runs natively on
macOS, Windows and Linux. CueRithm writes straight into Mixxx's own library, so your
beatgrids, keys, hot cues, saved loops and playlists are there the next time you open
Mixxx. Nothing is written into your music files, and every sync saves a backup you
can restore.

## Before you sync

1. **Start Mixxx once** so it creates its library, then **quit it**. CueRithm will not
   write while Mixxx is open and never closes it for you.
2. Use **Mixxx 2.4 or newer**. CueRithm recognizes the library formats used by
   Mixxx 2.4, 2.5 and the current development versions, and refuses anything else
   with a message that names the version it found. Open an older library in a current
   Mixxx once to upgrade it.
3. If Mixxx did not close properly last time, open and quit it normally. CueRithm
   stops when it finds a half-written library file.

Choose **Sync to → Mixxx**. CueRithm lists the libraries it finds. Otherwise choose
the **Mixxx settings folder** or its `mixxxdb.sqlite` file:

| System | Where Mixxx keeps its library |
| --- | --- |
| macOS | `~/Library/Application Support/Mixxx` |
| Windows | `%LOCALAPPDATA%\Mixxx` |
| Linux | `~/.mixxx` |
| Linux, Flatpak | `~/.var/app/org.mixxx.Mixxx/.mixxx` |

If you started Mixxx with `--settings-path`, choose that folder.

## Choose a mode

**Existing Mixxx tracks** decides what happens to tracks Mixxx already has. Either
way, tracks Mixxx does not have yet are added with their CueRithm details, key,
beatgrid and cues.

| | Add only (default) | Manage library |
| --- | --- | --- |
| Tracks missing from Mixxx | Added | Added |
| Tracks already in Mixxx | **Left as they are** | CueRithm **replaces** the selected data |
| Your playlists and crates in Mixxx | Untouched | Untouched |
| Same-name playlist already in Mixxx | Missing tracks are appended; nothing is removed | Its tracks and order are set from CueRithm |

Use **Add only** to put a playlist or folder of tracks into a Mixxx library you have
already built. Use **Manage library** when CueRithm should be the source of your
beatgrids, keys and cues. Under **Manage library**, switch off **Replace track
details**, **Replace keys** or **Replace hot cues and loops** to keep that part of
Mixxx's data. A value CueRithm does not have, such as a track with no key, never
clears what Mixxx holds.

Both modes combine with any **What to sync** choice. For example, **Playlist or
folder** with **Add only** adds just those tracks and playlists.

In either mode a track you had removed from Mixxx, or that Mixxx had marked missing,
becomes visible again when it is part of the sync, as adding it in Mixxx would do.

Hot cues and saved loops are replaced as a set for a track that has CueRithm cues.
Mixxx's main cue, intro and outro markers are kept.

## Playlists and crates

**Playlists become** chooses where CueRithm playlists go:

- **Playlists (keep order)** keeps track order and repeated tracks. This is the default.
- **Crates (unordered)** puts each list in a Mixxx crate. Crates have no order and no
  duplicates.

Mixxx has no folders and no smart playlists, so:

- A playlist inside folders becomes one list named **Folder / Playlist**.
- A smart playlist is exported as its saved tracks. The preview tells you which ones.

CueRithm remembers the lists it created. Renaming or reordering a list in CueRithm
updates the same list in Mixxx, and removing it in CueRithm removes it from Mixxx
**only if you have not changed it in Mixxx since**. A list you edited, locked or
created yourself is never deleted. If you edit a list CueRithm created, **Manage
library** puts it back to CueRithm's version at the next sync and **Add only** just
adds any missing tracks. Mixxx's Auto DJ and history playlists are never touched.

## What is transferred

| CueRithm | In Mixxx |
| --- | --- |
| Title, artist, album, genre, comment, year, rating, track colour | The same fields |
| BPM and beatgrid | A constant grid, or a beat map when the tempo changes |
| Key | The key, shown in whichever notation your Mixxx uses |
| Hot cues and loops | Hot cue slots 1–36, with names and exact colours |
| Playlists and folders | Playlists or crates, as above |

Not transferred: album art (Mixxx finds it from your files), the record label (map it
into **Comment** with **Field mappings** if you want it), whether a loop starts
active, and cues beyond slot 36. The preview names skipped cues. **Do not send
beatgrids** leaves the grid to Mixxx's own analysis. **Beatgrid locks** sets Mixxx's
BPM lock so Mixxx will not re-analyze a grid you trust.

CueRithm does not change Mixxx's settings, the folders it scans, play counts, replay
gain or cover art.

## Timing

Mixxx's MP3 decoder starts a little differently from the one CueRithm analyzes with,
so CueRithm shifts MP3 cues and grids to match Mixxx, by roughly 25 ms for typical
files. The shift comes from each file's own header and is applied once. FLAC, WAV,
AIFF, Ogg and Opus need none.

For **AAC** (`.m4a`, `.mp4`, `.aac`) and **WMA**, Mixxx decodes differently on each
system and CueRithm cannot confirm the alignment, so the preview warns that cues
may sit a few milliseconds off. Check one such track by ear before relying on them.

## Preview, apply and restore

1. Quit Mixxx.
2. Choose **Preview sync**. The first line summarizes what will be added, replaced
   and left unchanged.
3. Review the warnings, then choose **Sync to Mixxx**. A second sync of the same
   data changes nothing and says **Everything is already synced.**
4. **Restore backup** in the sync history returns the library to how it was, as long
   as you have not changed it since.

If you change the library, the folder or the options after previewing, preview
again. Files you move in CueRithm's **Organize** flow are repaired in Mixxx too, so
synced tracks stay linked.

## Flatpak and folders Mixxx scans

The Flatpak version of Mixxx can only read your Music folder, `/media` and
`/run/media` unless you allow more. The preview warns about tracks outside those
folders, which Mixxx would show as missing.

Tracks outside the folders Mixxx scans still play and stay in its library, but Mixxx
will not find new files next to them. CueRithm does not change that list; add the
folder in Mixxx's preferences if you want it scanned.

## What has been tested

The Mixxx library format was read from Mixxx's source code and checked with generated
libraries for the two supported schema versions. See the
[format notes](https://github.com/johnneerdael/Cuerithm/blob/main/docs/research/mixxx-sync-contract.md)
for what was measured and what was only read. Windows and Linux builds of Mixxx have
not been run against CueRithm yet, and AAC alignment is unverified as described above.
