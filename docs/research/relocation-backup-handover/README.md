# Relocation, portable backup and Lexicon import handover

Research date: 2026-10-23. Target: CueRithm, inspected checkout `673c017`.
Status: implementation specification; no application changes implemented or tested.

This package gives an engineer the behavior, data contracts, integration points and
acceptance cases needed to build these features without reading Lexicon source.
The vendor evidence is descriptive. Sections marked **Decision** define CueRithm's
proposed behavior; they do not claim that CueRithm already implements it.

Read in order:

1. This overview and the relocation contract below.
2. [Portable format and restoration](portable-format.md).
3. [Lexicon backup import profile](lexicon-import.md).
4. [Evidence and compatibility boundaries](evidence.md).
5. [Implementation plan](../../plans/2026-10-23-001-feat-relocation-portable-backups-plan.md)
   and [acceptance matrix](acceptance.md).

## Outcomes and scope

| Requirement | Result expected by the user |
|---|---|
| R1: relocate one track | Point an existing entry at a selected replacement file, retaining its library identity and work. |
| R2: relocate a selection | Recursively find externally moved files under a chosen folder; review and reconnect matches. |
| R3: replace a root | Repair drive-letter, volume, parent-folder or extension changes across missing or all tracks. |
| R4: portable backup | Export a compact `.cuerithm` containing library data, without audio or disposable caches. |
| R5: restore elsewhere | Restore on macOS, Windows or Linux; map old music roots to available folders. |
| R6: import Lexicon | Read a real Lexicon backup ZIP and preserve its tracks, cues, tempo markers, playlist tree and supported smart rules. |
| R7: preserve work | Make losses visible; avoid silent preparation, tag changes or external DJ database writes. |
| R8: independent delivery | Implement from this contract, synthetic fixtures and CueRithm source. No vendor source/runtime dependency. |

**Decision:** `.cuerithm` restoration replaces the active library. Lexicon import
creates a new library from the backup, with the current library retained for
recovery. Merging independent libraries is outside this delivery. This makes
identity, playlist and conflict behavior concrete without inventing a merge engine.

Music must be copied separately. Files may remain unresolved after restore/import;
all metadata and playlist entries must still be usable. Cloud backup, a music-file
archive, streaming playback, XML sync, automatic cross-computer synchronization,
and new DSP inference are outside scope. Imported streaming records remain visible
as unavailable references; replacing one with a local file is supported manually.

## Existing CueRithm constraints

These are **seen in current source**, not inferred from earlier research:

| Constraint | Owning source and consequence |
|---|---|
| Stable track ID; unique native path BLOB | `crates/cuerithm-library/migrations/001_initial.sql`. Relocation must update the same ID. |
| Native paths are `unix:` bytes or `windows:` UTF-16LE | `crates/cuerithm-library/src/model.rs`, `path_bytes` / `bytes_path`. Foreign paths currently fail; portable locators are required. |
| Enqueue deduplicates by canonical path | `crates/cuerithm-library/src/jobs.rs`, `enqueue_one`. A moved path otherwise becomes a new ID. |
| Incoming preparation may tag, clean and replace cues | `jobs.rs` and `prepare.rs`. Restore/import must bypass preparation. |
| SQLite schema 11, WAL, exclusive lock; startup replays journals | `store.rs`, `Library::open`. Never open an untrusted source DB as a CueRithm library or copy a live database file alone. |
| Missing and unavailable are different | `maintenance/files.rs`, `availability`; `maintenance/broken.rs`. A disconnected parent or permission error must not mean deletion. |
| Current published grid has one BPM/anchor | `crates/camelot-core/src/lexicon_publication.rs`, `Publication`. Variable tempo needs an independent lossless timing model. |
| Player and smart snapshots depend on DSP analysis | `apps/desktop/src/published-analysis.ts`; `playlists/snapshot.rs`. Imported scalar metadata/cues need explicit projections. |
| `ready` entries can have null analysis | `playlists/queries.rs`; `store.rs`, `analysis`. Keep library visibility separate from DSP completeness. |
| Watcher skip currently needs a current job | `jobs.rs`, `enqueue_discovered`. Fix before enabling watchers around restored/imported entries. |
| Native targets permit only rekordbox | `migrations/011_native_import.sql`. Add separate import provenance; do not impersonate a rekordbox target. |

