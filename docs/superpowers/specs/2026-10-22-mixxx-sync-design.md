---
title: Mixxx native library sync
date: 2026-10-22
status: approved by directive; implementation in progress
---

# Mixxx native library sync

## Intent

Mixxx is the open-source DJ application that runs natively on macOS, Windows and
Linux. Add it as a CueRithm sync destination so a DJ can send prepared tracks,
beatgrids, harmonic keys, hot cues, saved loops and playlists straight into their
Mixxx library on any of the three systems, and keep that library managed from
CueRithm afterwards. The result should be good enough to show to the Mixxx
community: honest about what Mixxx can and cannot represent, safe on a library the
user has already built, and reversible.

The contract Mixxx stores is recorded in
[`docs/research/mixxx-sync-contract.md`](../../research/mixxx-sync-contract.md).
This document records decisions.

## Goals

- Write directly to Mixxx's `mixxxdb.sqlite`: tracks, whole-track BPM and beatgrid,
  key, hot cues and saved loops, ratings, colours, and ordered playlists or unordered
  crates. The adapter writes constant grids and variable-tempo maps; CueRithm's
  projection currently supplies one constant grid per track, so variable tempo is an
  adapter capability covered by unit tests, not yet reachable end to end.
- Two operating modes on one target, see below: **Manage library** and **Add only**.
- Work the same on macOS, Windows and Linux (including the official Flatpak),
  discovering the library per platform and allowing manual selection.
- Reuse the existing preview, stale-preview rejection, process guard, backup,
  recovery, restore, ownership ledger and desktop Sync workspace.
- Make timing correct for Mixxx's own decoders, with the residual risk stated.

## Non-goals and anti-goals

- No writing into music files or Serato-style tags; the source music is never
  modified.
- No edits to Mixxx's settings, to a `directories` scan list that has any row, or to
  any table the adapter does not own the rows of. The one exception is filling an
  empty scan list (below). No markers or extra columns in the Mixxx
  database. CueRithm never deletes Mixxx tracks.
- No Windows-compatibility (Wine/Proton) environment: Mixxx is native everywhere,
  so only the native environment is accepted.
- No claim of alignment or ingestion that was not exercised. Source reading,
  authored fixtures and a run of the installed application are reported separately.
- Not a re-import path: reading Mixxx's library back into CueRithm is out of scope.
- Waveforms, play counts, replay gain, cover art and analysis data stay Mixxx's.

## Two modes

The mode is an explicit Mixxx option, independent of the existing scope selector
(full library, playlist or folder, modified, changed), so any scope can be used
with either mode.

| | **Manage library** (`replace`) | **Add only** (`keep`) |
| --- | --- | --- |
| Tracks missing from Mixxx | added with CueRithm data | added with CueRithm data |
| Tracks already in Mixxx | selected fields **replaced** by CueRithm: grid, key, hot cues and loops, metadata | **untouched**, except that tracks in scope are made visible again (see below); playlist membership refers to them |
| Playlists/crates CueRithm created | kept equal to CueRithm | kept equal to CueRithm |
| A same-name playlist already in Mixxx | membership replaced and order set | missing tracks appended; nothing removed |
| Tracks CueRithm does not know | never touched | never touched |

*Add only* is the default, because the first sync into a library the user has
curated must not overwrite it. *Manage library* is the explicit opt-in that
delivers "apply our beatgrid, key and cues on top of the library". Within *Manage
library* the user picks which of **metadata**, **key** and **hot cues and loops** to
replace; beatgrids use the existing *skip beatgrids* option. A field with no
CueRithm value is never used to clear a Mixxx value. The preview counts replaced
versus untouched tracks and replaced cues, and lists skipped items with reasons.

In both modes a track in scope that Mixxx has marked removed (`mixxx_deleted`) is
restored, as Mixxx's own re-add does, and a missing-file flag (`fs_deleted`) is
cleared because the file's presence was verified. Choosing a track for a playlist or
scope is the request to see it. These two flags are the only fields *Add only* ever
changes on an existing track, and the preview counts them ("N restored").

