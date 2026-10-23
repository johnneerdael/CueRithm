# `.cuerithm` version 1 format and restore protocol

All requirements here are **Decision**, not an existing file-format claim.
The extension identifies a CueRithm library archive. It provides portability and
validation; it does not encrypt the user's library.

## Container and version negotiation

Use an ordinary ZIP/ZIP64 with local-file magic `PK`, extension `.cuerithm`, and
media type `application/vnd.cuerithm.library+zip`. Accept only Stored or Deflate
entries. Do not add a custom binary prefix. Exactly one root `manifest.json` and
the manifest-listed payload entries are allowed. No nesting in an arbitrary folder.

The UTF-8 manifest is a JSON object with these required fields:

| Field | Type / meaning |
|---|---|
| `format` | Literal `cuerithm-library` |
| `version` | Object `{major:1, minor:0}`; nonnegative integers |
| `min_reader` | Object `{major:1, minor:0}` |
| `archive_id` | UUID string identifying this export |
| `library_id` | UUID string identifying the logical library, retained on restore |
| `created_at` | RFC 3339 UTC timestamp |
| `producer` | Object with `app`, `version`, `source_schema` integer, `platform` (`macos|windows|linux`) |
| `required_features` | Array of strings; v1 requires `logical-records-v1`, `portable-paths-v1`, `accepted-musical-data-v1` |
| `contains_music` | Literal `false` |
| `entries` | Array of entry descriptors below |
| `excluded` | Array of domain/reason objects, including counts where applicable |
| `extensions` | Object; empty permitted; keys reverse-domain namespaced |

Each entry descriptor has `name`, `kind`, `bytes` (uncompressed unsigned integer),
`records` (unsigned integer), `sha256` (64 lowercase hex characters),
`schema` (literal `cuerithm-logical-v1`) and `required` boolean. Hash exact
uncompressed bytes, not reserialized JSON. Counts and lengths are checked, not
trusted. All required payload kinds listed below appear once, even if empty.
Unknown major/min-reader/required feature fails before changing the library.
A newer minor is readable only if all required features and required kinds are
known. Unknown optional payloads are retained as bounded inert extension data and
re-exported; do not claim to interpret them.

Manifest itself is not self-hashed. ZIP CRC plus per-entry SHA-256 detects damage,
not malicious authorship; structural and semantic validation is still mandatory.
No signature, secret or license key is needed to export or restore.

[portable-v1-examples.json](portable-v1-examples.json) supplies a complete minimal
archive as manifest plus exact payload strings, including checked byte counts and
SHA-256 values, and an independently authored populated musical-revision example.
It is format reference data, not an exported user library or implementation test.

## Payload grammar and domain records

Payloads are UTF-8 JSON Lines, LF terminated, one object per line, no blank lines,
BOM, duplicate JSON keys, NaN or infinity. IDs are strings throughout; encode
large foreign integer IDs as decimal strings, avoiding JavaScript precision loss.
All epoch timestamps are integer milliseconds. All audio times are finite seconds.
JSON null means unknown; empty text is not inferred to be a known value.

Every line has `id`, `revision` (positive integer), `value` object and `extensions`
object. Nested referenced IDs must exist in the same library. Export order is
stable by bytewise ID, except arrays whose order is semantic. `revision` is the
domain revision, not a SQLite rowid. Import preserves it as historical provenance
and initializes current optimistic counters consistently in the new generation.

