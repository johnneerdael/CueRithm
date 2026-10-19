# Direct rekordbox contract: research revision 1

This is the contract and progress record for the native Rust adapter. The desktop app does
not yet sync to rekordbox. No collection XML transport or fallback is planned.

## Evidence and limits

On 2026-10-18, the installed rekordbox app identifies itself as **7.2.19.0342**
(`CFBundleShortVersionString` and `CFBundleVersion`). Its library `master.db`
has an encrypted header. Private copies of the database and available sidecars,
with file hashes, are under the ignored `.analysis/rekordbox-fixtures/` in the
research checkout. Their transactional consistency has **not** been established;
they are evidence copies, not a tested backup. No live database was changed.

The licensed Lexicon **1.11.15** artifact was inspected with leanrea on a copy.
Its app.asar SHA-256 is
`85fdbba4af97886ff1a626e6a4c96b4f9866dca3624fb2348a0332d7d6b63860`.
The following references are line numbers in the private formatted renderer
extraction, not redistributed source. They describe static shipped behavior;
they do not establish actual rekordbox execution or universal schema support.

| Evidence | Seen in Lexicon extraction |
| --- | --- |
| Writer opens with exclusive locking, obtains database/device identity and allocates unused 32-bit IDs | `Rekordbox6DatabaseWriter`, 248837–248899 |
| Change counter comes from agent registry `localUpdateCount`; rows carry `rb_local_usn` | 248987–249016; 249237–249243 |
| Existing tracks match paths, original paths and master-song identity, including soft-deleted rows | 249150–249245 |
| Cue records use content identity/UUID, in/out milliseconds, kind, color, loop size and comment; content-cue rows also change | 249251–249345 |
| Track analysis paths and cue/analysis timestamps are separate from ordinary metadata | 250326–250405 |
| Playlist hierarchy, sequence, attributes, smart rule payload and track links are distinct records | 250690–250760 |
| Export creates a database backup and checks whether the target program is running | `Rekordbox6Exporter`, 251390–251439 |
| Analysis assets live under `share/PIONEER/USBANLZ`; artwork has its own directory | `Rekordbox6DatabaseBase`, 270973–270995 |
| Native intelligent rules use a serialized rule document inside the direct database | 270466–270550 |

No rekordbox6 installation was found. At the initial checkpoint neither version
had a decrypted schema identity, real cue/beatgrid round trip or restored-backup
acceptance. The later native-read checkpoint below supplies an observed rekordbox 7 schema
identity and coherent database snapshot; target-app round trips remain pending.
**No real schema revision is allowlisted for writes.** An encrypted file copy
or read-only schema match alone does not fill those gates.

## Mapping and ownership requirements

These are Cuerithm design requirements inferred from the evidence, not a claim
that the corresponding writer has been implemented.

| Cuerithm identity/data | Target relationship | Required persisted baseline |
| --- | --- | --- |
| Stable library track ID and source hash | `djmdContent` ID, MasterSongID and UUID; metadata foreign keys | Target database ID, target row ID/UUID, source identity, prior exported field values |
| Validated replacement cue revision | `djmdCue` and associated content-cue records, plus matching analysis assets | Exported cue revision and normalized cue content hash |
| Playable PCM clock and beatgrid revision | Referenced analysis files and track analysis fields | Decoder/PCM recipe, grid hash, asset hashes and paths |
| Playlist ID, parent and ordered membership | `djmdPlaylist` and `djmdSongPlaylist` | External playlist ID, parent/order, prior membership hash |
| Custom tags | `djmdMyTag` and `djmdSongMyTag` | Owned categories/tags and link IDs |
| Smart playlist rule tree | Target native rule payload when representable | Original Cuerithm rules, conversion version, exact capability result |

The existing `sync_links` table is an initial external identity boundary; a later
migration must add structured conflict baselines and asset ownership before
writing. Do not infer ownership from matching a title or playlist name. Scope
external IDs by target database, preserve existing identities, allocate unused
IDs within actual target constraints, and preserve foreign references.

