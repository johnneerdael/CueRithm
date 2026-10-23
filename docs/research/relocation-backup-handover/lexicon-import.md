# Lexicon database-backup import profile

## Supported input and evidence boundary

**Seen:** the user-selected `21082925.zip` is a 63,829,065-byte ZIP containing
only root `main.db` (77,746,176 bytes), a normal SQLite 3 database. SHA-256:
`3700383645a5ed2de110f8651afe2059632f9375bc7d8091ce38c4c4dc84232f`.
`Database.version=26`; `PRAGMA user_version=0`. Use the former plus structural
schema validation, not SQLite user_version, to identify this profile.

**Decision:** initial compatibility is `lexicon-main-db-v26`. Validate required
tables/columns and storage classes, permit unknown nullable/additive columns as
inert extensions, and reject other `Database.version` values with an informative
unsupported-profile error. Do not promise all historical/future Lexicon backups.
The previously encountered older backup is excluded from this delivery's evidence.
An engineer can add another profile from a separately supplied fixture later.

The ZIP/SQLite decoder must not require Lexicon installed, a license, login,
network access, Python, Electron or vendor libraries.

The machine-readable [profile inventory](lexicon-v26-profile.json) gives all 93
required columns, declared nullability/key positions, foreign keys and the six
known inert view/trigger fingerprints. It contains schema facts only, with no
user records or vendor implementation. It supplements the semantic mappings below.

## Reader protocol

Use the common archive limits in `portable-format.md`. For this input require one
root `main.db`; tolerate only directory entries and `__MACOSX/` resource metadata
that are ignored without extraction, subject to the same bounds. Reject a second
database, case alias, WAL/SHM, encrypted payload, unexpected active file or nested
alternative database. Do not accept a live database with an uncopied WAL.

Copy only `main.db` to a generated private staging filename. Check SQLite magic,
set query-only/read-only mode, disable extension loading, enable untrusted-schema
defenses and resource limits before querying. Do not call CueRithm `Library::open`
on it. Select explicit columns from verified ordinary tables; views, triggers and
source SQL are never executed or copied. Unexpected views/triggers are rejected.
The six objects in `lexicon-v26-profile.json` may be present only with matching
kind/name/table/normalized-SQL digest; never query those views or invoke triggers.
Known FTS virtual/shadow tables are ignored and never queried. Run integrity and
referential checks in a bounded
worker. Reject malformed required rows, duplicate IDs, orphan references and
cycles before any active-library change.

The profile's `inert_fts_tables` lists `Track_FTS` and its four observed shadow
tables with normalized-SQL fingerprints. Permit only these recognized virtual/
shadow objects, never every table with an FTS-looking name. They are optional
inert indexes for import; no virtual-table query or reconstruction is required.

All table/column names below are format facts, not code to copy. Primary IDs are
SQLite integers; decode into checked signed 64-bit values and convert to decimal
strings immediately. Do not route IDs through a JS number.

Read `Database` as exactly one row with integer `version` and nonempty bounded
text `uuid`. The latter is opaque source-library identity, not necessarily a UUID.
Never substitute a CueRithm library UUID directly from it. Derive source-library
ID using UUIDv5 with the standard URL namespace and the exact UTF-8 name
`urn:cuerithm:lexicon-library:` followed by that opaque identity. Then derive each
track/playlist/tag/category/cue/marker ID using UUIDv5 with that source-library
UUID and `<table>:<decimal-id>`. If identity is absent, the profile is unsupported;
do not silently choose a filename-based identity.

Store archive digest and source profile separately. Importing the same backup as
a new replacement deterministically preserves source IDs; it never duplicates rows
in an existing library because merge is unsupported. Two backups from the same
source DB retain record identities without implying that their changes can merge.

## Table inventory and loss policy

The selected sample's aggregate counts are **seen**, not expected constants for
other libraries:

| Table | Sample count | Treatment |
|---|---:|---|
| `Track` | 894 | Every row becomes an accepted track/reference. |
| `Cuepoint` | 5,388 | All cues preserved; 73 have a negative start time. |
| `Tempomarker` | 894 | All markers preserved; do not assume one per track for other input. |
| `Playlist` | 1,143 | 316 folders, 8 ordinary playlists, 819 smartlists. |
| `LinkTrackPlaylist` | 434 | Ordered ordinary membership and any source membership evidence. |
| `TagCategory` / `Tag` | 4 / 76 | Preserve taxonomy even when no assignments exist. |
| `LinkTagTrack` | 0 | Reader still supports relationships; sample does not prove this path. |
| `Setting` | 97 | Only explicit safe preference translation; see below. |
| `Waveform` / `AlbumartPreview` | 894 / 18 | Ignore regenerable caches; report exclusion. |
| `HistorySession` / `HistoryTrack` | 0 / 0 | Preserve as bounded inert import extensions if present; no playback-history UI promised. |
| `ChartItem` / `CloudFile` | 0 / 0 | Ignore with report; never restore cloud authorization or download queues. |
| `Track_FTS*`, `sqlite_*` | Implementation details | Ignore indexes/statistics/internal sequencing. |

Sample paths are all POSIX; all tracks are local, all cues are ordinary cue kind,
and all marker `data` objects are empty. Loop, streaming, Windows-path, variable-
tempo, custom-tag-assignment and nonempty timing-data behavior requires synthetic
fixtures and later real examples. Do not label those sample-tested.

## Track mapping: every source column

Required source columns for v26 are the following. SQL declared type is advisory:
`duration` is declared INTEGER but the sample has fractional REAL values. Accept
finite numeric storage where semantics permit; never truncate to declaration.

| Source column(s) | CueRithm logical target / conversion |
|---|---|
| `id` | Namespaced track ID; preserve decimal source-record ID. |
| `location` | Exact original portable locator, parsed by its syntax; no host canonicalization. |
| `locationUnique` | Inert source uniqueness evidence only. Never use Lexicon's case-folded key as CueRithm identity. |
| `type` | Text `0` local, `1` streaming. Unknown kind preserved as unsupported reference. |
| `streamingService`, `streamingId` | Streaming locator service/remote ID; no generated local path. |
| `title`, `artist`, `albumTitle`, `genre`, `comment` | Text-list metadata `title`, `artist`, `album`, `genre`, `comment`. A nonempty scalar becomes one array item; do not split artists/genres on punctuation. |
| `remixer`, `mix`, `label`, `composer`, `producer`, `grouping`, `lyricist` | Same named text metadata (`mix` remains `mix`), scalar-to-one-item array. |
| `key` | Parse Camelot/Open Key/musical notation; preserve original text and normalized pitch/mode if recognized. Unknown nonempty keys stay visible with a warning, never become a prediction. |
| `bpm` | Accepted imported BPM when finite >0; zero means unknown. Tempo-map BPM is independent and authoritative for grid projection. |
| `energy` | Imported whole-track rating 1–10; zero unknown; retain out-of-range originals inert with warning. Never attach it to individual cues. |
| `duration` | Finite seconds, fractional precision retained; zero/negative unknown with source retained. |
| `rating` | Integer 0–5 accepted; 0 means unrated. Unknown/out-of-range retained and warned. |
| `playCount`, `lastPlayed` | Play count and optional source date/time; explicit timezone handling below. |
| `year`, `releaseDate`, `trackNumber` | Attributes; zero year/number means unknown. Do not infer a disc number. |
| `color` | Color string retained; empty means none. Unknown values visible as unsupported color. |
| `bitrate`, `sampleRate`, `sizeBytes` | Technical metadata: kbps, Hz, bytes respectively. Zero unknown. Re-probe only after user resolves a local file; imported values are not verification. |
| `dateAdded`, `dateModified` | Source creation/update timestamps, plus original source strings. |
| `archived`, `archivedSince`, `incoming` | Independent archived flag/date and Incoming intent. Restored Incoming is not auto-enqueued. |
| `extra1`, `extra2` | Dedicated extra attributes; retain exact strings. |
| `danceability`, `popularity`, `happiness` | Namespaced numeric source extensions, available for later display/rules; do not reinterpret as energy. |
| `importSource`, `beatshiftCase`, `fingerprint`, `data` | Inert provenance/extensions. `data` must be a bounded object. Do not treat fingerprint as encoded-file SHA-256 or beatshiftCase as an instruction to shift cues. |