| Required entry (`kind`) | `value` fields and invariants |
|---|---|
| `data/roots.jsonl` (`roots`) | `label`, `source_flavor`, `source_root` exact representation, `case_rule: sensitive|insensitive|unknown`. Never auto-bind by label. |
| `data/locators.jsonl` (`locators`) | Tagged union defined below: local, streaming or unsupported reference. |
| `data/settings.jsonl` (`settings`) | `portable_options` allowlisted preparation preferences, `historical_revision`, `model_identity`, `created_at_ms`, `selected` boolean. One selected record. Native model paths and watch bindings omitted. |
| `data/tracks.jsonl` (`tracks`) | Fields defined below; IDs retain track identity. |
| `data/musical-revisions.jsonl` (`musical_revisions`) | `track_id`, `origin`, `created_at_ms`, `selected`, `key`, `bpm`, `energy`, `duration_seconds`, `cues`, `tempo_markers`, `source_revision`; complete history preserved. Exactly one selected revision when any exist. |
| `data/analysis-revisions.jsonl` (`analysis_revisions`) | `track_id`, `created_at_ms`, `schema`, `engine_identity`, `settings_revision`, `model_identity`, `result`, `selected`. Result is versioned `FileAnalysis` data without embedded audio or native file/model paths. Unknown analysis schema is preserved, marked unusable for playback calculations. |
| `data/tag-categories.jsonl` (`tag_categories`) | `name`, `position` integer, `color` nullable string; unique ID, names may collide in foreign data. |
| `data/tags.jsonl` (`tags`) | `category_id`, `name`, `position`, `shortcut` nullable; ID is authoritative, not display name. |
| `data/track-tags.jsonl` (`track_tags`) | `track_id`, `tag_id`; unique pair; deterministic composite ID. |
| `data/playlists.jsonl` (`playlists`) | `parent_id`, `kind:folder|playlist|smartlist`, `name`, `position`, `favorite_position` nullable, `deleted` boolean, `deleted_by` nullable ID, `definition` nullable typed SmartDefinition, `source_definition` nullable inert source JSON, `definition_status:active|unsupported`; references described below. |
| `data/playlist-members.jsonl` (`playlist_members`) | `playlist_id`, `track_id`, `position`; ordinary membership only, unique pair and position. |
| `data/preferences.jsonl` (`preferences`) | Exactly one `id=library`; `key_mixing_mode`, `time_zone`, `display_options`, `source_smart_options`; typed allowlist, no arbitrary application settings. |
| `data/import-sources.jsonl` (`import_sources`) | `vendor`, `source_library_id`, `profile`, `archive_sha256` nullable, `imported_at_ms`, `source_schema` object; applies to Lexicon and historical DJ source observations. |
| `data/external-links.jsonl` (`external_links`) | `track_id` nullable, `source_id`, `record_kind`, `record_id`, `observation`, `baseline`, `source_locator_id` nullable, `binding_status:inactive`. Bounded sanitized source observations and aliases only. |

Use namespaced `extensions` for unsupported fields, including original field
names and exact bounded JSON types. An absent required field is an error, not a
default. Fields explicitly described as nullable must be present with null.
Portable schema validation must precede target DB projection.

Tracks have these required fields:

- `locator_id`, `origin` (`cuerithm|lexicon`), `source_id` nullable,
  `source_record_id` nullable, `state` (`ready|incoming`), `archived` boolean,
  `archived_at_ms` nullable, `created_at_ms`, `updated_at_ms`.
- `content`: `sha256` nullable, `size_bytes` nullable unsigned integer,
  `observed_mtime_ns` nullable decimal string. Mtime is a hint, never identity.
- `metadata`: text-list fields `title`, `artist`, `album`, `album_artist`, `genre`,
  `comment`, `grouping`, `label`, `remixer`, `mix`, `producer`, `composer`, `lyricist`;
  each is an ordered string array, including empty arrays.
- `attributes`: `rating` nullable 0–5 integer, `release_year` nullable positive
  integer, `release_date` nullable original ISO date text, `track_number` and
  `disc_number` nullable nonnegative integer, `color` nullable string,
  `play_count` nullable nonnegative integer, `last_played_at_ms` nullable,
  `extra1`, `extra2` nullable strings. Preserve original invalid/unknown source
  values in extensions and report them; never invent a valid value.
- `media`: `codec`, `sample_rate`, `channels`, `bits_per_sample`, `bitrate_kbps`
  all nullable, plus `tags` string map and `properties` bounded JSON object.
- `analysis_status`, `selected_musical_revision_id` nullable,
  `selected_analysis_revision_id` nullable, `historical_preparation` containing
  bounded tagging/cleanup provenance with local operation paths removed.