Treat local change sequence and deletion/status fields as part of the database
protocol. Do not assume a generic SQL UPDATE is enough or increment the global
counter once per row without verifying the actual target semantics. Preserve
unowned rows/assets and edits made in rekordbox after the recorded baseline.
Concurrent edits require an explicit conflict result, rather than silent loss.

Cues, loops and beatgrids must derive from the same playable audio clock as
preview. Convert to target time units once at the boundary with a documented
rounding rule. Preserve negative leading beat anchors where representable;
playable cue points remain bounded by duration. Export the binary assets and
their database references as a recoverable unit, with backup, staged files and
an interrupted-write recovery journal. Never delete live WAL/SHM files as a
general cleanup strategy.

Native smart-rule representability is a separate question from Cuerithm's full
58-operator evaluation. The shipped converter handles a subset of field/operator
combinations. The writer must expose an exact capability matrix; unsupported
rules need an explicit materialized-playlist option. Preserve native rule
payloads inside direct records where supported; this does not add collection
XML import/export.

## Original synthetic invariants

Run `python3 -m unittest discover -s tools/fixtures/rekordbox -v`.
The original generated manifest represents **our normalized test contract**,
not an extracted database schema. Tests reject duplicate scoped external IDs,
missing analysis assets, unknown contract revisions, out-of-range cue points,
unordered beatgrids, nonfinite duration and asset/PCM identity mismatch. Invalid
cases were defined before the validator and observed failing before it existed.
These tests constrain a future adapter; they do not claim target compatibility.

Before enabling direct writes, obtain coherent read-only schema snapshots for
each supported 6/7 revision, identify database/asset formats and timing units,
and test actual target-app import, repeated idempotent sync, cue/loop/grid
playback, playlist hierarchy/order, native smart rules, concurrent edit conflicts,
failure recovery and restoration. Publish sanitized schema identities and
acceptance results, without music-library rows, credentials or encryption material.


## Native encrypted access checkpoint

`cuerithm-rekordbox` now provides read-only encrypted access, schema/integrity
inspection and an owned standalone encrypted snapshot using SQLite's online
backup API. It uses SQLCipher 4 compatibility and bundled OpenSSL. Keys are
runtime-only zeroizing values with redacted Debug output; the native length-bearing
key API keeps quotes/NULs and key material out of SQL text. Missing files are not
created, symbolic database inputs are rejected, and cancellation between native
page batches discards only the owned output. No collection XML path is provided.

The rusqlite 0.39.0/libsqlite3-sys 0.37.0 pin uses SQLCipher 4.10.0 with SQLite 3.50.4,
so enabling encryption does not lower the previous library's SQLite 3.50.2 core.
The initial 0.37.0 SQLCipher feature bundled SQLite 3.46.1; 0.40.1 required compiler
features outside this project's Rust floor. Checked unsigned conversions and
statement caching remain enabled explicitly. Current-head platform checks and
the Rust 1.90 floor must verify this shared dependency boundary.

A fresh private main/WAL/SHM capture from installed 7.2.19.0342 was hash-stable
across capture and initial inspection. The reference reader reported 48 tables,
336 schema objects, user_version 0 and no foreign-key issues. Native Rust access
and a fresh encrypted standalone snapshot produced the same schema fingerprint:
`6bd02ac0a3f9cadfd3ae9b4c95e1ca0663fd3e7717a74e89a4c153834611d2eb`.
Both passed integrity and foreign-key checks. Private copied main/WAL hashes
stayed unchanged, and the owned temporary snapshot was removed after inspection.
No user rows, actual keys or proprietary schema/trigger bodies are published.

The live WAL changed later while the main program was absent; read-only handle
inspection identified `rekordboxAgent` holding it. Main-process absence is thus
insufficient write preflight. No live connection was opened by these probes and
no process was killed. Database-only snapshot coherence does not establish a
coherent backup of analysis/artwork assets or target-app playback acceptance.

