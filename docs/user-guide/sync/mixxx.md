# Sync to Mixxx

<p class="lead">In about ten minutes you will send the Club nights folder into Mixxx, the free open-source DJ program for macOS, Windows and Linux, so its beatgrids, keys, named hot cues and playlists are there the next time you open Mixxx. CueRithm writes straight into Mixxx's own library, changes nothing in your music files, and keeps a backup you can restore.</p>

**You need:**

- [ ] Mixxx 2.4 or newer, or a computer where Mixxx has never been started (CueRithm can create the library)
- [ ] Prepared tracks in **Tracks**, and a playlist or folder to send; this tutorial uses **Club nights**
- [ ] Mixxx closed
- [ ] On macOS with the Mixxx download: Mixxx started once, so macOS has created its container (step 1)

## 1. Quit Mixxx and check your version { #before-you-sync }

CueRithm will not write while Mixxx is open and never closes it for you. It looks for a process called `mixxx`, which also covers the Flatpak and Windows builds.

- **Never started Mixxx?** You can sync first. CueRithm creates the library, and Mixxx then starts with your tracks and does not ask for a music folder. On macOS, start the Mixxx download once beforehand so that macOS creates its container; CueRithm does not create another program's container.
- **Mixxx did not close properly last time?** Open it and quit it normally. CueRithm stops when it finds a half-written library file: "Mixxx did not close cleanly: a -journal file is left beside mixxxdb.sqlite. Open Mixxx and quit it normally, then sync again".
- **Which versions?** CueRithm reads the library itself instead of keeping a list of releases. It was checked with Mixxx 2.5, and it also writes newer libraries that Mixxx marks as usable by older software and whose tables keep the layout CueRithm writes; the preview says when your library is newer than that. It refuses a library older than Mixxx 2.4 until you have opened it once in a current Mixxx, and one that needs newer software than CueRithm knows ("update CueRithm before syncing to this Mixxx").

## 2. Choose Mixxx and its settings folder { #choose-library }

Choose **Sync** in the sidebar, open **Sync to** and choose **Mixxx**. CueRithm lists the libraries it finds; with more than one, choose from **Found libraries**. Otherwise set **Mixxx settings folder** by hand, either the folder or its `mixxxdb.sqlite` file:

| System | Where Mixxx keeps its library |
|---|---|
| macOS, the Mixxx download | `~/Library/Containers/org.mixxx.mixxx/Data/Library/Application Support/Mixxx` |
| macOS, a build you made yourself | `~/Library/Application Support/Mixxx` |
| Windows | `%LOCALAPPDATA%\Mixxx` |
| Linux | `~/.mixxx` |
| Linux, Flatpak | `~/.var/app/org.mixxx.Mixxx/.mixxx` |

If you started Mixxx with `--settings-path`, choose that folder. Mixxx always runs natively, so Sync shows no Wine setup for it.

## 3. Start from nothing, or replace everything { #library-modes }

The **Mixxx library** menu has two choices.

