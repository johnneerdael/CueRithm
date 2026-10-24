# Import and synchronization contract

Status: proposed implementation contract, researched 2026-10-23. This document specifies new behavior; it does not claim those features already exist. Companion adapter documents label source facts **Seen**, interpretation **Inferred**, and unresolved behavior **Unknown**. No source-app library was modified in this research.

## Product boundary

Support onboarding from every import source listed by Lexicon, plus Mixed In Key 11, DJ.Studio and Mixxx. “Import” is a read of a source library into CueRithm. “Refresh” repeats that read with stable source identities. “Write back” modifies a source application's library; DJ.Studio is explicitly in scope for write research. Do not present import capability as proof of two-way synchronization, or add automatic writes to the other sources as part of this handover.

Rekordbox 6/7 remains direct-only. The repository prohibits rekordbox XML transport. The Lexicon-compatible rekordbox 5 XML format is documented as a researched coverage gap, not an implementation instruction or supported route. Apple/iTunes, VirtualDJ and Traktor XML formats have independent contracts and are not covered by that rekordbox restriction. No release/version bump or implementation changes accompany this pack.

## User flow

1. Choose source and source library/profile/volume. Show detected product version and schema compatibility. Allow a manual file/folder selection; do not rely on one hard-coded user directory.
2. Take a consistent read-only snapshot. Scan metadata without processing audio. Show totals: source rows, selected tracks, missing files, streaming-only tracks, playlists/folders, unsupported fields, parser failures and identity ambiguities.
3. Choose whole library or selected playlists/folders, including descendants explicitly. Explain whether tracks outside playlists are included. Keep repeated playlist occurrences and order. A folder is a hierarchy node, not a tag or filesystem relocation request.
4. Choose field groups and processing policy below. Preview additions, updates, conflicts, losses and skipped records. Show source values alongside current CueRithm values for conflicts.
5. Apply one reviewed immutable plan to CueRithm. Save a receipt, policy revision and source snapshot identity. Cancel before commit leaves no partial import. Source audio/database stay unchanged.
6. Offer refresh from the same source, maintaining source identity links. DJ.Studio write-back is a separate reviewed plan, disabled until its acceptance gates pass.

No “replace entire CueRithm library” default. The existing library and unrelated playlists must survive onboarding. Selecting fewer fields must not silently imply destructive playlist replacement.

## Field selection

Capabilities are per adapter and per detected schema, including reason strings. Render unavailable fields disabled with an explanation. Supported means a decoder and mapping are implemented and tested, not merely that the source application displays a field.

| Independent selection | Contents and invariants |
| --- | --- |
| Descriptive tags | Title, artists (ordered), album, album artist, genre, label, composer, remixer, mix/version, comment, grouping, year/date, track/disc numbers and totals. Keep multi-value and raw text. File path is not a title fallback unless displayed as such. |
| Musical key | Raw source token + notation + normalized pitch class/mode + Camelot rendering. Preserve unknown/ambiguous tokens verbatim. Conversion changes notation, not pitch. |
| BPM | Scalar tempo, source precision and lock if known. Separate from grid. No automatic half/double tempo correction. |
| Beatgrid | Constant or variable tempo structure, anchors, offset, sample rate/time base, downbeat information only when known, lock and source provenance. An estimated first beat is not a downbeat. |
| Hot cues | Stable source cue ID/slot, position, label, color, kind and flags. Preserve sparse slots and zero-time cues. |
| Generic source cues / markers | Untyped persisted markers such as MIK CuePoint, with source ID/name/time and no fabricated pad slot or hot/memory semantics. Offer this group even when hot-cue capability is unavailable. |
| Memory/main/load cues | Separate from hot cues. A main/load cue changes deck behavior; do not silently promote it to hot cue A. |
| Loops | Start, end/length, slot, name/color, active/locked status where represented. No zero-length loop fabrication. |
| Ratings, color, play history | Preserve raw scale, timestamps/timezone and play counts. Do not merge unlike rating scales implicitly. History and Auto DJ queues are opt-in and distinct from normal playlists. |
| Energy | One whole-track rating with original scale/provider. Do not label imported structural cues with energy, or interpret MIK's time-varying energy as CueRithm's whole-track rating. |
| Artwork, custom tags | Owned categories, raw artwork references and embedded-vs-external provenance. No remote fetch or file write simply because metadata is imported. |
| Playlists and folders | Stable IDs, parent IDs, ordered membership occurrences, source type, smart rules if portable; otherwise a clearly labeled materialized snapshot. |

