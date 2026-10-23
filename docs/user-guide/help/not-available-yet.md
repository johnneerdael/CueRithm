# What CueRithm does not do yet

<p class="lead">CueRithm is early software. This page lists what is missing, what is limited and what is in development, so you can tell a gap from a fault. Everything under <em>In development</em> is planned and not available in any release. Reading the whole page takes about five minutes.</p>

Check the release notes of your download for the features it includes. For what has been tested in each DJ program, see [Check what works with your DJ software](compatibility.md).

## Not in the app { #not-in-the-app }

### Unrecognised rekordbox formats { #rekordbox-formats }

Compatible rekordbox 6 and 7 builds can be written to on macOS, Windows and Wine / Proton. A database or asset format that CueRithm does not recognise stays **Read only**. Collection XML transport is intentionally excluded. Pioneer USB export is a separate drive workflow. See [Sync to rekordbox 6 or 7](../sync/rekordbox-direct.md) and [My sync does not show up in rekordbox](troubleshooting/rekordbox-sync-problems.md).

### Traktor and legacy Serato { #traktor-serato }

Sync to Traktor, and to Serato versions before Serato DJ Pro 4, is not available. Serato DJ Pro 4 sync is included ([Sync to Serato DJ Pro](../sync/serato-dj-pro.md)).

### Standalone playlist import and export { #playlist-import-export }