The research example `cargo run -p cuerithm-rekordbox --example inspect --release --
--database <copied-db> --key-file <private-file>` prints aggregate diagnostics.
`--snapshot-cache <private-cache>` also verifies a temporary encrypted snapshot.
This explicit key-file interface is a research tool, not the intended desktop
setup flow. The automatic provider described below replaces that private bootstrap
for the inspected target version. No purchased reference application or Python
runtime is a distributed dependency.

Target discovery/import, structured conflict baselines, IDs/USNs, metadata and
MyTags, playlists/native smart rules, cues/loops/beatgrid/artwork assets, staged
write recovery and actual 6/7 target-app acceptance remain required. Version 6 is
not installed on this host. No schema is allowlisted for live writes yet; a
read-only schema match is not a completed sync implementation.

Primary implementation references: [SQLCipher API](https://www.zetetic.net/sqlcipher/sqlcipher-api/),
[SQLite3 Multiple Ciphers legacy modes](https://utelle.github.io/SQLite3MultipleCiphers/docs/ciphers/cipher_legacy_mode/),
and the pinned [rusqlite 0.39.0 source](https://github.com/rusqlite/rusqlite/tree/v0.39.0).


## Typed collection import and target discovery checkpoint

The SDK now reads 14 related tables in one read transaction with schema identity,
row-budget/cancellation checks and lossless raw cell/hash retention. It scopes
content IDs by database identity, preserves UUID/master-song IDs and native
update counters, resolves descriptive foreign metadata and keeps source/analysis/
artwork references distinct. BPM is hundredths; duration and cue milliseconds
are converted at their respective boundaries. Actual fractional BitRate values
are retained despite the target column's declared integer affinity.

Soft-deleted, unresolved and unsupported-service rows remain available as raw
target data. The explicit active-local view does not invent locations or master
identities. Playlists/native smart-rule payloads, ordered links, MyTag hierarchy
and cue kind/slot/memory/loop fields are retained. Unknown cue kinds or missing
foreign records produce issues. Native color fields and analysis references are
preserved for later palette/asset handling, not claimed as decoded playback data.
Unrequested play statistics are not descriptive fields; unrelated agent registry
rows are not read. The raw selected record keeps unowned columns so later writes
can preserve them rather than resetting them.

A private component comparison executed the original simple-track method on
all 3,417 copied rows with controlled location/color/numeric helper callbacks.
Rust's 12 descriptive text fields plus BPM matched its projection exactly. This
is the measured component boundary, not complete Lexicon import, cue-color,
asset or target-app parity. The same copy yielded 3,409 active local rows,
36,752 cues, 1,033 playlist records, 13,355 playlist links and 100 MyTag records.
Sixteen missing-foreign-record issues were surfaced; row values, paths and
per-row answers remain private. The SQLite integrity/foreign-key checks stayed
separate from these higher-level reference issues.

Read-only discovery finds installed 6/7 bundle versions, Windows version resources
(or explicitly labelled directory hints) and `masterDbDirectory` in the shared
settings. It skips directory symlinks, rejects conflicting/malformed settings and
reports typed location states. The actual host detected 7.2.19.0342 with its
configured database present and no discovery issues. Linux accurately reports
that a local target is unavailable. Generated 6 bundle fixtures test discovery
logic only; they do not establish actual 6 acceptance. Discovery neither launches
rekordbox nor opens/mutates its live database.

This collection/discovery layer does not persist imported records/baselines into Cuerithm,
resolve the runtime database key automatically, decode/write binary analysis
assets, enable a direct writer or provide a desktop Sync workflow. Continue the
full implementation sequence and actual 6/7 round-trip gates.

## Native analysis-file checkpoint

`cuerithm_rekordbox::anlz` reads PMAI envelopes and retains each tag's original
bytes, including unknown sections, waveform data, header extensions and trailers.
Typed readers cover PQTZ beats, raw PQT2 extended-grid words, PCOB/PCO2 cue lists
and UTF-16BE paths. Bounds and byte/tag/entry/string budgets are checked before
decoding; unknown nested cue types remain available as raw records and produce
an explicit error from the typed cue reader. Short extended cues can lack comment
and color fields without being discarded. Comment length uses all four bytes,
including synthetic Unicode comments longer than 255 bytes.

The staging API can replace a selected section, edit grid/path payloads while
retaining unowned header/trailer bytes, and assemble a merged cue-record list
without rebuilding retained entries. It returns bytes only. It neither touches
live files nor substitutes for the database/assets recovery transaction. Original
native units remain explicit; no negative anchor is silently clamped into an
unsigned millisecond field. Color-table codes and LED RGB values stay distinct.

Seen in Lexicon 1.11.15: modules 190/38737 provide the native cue, path and grid
builders; its direct writer also constructs PQTZ with the header value 524288.
Thirty authored synthetic cases were executed through the unchanged shipped
builders in Electron's Node 20.18.1. Our output bytes match all 30, including
Unicode/long comments, hot/memory lists, loops, empty lists and the extended-grid
microsecond dialect. The fixture records source identity and this isolation
boundary. It contains no extracted source, music, real paths or library labels.

PQT2 body words cannot universally be interpreted as microsecond fractions:
producers also encode beat positions there. The reader retains these words and
header fields without guessing; the microsecond builder explicitly implements
the inspected Lexicon dialect. Cross-file dialect selection and precision-aware
beatgrid reconstruction remain adapter work.

Private copied assets were also checked: one sample of 52 DAT/EXT/2EX files
round-tripped exactly and decoded 15,016 PQTZ entries. A second, more recent
sample of 128 DAT files round-tripped exactly and decoded 96,208 entries. These
samples were not deduplicated; their counts must not be added as unique files. Their cue
lists were empty and they contained no PQT2 sections; populated cue and extended
grid evidence is synthetic component evidence, not native target acceptance.
No source asset was written. Individual captures were stable during copying;
this is not an all-assets coherent backup while the background agent is active.

The remaining integration includes asset-path containment, sibling-file planning,
per-field/record ownership baselines, supported-version key-provider acceptance, Cuerithm persistence,
database/asset staging and interrupted-write recovery, followed by real 6/7
round-trip acceptance. This module alone does not enable Sync.

Primary format reference: [DJ Link Ecosystem analysis-file specification](https://djl-analysis.deepsymmetry.org/rekordbox-export-analysis/anlz.html).

## Automatic native key-provider checkpoint

`NativeKeySource` uses the installed main executable and the selected encrypted
`dp` option from local settings. It scans a bounded stream for the inspected
`rekordbox_client` C-string profile and requires a unique sixteen-byte seed.
Identical copies in a universal binary are deduplicated; unknown, missing or
ambiguous profiles fail explicitly. No seed or key value is compiled in.

Unwrapping runs in Rust using RustCrypto Blowfish 0.9.1, big-endian ECB, with
PKCS5 padding when present. The actual stored option was block-aligned without
padding, which the original decoder also accepts. Authored nonuniform-seed
vectors cover padded and unpadded variants against unchanged egoroof-blowfish
2.2.2. Invalid padding and non-text results are rejected explicitly rather than
being returned as apparently valid keys. The pinned cipher's zeroize feature
clears its expanded key schedule on drop; the included MIT/Apache notices record
its provenance.

The provider reads data only: it launches no agent, Node/Python interpreter,
helper process or inspector session. The earlier agent bridge is removed from
production code and its generated-cache CI tooling is removed. Its isolated
research execution of `buildBFKey` supplied a comparison oracle. Both native
architecture sections contained the same seed immediately after the marker,
and the native extraction matched that oracle without publishing the value.

Inspected native executable: version 7.2.19.0342, SHA-256
`07dcecbaf304f777e435539c83c7dcc02e41de1eaf2e0a75e8a9852e4411a244`. Automatic native access
opened the private copied database and read all 3,417 tracks, with the same
schema and descriptive projection hashes reported above. Integrity and foreign-key
checks passed. The source database, music and installed bundles were not written.
The access path used no manual key file or reference application.

The options parser selects only `dp` and skips unrelated account/authentication
values. Its captured input size is checked against the opened file and changed
inputs fail. Keys, seeds and work buffers have zeroizing ownership; no actual
key, encrypted option, seed or user row is included in the implementation,
fixtures or diagnostics. Cancellation/deadlines are checked during bounded
native scanning. Symbolic inputs and oversized files are rejected.

Generated tests cover the independent ciphertext vectors, real encrypted
fixture access, chunk-boundary scanning, duplicated architecture data, ambiguous
profiles, unsupported headers, malformed options/ciphertext, budgets and
cancellation. These fixtures do not prove actual version 6 or Windows binary
layout acceptance. Local Linux target discovery remains unavailable. Native
read access alone does not complete Cuerithm import/baseline persistence, the
desktop Sync workflow or the coordinated writer/recovery/round-trip gates.

Primary implementation reference: [RustCrypto Blowfish 0.9.1](https://docs.rs/blowfish/0.9.1/blowfish/).

## Durable collection import and initial baselines

The library now consumes the native SDK collection in a single local SQLite
transaction. Schema 11 adds database/device-scoped targets, normalized latest
raw records and content-to-local-track links. Multiple native content IDs can
reference one local track; IDs from different database/device scopes remain
separate. Existing unscoped `sync_links` and their baselines are preserved.
Raw records update only when their serialized contents change, rather than
rewriting the full collection for one changed record. The SDK dependency now
also enables its bundled SQLCipher core for library/desktop builds.

New, selected, available files enter the existing preparation queue using the
captured selected-settings revision. The import seeds their primary display
metadata without creating measured key, BPM, duration or analysis results.
Existing local metadata, analysis, archive state and jobs are retained. Missing,
deleted, unselected and unsupported references remain inspectable. Reused native
IDs with conflicting UUID/master-song identity are retained as explicit identity
conflicts, and target-side removals never delete local music or library rows.

Each linked track stores separate initial local/remote field values, explicit
ownership choices and the captured native identity. Remote observations advance
without resetting those initial values. Local mapped paths and raw native paths
remain distinct. Initial fields are **not acknowledged as synchronized**, even
when their values happen to match: first reconciliation, destination value
conversion and confirmed publication belong to the later sync stages. This
checkpoint does not apply remote edits over existing local work, acknowledge
field ownership changes, merge all metadata into existing ready tracks or enable
native writes. Native playlist/tag/rule records are preserved; materializing their
local equivalents remains part of the dedicated adapter work.

Tests cover restart/idempotence, duplicate paths and cross-database IDs, local
edits, field-baseline retention, selection changes, missing/deleted/reused IDs,
opaque blobs/nulls, incomplete snapshots, scoped registry reads, cancellation,
schema 10 migration, legacy links, foreign keys and injected SQL rollback. The
older schema-upgrade tests now expect schema 11. No file metadata is written by
this import transaction; subsequent preparation follows its configured settings.

A private capture import retained all 3,417 native track observations and 62,208
raw records. Every stored raw cell and native fingerprint matched the input.
Its default resolved paths found 9 existing WAV files; 3,400 missing references
and 8 target-deleted records remained explicit. Those nine files were queued
without starting preparation, and their hashes stayed unchanged. Separately,
an explicit test mapping to a copied user-supplied FLAC exercised complete
preparation, preserved the initial baselines and left the original audio
unchanged. That mapping tests the workflow; it does not prove automatic matching
between the capture's unavailable files and the current music library.

The backend target/link/record APIs expose this durable state. The branded
Sync workflow, supported native rule/asset mappings, per-field reconciliation,
coordinated DB/asset writes and recovery, and actual 6/7 desktop round trips
still require implementation and acceptance. No collection XML fallback is used.
