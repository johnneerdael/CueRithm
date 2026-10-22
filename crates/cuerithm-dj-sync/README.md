# Native DJ library sync

Independent Rust format adapters for VirtualDJ 8.5, current Engine DJ (schema 3),
djay Pro (current TSBF/Yap library), Serato DJ Pro 4, Pioneer USB and Mixxx (schema 39-40). Older software
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

`mixxx` edits a staged copy of `mixxxdb.sqlite` (macOS `~/Library/Application
Support/Mixxx`, Windows `%LOCALAPPDATA%\Mixxx`, Linux `~/.mixxx` or the Flatpak
`~/.var/app/org.mixxx.Mixxx/.mixxx`) and the shared journal publishes it. The storage
contract is in `docs/research/mixxx-sync-contract.md`; the decisions are in
`docs/superpowers/specs/2026-10-22-mixxx-sync-design.md`.

- **Modes.** `MixxxOptions.existing_tracks` is `Keep` (add only: tracks Mixxx lacks are
  added, existing ones are untouched) or `Replace` (CueRithm replaces the grid, key, hot
  cues and details chosen by `replace_*`). `container` writes playlists or crates.
- **Schema.** Only schema 39 and 40 are written; the writer names only columns that
  exist and checks every table it needs.
- **Safety.** Rows are matched by canonical path (case-insensitive only where the file
  system folds case; ambiguity is an error). New rows are written parsed but never
  synchronized, so Mixxx does not re-read tags over them. The staged copy is read
  back before it is published: unrelated rows are digest-identical and no cue Mixxx
  would discard is written. A repeat sync publishes nothing.
- **Timing.** MP3 offsets come from the first frame's Info/LAME header (`mixxx/lame.rs`),
  not from a measured profile; AAC and WMA are zero and reported as unverified.
- **Tests.** `src/mixxx/` holds authored fixtures (`fixture.rs`, also exported with the
  `test-support` feature for other crates' tests). `real_encoder_files_match_what_ffmpeg_trims`
  is opt-in: set `CUERITHM_MP3_SAMPLES` to a folder of `lame` and FFmpeg output.