Canonical musical fields belong in musical revisions, rather than independent
conflicting copies. Database scalar columns are projections of the selected
revision. A track with no accepted values has a null selected musical revision.
Imported musical revision ID is distinct from DSP implementation identity.

For musical revisions, `origin` is `cuerithm|lexicon|manual`; `source_revision` is
nullable text, `created_at_ms` integer and `selected` boolean. `key` is null or
`{original:string,pitch_class:integer|null,mode:major|minor|null}`; pitch class is
0=C through 11=B. Unknown key text has null pitch/mode. `bpm` is null or positive
finite number, `energy` null or integer 1–10, `duration_seconds` null or nonnegative
finite number. The complete cue/map object types are specified below. All these
fields are required, with null for unknown values. Zero is not a known BPM/energy.

For native observed sources, export one import-source per `native_targets` row:
`id` is its existing stable ID, `vendor=rekordbox`, `source_library_id` is its
database ID, `profile=rekordbox-observation-v1`, `source_schema` contains the
schema fingerprint, `archive_sha256=null`, and `imported_at_ms` is its created
timestamp. Project `native_track_links` into external-links, retaining content ID,
local track ID, baseline and observation. Use provider DTO field allowlists; no
arbitrary native-record JSON export. Legacy `sync_links` gets a deterministic
UUIDv5 source ID from its target string, `profile=legacy-sync-link`, with the same
inactive treatment. Raw native_records are excluded/reported when no vetted
field allowlist exists. No external-link source_id may dangle.

### Paths, roots and local resolution

`source_flavor` is `posix|windows-drive|windows-unc|opaque-posix|opaque-windows`.
This field applies only to local locators. Local locator fields are `kind=local`,
`source_flavor`, `original`, `root_id` nullable and `components` nullable. Streaming
locators contain only `kind=streaming`, `service` and `remote_id` strings; filesystem
fields are forbidden. Unsupported locators contain only `kind=unsupported`,
`source_kind` string and `source_value` bounded inert object, never a local binding.
Unknown Lexicon track kinds map to the latter and remain visible/unavailable.
`original` / `source_root` is `{encoding:utf8|base64-bytes|base64-utf16le, data}`.
UTF-16LE bytes must have even length, and unpaired code units remain opaque.
POSIX bytes need not be valid UTF-8. Preserve exact source bytes/code units;
lossy replacement characters are forbidden. Ordinary Unicode path components
are strings without separators, NUL, `.` or `..`. Opaque components use the same
encoding object form. An unparseable original has null root/components and is
retained unresolved. A rooted locator must not escape its declared source root.

Validate decoded opaque components too: reject NUL, `/` for POSIX, `/` or `\`
for Windows, exact dot/dot-dot and absolute/device prefixes in either encoding.
Before binding, walk destination components without following symlink/reparse
parents and verify the resulting regular-file handle remains under the selected
root identity. Repeat containment/file proofs at Apply. Retaining an unsafe
original as inert provenance does not authorize joining it into a local path.

Roots represent separately mapped source subtrees, not merely drive letters.
Examples are POSIX `/Music`, Windows `D:/Music` and UNC `//server/share/Music`.
Drive-relative `D:track.mp3`, relative paths and device namespaces are invalid
binding inputs; retain their original form as unresolved import evidence.
Never interpret `D:` on POSIX as a directory or convert UNC into a web URL.

Restore combines an explicitly chosen destination root with relative components.
On the destination, probe actual case and normalization behavior and detect
collisions. Linux `A.wav` / `a.wav` may not both bind on a case-insensitive volume.
POSIX `:` or Windows reserved basenames/trailing dots are not silently renamed.
Keep unrepresentable paths unresolved and offer manual file selection. Do not
split a valid POSIX filename containing a backslash as a Windows path.

Local destination bindings, security bookmarks, volume IDs and access grants are
machine state. They never ship in the archive. Source roots and exact source
locators do ship. Same-machine restore may propose existing roots but requires
confirmation before accessing them. Foreign roots never become active watchers.
Portable locators retain the canonical source path across repeated migrations;
export preserves them alongside any explicitly accepted permanent relocation.