For existing tracks, unchecked means **do not modify the local field**. For new tracks, unchecked source data may be retained in the private raw observation for fidelity/reconciliation but must not become the published local value. Essential identity/location data is required for linking and explained separately. Null, absent, empty string and an explicit delete are distinct states. Parse errors do not become empty arrays that erase valid cues.

Selecting a beatgrid without BPM preserves local scalar BPM, records the source grid's tempo in grid data, and warns if the two differ. Selecting BPM without a grid preserves the grid. Never synthesize an allegedly imported grid from scalar BPM alone.

## Processing policy: separate import from preparation

The default is **Store as imported**. Library import is not ordinary watched-folder Incoming ingestion. The existing Incoming behavior remains the choice for users who request preparation; a new import policy must bypass its mandatory work.

| Policy | Import fields | Store lookup/cleanup | DSP | Cues | File tag writes/moves |
| --- | --- | --- | --- | --- | --- |
| Store as imported | Only selected groups | Off | Off | Source selection retained | Off |
| Fill missing analysis | Selected groups | Off unless separately enabled | Only missing selected key/BPM/whole-track energy/grid results | Retain source cues; generate only if explicitly requested and missing | Off |
| Reanalyse selected fields | Selected groups | Off unless enabled | User-selected fields, explicit replacement preview | Retain source cues by default; separate replace-generated/user-cues switch | Off |
| Retag / cleanup | Selected groups | Selected providers/rules with preview | Independent choice | Retained unless separately selected | Separate explicit publication and move choices |
| Full CueRithm preparation | Selected source fields archived as provenance | Configured tagging/cleanup | Full current engine analysis | Current validated Lexicon-compatible generation, replace only after explicit cue replacement choice | Current preparation publication rules explicitly disclosed |

“Retag” must not be one ambiguous switch meaning both metadata enrichment and mutation of source music. Represent `metadata_enrichment`, `cleanup`, `analysis_fields`, `cue_generation`, `write_audio_tags`, and `relocation` independently in the saved policy. A preset may set several, but the preview must state them. Default writes and moves are false. Refresh inherits the saved policy and shows it; it must not reanalyse every unchanged track on each scan.

Generated results have separate provenance and an immutable engine/config identity. Imported key/BPM/energy/cues are authoritative observations, never ground-truth predictions produced by CueRithm's DSP and never training labels mixed into analysis inputs. Preserve the validated `lexicon_cues.rs` calculations. Do not restart MIK cue inference. Format support remains MP3, AAC/ADTS, AAC/M4A, ALAC/M4A, FLAC, WAV, AIFF, Ogg Vorbis, Opus and WMA; missing decoders cannot justify discarding metadata.

For Store as imported, an audio file need not be decoded or written to enter the library. Missing/unmounted/streaming records retain metadata and memberships as unavailable references. Lazy waveform/technical decoding later must not overwrite imported analysis. No invented energy, BPM, key or duration when absent. Untyped MIK cues remain selectable source markers; do not coerce them to hot cues simply to fit a current DTO. A new status/provenance representation is needed; do not fake a successful preparation job to satisfy the current state machine.

## Canonical observation model

Use a provider-neutral envelope; do not rename every third-party DB to RekordboxCollection. Proposed logical fields (see `fixtures/normalized-import-example.json`):

