# Native DJ library sync

Independent Rust format adapters for VirtualDJ 8.5, current Engine DJ (schema 3),
djay Pro (current TSBF/Yap library), Serato DJ Pro 4, Pioneer USB and Mixxx (schema 39 and newer). Older software
formats are outside this port. Rekordbox desktop retains its separate direct-only
adapter; there is no Rekordbox XML transport.

Portable input is `SyncLibrary`: prepared metadata, BPM/key, sparse downbeat tempo
segments, numbered cues/loops and ordered playlists/folders. `prepare_sync` or
`prepare_sync_with_runtime` constructs native files on private copies and returns
a file preview. `apply_sync` publishes only those staged bytes after process,
source and target fingerprint checks. `restore_sync` checks for external changes
before restoring journaled original bytes. Retain the entire journal directory.
Interrupted publication leaves recoverable backups; it is never an acknowledged
successful sync. The library service coordinates local revisions and modified-scope
baselines around this file journal.

`FileTransaction::discard` releases an unused Prepared preview under the same
transaction lock as apply and restore. Published or interrupted journals cannot
be discarded. Cleanup failures retain a retryable journal. The desktop cancels
superseded previews and exposes Prepared cleanup in sync history.

`prepare_sync_owned_with_runtime` accepts durable playlist bindings and protected
source paths from the library service. It preserves native identity across rename
and move, removes only proven owned playlists for explicit source tombstones, and
retains matched external lists. Typed directory and case-rename operations restore
the original tree; batch staging avoids repeatedly rewriting growing journals.
The service recovers both publication and restoration acknowledgement windows.
Private path serialization preserves raw platform names and path-keyed evidence;
valid Unicode paths keep their ordinary JSON form. Destination text encoders retain
their own representability checks, while portable media copies use safe names.

Timing profiles bind codec measurements to source bytes and the bundled runtime.
Native clock adjustments affect export clones, leaving saved analysis and playback
unchanged. Native smart rules are emitted only when the complete definition fits;
unsupported semantics retain the captured ordinary membership with a preview reason.

Targets:

| Target | Native data |
|---|---|
| VirtualDJ | `database.xml`, current `MyLists/*.vdjfolder` hierarchy |
| Engine DJ | `Engine Library/Database2/m.db`, compressed PerformanceData, artwork blobs |
| djay Pro | `MediaLibrary.db` binary records, playlists, native index invalidation |
| Serato 4 | Root/location SQLite databases, aggregate master, native media tags |
| Pioneer USB | Copied media, PDB, encrypted Device Library Plus, DAT/EXT/2EX and artwork |
| Mixxx | `mixxxdb.sqlite`: tracks, BeatGrid/BeatMap and KeyMap blobs, hot cue and loop rows, playlists or crates (see below) |

`SyncOptions` contains field mapping, key notation, palette conversion, grid
preservation/locks and target-specific artwork/device/cue/compatibility settings.
Serato previews also stage every affected audio file; other media tags survive
and source PCM remains unchanged. Unsupported target containers produce explicit
errors. Device conversion uses the application's bundled media runtime and writes
only staged copies. No Python/Node runtime is required by the Rust adapters.

Format tests use synthetic data. Reference parser/serializer comparisons establish
format agreement, not actual acceptance by a running DJ application or CDJ hardware.
See `docs/research/dj-software-formats.md` for the evidence boundary and open work.

## Mixxx

`mixxx` edits a staged copy of `mixxxdb.sqlite` and the shared journal publishes it. The
settings folder is `~/Library/Containers/org.mixxx.mixxx/Data/Library/Application
Support/Mixxx` for the macOS download (it is App Sandboxed), `~/Library/Application
Support/Mixxx` for a build that is not, `%LOCALAPPDATA%\Mixxx` on Windows, and
`~/.mixxx` or the Flatpak `~/.var/app/org.mixxx.Mixxx/.mixxx` on Linux. The storage
contract is in `docs/research/mixxx-sync-contract.md`; the decisions are in
`docs/superpowers/specs/2026-10-22-mixxx-sync-design.md`.

- **Modes.** `MixxxOptions.library` is `Update` (work inside the library Mixxx has) or
  `Overwrite` (build a new library from the full sync and replace the existing one; the
  old file stays in the journal for restore). A folder with no library gets a new one
  either way, made from the layout Mixxx 2.5.6 itself creates (`mixxx/schema39.sql`,
  schema 39, which newer Mixxx versions upgrade). `existing_tracks` is `Keep` (add only)
  or `Replace` (CueRithm replaces the grid, key, hot cues and details chosen by
  `replace_*`). `container` writes playlists or crates.
- **Scan folders.** Mixxx asks for a music folder at every start while its `directories`
  list is empty, so an empty list is seeded with the outermost folders of the synced
  tracks. A list with any row is never edited.
- **Schema.** Schema 39 or newer. A library newer than 40 (the newest checked) is
  written when its `mixxx.schema.min_compatible_version`, which is Mixxx's own rule for
  opening a newer library, is 40 or lower, and the preview says so; the writer names only
  columns that exist and refuses a new mandatory column it cannot fill. Mixxx runs with foreign keys off and its schema
  could not be written with them on, so the writer turns them off too.
- **Safety.** Rows are matched by canonical path (case-insensitive only where the file
  system folds case; ambiguity is an error). New rows are written parsed but never
  synchronized, so Mixxx does not re-read tags over them. The staged copy is read
  back before it is published: unrelated rows are digest-identical and no cue Mixxx
  would discard is written. A repeat sync publishes nothing.
- **Timing.** The offset depends on the decoder the Mixxx on this computer uses:
  CoreAudio on macOS (asked through `AudioFile`), libmad on Linux and Windows (from the
  first frame's Info/LAME header, `mixxx/lame.rs`). AAC is measured on macOS only; WMA is
  unverified everywhere.
- **Tests.** `src/mixxx/fixture.rs` (also exported with the `test-support` feature for
  other crates' tests) builds databases from Mixxx's own table definitions.
  `real_encoder_files_match_what_ffmpeg_trims` is opt-in: set `CUERITHM_MP3_SAMPLES` to a
  folder of `lame` and FFmpeg output. `tools/mixxx-acceptance/run.py` runs the installed
  macOS Mixxx against libraries CueRithm wrote; its results are in the research document.