### Cues and tempo maps

Each cue has `id`, `kind:cue|fade_in|fade_out|load|loop|unknown`, `start_seconds`,
`end_seconds` nullable, `slot` nullable nonnegative integer, `memory` boolean,
`name` nullable string, `color` nullable string, `active_loop` boolean, `source_data`
bounded object. Retain signed finite start times; negative lead-in is valid stored
data. Clamp only a playback seek to zero, with a displayed inspection notice.
Do not shift all imported cues. Loops require end > start; invalid loops are
retained inert with a warning, not executed. Unknown kinds remain visible/inert.
Preserve coincident cues; sort for display by time, slot, source ID.

Each tempo marker has `id`, `start_seconds`, `bpm` positive finite,
`beat_number` nullable positive integer, `beats_per_bar` nullable positive integer,
and `source_data`. Sort by start time then source ID; duplicate anchors with
different tempos are an import conflict requiring review. Retain all markers.
For beat projection, tempo is piecewise constant until the next marker; anchors
are authoritative. Unknown beat phase stays unknown. Never assume that every
anchor is beat one or flatten to the first BPM. Existing fixed-grid publications
adapt to one marker without changing their cue calculations.

### Smartlists and external links

Native SmartDefinition keeps exact typed values, grouping, rule IDs and stable
playlist/tag IDs. Paths within a playlist-reference value are display provenance;
ID is authoritative. Validate graph cycles and missing IDs. Rebuild evaluated
memberships after activation; imported unsupported definitions stay disabled with
their raw definition preserved. Do not turn an unsupported condition into true.

For active native definitions the frozen wire shape is
`{match_all:boolean,rules:[{id:string,field:string,operator:string,value:object,or:boolean}]}`.
`value` is tagged by `kind` and `value`: `none` has no value; `text` string;
`number` finite; `number_range:{start,end}` finite; `date` epoch-ms integer;
`date_range:{start,end}` epoch-ms; `relative:{count:integer,unit:days|weeks|months}`;
`color` string/null; `ids` string array; `playlist:{id,kind:playlist|folder|smartlist,
path:string[]}`. Operator spelling and field IDs are frozen to the native model
at baseline `673c017` in `crates/cuerithm-smartlists/src/model.rs`; unsupported
later values must be preserved disabled rather than interpreted as v1 operators.
Max 128 active rules, unique rule IDs 1–128 bytes and field IDs at most 256 bytes.

Retain scoped external IDs and acknowledged baselines as historical observations,
with machine paths converted to locator references. Export only provider-allowlisted
record fields, not whole unknown JSON blobs that might contain secrets. Record
omissions in `excluded`. All bindings remain inactive after restore, even on the
same machine; user must re-observe and explicitly bind an accepted DJ profile.
Restoration is never authorization to write to a DJ library.

## Include/exclude and settings policy

Preserve tracks including Incoming/archived records, selected and historical cue/
musical revisions, real analysis and engine provenance, playlist tree/favorites/
tombstones, custom tags, portable settings/preferences and scoped external IDs.
Resolve historical cue ordering into explicit selected revisions; never depend
on equal timestamps plus SQLite row insertion order after restoration.

Exclude music, PCM snapshots, album art/waveform caches, FTS/search indexes,
materialized smart results, scan results/previews, operation locks, jobs and
current-job pointers, executable undo/edit history, retained original audio,
in-progress file journals, credentials, subscriptions/licenses, tokens, host paths,
device/audio selections, keychain references, watch/output/backup folders and
model executable paths. Omit associated operational rows, not only their files.
Keep user-facing counts/reasons in the manifest and preview.

Playlist tombstones remain, but `last_edit_id`, `deleted_by` and old edit references
must be rewritten to non-executable imported provenance if their operational edit
rows are excluded. Do not create dangling foreign keys. Historical settings
revisions are immutable provenance. Derive a new active local settings revision
after rebinding bundled models; omit watch bindings and disable automatic tagging,
cleanup and external writes until explicitly configured locally.