- `source`: provider, product version, library identity, schema fingerprint, completeness, snapshot hash, adapter version, read time and selected scope.
- `tracks[]`: opaque external ID, source identity attributes, locations (original and mapped), media availability, typed selected values, raw safe extension fields, parser issues, observation hash.
- `key`: raw text, notation, pitch class C=0…B=11, mode major/minor/unknown, Camelot token or null; never infer a mode solely from a bare number.
- `grid`: original time base and sample rate, representation constant/anchors/full-beats, normalized times, tempo, optional beat indices/meter/downbeat markers and locks. Preserve dynamic tempo, negative pre-roll and original integer positions.
- `cues[]`: external ID, family, source slot, normalized start/end in seconds, original numeric values/time base, name, color, flags, optional source-specific fields. Unknown kind remains an extension, not a guessed hot cue.
- `collections[]`: external ID, parent ID, kind folder/playlist/crate/smart/history/queue, display name and ordered occurrence list. An occurrence has its own identity, track ID and source order; a track may repeat.
- `issues[]`: stable code, source record/field, severity, loss detail, retryability; counts use all source records as denominator.

The JSON Schema fixture validates the envelope shape only. It is not the complete semantic validator. Implement the numeric, clock-presence, relationship, uniqueness and budget invariants below and in `validation-and-acceptance.md` separately; schema success alone is insufficient.

Public fixtures use invented data. Real raw observations live only in the user's local CueRithm data store, never diagnostics uploaded by default. Filter settings/account/license/device-token tables; lossless preservation is scoped to music library records, not a blanket dump of the app profile.

## Native seconds and canonical audio clocks

Source byte-unit conversion and decoded-audio alignment are separate operations. Milliseconds/1000, microseconds/1000000, or frames/sample-rate first recover **native seconds**, which may use a different audio origin from CueRithm's FFmpeg PCM. Store the raw value, time base, source environment and native seconds before any alignment.

Use `clock_status=validated_profile` only when a qualified profile establishes the relation for the exact source app/renderer/codec/rate/bytes/decoder environment. The export relation is `native = canonical + offset`; import subtracts that same qualified offset exactly once. Store finite canonical endpoints plus the timing-profile identity. A profile from another OS, encoder, app build or media revision is not interchangeable. Existing export conversion is evidence to investigate, not proof every source-generated marker uses that clock.

Use `clock_status=unresolved_native` when the relation is unqualified, the source environment is ambiguous, media is missing, or an earlier lossy conversion prevents inversion. Keep all native cues/grids/segments, with canonical endpoints absent, and show their alignment status. Do not discard metadata, synthesize zero offsets, or trigger full analysis to conceal an unknown clock. A separately disclosed lightweight header/probe may qualify timing without replacing imported musical results. Negative-time clamping is not invertible and must be reported.

MIK's static seconds evidence, DJ.Studio's beat/time lookup, and Mixxx's frame conversion establish native units only. See `cuerithm-integration.md` and each adapter's acceptance gates for profile boundaries. For end-only ranges, canonical end can exist while start is absent; validation must not invent a start at zero.

## Identity and path resolution

`(provider, library_instance_id, external_track_id)` identifies a source row. A copied library with the same IDs requires an explicit “same source replica” versus “new library” choice; filesystem path alone is insufficient. Prefer persisted library UUID; otherwise create a local registered identity with source metadata and attach it to the user's selected library. Renaming a playlist does not create a new playlist.

Resolve exact existing source links first. Next resolve an exact canonical path with volume-aware rules, then a verified content identity if explicitly permitted. Filenames/title/artist/BPM are candidate hints only. Never collapse two same-name USB tracks automatically. Full-file SHA256 changes after tag rewriting; distinguish physical file hash, stable source ID and audio-content fingerprint. A tag-only change should not automatically create a second music identity.

Decode file URLs once, preserving Unicode, spaces, `%`, plus signs, UNC/drive letters and case semantics. Use platform paths, not JavaScript string concatenation. Longest-prefix user mappings must respect path component boundaries (`/Music/A` does not match `/Music/AB`). Preview mapping before access; reject traversal outside selected roots for relative archive/USB entries. Retain original path even when resolved path changes. Network/streaming URLs remain reference-only; no implicit downloads. Symlinks and hard links require a documented identity policy. Security-scoped bookmarks are platform handles, not portable paths; preserve only in local private records where appropriate.

## Refresh and three-way merge

Maintain last **observed** source value separately from last **agreed/applied** baseline and local current value. A successful read is not synchronization acknowledgement. Preserve unselected values/baselines. Advance a baseline only for fields actually applied and verified.