## Container: playlists or crates

A second option chooses what CueRithm playlists become: ordered **Playlists**
(default; preserves order and duplicates) or **Crates** (unordered, de-duplicated,
the idiomatic Mixxx way to group). Folders flatten to `Parent / Child`. Smart
playlists export their captured members as ordinary lists and the preview says so.
Names are made unique per kind with a numeric suffix. Hidden playlists (Auto DJ,
history) and locked containers are never modified.

Edits made in Mixxx are expected, so the policy is more forgiving than the djay
adapter's, which aborts on any fingerprint change. A container CueRithm created that
the user has since edited is *reset* to CueRithm's version in *Manage library* (the
preview says so) and receives *additions only* in *Add only*. One the user deleted is
created again. One removed from CueRithm is deleted only when it is still unchanged
and unlocked; otherwise it is kept and unbound. A same-name container CueRithm did
not create is adopted for update only when exactly one candidate exists (replaced in
*Manage library*, appended to in *Add only*) and is never deleted later. Two
same-name playlists are not adopted; a separate one is created and the preview warns.
Switching the container kind keeps what was exported before, unbound, and creates the
new kind.

## Architecture

```
crates/cuerithm-dj-sync/src/
  mixxx.rs            public API: discovery helpers, resolve_database_path,
                      write_staged_owned(stage, library, options, ownership)
  mixxx/schema.rs     version window, required-column check, track insert/update
  mixxx/proto.rs      hand-encoded BeatGrid/BeatMap/KeyMap messages (+ test decoder)
  mixxx/cues.rs       cue rows, colours, slot validation
  mixxx/owned.rs      playlist/crate planning and ownership report
  mixxx/lame.rs       MP3 start facts (Info frame, encoder identifier, delay)
  mixxx/repair.rs     reference id and path repair for Organize
```

- `DjSoftware::Mixxx` joins the shared model. `prepare_sync` stages a SQLite
  snapshot of `mixxxdb.sqlite`, mutates the staged copy, and queues the replacement,
  exactly as the djay Pro adapter does. Apply, recovery, restore and backup are the
  existing transaction machinery.
- Discovery adds the per-platform locations from the research document, the Flatpak
  folder and, on macOS, the App Sandbox container of the official build (listed before
  the plain `Application Support` folder). The process guard knows the executable names.
- `SyncOptions` gains a nested `mixxx` options block, omitted from serialization when
  default so existing baselines and journals keep their hashes.