Existing Missing files UI (`apps/desktop/src/views/maintenance/BrokenTab.tsx`)
scans and archives problems. Existing retained-original Restore actions concern
maintenance recovery copies. Neither is the portable backup feature proposed here.

## Shared data model decisions

**Decision:** keep `tracks.state` as `ready|incoming`; add orthogonal source,
availability and analysis information. Do not fabricate a complete `FileAnalysis`,
analysis job, configuration hash or local content hash for imported offline files.

Add persisted concepts, with final SQL names chosen by the implementer:

| Concept | Required fields and invariants |
|---|---|
| Portable locator | ID, source path flavor, exact source representation, optional root ID and relative components; see portable format. |
| Local binding | Locator ID, machine-local native path, resolution state, verification proof and revision. Unique local paths refer to at most one active track. |
| Track origin | `cuerithm|lexicon`, source-library ID, source-record ID; durable and scoped. |
| Content proof | Optional SHA-256 of full encoded bytes; optional size/mtime/file identity; `unknown` is explicit. Never use a fake digest. |
| Analysis status | `complete|imported|not_analyzed|stale`; independent of availability and Incoming. Actual analysis remains revisioned with engine/config/model identity. |
| Accepted musical data | Key, BPM, whole-track energy, duration, complete cue list and tempo map, provenance, revision. DSP and imported data remain distinguishable. |
| Import extensions | Versioned, bounded, inert JSON for source fields not represented in CueRithm. Preserved in subsequent `.cuerithm` exports. |
| Operation plan | ID, library-generation ID, schema/algorithm version, entity revisions, selections, root bindings, file proofs, expiry and result state. |

Native `tracks.path` cannot remain the only required locator for unresolved foreign
files. Migrate it to nullable local binding storage (with a partial uniqueness
constraint for resolved paths), or move it behind the binding table and adapt all
readers. Never put a foreign path string or placeholder into native path bytes.
Do not canonicalize an unavailable source on the destination computer.

`source_hash` must become nullable or gain a separately enforced known/unknown
representation; all comparison/enqueue callers must handle unknown. Keep saved
native `.cuerithm` hashes. A Lexicon fingerprint is not this encoded-byte hash.

Imported records are accepted `ready` entries unless `incoming=1`, which restores
Incoming intent without enqueueing a job. Their analysis status is `imported`.
Watchers skip accepted records already bound to that file even without a job.
Explicit Prepare may enqueue them later and must describe its cue replacement.

**Decision:** `Detail` returns accepted cues/timing and musical values independently
of optional DSP analysis. Player, smartlists, export and relocation consume this
projection. Imported cues win until the user explicitly replaces them. Keep the
existing Lexicon-compatible DSP calculations unchanged.

## User flows

### Relocate selected tracks

Show Relocate when a selection contains a missing/unavailable local reference, or
one unsupported streaming reference. Refresh selected availability on entry.
For one eligible record, open a file picker. For several, open a folder picker
and search recursively. Existing available entries in the selection are skipped
and counted. Archived and Incoming entries can be explicitly selected; filters
must not silently remove them.

Review displays old path, candidate path, evidence, collision, availability and
decision. Confirm applies selected rows. A single available replacement can have
a different name or format; this is an explicit replacement, not proof of identical
audio. Show that saved timing may need checking. Do not automatically rewrite cues.

### Find Lost Tracks utility