There is no source `album_artist`, `disc_number` or `channels` column in this
profile. Those stay unknown unless safely represented in preserved source data;
do not read audio tags automatically to fill them during import.

Lexicon streaming service codes seen statically: `0` Beatport LINK, `1` Beatsource
LINK, `2` SoundCloud, `3` Tidal, `4` Spotify, `5` Deezer, `6` Apple Music, `7` iTunes.
Store code and label; these values do not grant playback access.

Use declared offset when source time text has one. For offsetless values, preview
requires an import timezone (propose current library timezone); interpret there,
retain the original string/timezone and report DST ambiguity. Never silently use
the host's current timezone for historical data. Invalid dates remain null with
source preserved and warnings. Do not infer source machine OS from a timestamp.

## Cue and tempo schema and mapping

`Cuepoint`: `id`, `trackId`, nullable `name`, `type`, `startTime`, nullable `endTime`,
`position`, nullable `color`, nullable `activeLoop`, nullable JSON `data`.
`Tempomarker`: `id`, `trackId`, `startTime`, `bpm`, nullable JSON `data`.

Times are seconds. Source cue type text is `1` cue, `2` fade-in, `3` fade-out,
`4` load, `5` loop. Preserve all; unsupported playback kinds remain inert markers.
`position` is zero-based; preserve an out-of-normal-range slot in source data and
mark it unassigned if the UI cannot expose it. Do not silently limit imports to
eight cues. `activeLoop` is boolean/null; only a valid loop may execute it.
Memory-cue evidence is `data.rekordbox.originalNum == -1`; retain that object and
map to explicit `memory=true`. When no memory evidence exists, map to ordinary
slot cue. Source palette values include `yellow`, `red`, `violet`, `blue`, `orange`,
`green`, `magenta`; preserve the string, never guess a vendor packed integer.

Store all tempo markers, ordered by time then ID. Do not substitute Track.bpm for
an existing map. Preserve unknown/nonempty marker `data`; no beat phase is proven
by the selected sample. `beat_number` and `beats_per_bar` remain null unless a
separately documented source field proves them. If no markers exist but Track.bpm
exists, display BPM without fabricating a grid anchor. Imported cue visualization
must work without a grid and without a complete DSP FileAnalysis.

## Playlist tree, membership and tags

`Playlist` columns: `id`, `name`, `dateAdded`, `dateModified`, `type`, nullable
`folderType`, nullable `smartlist`, nullable `parentId`, `position`, nullable `data`.
Types: `1` folder, `2` ordinary playlist, `3` smartlist. Folder subtypes statically
include root `1`, Lexicon `2`, mobile import `3`, DJ playlists `4`.

Preserve every row including empty folders and source root. Put a source root
under CueRithm's library tree rather than treating it as the destination's hidden
system root. Parents must be folders; missing parents/cycles fail preview. Preserve
sibling order by `(position,id)` then compact to nonnegative positions. Preserve
original positions/dates/folder type in source extensions. Duplicate names are valid.

`LinkTrackPlaylist`: `id`, `playlistId`, `trackId`, `position`. Retain ordinary
playlist order sorted by `(position,id)`. Current CueRithm allows one occurrence
per track: keep the first when source repeats the same track and report each
collapsed occurrence before activation. The selected sample has none. Relations
on folders are invalid required data. Smartlist relations are source snapshots,
preserved inert for comparison; active membership is recomputed, not installed as
ordinary members. Empty unsupported smartlists remain visible, not quietly deleted.

`TagCategory`: `id`, `label`, `position`, nullable `color`.
`Tag`: `id`, `categoryId`, `label`, `position`, nullable `shortcut`.
`LinkTagTrack`: `id`, `tagId`, `trackId`.
Import by ID, preserving ordering/color/shortcut as logical fields. Existing name
uniqueness constraints need a deliberate migration to ID-based taxonomy where
names collide; do not concatenate or rename labels silently. Duplicate identical
link pairs collapse with warning; missing IDs fail validation. No default tag
taxonomy is injected ahead of the source categories.