Native playlist export is part of [Send your library to DJ software](../sync/index.md). The app has no menu command to export an M3U or collection XML file, or to import a playlist file. A developer-oriented command line mode can write an M3U8 for ready tracks ([library automation](https://github.com/johnneerdael/CueRithm/blob/main/docs/automation.md)).

### Variable-tempo grids and downbeat detection { #variable-tempo }

Each track has one BPM and one first beat. A live drummer or a tempo change drifts away from the beat lines. You can correct the saved constant grid ([Fix a wrong beatgrid](../analysis/corrections/fix-the-beatgrid.md)), but there is no variable grid. The detector searches 98 to 195 BPM and folds other tempos into that range, with no setting to change it ([My key or BPM differs from another app](troubleshooting/key-or-bpm-differs.md)).

### Per-segment energy and energy-labelled cues { #per-segment-energy }

Deliberately not offered. CueRithm rates the whole track once, from 1 to 10 ([How is energy rated?](../how-it-works/energy.md)).

### Plays, last played and danceability { #plays-danceability }

Deliberately excluded.

### Other OneTagger workflows { #other-onetagger }

Spotify and quick tag are not included, and automatic preparation rejects Shazam, post-commands and lyric (LRC) files. Renaming files is available through [Rename and sort files into folders](../rename/organize-files.md). See [Credits and open source](about/credits.md#onetagger) for what CueRithm takes from OneTagger.

### A manual key correction { #manual-key }

You can correct cues, loops and the beatgrid by hand, but not the measured key. **Edit metadata** changes the key tag in the file only ([Edit tags and artwork](../tagging/edit-tags-by-hand.md)).

### Loop playback in the player { #loop-playback }

Saved loops are kept with their start and end and are sent to DJ software that supports them. The player does not repeat a loop: local preview seeks instead of looping ([Set a loop](../analysis/corrections/set-loops.md)).

### Backup retention controls { #backup-retention }

CueRithm keeps the original file bytes of every tag write and cleanup, and nothing deletes them. Cleanup has **Undo**, and Sync history offers a guarded **Restore backup** and recovery for interrupted work. Disposal of backups is explicit only for reviewed unused-file removals. General retention settings are unfinished. The size is shown as **Recovery backups** in **Settings** → **Automatic tagging**.

### An audio device picker { #no-device-picker }

Playback uses the system's default output. Choose the device in your system's sound settings ([A track shows a dash or will not play](troubleshooting/no-waveform-or-playback.md)).

## Known limitations { #known-limitations }

### Long tracks do not prepare { #long-tracks }

Decoding stops at about 24 minutes, so long DJ mixes fail with `Media scan exceeded its output limit` and stay in Incoming. There is no setting to raise the limit ([A track is stuck in Incoming](troubleshooting/incoming-stuck.md), [How are audio files decoded?](../how-it-works/decoding.md)).

### Catalog matches are not guaranteed { #catalog-coverage }

Traxsource may need a separately hosted [FlareSolverr-compatible solver](../tagging/cloudflare-solver.md). Live fetching and parsing were verified with Trawl, but universal catalog coverage and packaged-app acceptance are unverified. Beatport has one successful direct API probe. A missing match is not always a fault ([Catalog tagging finds nothing](troubleshooting/no-catalog-match.md)).

### Windows, Linux and Intel Macs { #platforms }

There is no Intel Mac build. Windows and Linux builds are unsigned, and their native playback and restart acceptance is not complete ([My computer blocks CueRithm at first launch](troubleshooting/first-launch-blocked.md)).

### Older versions cannot open a newer library { #library-schema }

The library schema is version 28. An older CueRithm refuses a library that a newer one has opened, with `Library schema is newer than this application`, and there is no downgrade. [Back up your library](../files/library/back-up-library.md) before you update.

### Measured agreement, not accuracy { #accuracy-not-measured }

Published figures measure agreement with reference tools on the owner's own library. Held-out accuracy, real-music energy agreement and large-library performance are not measured ([How accurate is it?](../how-it-works/accuracy.md)).

### How much of each DJ program was tested { #dj-program-checks }

Mixxx has not been run on Windows. Linux acceptance covers Debian's 2.5.0 package. Other packages and FAAD-based AAC decoding are unverified, and the tested macOS and Debian Mixxx builds cannot play WMA. The other destinations were checked against file formats only. See [Check what works with your DJ software](compatibility.md#other-destinations).

### No auto-update { #no-auto-update }

CueRithm does not update itself. Download new releases from the [releases page](https://github.com/johnneerdael/CueRithm/releases/latest) ([What's new in CueRithm](../get-started/whats-new.md)).

### A raw word in the Incoming stage line { #cleanup-stage-label }

While cleanup runs, an Incoming row can show the lower-case word `cleanup` instead of a stage name. It is a display quirk. The stage is the cleanup step of preparation ([Follow a track through Incoming](../files/follow-incoming.md)).

## Missing and moved files { #missing-and-moved-files }

In CueRithm 0.1.14 a track is known by the location of its file. This has four consequences:

- A file you move or rename outside CueRithm leaves its track pointing at nothing.
- Moves inside a watch folder are not tracked. The watch folder adds the moved file again as a new track.
- **Maintenance** → **Missing & broken** only lists unavailable files. The screen says *Relocating files is not available in this maintenance screen.* There is no action to relocate a track or to remove one from the library.
- The **Duplicates** scan can list a missing track beside its present twin. Never choose a missing track as the keeper.

What you can do today: move the files back, or archive the tracks you no longer want. Archiving never deletes a file, and an archived track leaves Tracks and later Missing & broken scans. [I moved or renamed my files](troubleshooting/files-moved.md) shows the symptoms, and [Find missing, moved or broken files](../files/library/find-missing-files.md) has the repair steps. [Move or rename files without breaking the library](../files/move-files.md) explains how to avoid the problem.

## In development { #in-development }

Nothing in this section is available in a release. Work on it is under way, and no delivery dates are promised. Broader DJ-application, physical-device and platform acceptance also continues.

### Missing and moved files { #in-development-missing-files }

Changes for the four problems described under [Missing and moved files](#missing-and-moved-files) are in development:

- **Missing & broken** has no action to relocate a track or to remove a stale one. Tools for both are in development.
- Moves inside a watch folder are not tracked. Following them, so that a move no longer leaves a missing track and a new twin, is in development.
- Reconnecting moved music is in development (see below).
- The **Duplicates** scan can pair a missing track with its present twin. Keeping missing files out of it is in development.

Until these ship, the guide describes the released behaviour.

### Bring an existing DJ library into CueRithm { #dj-library-import }

Full DJ-library import is being developed, so that an existing managed database can become the starting point for managing music in CueRithm. The intended workflow covers the import options that Lexicon offers:

- Import a whole library, or selected playlists and their tracks.
- Merge libraries from different DJ applications, updating existing entries and adding new ones.
- Replace a library when starting over is the intention.
- Choose which fields to import, including cues, with BPM and beatgrids selectable separately.
- Back up the current library before importing.

Coverage is planned across the DJ software destinations and Mixxx. DJ.Studio is a possible additional integration that is still under consideration. This work extends today's one-way sync from CueRithm to DJ software.

### Reconnect moved music { #reconnect-moved-music }

Planned relocation tools will reconnect a single track, search a chosen folder and its subfolders for missing tracks, or replace a music-folder root or drive letter across the library. You will review matches and replacements, and tracks will keep their identities, cues and playlist membership. This complements the organizer, which already renames and moves files from within CueRithm.

### Portable backup, restore and Lexicon migration { #portable-backup }

A **`.cuerithm` portable backup** will hold library data without music files or disposable caches. You will restore it on macOS, Windows or Linux and map the old music folders to their places on the new computer. Unresolved music references will stay visible in the library and playlists.

A separate **Lexicon backup ZIP import** is specified to bring over tracks, cues, tempo markers, the playlist tree and supported smart rules. The first migration creates a new library and keeps the current one for recovery. Merging independent libraries is outside that delivery, although the broader DJ-library import above includes merging as its own capability.

A portable restore will replace the active library, with recovery available. Music has to be copied separately. Local library backup and restore, as described in [Restore your library from a backup](../files/library/restore-library.md), stays available while portable backups are developed.

## Related

- [Check what works with your DJ software](compatibility.md)
- [Frequently asked questions](faq.md)
- [Report a problem or ask for a feature](report-a-problem.md)
- [Fix a problem](troubleshooting/index.md)

<!-- verified: legacy about/roadmap.md (updated version: Not in the app, Known limitations, In development); coordinator messages on missing and moved files; README.md (Compatibility and validation, In development); docs/release-readiness.md (loop playback "seeks rather than looping", backup retention, platform acceptance); crates/cuerithm-library/src/store.rs (schema <= 28 check, migrations to 028); crates/cuerithm-media/src/decode.rs (64_000_000*8 byte cap, about 24 min); crates/cuerithm-autotag/src/config.rs (excluded tools: post_command, spotify, shazam, move_success/failed, LRC); apps/desktop/src-tauri/src/headless.rs and docs/automation.md (M3U8 command); apps/desktop (no updater in Cargo.toml/tauri.conf.json; playback.rs from_default_device; editor-fields.ts File key tag only; PlayerPanel.tsx no loop playback; TrackTable.tsx stages map lacks cleanup, so the raw word shows); MaintenanceBroken.tsx note text; maintenance/broken.rs (WHERE archived=0); store.rs set_archived; .github/workflows (arm64 macOS only, unsigned Windows/Linux). Plans in the In development section are quoted from the roadmap and are not implemented -->
