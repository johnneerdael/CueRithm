# Cuerithm library workspace — proposed design

Status: written specification approved on 2026-10-18; common-format coverage
clarified afterward as a first-delivery requirement. Implementation plan in progress.
Date: 2026-10-18.

## Purpose and confirmed requirements

Build a desktop music-preparation and library-management application combining
Cuerithm's analysis, Lexicon's requested library features and OneTagger's
Beatport/Traxsource auto-tagging. The stack remains Tauri 2, React/TypeScript
and Rust, with a local SQLite library. Support macOS, Windows and Linux.

The user approved:

- A library/playlist sidebar, central track table and persistent waveform/cue
  panel, using OneTagger's charcoal/teal visual direction.
- Dedicated library cleanup and direct rekordbox sync views.
- Automatic preparation from watched incoming folders and drag/drop imports.
- All preparation/tagging settings configured persistently in Settings.
- Automatic promotion from Incoming into Tracks only after complete preparation.
- Full analysis and generated cues replacing existing cues during preparation.
- Key, BPM, beatgrid, structural cues and one energy rating per track.
- Beatport and Traxsource auto-tagging; other OneTagger tools are outside scope.
- Smart Fixes, Genre Cleanup and Artist Cleanup.
- Ordinary playlists and full Lexicon smart-playlist behavior as a tracked
  parity target.
- Direct rekordbox 6/7 synchronization only. No XML mode or fallback.
- Common audio formats from the first usable delivery: MP3, AAC (including raw
  ADTS and AAC in M4A), ALAC in M4A, FLAC, WAV, AIFF, Ogg Vorbis, Opus and WMA.

This specification is a proposed implementation contract, not a claim these
new application features are already built. Evidence is recorded separately in
`docs/research/library-preparation-discovery.md`.

## Workspace and interaction

Use one persistent application shell:

- Sidebar: Tracks, Incoming with active/attention counts, Archive, playlist
  folders and favorites; Cleanup and Sync destinations; Settings at the bottom.
- Header: library search, current view title, context actions and background
  processing status. Avoid a setup wizard for each import.
- Main table: virtualized rows, selectable/reorderable columns, multi-select,
  search/filter/sort, keyboard navigation, context actions and drag/drop into
  playlists. Default columns include artist, title, genre, BPM, Camelot key,
  energy, duration and preparation/sync status.
- Player panel: persistent playback, overview/detail waveform, beatgrid,
  named/colored cues and loops. Selection and playback identity are separate,
  so editing another row does not unexpectedly interrupt playback.
- Inspector: optional details/provenance/analysis and cue editing, without
  turning the track table into a settings form.

Use page `#181818`, surfaces around `#202020`, teal `#00D2BF` and darker teal
`#106c61`. Use clear focus/selection states and semantic status icons in
addition to color. Build original Cuerithm branding and components. Preserve
table density and readability; branding typography must not reduce metadata
legibility. Resizable sidebar/player/inspector dimensions persist.

Incoming shows stages, progress, retry actions and actionable errors. Successful
tracks leave automatically. There is no approval button on the success path.
Empty views explain the next useful action, not the implementation machinery.

## Automatic preparation contract

The logical flow is:

```text
watch folder / drop files
  -> wait for a stable supported file and create a durable job
  -> read original metadata and build normalized matching input
  -> find metadata using configured Beatport/Traxsource policy
  -> apply selected Smart Fixes and genre/artist mappings
  -> full-track key, BPM, beatgrid, energy and structural cue analysis
  -> validate and apply configured metadata plus replacement cues
  -> commit prepared track -> Tracks -> refresh affected smart playlists
```

Stages may run concurrently where independent, but a track becomes ready only
after every enabled required stage succeeds. Full-track analysis is mandatory
for this workflow; preview/truncated analysis cannot mark a track ready.

Capture a settings/model/engine revision for each job. Changing Settings affects
new jobs; retry/reprepare can explicitly select the new revision. Persist stage
state and recover interrupted jobs after restart. Do not repeat completed
expensive stages unless their input/configuration changed.

Wait for file copies/downloads to settle. Deduplicate watcher events, tolerate
disconnected folders, rescan after missed events and suppress feedback loops
from the application's own tag writes. Track logical promotion separately from
physical movement: by default promotion does not relocate the audio file.
Physical file organization, if enabled in Settings, must update all references.

Analysis reads audio rather than using catalog/file key, BPM, energy or cue tags
as predictions. Store measured results and provider metadata separately. A
field-source policy decides what is written/presented; default key/BPM/grid
authority is the analysis engine. Catalog metadata cannot silently make a
beatgrid disagree with its BPM. Changing that authority is an explicit setting.

Apply every enabled tag field using its configured overwrite/merge and format
settings. Preserve unselected fields. Capture recoverable previous values.
Generate and validate the complete replacement cue set before replacing the old
set. Never clear existing cues merely because analysis started or failed.
Repreparing a track uses the same replacement behavior.