<!-- figure:sync-mixxx-library-modes -->
<figure markdown>
![The Mixxx library menu set to Replace the whole library with its explanation, the Existing Mixxx tracks menu below it, and the Mixxx settings folder path blurred](../assets/screens/sync-mixxx-library-modes-light.webp#only-light){ .screen loading=lazy }
![The Mixxx library menu set to Replace the whole library with its explanation, the Existing Mixxx tracks menu below it, and the Mixxx settings folder path blurred](../assets/screens/sync-mixxx-library-modes-dark.webp#only-dark){ .screen loading=lazy }
<figcaption>Replace the whole library builds a new Mixxx library from the sync and keeps the old one in the backup.</figcaption>
</figure>
<!-- /figure -->

| Choice | What happens |
|---|---|
| **Update the library Mixxx has** (default) | Works inside the library that is there. If the folder has no Mixxx library yet, CueRithm creates one first: all of Mixxx's tables, its Auto DJ playlist, then your tracks, cues, keys, beatgrids and playlists. The folder is created too when the folder above it exists. The preview says so, and **Restore backup** removes what was created |
| **Replace the whole library** | Builds a new library from this sync and replaces the one in the folder. What Mixxx had, meaning its tracks, playlists, crates, history and play counts, is not carried over, and neither are its saved waveform analyses, so Mixxx analyses the tracks again. The preview counts what will be dropped, the old library is kept in the sync's backup, and **Restore backup** brings it back. It needs **Full library** as the scope |

For this tutorial keep **Update the library Mixxx has**.

## 4. Choose a mode { #choose-a-mode }

**Existing Mixxx tracks** decides what happens to tracks Mixxx already has. Tracks Mixxx does not have yet are added with their CueRithm details, key, beatgrid and cues either way. The menu is unavailable under **Replace the whole library**.

<!-- figure:sync-mixxx-manage-library -->
<figure markdown>
![Mixxx options with Existing Mixxx tracks set to Manage library, the three Replace checkboxes ticked, Playlists become set to Playlists (keep order), Do not send beatgrids and Beatgrid locks](../assets/screens/sync-mixxx-manage-library-light.webp#only-light){ .screen loading=lazy }
![Mixxx options with Existing Mixxx tracks set to Manage library, the three Replace checkboxes ticked, Playlists become set to Playlists (keep order), Do not send beatgrids and Beatgrid locks](../assets/screens/sync-mixxx-manage-library-dark.webp#only-dark){ .screen loading=lazy }
<figcaption>Manage library lets CueRithm replace Mixxx's grids, keys and cues piece by piece.</figcaption>
</figure>
<!-- /figure -->

| | **Add only** (default) | **Manage library** |
|---|---|---|
| Tracks missing from Mixxx | Added | Added |
| Tracks already in Mixxx | Left as they are | CueRithm replaces the selected data |
| Your own playlists and crates in Mixxx | Untouched | Untouched |
| A playlist with the same name already in Mixxx | Missing tracks are appended; nothing is removed | Its tracks and order are set from CueRithm |

Use **Add only** to put a playlist or folder into a Mixxx library you have already built. Use **Manage library** when CueRithm should be the source of your beatgrids, keys and cues. Then three checkboxes become available, all ticked: **Replace track details**, **Replace keys** and **Replace hot cues and loops**. Untick one to keep that part of Mixxx's data. The beatgrid follows **Do not send beatgrids**, not these boxes. A value CueRithm does not have, such as a track with no key, never clears what Mixxx holds.

Both modes work with any **What to sync** choice ([Choose what to sync](choose-what-to-sync.md)). A track you removed from Mixxx, or that Mixxx marked missing, becomes visible again when it is part of the sync. Hot cues and saved loops are replaced as a set for a track that has CueRithm cues; Mixxx's main cue, intro and outro markers are kept.

## 5. Choose how playlists arrive { #playlists-and-crates }

**Playlists become** has two choices:

- **Playlists (keep order)** keeps track order and repeated tracks. This is the default.
- **Crates (unordered)** puts each list in a Mixxx crate. Crates have no order and no duplicates. If another synced list already uses the crate name, CueRithm numbers the new one, for example `Warmup (2)`.

Mixxx has no folders and no smart playlists. A playlist inside folders becomes one list named after its path, so the demo playlists arrive as `Club nights / Warm-up set` and `Club nights / Peak hour`. A smart playlist is sent as its saved tracks, and the preview names the ones affected.

CueRithm remembers the lists it created. Renaming or reordering one in CueRithm updates the same list in Mixxx. Removing it in CueRithm removes it from Mixxx only if you have not changed it in Mixxx since; a list you edited, locked or made yourself is never deleted. If you edit a list CueRithm created, **Manage library** resets it to CueRithm's version at the next sync, and **Add only** adds the missing tracks and nothing else. Mixxx's Auto DJ and history playlists are never touched.

## 6. Choose what to send and preview { #preview }

Open **What to sync**, choose **Playlist or folder** and then **Club nights**. Leave **Beatgrid locks** on **Default** and **Do not send beatgrids** unticked. Choose **Preview sync**.

**What you should see:** a first warning line that summarises the sync, such as `Mixxx: 16 tracks added; 0 replaced; 0 left unchanged; 2 playlists or crates created, 0 updated and 0 removed.` (the two playlists; the Club nights folder itself is not a list in Mixxx), followed by notices:

- that CueRithm will create the library, when there is none
- tracks outside the folders Mixxx scans, or that the macOS sandbox may not let Mixxx read
- AAC and WMA decoder warnings ([Timing](#timing))
- the folders CueRithm adds to Mixxx's scan list ([Folders Mixxx scans](#folders-and-sandbox))
- a library newer than CueRithm was checked with

The demo tracks are FLAC, so they need no timing shift.

## 7. Apply, check and undo { #apply }

Choose **Sync to Mixxx**. **What you should see:** **Sync complete. Your backup is available below.** Syncing the same data again changes nothing and says **Everything is already synced.** Start Mixxx: your tracks, grids, keys and playlists are there.

**Restore backup** in **Sync history** returns the library to how it was, as long as you have not changed it since ([Preview, apply and restore a sync](preview-apply-restore.md#restore)). If you change the library, the folder or an option after previewing, preview again. Files you move with [Organize files](../rename/organize-files.md) are repaired in Mixxx too, so synced tracks stay linked.

## What is transferred { #what-is-transferred }

| CueRithm | In Mixxx |
|---|---|
| Title, artist, album, genre, comment, year, rating, track colour | The same fields |
| BPM and beatgrid | A constant grid from CueRithm's current track analysis. 17 (Extended Mix) arrives as <span class="cr-bpm">128</span> BPM |
| Key | The key, shown in whichever notation your Mixxx uses. 17 (Extended Mix) is <span class="cr-key">6A</span>, which is G minor |
| Hot cues and loops | Hot cue slots 1 to 36, with names and exact colours |
| Playlists and folders | Playlists or crates, as in step 5 |

Not transferred: album art (Mixxx finds it from your files), the record label (map it into **Comment** with **Field mappings**, see [Set the sync options](sync-options.md#field-mappings)), whether a loop starts active, and cues beyond slot 36, which the preview names. **Do not send beatgrids** leaves the grid to Mixxx's own analysis. **Beatgrid locks** sets Mixxx's BPM lock so Mixxx will not re-analyse a grid you trust; **Default** leaves each track's lock alone and a new track starts unlocked.

CueRithm does not change Mixxx's settings, play counts, replay gain or cover art. CueRithm's energy rating is not sent.

## How cue timing is matched to Mixxx { #timing }

MP3 files start slightly differently in each decoder, and Mixxx uses a different decoder on each system. CueRithm therefore shifts MP3 cues and grids, once and only on the copy it writes, to match the Mixxx on the computer you sync from. It reads each file's header and, on macOS, asks the system's audio decoder what it will skip. The shift is usually zero on macOS and about 25 ms on Linux and Windows for typical LAME-encoded files. FLAC, WAV, AIFF, Ogg Vorbis and Opus need none, and neither do AAC and Apple Lossless in the macOS download or in Debian's Mixxx on Linux. If you copy a Mixxx library to another operating system, sync again from there.

**AAC on Linux depends on how your Mixxx was built.** A Mixxx built from its own source decodes `.m4a` and `.mp4` with FAAD, which plays the music later by the start padding the encoder added beyond 1024 samples: nothing for files from FFmpeg's encoder, and 1088 samples (about 25 ms) for files from Apple's, both measured. CueRithm reads your installed `mixxx` to see which decoder it has, moves the grid and cues to match and says how many tracks it moved. If it cannot tell which Mixxx you have (it finds none, one of them is a launcher script, or you have two installations, say a native one and a Flatpak one, that decode differently), it moves nothing and tells you how late such tracks would play if yours uses FAAD.

**AAC on Windows.** Mixxx decodes `.m4a` and `.mp4` with Windows' own Media Foundation, which does not skip any of the start padding the encoder added: 1024 samples (about 23 ms) for files from FFmpeg's encoder and 2112 samples (about 48 ms) for files from Apple's, measured with a probe that reads the way Mixxx does. When you sync from Windows, CueRithm moves the grid and cues of those tracks later by that much and says how many it moved. Apple Lossless needs no move. Raw `.aac` files (ADTS) were not measured, so the preview warns about them.

**WMA** cannot be opened by the Mixxx downloads for macOS or Windows (checked in 2.5.6), so those tracks are added but will not play there; a Mixxx build that includes FFmpeg may play them, which is not verified. Check one such track by ear before relying on its cues.

## Folders Mixxx scans, Flatpak and the macOS sandbox { #folders-and-sandbox }

While Mixxx's list of scanned folders is empty it asks for a music folder at every start. So when that list is empty, and only then, CueRithm adds the folders that hold the synced tracks (never your home folder or a drive root) and says so in the preview. A list that already has a folder is never edited. Tracks outside the scanned folders still play and stay in the library, but Mixxx will not find new files next to them; add the folder in Mixxx's preferences if you want that.

**Flatpak.** The Flatpak build of Mixxx can read only your Music folder, `/media` and `/run/media` unless you allow more. The preview warns about tracks outside those, which Mixxx would show as missing.

**macOS sandbox.** The Mixxx download for macOS is sandboxed: it can read your Music and Downloads folders and any folder you chose in one of its own file dialogs, and it asks before reading anything else. That is why its library sits inside `~/Library/Containers/org.mixxx.mixxx`. Music elsewhere still plays, but Mixxx asks you to pick the folder the first time, and a folder CueRithm added to the scan list does not count as chosen. The preview counts the tracks this affects; keeping music in `~/Music` avoids it. macOS may also ask whether CueRithm may access another app's data, because the library is in Mixxx's container; that prompt has not been tested with the CueRithm application.

## What has been tested { #tested }

CueRithm's preview and apply code wrote libraries, and Mixxx was started on the results, using a test harness rather than the CueRithm window:

| System and Mixxx build | Result |
|---|---|
| macOS, Mixxx 2.5.6 | 240 checks passed: 168 across fresh, existing, replaced and newly created libraries, and 72 for additional formats |
| Linux, Debian 13's Mixxx 2.5.0 in Docker | 157 checks passed for MP3, WAV, FLAC, AAC from two encoders, ALAC in M4A, AIFF, Ogg Vorbis and Opus, and a library CueRithm created |
| Linux, Mixxx `main` (schema 40) built from source with FAAD, in Docker | 157 checks passed, including AAC from Apple's encoder moved 1088 samples for FAAD, and Mixxx's own upgrade of a CueRithm-created library from schema 39 to 40 |
| Windows Server 2019, Mixxx 2.5.6 | 156 checks passed for AAC from two encoders (moved 1024 and 2112 samples for Media Foundation), Apple Lossless, AIFF, Ogg Vorbis, Opus, WAV, FLAC and MP3 (through libmad), and a library CueRithm created, including Mixxx's analyzer leaving locked grids alone; Mixxx has no WMA decoder there |

Mixxx started without asking for a music folder, found the library current, read the grids and keys, left BPM-locked grids alone and kept the synced rows. MP3 timing was measured with Apple's decoder calls on macOS and with libmad on Linux and Windows, where the first beat landed within two frames of the decoded click. FAAD and Media Foundation timing was measured with probes that follow Mixxx's decoder, with two encoders each, so another encoder may differ.

Not tried: Windows 10 and 11 (the Windows run used Windows Server 2019), AppImage and Flatpak packages, Mixxx 2.4 and release builds newer than 2.5 (the schema 40 run used the development version), external drives through the macOS sandbox, audible playback and waveform alignment inside Mixxx, and CueRithm's own window, discovery and process checks on Linux and Windows. The tested macOS, Debian and Windows builds cannot play WMA. [Check what works with your DJ software](../help/compatibility.md) records the scope.

## What can go wrong { #what-can-go-wrong }

| What you see | Why | What to do |
|---|---|---|
| "Close the target DJ software before syncing" | Mixxx is running | Quit it, preview again |
| "Mixxx did not close cleanly…" | A journal file is left from a crash | Open Mixxx, quit normally, sync again |
| "Neither the Mixxx settings folder nor the folder it would be in exists; check the location, or on macOS start Mixxx once so the system creates its container" | Nothing to write into | Check the path; on macOS start Mixxx once |
| "Replacing the whole Mixxx library needs the full library; choose the full scope" | **Replace the whole library** with a playlist scope | Choose **Full library** |
| Tracks show as missing in Mixxx | They are outside the folders Flatpak or the sandbox allows | Move them under `~/Music`, or allow the folder |
| Cues are a few milliseconds off | AAC or WMA decoder differences | Read the preview warning and check by ear |

## What you learned

- Mixxx can start from nothing: CueRithm creates the library, or replaces the whole one and keeps the old one in the backup.
- **Add only** puts new tracks in and leaves the rest; **Manage library** makes CueRithm the source of grids, keys and cues, piece by piece.
- Playlists keep their order; folders flatten to `Folder / Playlist`; smart playlists arrive as their saved tracks.

## Next steps

- [Set the sync options](sync-options.md): field mappings and beatgrid locks
- [Choose what to sync](choose-what-to-sync.md): scopes and smart playlists
- [Preview, apply and restore a sync](preview-apply-restore.md): recovery after an interruption
- [Check the beatgrid at beat level](../analysis/check-the-beatgrid.md): verify a grid before you send it

<!-- verified: apps/desktop/src/views/Sync.tsx (Mixxx controls, notes, labels); sync-preferences.ts (Mixxx never restores a Wine environment); sync-types.ts (MixxxOptions defaults); src/test/sync.test.tsx (Mixxx sync); crates/cuerithm-dj-sync/src/mixxx.rs (candidates, summary line, advisories, host); mixxx/schema.rs (version rules and messages, ensure_closed_cleanly); mixxx/folders.rs; mixxx/tracks.rs (replace, restore, bpm_lock); mixxx/cues.rs; mixxx/owned.rs (ownership, locked, adoption); discovery.rs (process, candidates); dj_sync_service.rs (full scope for Replace); path_repair.rs (Mixxx repair); docs/research/mixxx-sync-contract.md; gh PR 39 body; legacy guides/sync-mixxx.md and about/roadmap.md re-checked -->
<!-- UNVERIFIED: the sample counts in step 6 (16 tracks, 2 lists) are placeholders for what the app prints -->