Portable options allowlist: analysis/key/BPM/cue preferences, tagging provider
selection without auth, cleanup rules, auto-playlist definitions and display
preferences. Any field capable of opening a path, running a process, requesting
an authenticated service or enabling a filesystem mutation is excluded from
active restoration. User-created rules are validated as data, never executed.

### Closed v1 settings and provenance value types

The following freezes the wire types; do not serialize arbitrary runtime config.
Unknown object keys go only in explicitly inert `extensions`, or fail validation.
`analysis_status` is exactly `complete|imported|not_analyzed|stale`.
Historical revision IDs are nullable strings. `model_identity` is an array of
`{name:string,sha256:string|null}`; it contains no model paths. Engine identity is
`{name:string,version:string,implementation_sha256:string|null}`. Analysis schema
`cuerithm-file-analysis-v1` freezes the path-free FileAnalysis representation from
`crates/camelot-core/src/engine.rs` at baseline `673c017`; implement the versioned
adapter rather than serializing future runtime structs unchanged. Restore unknown
analysis schema as inert data; selected musical data remains independently usable.

`portable_options` contains exactly:

- `mik_style_energy`: boolean; `stability_millis`: integer 10–60000.
- `analysis`: null or object with `sample_rate` positive integer Hz, `config`,
  `structure_features` boolean, `source_profile_timing` boolean, `lexicon_timing`
  boolean, `lexicon_energy` boolean, `lexicon_tempo` nullable
  `{min_tempo:number,max_tempo:number}` with 50 ≤ min < max ≤ 300 and max − min ≥ 20,
  `lexicon_config` nullable object and `cue_template` nullable object. No max_seconds
  cutoff restores into full-track analysis. `config` has integer FFT sizes
  `key_frame_size`,`rhythm_frame_size` (powers of two 256–32768), integer hops
  `key_hop_size`,`rhythm_hop_size` (1..corresponding frame), `min_bpm`,`max_bpm`
  finite with 30 ≤ min < max ≤ 300. Validate sample rate with bundled engine support
  before local activation; unsupported values remain historical and use defaults.
- `lexicon_config`, when present, has `genre:string|null`,
  `drop_at_start:Auto|Never`, `min_cue_length_beats` positive integer,
  `add_emergency_loop` boolean. `cue_template` uses the data-only CueTemplate v1
  representation described below.
- `cleanup`: null or `{fixes:array,genres:object,artists:object}` using the frozen
  CleanupConfig v1 representation below.
- `tagging`: null or `{providers:array,no_match:keep_existing|attention,
  analysis_tag_source:measured|catalog,write_energy:boolean,strictness:number,
  match_duration:boolean,max_duration_difference:integer,match_by_id:boolean,
  short_title:boolean,merge_genres:boolean,album_tagging:boolean,
  album_tagging_ratio:number}`. Providers are unique `beatport|traxsource`;
  strictness/ratio are 0..1; duration difference is nonnegative seconds. No unknown
  provider custom object, post-command, Spotify configuration, path, file-art output,
  LRC writer, move setting or auth material is allowed. Other source options are
  excluded and reported, not silently considered restored.

CueTemplate v1 uses camelCase names: `overwrite`, nullable `startCueBehavior`
(`atFirstBeat|atExistingCue|atZero`), nullable `useFirstCueAsStart`,
`breakdownMinBeats` integer 0–4096, `keepCuePositions`, `enableCustomAnchors` booleans,
`dropAtStart` (`auto|never`), `customAnchors` bounded inert JSON object, `rows` array.
Each row has `checked` boolean, `markerType` from `start|drop|breakdown|second_drop|
second_breakdown|lastbeat|emergency_loop|normal`, `name` string, `cueType` from
`hotcue|loop_hot|loop_active|memory_cue|memory_cue_loop`, `color` string/null,
and nullable `label` string, `anchor` marker, `position:before|after`,
`beatsOffset` finite number, `loopDuration` positive finite beats. Max 64 rows.
Custom anchors remain inactive until validated by the local CueTemplate adapter;
unsupported anchor shapes are preserved with warning, never evaluated.