Database transactions and recoverable file-write jobs coordinate changes;
SQLite transactions alone do not make audio-tag and database writes atomic.
Promotion occurs only after required writes complete. Partial failures retain
retryable state and the previous valid cue set where replacement did not finish.

No provider match or provider/network failure stays visible in Incoming unless
the user has explicitly configured that stage as optional. Offline audio
analysis can continue while metadata lookup waits for connectivity. Do not
substitute a low-confidence match just to empty the queue.

Track a format/stage capability matrix. Common-format support is required in
the first usable delivery; a FLAC-only release does not meet the requirement.
The selected prototype source-timing and energy entry points currently have
FLAC-specific input requirements. Implement and validate a shared decoding and
PCM/profile boundary so every required analysis stage operates on all listed
formats. This boundary may use a pinned, bundled native decoder under Rust
orchestration; users must not install FFmpeg or another runtime themselves.
Preserve decoder-delay and audio-clock relationships through analysis, playback
and external cues/grids, particularly for MP3. A container conversion alone is
not evidence of equivalent timing or energy features.

## Tagging and persistent Settings

Adapt OneTagger's Rust provider/matching/tag-format functionality, preserving
provenance and applicable notices. Isolate it behind a Cuerithm interface that
accepts explicit file sets and returns candidates, match reasons, selected
metadata and write plans. Avoid importing its entire UI/runtime or global
batch-cancellation state.

Settings include:

- Watch folders, recursion, concurrency, retries and optional file organization.
- Provider order; first-success fallback versus configured multi-provider merge.
- All relevant common auto-tag matching, selected-field, overwrite, genre/style,
  artwork, album, filename-parsing, separator and file-format options.
- Beatport-specific artwork resolution, result-page count and version matching.
- Traxsource's provider concurrency restriction.
- Field-source preferences, cleanup order/mappings and protected values.
- Built-in offline key/energy/cue engines, cue template and replacement policy.
  No model selection or downloads are required (user correction, 2026-10-18).
- Direct rekordbox version/location, field mappings, cue destinations and sync
  inclusion policy.

Options for excluded services/tools do not appear as broken settings. There is
no Spotify audio-features, Quick Tag, separate tag-editor application or
auto-renamer workspace in this scope. Any imported setting whose meaning is
replaced by the incoming pipeline must have a documented mapping.

Reuse of GPLv3 OneTagger code was explicitly approved on 2026-10-18 for a GPLv3 desktop application, with
notices and provenance retained. Existing independently maintained MIT engine
code can retain its own notices. Do not import code while leaving an inaccurate
MIT-only description for the combined desktop distribution.

## Cleanup

Implement the eleven fixes found in the inspected Lexicon build as independent
Rust transformations with explicit options and deterministic fixtures. Expose
scan/preview/apply/ignore and undo for existing-library cleanup. The same
configured transformations run unattended in Incoming.

Genre Cleanup lists distinct values and affected counts, supports bulk mapping
to canonical genres, protected genres and saved rules applied to future imports.
Preserve original/provider genre provenance so mapping can be revised. The
user's purpose is a controlled useful taxonomy rather than accumulating every
store subgenre. Genre-to-custom-tag conversion belongs in the cleanup surface.

Artist Cleanup supports selected contributor roles: artist, remixer, producer,
composer and lyricist. Preserve multiple contributors and explicit exceptions.
Offer counted previews and canonical-name locks. Only approved/configured
mappings apply automatically; similar spelling alone is not an identity merge.

## Playlists and smart-playlist parity

Ordinary playlists support folders, stable IDs, membership/order, drag/drop,
rename, duplicate/copy, favorites and the requested Lexicon playlist tools
(merge, sort, cross-reference, prefixes and rewrite order). Archive behavior
must preserve playlist intent and be documented/tested.

Represent smart rules as typed data. Track all 58 observed operators in a
parity matrix, including text/regex, numeric/date/color/boolean, existence,
tags/categories, tempo markers, exact BPM, membership, cues/loops/memory cues
and key similarity. Implement Lexicon's actual Any/All plus OR semantics;
arbitrary Boolean nesting is not assumed to match the reference.

Cover empty rules, missing values, case/Unicode, date boundaries/time zones,
numeric bounds, archive inclusion, regex flags/length behavior and recursive
playlist references. Detect dependency cycles. Refresh affected smart playlists
after preparation/editing; relative-date rules also refresh with time. Define
stable sorting and useful loading states for large libraries.

Include edit rules, duplicate as ordinary playlist and bulk smart-playlist
generation. Do not permit misleading manual membership edits to a rule result.
Fields backed by excluded services, such as cloud-upload status, require an
explicit availability decision before a full parity claim.

Keep Cuerithm rule support separate from destination compatibility. Native
rekordbox-compatible rules remain intelligent playlists. Rules rekordbox cannot
represent are clearly identified and can sync their evaluated membership as an
ordinary playlist through the direct adapter; preserve the original smart rule
inside Cuerithm. Never claim that fallback remains dynamic inside rekordbox.

