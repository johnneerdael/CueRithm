# Direct rekordbox contract: research revision 1

This is a constraint document for the future Rust writer. The desktop app does
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

No rekordbox 6 installation was found in this probe. Neither version has a
decrypted schema fingerprint, real cue/beatgrid round trip or verified restored
backup in this delivery. **No real schema revision is allowlisted for writes.**
The encrypted copy alone is insufficient to fill those gaps.

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