Provide source root, destination root, Missing only / All tracks, optional new
extension, and a preflight-backup toggle enabled by default. Show source roots
and counts, then a per-track preview. Include archived records in this explicit
utility and distinguish Incoming in results. Default scope is Missing only;
unavailable records are a separate opt-in group so an offline drive is not treated
as a lost file automatically.

Before Apply, the default preflight backup writes a verified `.cuerithm` into the
locally configured backup folder; on first use require a folder choice. Backup
failure blocks Apply and leaves the review intact. The user may explicitly turn
off the toggle and retry, retaining that decision in the operation report. Do not
silently proceed on backup failure. A backup taken for Apply is the same logical
format described here, not a retained music-original copy.

In All tracks mode, mapping a present file is allowed after review. This updates
the library reference; it does not move the source audio. Persist a root mapping
only when the user chooses it. A restore root binding and a permanent relocation
are separate operations: the former resolves portable locators on this machine;
the latter accepts a new canonical location for future exports.

### Back up / restore / import

Use a distinct Library backup area; do not mix it with retained audio originals.
Export text states: “Library data only. Music files are not included.” Restore and
Lexicon import show the archive identity, library counts, excluded domains,
compatibility issues, required disk space, music-root mapping and replace effect.
The last confirmation names the library that will become active and the retained
recovery library. No subscription or cloud connection is needed.

After activation show tracks connected / unresolved / unavailable, restored cues,
playlists, smartlists and compatibility warnings. Keep unresolved entries in their
playlists. Offer Relocate and rescan; do not auto-start tag-writing or DJ sync.

## Relocation matching contract

The following is **Decision**. The Lexicon baseline and deliberate differences are
recorded separately in `evidence.md`.

### Candidate discovery

1. Search only user-selected local roots, recursively, with no network discovery.
2. Support every application audio format: MP3, AAC/ADTS, AAC/M4A, ALAC/M4A,
   FLAC, WAV, AIFF/AIF, Ogg Vorbis, Opus and WMA. Case-insensitive extension
   recognition is a routing hint; verify a selected target with the bundled probe.
3. Use no-follow directory enumeration. Do not recurse through symlink/reparse
   directories. Reject symlink target files by default with an explanation; a
   separate reviewed resolution to their canonical regular file may be offered.
   Record canonical containment and visited directory identities for races/aliases.
4. Never cross the selected root through `..`, a junction, mount redirection or a
   path alias unnoticed. A mounted volume deliberately chosen as a root is allowed.
5. Record inaccessible subtrees as skipped/unavailable; a partial search cannot
   assert “no match” for the inaccessible part. Report matched and unsearched counts.
6. Poll cancellation during traversal and file probing, without a DB transaction.
   Bound concurrent probes to four and open files to 32. Spill the candidate index
   to a task-owned temporary SQLite database instead of retaining unbounded maps.
   Return progress at least every 250 ms while work advances. At one million
   candidates or 15 minutes, pause with a resumable partial result and explicit
   Continue; do not silently label remaining tracks unmatched.

### Candidate ranking and confidence

Retain exact original spelling. For matching signatures only, apply Unicode NFC
to text and remove a final `_pn` from the filename stem, case-insensitively.
This gives the known Lexicon filename compatibility without altering files.
First group by exact parent basename + filename including extension, then by
filename including extension. Signatures are case-sensitive. Offer a distinct
case-insensitive fallback suggestion; it is never silently equated on Linux.

| Evidence | Result |
|---|---|
| Explicit component-aware root map, target exists and encoded hash equals known source hash | Verified match; preselected. |
| Exact signature and exactly one candidate with matching known source hash | Verified match; preselected. |
| Unique candidate at the highest available exact-signature tier, size agrees or is unknown | Filename match; preselected in review, clearly not content verification. |
| Several candidates at that tier, exactly one size match | Size-assisted filename match; preselected in review. |
| Several size matches, or several candidates with no matching size | Ambiguous; no selection. Show all candidates in deterministic path-byte order. |
| A sole candidate has different size, known hash differs, only case-fold match exists, or extension was changed | Replacement suggestion; explicit row selection required. |
| No candidate in fully searched roots | Unmatched. |
| Inaccessible roots or cancelled/limited traversal could contain a candidate | Search incomplete. |