## Direct rekordbox 6/7 integration

Use a dedicated Rust adapter for import and export, not an XML transport.
Target actual installed versions and schema revisions. The adapter must handle
encrypted database access, track/foreign IDs, local update sequence numbers,
playlist hierarchy/order, intelligent-playlist definitions, metadata/MyTags,
cue destinations/colors/loops, beatgrid analysis files and artwork references.

Operate on copied database/analysis fixtures first. Live writes require the
target application to be fully closed, a valid backup and a successful
preflight. Never automatically kill rekordbox or delete its WAL files as a
shortcut. Use transactions/staging where appropriate and a recovery journal
for associated files. A repeated unchanged sync must not duplicate data.

The sync view shows detected target/version, changes, unsupported mappings,
pending work and previous result. Import must be able to preserve target edits
and external IDs. Resolve conflicts using explicit per-field ownership and
sync baselines; do not assume timestamps alone are sufficient.

Automatic preparation does not imply continuous writes to rekordbox while it
is open. Prepared changes become pending direct-sync work. The first delivery
uses an explicit Sync action after preflight; optional scheduling can later use
the same guarded adapter.

Cuerithm's library/preparation/UI supports macOS, Windows and Linux. Native
local rekordbox integration is targeted on macOS/Windows, where rekordbox runs.
Show accurate availability on Linux. This does not change the direct-only rule.

## Rust and frontend boundaries

Keep the existing analysis engine independent of Tauri. Add focused boundaries
for library storage, job orchestration, metadata/tagging, cleanup, playlists,
direct rekordbox integration, playback/waveform data and the Tauri shell.
Choose final crate splits during planning, rather than making every boundary
a crate in advance. Rust owns durable state and mutations; React renders typed
commands/events and transient interaction state.

SQLite stores tracks and audio/file identities, provider matches/provenance,
analysis/config/model revisions, cues/grid, playlist trees/rules, mappings,
settings, preparation jobs, undo records and external sync identities/baselines.
Support migrations and application-data locations on all desktops. The UI
cannot require a Python service or Electron/OneTagger process.

## Delivery and acceptance

Implement connected slices in this order, with dedicated plans where needed:

1. Library schema, durable jobs and desktop shell: watched/drop import, actual
   full analysis for all common formats above, replacement cues, Incoming status
   and automatic Tracks promotion.
2. Beatport/Traxsource adaptation plus persistent Settings and file-tag writes;
   integrate configured cleanup/mappings into that same job pipeline.
3. Cleanup workspace and ordinary/smart playlists, using the complete parity
   matrix rather than a reduced filter builder.
4. Direct rekordbox import/export, developed against fixtures early and accepted
   only after real version-specific round trips.
5. UI refinement, large-library behavior, packaging and cross-platform acceptance.

Start direct-sync feasibility work while the library contract is being finalized
so target identities/grid/cue requirements shape the schema from the beginning.
Do not defer this uncertainty until a finished UI depends on an unsuitable model.

Required evidence includes restart recovery, stable-file detection, watcher
idempotency, settings application, full-analysis completion gates, replacing
existing cues, metadata-field precedence, failed-provider behavior and safe
retries. Test the eleven fixes and all supported smart operators against
independent reference cases, including edge cases and destination limitations.

For rekordbox, compare database plus analysis assets before/after, then reopen
in real versions 6 and 7 to check cues/grid, playlist order/rules, artwork,
MyTags and repeated-sync behavior. A successful SQL transaction is insufficient.
Use copied test libraries until that acceptance work explicitly reaches live use.
Include format-specific cue/grid time offsets and decoder padding in those
round trips, rather than validating only FLAC timestamps.

Run Rust/TypeScript checks and desktop builds on all target systems. Verify real
rendered screens for loading, failure, empty, populated and large-library states.
Existing engine CI and the 47 cue cases remain regression gates; they do not
establish the new application's UI, tagging or sync correctness.

User clarification (2026-10-18): Plays, Last Played and Danceability are excluded.


## Added library-maintenance scope (2026-10-18)

The user requested the installed Lexicon's duplicate and orphan-file cleanup as
well. Static inspection of 1.11.15 confirms duplicate scanning/review/merge,
unused-file scanning and missing/corrupt-track scanning. Include these in the
Rust port, using the evidence in `docs/research/lexicon-library-maintenance.md`.
Keep unused disk files separate from missing library records. Duplicate merge
must support a selected keeper, locks/ignores, cue/tag transfer and playlist
reference replacement before archiving other copies. Its playlist updates depend
on the playlist store; source audio removal is a separate reviewed action.
Scans do not delete files. Preserve current file identities, referenced files,
analysis provenance and recovery behavior across desktop platforms. This scope
does not add streaming-library support or the excluded Plays, Last Played and
Danceability metrics. It remains an additional delivery after the current
metadata cleanup work, alongside the planned playlists and direct rekordbox sync.
