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

- Write directly to Mixxx's `mixxxdb.sqlite`: tracks, whole-track BPM and beatgrid
  (constant or variable tempo), key, hot cues and saved loops, ratings, colours, and
  ordered playlists or unordered crates.
- Two operating modes on one target, see below: **Manage library** and **Add only**.
- Work the same on macOS, Windows and Linux (including the official Flatpak),
  discovering the library per platform and allowing manual selection.
- Reuse the existing preview, stale-preview rejection, process guard, backup,
  recovery, restore, ownership ledger and desktop Sync workspace.
- Make timing correct for Mixxx's own decoders, with the residual risk stated.

## Non-goals and anti-goals

- No writing into music files or Serato-style tags; the source music is never
  modified.
- No edits to Mixxx's settings, its `directories` scan list, or any table the
  adapter does not own the rows of. No markers or extra columns in the Mixxx
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
| Tracks already in Mixxx | selected fields **replaced** by CueRithm: grid, key, hot cues and loops, metadata | **untouched**; only playlist membership refers to them |
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

A track in Mixxx marked removed (`mixxx_deleted`) that falls in scope is restored,
the same as Mixxx's own re-add. A missing-file flag (`fs_deleted`) is cleared for
tracks in scope because the file's presence was verified.

## Container: playlists or crates

A second option chooses what CueRithm playlists become: ordered **Playlists**
(default; preserves order and duplicates) or **Crates** (unordered, de-duplicated,
the idiomatic Mixxx way to group). Folders flatten to `Parent / Child`. Smart
playlists export their captured members as ordinary lists and the preview says so.
Names are made unique per kind with a numeric suffix. Hidden playlists (Auto DJ,
history) and locked containers are never modified. Switching kind keeps objects
exported earlier, with a warning, rather than deleting them.

## Architecture

```
crates/cuerithm-dj-sync/src/
  mixxx.rs            public API: discovery helpers, resolve_database_path,
                      write_staged_owned(stage, library, options, ownership)
  mixxx/schema.rs     version window, required-column check, track insert/update
  mixxx/proto.rs      hand-encoded BeatGrid/BeatMap/KeyMap messages (+ test decoder)
  mixxx/cues.rs       cue rows, colours, slot validation
  mixxx/owned.rs      playlist/crate planning and ownership report
```

- `DjSoftware::Mixxx` joins the shared model. `prepare_sync` stages a SQLite
  snapshot of `mixxxdb.sqlite`, mutates the staged copy, and queues the replacement,
  exactly as the djay Pro adapter does. Apply, recovery, restore and backup are the
  existing transaction machinery.
- Discovery adds the per-platform locations from the research document, the Flatpak
  folder included. The process guard knows the executable names.
- `SyncOptions` gains a nested `mixxx` options block, omitted from serialization when
  default so existing baselines and journals keep their hashes.
- Timing gets a Mixxx rule in `timing.rs`: offsets for the four MP3 header cases from
  the research document, zero for lossless/PCM/Ogg/Opus, and zero with a warning for
  AAC/M4A/MP4.
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
   file read-only; see the research document for why.
4. Beats: constant tempo writes `BeatGrid-2.0`; several tempo anchors write a dense
   `BeatMap-1.0` that keeps every anchor exactly. `bpm` is the track average.
   `bpm_lock` follows the lock option: *Default* keeps an existing row's lock and is
   unlocked on a new row, *Lock* and *Unlock* set it.
5. Key: `KeyMap-1.0` blob with source *user*, ID3v2 `key` text and `key_id`.
6. Cues: replace only hot cue and loop rows (types 1 and 4) for a track that has
   CueRithm cues; main cue, intro/outro and other rows stay. Positions are
   `frames × 2`; loops need at least one frame; colours snap to the Mixxx palette
   only with *nearest colours*, otherwise exact RGB, and a missing colour takes the
   palette colour for the slot.
7. The whole sync is one SQLite transaction in the staged copy. Before it is
   returned, the staged database is re-read and checked: foreign-key targets exist,
   hot cue slots are unique, every loop has length, blobs decode, row counts match
   the plan, and rows outside the plan are unchanged.

### Ownership

The local ownership ledger, never the Mixxx library, records which playlists or
crates CueRithm created, keyed by the source playlist id and a kind-qualified
native id with a structural fingerprint of its name and ordered members. Rename and
reorder update the bound object in place. Only objects the sync created and that
still match their fingerprint are deleted when the source playlist is removed;
edited or foreign objects are retired and kept. Matching by name is allowed only to
adopt an existing object for update (`created = false`), never for deletion.

## Platform and safety rules

- Preflight reports: library not found, unsupported or newer schema (named, with
  the supported window), Mixxx running, open journal from a crashed session,
  missing source audio.
- Mixxx must be closed. The process list is read fresh at preview and again at
  apply; a failure to read it blocks the sync. Mixxx is never killed.
- A write is preceded by the existing durable backup and journal; Restore returns
  the previous file. A repeated sync that changes nothing writes nothing.
- Paths with non-Unicode bytes are rejected because Mixxx stores text.
- Formats Mixxx may not decode on the host are reported (for example WMA depends
  on the Mixxx build); the rows are still written.

## Validation and acceptance

Authored fixtures create Mixxx-shaped databases for schema 39 and 40 with
independently written DDL (no upstream schema text is copied) and cover: the
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
grid, key and cues, and does not rewrite them from tags. Windows and Linux Mixxx
runs are outside what this host can do and stay listed as unverified.

## Decisions made without asking (flag if wrong)

- *Add only* is the default mode, *Playlists* the default container.
- Schema window 39–40 only; older databases must be upgraded by Mixxx first.
- `source_synchronized_ms` stays NULL on rows the adapter creates.
- Hot cue and loop rows are replaced as a set per track; other cue types survive.
- AAC alignment is declared unverified instead of guessed.
- No change to `cuerithm-media` (its timing identity would invalidate saved
  measurements); the LAME delay is read from the file header at export time.
- Cover art, record label, active-loop state and cue slots above 35 are not
  transferred.