Never choose a lower signature tier merely to avoid an ambiguity in a higher tier.
Evaluate vetoes first: a known source/candidate hash disagreement, extension
change or sole-candidate size disagreement prevents filename/size preselection.
Extension change requires explicit replacement approval even if encoded hashes
agree. Hash equality may resolve a same-name ambiguity only without those vetoes.
Size is compared as unsigned bytes, never rounded. Hash only shortlisted candidates,
with streaming reads and cancellation. A missing saved hash does not equal an empty
hash. Retagged/transcoded files may fail encoded equality; permit explicit replacement
after probe, preserving old analysis as stale and imported data as saved provenance.
Do not use artist/title, source tags or acoustic similarity as automatic proof.

### Root and extension transformation

Parse source and destination by their declared path flavor, not the host parser.
Match entire root components at the beginning. `D:/Music` must not match
`D:/Music2`, and `/music` must not match `/home/music`. Use the source platform's
declared case rule for root matching, with a recorded override when necessary;
verify destination collisions using the actual destination filesystem semantics.
Choose the longest matching source root when maps overlap; equal conflicting maps
are an error. Preserve the suffix components exactly.

Extension input is a single allowlisted extension, without dot/slash/wildcard;
replace only the last extension. A path-only map does not change it. Normalize
separator syntax, not filename bytes or Unicode spelling. Every target must exist,
be regular and probe successfully before default Apply. A destination not yet
connected may instead be saved as an unresolved portable root binding, explicitly
labelled; do not report those rows relocated.

### Occupied destination and survivor selection

The same resolved file cannot be claimed by two active track records. Detect
canonical aliases, destination case/normalization collisions, hardlinks and paths
already occupied by another pending row before applying. A hash equality alone
does not imply a merge: identical files at distinct paths can be separate entries.

Default collision action is Skip. Per collision, offer Keep source entry / Keep
existing entry / Cancel. No global unreviewed force deletion. For a chosen survivor:

- Retain its ID, metadata, cues, tempo map, tags and analysis as a complete set;
  do not mix fields from the loser silently. Keep the losing set in undo/provenance.
- Repoint playlist memberships and source-identity links from loser to survivor.
  For an ordinary playlist where both already occur, keep the survivor's existing
  position and remove the losing occurrence, then compact positions. Where only
  loser occurs, replace in place. Report collapsed memberships.
- Preserve external remote observations/baselines as historical data. If both have
  conflicting identities for the same native content, leave an explicit sync
  conflict; never acknowledge a remote write or activate an external target.
- For smartlists, recompute from the surviving accepted data. Do not copy cached
  results as memberships. Keep external import identity aliases for both source IDs.
- Archive/tombstone the loser as part of the reversible operation, rather than
  cascading away evidence before undo is durable. Its former local binding is freed.

Two independent missing source entries targeting the same unoccupied file become
a group requiring the same survivor decision. Choosing a file already bound to
the source is a no-op after verification, not a new revision.

### Preview, publication and concurrency

Persist the reviewed plan with a 24-hour expiry, library-generation ID, selected
track/cue/tag/playlist revisions, old binding, root revision, candidate file
identity, size, mtime and optional hash. Bind later choices to that plan ID.
Selections are explicit stable IDs; UI filter changes cannot alter the plan.

