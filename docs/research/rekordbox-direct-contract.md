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
setup flow. Automatic runtime key discovery remains to be implemented; it must
not require a purchased reference application or a Python runtime in the shipped
app. The private reference bootstrap is not a distributed dependency.

Target discovery/import, structured conflict baselines, IDs/USNs, metadata and
MyTags, playlists/native smart rules, cues/loops/beatgrid/artwork assets, staged
write recovery and actual6/7 target-app acceptance remain required. Version 6 is
not installed on this host. No schema is allowlisted for live writes yet; a
read-only schema match is not a completed sync implementation.

Primary implementation references: [SQLCipher API](https://www.zetetic.net/sqlcipher/sqlcipher-api/),
[SQLite3 Multiple Ciphers legacy modes](https://utelle.github.io/SQLite3MultipleCiphers/docs/ciphers/cipher_legacy_mode/),
and the pinned [rusqlite 0.39.0 source](https://github.com/rusqlite/rusqlite/tree/v0.39.0).