CleanupConfig v1 uses source snake_case field names. Each fix is
`{id:string,enabled:boolean,ignored_values:string[],rule:object}` with max 64 fixes.
Rule is tagged by `kind`:
`title_case{field,uppercase,lowercase}`, `replace_characters{characters}`,
`decode_entities{}`, `extract_artist{separator,part}`, `extract_remixer{word,remove_title}`,
`remove_garbage{field}`, `remove_number_prefix{}`, `remove_urls{field,clear_field}`,
`mix_parentheses{}`, `remove_common_text{pattern}` or `remove_promotional_text{field}`.
Field is one of the text-list metadata fields. Pattern is
`CamelotKeyFromTitle|OriginalMixFromTitle|CamelotKeyEnergyFromTitle|BpmFromTitle`.
Separator/word max 64 characters, artist part 1–256, replacement max 64 characters.
Genres is `{enabled:boolean,mappings:[{from:string,to:string|null}],
protected_values:string[],conversion:null|{category:string,genres:string[],remove_genre:boolean}}`.
Artists is `{enabled:boolean,roles:array,mappings:array,protected_values:string[]}`;
roles are `artist|remixer|producer|composer|lyricist`, mappings share the genre shape.
Arrays representing sets have no duplicates. Rule IDs are unique 1–128 bytes.

Preferences have `key_mixing_mode:harmonic|fuzzy`, `time_zone` valid IANA timezone,
`display_options:{key_notation:original|camelot|open_key|musical}` and
`source_smart_options:{lexicon_key_mixing_mode:harmonic|fuzzy|null}`. No other active
display/source option keys in v1. Auto-playlist definitions are regular smartlist
records, not executable preferences. Defaults for a Lexicon-only/minimal library
are harmonic, chosen import timezone and original notation. Portable backup keeps
the source values.

`historical_preparation` is `{settings_revision:string|null,tagging_summary:string|null,
cleanup_summary:string|null}`; no pending operation path or runnable job. Export
only existing bounded summaries, otherwise null. Richer state is excluded/reported.
An external `baseline` is a map from the text metadata fields plus
`key|bpm|energy|duration|rating|release_year|release_date|track_number|disc_number|
source_path|cues|tags` to `{local:value,remote:value,owner:local|remote|shared|unowned,
acknowledged:boolean}`. Scalar value types follow the track/musical fields;
`source_path` is a locator ID, cues use canonical cue arrays, tags are source-ID
arrays. Unsupported vendor values stay in reported inert extensions. Observation
is null or `{metadata:object,musical:object,source_locator_id:string|null}` using
those same canonical field types; only known fields appear. Rewrite paths into
locators on both sides before export. Historical acknowledgement never becomes
active authority to sync. This is the complete native observation allowlist for v1.

## Consistent export

1. Obtain Host export gate, stop accepting writers and drain current mutations.
   If an unfinished filesystem journal requires recovery, fail with a recovery
   action; do not silently omit a half-applied operation. Do not resume old jobs
   just to export. Stop auto-enqueue while taking the snapshot.
2. Use SQLite online backup into a private scratch DB, then release the short
   snapshot gate. Read the logical snapshot from that copy. This includes committed
   WAL content without copying raw live files. Never hold SQLite locks while
   traversing music or compressing. No audio reads/hashes are required for export.
3. Project the allowlisted domains to logical streams, calculate counts/digests,
   write manifest and archive into an exclusive sibling temporary file.
4. Reopen and fully validate the produced archive, flush file data, then publish
   with no-clobber rename. If a destination exists, use the user's explicit Replace
   decision and retain the previous file until publication succeeds. No automatic
   deletion or retention policy in v1. Cancellation removes only task-owned temp files.

The output contains no SQLite DB and no platform-specific BLOB paths. It is a
logical snapshot of one library revision, not a ZIP rename of the live database.

## Untrusted archive validation

