# Native DJ software format contracts

The port targets the current paths shown in the supplied Lexicon screenshots:
VirtualDJ, Engine DJ, djay Pro, Serato DJ Pro 4 and Pioneer USB. Previous software
versions are not included. Existing Rekordbox 6/7 groundwork remains in its separate
crate and uses direct databases, without collection XML.

## Evidence

Observed shipped artifact: Lexicon 1.11.15. Its copied app archive matched the
installed artifact SHA-256 `85fdbba4af97886ff1a626e6a4c96b4f9866dca3624fb2348a0332d7d6b63860`.
Private extraction and probes remain outside tracked files. Rust code implements
observed data layouts independently; this repository contains no decompiled code,
proprietary application bundles, source music or license credentials.

| Format | Seen in shipped artifact | Independent validation |
|---|---|---|
| VirtualDJ | Version 8.5 XML; tempo in seconds/beat in Tags/Scan, actual BPM in beatgrid POIs; loop size in beats; current MyLists XML | Synthetic source serializer agrees on BPM, cue positions and loop size; unknown XML and unrelated tracks preserved in Rust tests |
| Engine DJ | Schema major 3; relative media paths; BE uncompressed-length prefix plus zlib performance records; PNG artwork blobs | Decompressed payload digests agree with reference outputs; synthetic database merge, grid preservation and normalized artwork tests |
| djay Pro | Yap `database2` collection/key/data records; TSAF aligned tagged binary objects with pointer references; native index reset | Both directions of synthetic record decoding tested with reference reader/writer; cue/grid/playlist and rollback tests |
| Serato 4 | Root/per-location SQLite databases plus aggregate master; cues/grids in native audio tags; revision and crate links | Reference reader accepted generated MP3/FLAC/M4A/AIFF cue/grid tags; decoded PCM and existing metadata preserved; synthetic multi-volume aggregate tests |
| Pioneer USB | PDB catalog plus SQLCipher Device Library Plus; copied media; DAT/EXT/2EX grids/cues/waveforms | Reference PDB parser accepted generated Unicode metadata and hierarchical playlists; native encrypted database and analysis assets tested on synthetic audio |

These probes are isolated serialization checks. Running the full Lexicon or target DJ
application was not part of these comparisons. Native application import/restart,
physical CDJ playback, cross-platform file publication and real-library scale remain
separate acceptance layers. Common-format analysis requirements are unchanged by
these adapters; a target's narrower playback compatibility is reported explicitly.

## Synchronization integrity

Sync builds files in a private journal before showing their diff. Publishing creates
recoverable byte backups, rechecks process state and rejects changed targets/sources.
Database WAL/journal absence is also a dependency: a newly created sidecar invalidates
the preview. Interrupted writes remain visible for recovery and never acknowledge a
modified-scope baseline. Restore refuses foreign post-sync edits.

Native smart-rule compilation is implemented for the supported whole-definition
subset. Unsupported predicates or missing target semantics materialize the complete
captured membership with a preview reason; Pioneer USB always uses ordinary lists.
Extra prepared metadata and explicit IANA timezone/key-mixing context are carried
through projection. See [smart-rule policies](dj-sync-smart-rules.md).

A separate durable local ownership ledger binds source playlists to native identities.
Full and Modified sync propagate established local removals only for playlists created
by CueRithm. Matched external playlists are retained, unrelated native records remain,
and structural conflicts stop destructive mutations. Typed file/directory journals
cover case-only tree renames and exact restore. Batch staging keeps the reviewed
mutation set together. Apply and restore each have recoverable acknowledgement
transitions; an interrupted transition blocks a fresh preview until settled in history.
See [ownership and recovery](dj-sync-playlist-ownership.md) and
[timeline conversion](dj-sync-timing.md).

## Final artifact review boundary

Targeted converter comparisons and synthetic probes informed fixes for Pioneer ANLZ
path collisions, hidden catalog discovery, dotted VirtualDJ names, Serato explicit
null clears, djay filename indexes and Engine raw BPM precision. These are static
and synthetic findings, not target-application or hardware acceptance.

Ghidra 12.1.4 reviewed only the shipped native SQLite addon, SHA-256
`9e351a1ed003cd743b341d0b0b4accc7d300d8608694fbffedf5371fa05cd8b7`.
The observed key-attach path and SQLCipher version-4 table parameters specify a
4096-byte page, 256000 KDF iterations, enabled HMAC and algorithm IDs 2. This is
native-addon static evidence, not JavaScript decompilation or a runtime acceptance
claim. REA's ASAR analysis uses the hash-matched archive identified above, but its
semantic coverage is partial because of heap and result limits. Targeted converters
were compared manually and probed; no whole-application semantic completeness is
claimed. Final full checks, PR review and merge remain pending.
