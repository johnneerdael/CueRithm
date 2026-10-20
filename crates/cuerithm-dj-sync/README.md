# Native DJ library sync

Independent Rust format adapters for VirtualDJ 8.5, current Engine DJ (schema 3),
djay Pro (current TSBF/Yap library), Serato DJ Pro 4 and Pioneer USB. Older software
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

`SyncOptions` contains field mapping, key notation, palette conversion, grid
preservation/locks and target-specific artwork/device/cue/compatibility settings.
Serato previews also stage every affected audio file; other media tags survive
and source PCM remains unchanged. Unsupported target containers produce explicit
errors. Device conversion uses the application's bundled media runtime and writes
only staged copies. No Python/Node runtime is required by the Rust adapters.

Format tests use synthetic data. Reference parser/serializer comparisons establish
format agreement, not actual acceptance by a running DJ application or CDJ hardware.
See `docs/research/dj-software-formats.md` for the evidence boundary and open work.