- Timing gets a Mixxx rule in `timing.rs` that follows the decoder this system's Mixxx
  uses (research document, *Timing model*): Apple CoreAudio on macOS, MAD on Linux and
  Windows. For MP3 it reads the file's header facts (Info/Xing frame; `LAME`, `Lavf` or
  `Lavc` identifier and its encoder delay) and, on macOS, asks CoreAudio's `AudioFile`
  for the priming it will discard, because Apple's tag rules differ from FFmpeg's. It
  does not use the media crate's `Mp3TimingCase` letters or a measured profile, so it
  works without the media runtime. Lossless, PCM and Ogg are zero. AAC in M4A or MP4 is
  zero where the decoder drops the encoder's priming (CoreAudio on macOS, measured;
  FFmpeg, measured in Debian's Mixxx). Where Mixxx decodes it with FAAD (Linux builds that
  have it) the music plays `priming - 1024` frames late, measured with two encoders, so
  the adapter adds that, reading the priming from the file's edit list or Apple's
  `iTunSMPB` tag and the decoder from the installed `mixxx` executable; an unfound
  decoder is zero with a warning that says how late FAAD would play it. On Windows,
  Mixxx decodes M4A with Media Foundation, which keeps all of the priming: the music
  plays `priming` frames late (1024 and 2112 measured with a probe that reads the way
  Mixxx's source does), so the adapter adds that on a Windows host and says how many
  tracks it moved; a raw `.aac` stream was not measured there and is warned about. WMA
  has no decoder in the Mixxx 2.5.6 downloads for macOS and Windows (their logs list
  none), so those tracks are added and warned about as unplayable; elsewhere they are
  zero with a warning that alignment is unverified. A cue pushed before
  the first Mixxx sample is moved to the start; a loop that would end before it is
  skipped with a warning.
- Moving a synced file in CueRithm's Organize flow repairs the Mixxx reference
  (`mixxx/repair.rs`: the `track_locations` row), so a synced library stays
  organizable. Without it every synced track would block reorganization.
- The desktop Sync workspace gets a Mixxx entry and its options. The service layer
  resolves the database path from a folder or file and registers identity the same
  way as other desktop databases.

### Writing a track

1. Resolve the file's canonical path (forward slashes, no verbatim prefix) and find
   the row by exact match, or case-insensitive on macOS and Windows with an error on
   ambiguity.
2. Existing row, *Add only*: leave it. Existing row, *Manage library*: update only
   the selected columns. No row: insert `track_locations` and `library`.
3. New rows get `header_parsed = 1`, `source_synchronized_ms` NULL and the audio
   facts (`duration`, `samplerate`, `channels`, `bitrate`, `filetype`) read from the
   file read-only. A probe failure (for example WMA, which the tag reader does not
   support) falls back to 2 channels and bitrate 0 with a warning; Mixxx refreshes
   exact stream facts on first decode.
4. Beats: constant tempo writes `BeatGrid-2.0`; several tempo anchors write a dense
   `BeatMap-1.0` that keeps every anchor exactly. `bpm` is the track average.
   `bpm_lock` follows the lock option: *Default* keeps an existing row's lock and is
   unlocked on a new row, *Lock* and *Unlock* set it when a grid is written. A new
   row without a grid stays unlocked so Mixxx can analyze it.
5. Key: `KeyMap-1.0` blob with source *user*, ID3v2 `key` text and `key_id`.
6. Cues: replace only hot cue and loop rows (types 1 and 4) for a track that has
   CueRithm cues; main cue, intro/outro and other rows stay. Positions are
   `frames × 2`; loops need at least one frame; colours are written as exact RGB
   because Mixxx displays any colour, so *nearest colours* is ignored for Mixxx; a
   missing or black colour takes the palette colour for the slot.
7. The whole sync is one SQLite transaction in the staged copy. Before it is
   returned, the staged database is re-read and checked: foreign-key targets exist,
   hot cue slots are unique, every loop has length, blobs decode, row counts match
   the plan, and rows outside the plan are unchanged.

### Ownership

The local ownership ledger, never the Mixxx library, records which playlists or
crates CueRithm created, keyed by the source playlist id and a kind-qualified
native id (`playlist:<id>` or `crate:<id>`; Mixxx ids are autoincrement integers, so
unlike djay the locator cannot be re-derived from the source id and is validated by
shape and uniqueness instead) with a structural fingerprint of its name and ordered
members. Rename and reorder update the bound object in place. Only objects the sync
created and that still match their fingerprint are deleted when the source playlist
is removed; edited or foreign objects are retired and kept. Matching by name is
allowed only to adopt an existing object for update (`created = false`), never for
deletion.

## Platform and safety rules

- Preflight reports: library not found, unsupported or newer schema (named, with
  the supported window), Mixxx running, open journal from a crashed session. A
  missing source audio file fails the whole sync, as it does for every target.
  Problems with individual cues, loops or slots are warnings that name the track.
- Flatpak Mixxx can only read `~/Music`, `/media` and `/run/media` unless the user
  grants more; for a library under `.var/app/org.mixxx.Mixxx` the preview warns about
  tracks outside those folders. The sandboxed macOS build reads only Music, Downloads
  and folders the user chose; for a library in its container the preview warns about
  tracks outside those and outside Mixxx's own `directories`.
- Mixxx must be closed. The process list is read fresh at preview and again at
  apply; a failure to read it blocks the sync. Mixxx is never killed.
- A write is preceded by the existing durable backup and journal; Restore returns
  the previous file. A repeated sync that changes nothing publishes nothing: SQLite's
  backup API rewrites header bytes even for an unchanged database, so the adapter
  reports whether any row changed and the replacement is queued only when it did.
- The shared implementation hash covers every source file, so adding this adapter
  re-baselines all targets once, like any release; the option block itself does not
  change existing hashes.
- Paths with non-Unicode bytes are rejected because Mixxx stores text.
- Formats Mixxx may not decode on the host are reported (for example WMA depends
  on the Mixxx build); the rows are still written.

## Validation and acceptance

Fixtures create Mixxx-shaped databases for schema 39 and 40 (and newer, to test the
version rule) from the table definitions Mixxx 2.5.6 itself wrote, and cover: the
schema window and refusals; insertion and update per mode; location matching
including case folding, Windows drive and UNC spelling and ambiguity; every
protobuf message against golden bytes checked with `protoc --decode` against
Mixxx's own `.proto` files; cue slots, loops, colours and the three loader-discard
traps; timing offsets for MP3 cases and zero cases with a measured WAV/FLAC
control; playlist and crate ordering, flattening, uniqueness, locked and hidden
objects, adoption in both modes; ownership rename, reorder, deletion, retire and
restore; idempotent repeat; stale preview; process guard; and journal recovery.
Desktop tests cover target selection, options, and mode defaults.

Separate evidence, reported on its own: a run of the installed Mixxx 2.5.6 against a
database the adapter wrote, checking from its log and database that it loads the
grid, key and cues, and does not rewrite them from tags. Mixxx was also run on Linux
(Debian's 2.5.0, and `main` with FAAD, in containers) and on Windows (2.5.6 on Windows
Server 2019, and with an MSVC build on hosted Windows Server 2022 and 2025); those results, and what each did not measure, are in the research contract.

## Decisions made without asking (flag if wrong)

- *Add only* is the default mode, *Playlists* the default container.
- Schema 39 is the floor; older databases must be upgraded by Mixxx first. Newer
  ones are accepted by the library's own marker, not a release list: a schema above 40
  (the newest checked) whose `mixxx.schema.min_compatible_version` is 40 or lower, with
  every named column present and no new mandatory column, is written and reported in
  the preview. This is Mixxx's own rule for opening a newer library.
- `source_synchronized_ms` stays NULL on rows the adapter creates.
- Hot cue and loop rows are replaced as a set per track; other cue types survive.
- AAC alignment is measured on macOS, in Debian's Mixxx (FFmpeg), in a Mixxx built
  with FAAD and with Media Foundation on Windows; WMA is unplayable in the macOS and
  Windows downloads and unverified elsewhere. Nothing is guessed, and the FAAD and
  Media Foundation rules are stated as two-encoder measurements.
- No change to `cuerithm-media` (its timing identity would invalidate saved
  measurements); MP3 start offsets come from the file header, not a case letter.
- Cover art, record label, active-loop state and cue slots above 35 are not
  transferred; colours are exact RGB.
- Edited CueRithm-owned containers are reset or appended to, never a sync error.
- A folder with no Mixxx library (a Mixxx never started) gets a new schema-39 library
  built from the layout Mixxx 2.5.6 itself creates, and a settings folder that is also
  missing is created when its parent exists. *Replace the whole library* builds the
  same new library in place of an existing one, from the full library only; the old
  file is kept in the journal and restore returns it. Both reset ownership, since the
  playlists CueRithm owned are gone. Mixxx opens its database with foreign keys off
  and the adapter must too. Only the full scope may replace a library, because a
  partial scope would leave most tracks out.
- Mixxx asks for a music folder at every start while `directories` is empty, so the
  adapter fills an empty list with the outermost folder of each synced track (never
  the home folder or a drive root) and says so in the preview. A list with any row is
  never edited, and it is checked before commit. On the sandboxed macOS build a folder
  the adapter adds has no permission bookmark, so the preview still warns about tracks
  outside Music, Downloads and folders with a bookmark in `sandbox.cfg`.
