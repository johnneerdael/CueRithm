# Sync to Mixxx

[Mixxx](https://mixxx.org) is free, open-source DJ software that runs natively on
macOS, Windows and Linux. CueRithm writes straight into Mixxx's own library, so your
beatgrids, keys, hot cues, saved loops and playlists are there the next time you open
Mixxx. Nothing is written into your music files, and every sync saves a backup you
can restore.

## Before you sync

1. **Quit Mixxx.** CueRithm will not write while Mixxx is open and never closes it for
   you. If you have never started Mixxx, you can sync first: CueRithm creates the
   library, and Mixxx starts with your tracks and does not ask for a music folder. (On
   macOS, start the Mixxx download once first, so that macOS creates its container.)
2. Use **Mixxx 2.4 or newer**. CueRithm goes by the library itself, not by a list of
   releases: it was checked with Mixxx 2.5, and it also writes the libraries of newer
   Mixxx releases as long as Mixxx still says older software can use them and the tables
   have the layout CueRithm writes. The preview tells you when the library is newer than
   what CueRithm was checked with. It refuses a library that needs newer software, with
   a message that names the version, and an older library until you have opened it in a
   current Mixxx once to upgrade it.
3. If Mixxx did not close properly last time, open and quit it normally. CueRithm
   stops when it finds a half-written library file.

Choose **Sync to → Mixxx**. CueRithm lists the libraries it finds. Otherwise choose
the **Mixxx settings folder** or its `mixxxdb.sqlite` file:

| System | Where Mixxx keeps its library |
| --- | --- |
| macOS, the Mixxx download | `~/Library/Containers/org.mixxx.mixxx/Data/Library/Application Support/Mixxx` |
| macOS, a build you made yourself | `~/Library/Application Support/Mixxx` |
| Windows | `%LOCALAPPDATA%\Mixxx` |
| Linux | `~/.mixxx` |
| Linux, Flatpak | `~/.var/app/org.mixxx.Mixxx/.mixxx` |

If you started Mixxx with `--settings-path`, choose that folder.

## Start from nothing, or replace everything

If the folder you choose has no Mixxx library, CueRithm creates one: all of Mixxx's
tables, an Auto DJ playlist, and your tracks, cues, keys, beatgrids and playlists. The
folder itself is created too, when the folder above it exists. The preview says so, and
**Restore backup** removes what was created.

**Mixxx library → Replace the whole library** does the same over a library Mixxx
already has, as you can with rekordbox. The new library holds what is in this sync and
nothing else: tracks, playlists, crates, history and play counts that Mixxx had are not
carried over. The preview counts them, the old library is kept in the sync's backup,
and **Restore backup** brings it back. It needs the full library, not a single
playlist or only the changes. Mixxx's saved waveform analyses are not carried
over either, so Mixxx analyzes the tracks again.

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

CueRithm does not change Mixxx's settings, play counts, replay gain or cover art. It
changes the list of folders Mixxx scans in one case only: when that list is empty.
Mixxx then asks you to choose a music folder every time it starts, so CueRithm adds
the folders that hold the synced tracks (never your home folder or a drive root) and
says so in the preview. A list you have built is never touched.

## Timing

MP3 files start slightly differently depending on the decoder, and Mixxx uses a
different one on each system, so CueRithm shifts MP3 cues and grids to match **the
Mixxx on the computer you sync from**. It reads each file's own header, and on macOS
asks the system's audio decoder (the one Mixxx uses there) what it will skip. The shift
is applied once and is usually zero on macOS and about 25 ms on Linux and Windows for
typical LAME-encoded files. FLAC, WAV, AIFF and Ogg need none. If you copy a Mixxx
library to another operating system, sync again from there.

**AAC and Apple Lossless** (`.m4a`, `.mp4`, `.aac`), **AIFF**, **Ogg Vorbis** and **Opus**
were checked in Mixxx on macOS and need no shift there (Opus is counted at 48 kHz, which
is how Mixxx decodes it). On Linux and Windows Mixxx uses different AAC decoders that
have not been checked, so the preview warns that cues may sit a few milliseconds off.
**WMA** cannot be opened by the Mixxx download for macOS, so those tracks are added but
will not play there; on Linux and Windows it only plays in Mixxx builds that include
FFmpeg (or Media Foundation on Windows) and is not verified. Check one such track by ear
before relying on its cues.

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
will not find new files next to them. CueRithm does not add folders to a list that
already has one; add the folder in Mixxx's preferences if you want it scanned.

## Mixxx on macOS runs in a sandbox

The Mixxx download for macOS is an App Sandbox application. It can read your Music and
Downloads folders and any folder you chose in one of its own file dialogs, and it asks
before reading anything else. That is why its library sits inside
`~/Library/Containers/org.mixxx.mixxx`. Music on an external drive or elsewhere in your
home folder still plays, but Mixxx asks you to pick the folder the first time it needs
it, and a folder CueRithm added to its scan list does not count as chosen. The preview
counts the tracks this affects. Keeping the music in `~/Music` avoids it.

macOS may also ask whether CueRithm may access data of another app, because the
library is in Mixxx's container. That prompt has not been tested with the CueRithm
application.

## What has been tested

The Mixxx library format was read from Mixxx's source code. On macOS, the installed
Mixxx 2.5.6 was then run against libraries CueRithm wrote (an existing library, one
replaced whole, one made before Mixxx had ever run, and one Mixxx had left empty): it
started without asking for a music folder, found the library current, read the grids
and keys, left the BPM-locked grids alone, and changed none of the synced rows. The
macOS decoder timing was measured with the same Apple calls Mixxx makes. See the
[format notes](https://github.com/johnneerdael/Cuerithm/blob/main/docs/research/mixxx-sync-contract.md)
for what was measured and what was only read. Windows and Linux builds of Mixxx have
not been run against CueRithm yet, so their timing for MP3 is derived from Mixxx's
source and their AAC timing is unverified, as described above. Nor has Mixxx 2.4 or a
newer Mixxx than 2.5, or a library on an external drive on macOS, where Mixxx asks for
access.