| Relative to last applied baseline | Action |
| --- | --- |
| Source unchanged, local unchanged | No-op |
| Source changed, local unchanged | Pull selected source field |
| Source unchanged, local changed | Keep local; an import does not push it |
| Both changed to equivalent normalized values | Keep raw observations and acknowledge equality for that field |
| Both changed differently | Conflict; require chosen source/local/custom result |
| No agreed baseline | Initial import decision; equality alone does not establish historical synchronization |
| Missing field / parser failure | Retain prior local value, show issue; no implicit delete |

Default cues/grid are atomic families for merge, preserving original IDs/slots in observations. An advanced per-cue merge requires explicit ID matching and conflicts on same slot, kind and independently edited position; do not union by rounded timestamp. Preserve hot and memory cues at identical times. Explicit duplicate suppression must retain the original in provenance and report the loss.

Never infer source deletion from a playlist-limited scan, parser failure, missing volume, unavailable file or filtered row. Whole-library successful snapshots may mark a source reference missing; do not delete CueRithm tracks or audio. Source-owned playlist membership refresh may replace that source snapshot only after preview; local-only additions require conflict handling or separate collection ownership. No cross-provider deletions.

Idempotence is defined on normalized values plus source identity, scope, completeness and policy revision. A second identical import creates zero tracks/jobs/cues/playlists and zero file writes. Unknown native fields survive refresh unmodified. Deterministic hashes must include semantic typed values and explicit null/absence, exclude read timestamp, and retain array order where order matters.

## Snapshot, planning and apply protocol

1. Discover with read permissions; record source files and detected schema before selecting an adapter.
2. Prefer SQLite online backup from a read-only connection into a private destination to obtain a coherent database including committed WAL data. A sequential copy of a live DB and WAL is not a guaranteed snapshot. When taking filesystem copies, require source app closed/stable and include sidecars before opening the copy. Never checkpoint or migrate the live source. `immutable=1` can hide WAL and must not be used on an active DB.
3. For multi-file libraries include all consistency-related DBs, metadata files and required audio tag frames. Verify source identities did not change across acquisition; retry or refuse incomplete snapshots. Backup only needed library data; don't read secrets.
4. Validate schema, row/binary budgets, numeric finiteness, relationship integrity and codecs before mapping. Use streaming parsing/pagination; do not allocate from untrusted length fields. For XML disable external entity/network expansion. Treat input as data, never executable SQL or JS.
5. Build immutable preview containing source fingerprint, CueRithm revision, policy hash, scope and all planned field/collection changes. Bind any user decision to that plan ID, not a later rescan.
6. Apply to CueRithm in one transaction, with optimistic revision checks. Recheck media identity if bytes will be touched. Keep long decoding/network enrichment outside DB locks. Jobs use immutable policy/engine/source identities and resume idempotently.
7. Store audit receipt: counts, losses, source/version/schema, plan/policy IDs, applied fields, conflict decisions, created IDs, prior values for undo. Undo affects only this operation and conflicts if a later edit changed the same field.
8. For DJ.Studio write-back, follow its adapter-specific offline snapshot, compare-and-swap and read-back protocol. Multi-file writes require a durable journal and recovery order; an atomic rename of one DB does not make the entire library atomic.

## Errors and progress

Report phases discovery/snapshot/decode/preview/apply/optional-processing and counts processed/total where known. Cancellation during preview is immediate; transaction rollback is mandatory during apply. Show per-track/parser issues with retry actions and aggregate redacted summaries. Unsupported schema disables write and falls back to supported read-only fields where that is provable. A parser succeeds only when every input record is either represented or accounted for by a typed issue; silent dropped rows fail acceptance.

## Proposed adapter capability response

Return source/version/schema, `can_read`, `can_write`, and a field matrix with `read`, `write`, `lossless`, `reason` and evidence/profile identifier. Include `scope` support (all/playlists/folders), `snapshot_kind`, and `runtime_acceptance` separately. DJ.Studio static write code is evidence for an experimental candidate, not `can_write=true` in a production UI. Reuse a negotiated profile only for matching app/schema identities; a broad application version prefix is insufficient.