Apply to both `.cuerithm` and Lexicon input. Require a regular selected file;
process in private task-owned storage. Do not extract general entry names.
Stream only allowlisted entries to generated internal filenames. Reject absolute,
drive/UNC, traversal, backslash-separated archive names, NUL, symlink/hardlink,
duplicate or case-equivalent names, overlapping entry offsets, multipart/encrypted
archives, unsupported compression and corrupt CRC/checksums. Bound both declared
and actual expanded bytes. Do not follow archive-provided paths or fetch URLs.

Initial configurable ceilings: manifest 1 MiB; 64 entries; 2 GiB compressed;
8 GiB total expanded; 4 GiB per data entry; 8 MiB per JSONL record; nesting depth
32; 256 rules per imported source definition (native active rules still max 128);
4096 cues/tempo markers/tags per track; 5 million tracks and 50 million relation
records. Abort with a resource-limit report and no active change. Expansion ratio
alone is not a rejection criterion: legitimate repetitive JSON compresses well.
Check free space continuously with a 256 MiB reserve, including source extraction,
staging DB/indexes, validation scratch and retained current generation.

Validate UUIDs/IDs, numeric bounds, enum values, references, unique bindings,
member ordering, tree parent types/cycles and counts before publication. Reject
ambiguous duplicate IDs. One bad required record fails the preview; a diagnostic
may identify it, but no partial “successful restore”. Lexicon compatibility warnings
must be separately approved when lossy projection is permitted.

## Staged replacement and crash protocol

**Decision:** switch closed library generations, never overwrite an open DB.
Introduce an app-data library manager owning `libraries/<generation-id>/library.sqlite`
and a tiny validated `active-library.json` containing generation ID and checksum.
Retain the old generation until explicit user deletion. First upgrade adopts the
legacy library through a consistent copy and leaves the original recoverable.

Preview/read/validate source without activating it. Build the new schema using
CueRithm migrations in a staging generation, insert validated domain data in one
transaction, rebuild indexes and check DB integrity/foreign keys. Import code
never runs vendor migrations, triggers or SQL from the source. For `.cuerithm`,
same logical library ID and record IDs survive; generation ID is always fresh.
For Lexicon, use the namespaced identity derivation in its profile.

Persist activation phases: `staged`, `validated`, `quiescing`, `switched`, `opened`.
Record old/new generation IDs and a digest of the validated staging DB. Finish
all root choices and warnings before Activate. At activation recheck source digest,
plan expiry and current active generation/revision; stale plans need preview again.

Seal staging before that digest: finish the insertion/index transactions,
checkpoint/truncate its WAL, close every staging handle, require no nonempty WAL,
flush the DB, hash its exact bytes, and write the validated journal durably. Never
open it through normal startup recovery before switching. Recheck the sealed
digest and no-WAL condition immediately before pointer replacement, rejecting any
change. After activation normal WAL operation may begin on the new generation.

At the final gate stop new tasks, drain writers, unload playback, stop/join
preparation and smart workers, close every DB handle and release its lock. Flush
the validated generation, then atomically replace the active pointer using the
platform's durable file-replacement primitive. Reopen only the new generation,
start read-only projections first, and mark opened. If opening fails, close it,
restore the old pointer and reopen the old library. No services may access both
generations as active. If services cannot drain/close, abort before switching.

On startup, validate pointers and the activation journal before opening a library.
Before switch, discard/retain incomplete staging and open old. After switch,
open new only if validated; otherwise restore old. Torn/invalid pointers recover
from the journal and retained last-good pointer, never guess the latest directory.
Use POSIX fsync/directory-fsync and Windows replacement/flush semantics behind a
tested platform abstraction; open-handle/antivirus denial is a recoverable error.
Cancellation before switch leaves old active; after switch complete/open or roll
back, then report the final active generation. Never delete music on recovery.

Restored libraries start with no runnable jobs or active watcher/tag/sync bindings.
Allow explicit future configuration. Same-machine rollback is selecting a retained
generation under the same quiescence protocol; it is not merging its later edits.