Use a Host-level operation gate. Before applying, prevent new preparation/tag/
cleanup/sync jobs, drain running writers, pause watchers and stop/unload affected
playback. Acquire gates in a documented single order: Host operation gate, existing
tag gate, then SQLite. Never acquire either gate from inside a DB callback.
Create the preflight snapshot under this same operation lease; the export helper
must accept the lease instead of reacquiring the gate. Finish and readback-verify
the archive before mutation, then recheck all plan/file proofs. Show the selected
backup destination in final confirmation and its resulting path in the report.
Reuse a completed preflight backup on retry only if its library revision matches
the still-valid plan; otherwise make a new snapshot. Backup failure/cancellation
leaves relocation unapplied. Apply carries the reviewed destination handle or
verified backup reference as well as the toggle, for every relocation mode.
Gather file proofs outside the SQLite write transaction; under the gate recheck
entity revisions and open-file identity proofs immediately before committing.

If any selected row is stale, return a structured stale result and apply nothing;
rebuild a preview with surviving user choices. When all pass, publish the selected
set in one DB transaction: bindings, same-ID path changes, collision rewires,
undo before/after data and revision invalidations. No physical file mutation is
needed. Cancellation before commit rolls back the whole set; after commit report
completed. A cancelled search never changes paths. No partially committed batches.

After commit bump library/smart input revisions, invalidate saved maintenance
previews for changed entities, clear affected PCM/artwork/probe cache entries,
emit one change event, and resume allowed services. Path changes mark local sync
fields dirty; they do not alter acknowledged remote baselines. Watchers must see
the new binding before resuming. A changed replacement invalidates DSP completeness
and derived caches; accepted saved cues remain accessible with an inspection flag.

Undo is the exact inverse DB transaction with after-state revision guards. It
does not move audio. If old media is still missing, undo restores the missing
reference. If the old binding is now occupied or any involved entry changed,
return a conflict and apply nothing. Persist undo across restart for relocation;
portable export deliberately excludes executable undo history.

A file can still disappear after commit. Record the proof timestamp and detect it
on next access; show unavailable/missing, retaining the committed library data.
Do not claim a database transaction makes the filesystem immutable.

## Service and IPC contract

Use typed command DTOs and the existing operation-ID/progress/cancellation pattern
in `apps/desktop/src-tauri/src/maintenance.rs`, `maintenance_commands.rs`, and
`apps/desktop/src/use-maintenance-task.ts` (resolve its actual directory when editing).
These are required operations; final Rust function names are implementation-owned.

| Operation | Inputs | Result |
|---|---|---|
| Refresh availability | Track IDs, generation | Per-ID present/missing/unavailable/unsupported and reason. |
| Plan relocation | Mode, stable IDs or utility scope, roots, extension, options | Task ID; later immutable plan ID, candidate rows, skipped/incomplete counts. |
| Apply relocation | Plan ID, row choices, survivor choices, backup toggle | Atomic operation ID, applied/no-op/skipped counts or stale/conflict. |
| Undo relocation | Operation ID | Applied inverse or guarded conflict. |
| Export backup | Destination handle, options | Task ID, bytes, counts, digest and verified final path. |
| Preview restore/import | Selected file handle, detected type | Source profile, counts, validation/loss report, plan ID and root requirements. |
| Bind roots | Plan ID, mapping choices | Revised plan, resolved/unresolved/collision counts. |
| Activate | Plan ID, explicit replacement decision | New generation ID or unchanged library with structured failure. |
| Cancel | Task ID | Cancellation requested; task owns slot until finished. |

Progress includes phase, completed/total when known, byte counts, skipped errors
and cancellation state. Result categories include unsupported version, malformed
archive, corrupt data, resource limit, unsafe path, stale preview, file unavailable,
permission denied, collision, insufficient disk, busy writers and activation error.
Full private paths belong in local detailed reports, never telemetry by default.

## Completion boundary

The research is ready for engineering when the linked contracts and plan have no
unresolved design blocker. Shipping requires the acceptance matrix, including real
cross-platform runs. Static reverse engineering and schema inspection do not prove
that any implementation, device playback, native sync or restoration already works.