## Smart-definition conversion

Source JSON is `{matchAll:boolean, rules:[{field,operator,values,or}]}`. Generate
stable rule IDs from playlist ID and source rule index. CueRithm's SmartDefinition
uses `match_all` and typed `value`; operator spellings are case-sensitive.

The selected sample uses six operators across `key`, `bpm`, `remixer`, `artist`,
`genre`, `comment`, `energy`. These must import as active definitions:

| Source | Native RuleValue | Required semantics |
|---|---|---|
| `StringContains` | `text` from `values[0]` | Lowercase and trim track text; lowercase criterion; empty criterion false. |
| `StringEquals` | `text` | Lowercase/trim track text equals lowercase criterion; criterion is not implicitly trimmed. |
| `StringEndsWith` | `text` | Same text normalization as Contains; empty criterion false. |
| `NumberBetween` | `number_range` from first two values | Inclusive range with endpoints ordered; missing/zero track value false. |
| `NumberGreaterThanEquals` | `number` | Inclusive threshold; missing value treated as zero in source numeric comparison. |
| `KeySimilar` | `text` | Parse notation and use preserved source key-mixing mode; unknown keys false with a warning. |

For BPM numeric rules use its two-decimal comparison value. Energy uses the whole-
track rating. Existing `crates/cuerithm-smartlists` implements these operator names;
characterize against this table, not merely a string rename. Its complete typed
value model is in `src/model.rs` (`RuleValue`, `SmartRule`, `SmartDefinition`).

Grouping: with no rules match nothing. `matchAll=false` means any rule. Otherwise,
when two or more rules have `or=true`, require every unflagged rule AND at least
one flagged rule; with fewer, require every rule. Exclude archived tracks unless
an explicit archived/BooleanTrue rule opts in. The sample has no flagged OR groups;
add synthetic coverage rather than claim sample validation.

Allowlist all further operator mappings individually. Number, text, date, boolean,
color, tags, tag categories, cue predicates and playlist-reference operators can
reuse the native rule types only when values and semantics validate. Translate
playlist signatures to stable source IDs (the source path separator is `%%` and
ID separator `%!#!%`), tag/category integer IDs to namespaced IDs, and dates to
milliseconds with the selected timezone. Reject cycles. `KeySimilar` defaults to
harmonic when no safe source preference exists; preserve that assumption in the
preview. Do not inherit a destination library's fuzzy mode silently.

An unknown field/operator, unresolvable reference, unsupported value, unsafe regex,
or more than 128 native rules disables the entire definition, preserves its raw
JSON and emits a per-playlist warning. Do not drop only the offending rule and
activate a broadened playlist. Raw source snapshots may be offered as an explicit
ordinary-playlist conversion only when complete membership evidence exists.

## Settings, unsupported data and activation

`Setting` has `id`, `feature`, `name`, nullable JSON/text `data`. Do not deserialize
all settings as active CueRithm configuration. Read an explicit allowlist for key
notation/key-mixing mode when a validated adapter exists; otherwise use the stated
harmonic default and current display defaults. Ignore auth/subscription/cloud,
watcher paths, provider configuration, delete policies, hotkeys, device settings,
scripts/plugins and sync state. Do not preserve potential secrets as raw extensions.
Report setting keys/counts excluded; never dump their values into diagnostics.

Produce an import report: supported profile, source table totals, accepted tracks,
unresolved paths, cue/marker totals, ordinary memberships, active/disabled smartlists,
tag taxonomy/assignments, every dropped/cache/setting domain, collapsed links,
unsupported marker kinds, date assumptions and preserved inert fields. If there is
any representational loss, Activate requires acknowledging that report. “Imported”
never implies local file availability or complete CueRithm DSP analysis.

Build a new CueRithm generation using the shared staging protocol. No vendor DB
replace, vendor migration, file-tag write, audio move, preparation job or external
DJ write occurs. After activation the source ZIP and installed Lexicon stay intact.
