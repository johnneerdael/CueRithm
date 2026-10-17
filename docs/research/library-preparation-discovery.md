# Library preparation discovery

Date: 2026-10-18. Scope: automatic preparation, library cleanup, playlists and
direct rekordbox 6/7 synchronization for Cuerithm. This is an evidence report,
not a claim that these features are implemented.

## Reference identities and evidence boundary

- Installed Lexicon: 1.11.15, bundle `com.rekord.cloud.lexicon`.
- Copied application archive SHA-256:
  `85fdbba4af97886ff1a626e6a4c96b4f9866dca3624fb2348a0332d7d6b63860`.
- OneTagger source: `Marekkon5/onetagger`, commit
  `36523f71f2d9a5947912f3cb930f1a31fcb2e3ee` (2026-02-13).
- Cuerithm integration: `d180f10`; its engine CI passed on macOS, Windows and
  Linux. Local all-feature tests: 449 passed, 7 optional tests ignored; local
  model checks were also run explicitly.

Lexicon was inspected from a separate application-archive copy using leanrea.
Its installed files, database and music files were not modified. Extracted
application code remains private. Findings below marked **seen** are static
artifact/source evidence; they do not establish runtime parity or current
online-provider availability. Recommendations are marked **proposed**.

## Incoming and automatic preparation

**Seen:** Lexicon's `FileWatch` checks supported extensions, ignores temporary
files, deduplicates paths, waits for files to stabilize, and imports them with
an incoming flag. It has restart/backoff behavior. The relevant private
formatted-renderer evidence is `FileWatch`, lines 257053-257368.

**Proposed:** implement a durable preparation queue rather than copying only
the folder-watcher UI. The user wants a stronger automatic completion rule:
all configured tagging, full analysis and replacement cues must finish before
automatic promotion into Tracks. Incoming retains active jobs and failures.

## OneTagger integration surface

**Seen:** the selected functionality is already Rust, separate from its Vue/
Quasar frontend. Useful source boundaries:

| Capability | Source at the pinned commit |
| --- | --- |
| Provider implementations | `crates/onetagger-platforms/src/beatport.rs`, `traxsource.rs` |
| Match configuration and shared matcher | `crates/onetagger-tagger/src/lib.rs` |
| File tags and format support | `crates/onetagger-tag/` |
| Batch orchestration and applying fields | `crates/onetagger-autotag/src/lib.rs` |
| Palette | `client/src/style/quasar.scss` |
| Typography and general styling | `client/src/style/app.scss` |

The batch runner processes providers in configured order. Successful files
leave subsequent fallback searches unless multi-platform matching is enabled.
Traxsource declares a maximum of one worker. Beatport exposes artwork
resolution, search-page count and version-matching options. Shared settings
cover selected fields, overwrite policies, matching strictness, duration
tolerance, filename parsing, format/separator settings, artwork, genre/style
handling and album matching.

**Seen:** the palette uses `#181818` for the page, `#202020` for dark surfaces,
`#00D2BF` as primary and `#106c61` as secondary. Its typography uses Dosis.
The root OneTagger license is GPL version 3; Cuerithm's current root license is
MIT. Code reuse must have an explicit attribution and distribution-license
decision before importing those modules.

**Proposed:** adapt the provider/matching/tagging boundaries into background
jobs; keep the Tauri frontend independent. Do not import the whole OneTagger
application, its global batch-stop mechanism or unrelated tools.

Source: [pinned OneTagger tree](https://github.com/Marekkon5/onetagger/tree/36523f71f2d9a5947912f3cb930f1a31fcb2e3ee).

## Smart Fixes and cleanup

**Seen:** this installed Lexicon version contains eleven Smart Fix classes:
casing, replacing characters with spaces, decoding encoded characters,
extracting artists, extracting remixers, garbage-character cleanup, number
prefix removal, URL removal, mix parentheses, common-text removal and
promotional-text removal. Their scan/preview/apply handlers are distinct.
Evidence: `SmartfixBase` and `Smartfix*`, renderer lines 283786-285507.

**Seen:** Genre Cleanup aggregates distinct genres with track counts, supports
locks and bulk replacement, and records undo information. Artist Cleanup can
target artist, remixer, producer, composer and lyricist fields, with protected
names. Evidence: renderer lines 345603-345636 and 291068-291195.

**Proposed:** retain previewable cleanup tools for the existing library and
persist configured normalization mappings so future imports are cleaned
automatically. A generic rule must not guess that two different artists are
the same artist. Genre-to-parent mappings should be editable and reusable.

Official behavior references:
[Genre Cleanup](https://www.lexicondj.com/manual/genre-cleanup),
[Artist Cleanup](https://www.lexicondj.com/manual/artist-cleanup).

## Playlists and smart playlists

**Seen:** ordinary playlists have stable identity, parent folders, ordering,
recursive copying and path-aware references. The installed smart-playlist
operator catalog contains 58 operators. It includes text/regular expressions,
numbers, dates, colors, booleans, presence checks, tags/categories, BPM/tempo
markers, playlist/folder/smart-playlist membership, cue labels/colors,
loops/active loops/memory cues and harmonic-key similarity.

**Seen:** rule evaluation has Any/All modes, a constrained OR group inside All,
explicit archive handling and rules referencing other playlists. This is not
evidence for arbitrary nested Boolean expressions. Evidence: renderer lines
230679-230761, `SmartlistFilter` 301300-301530 and `PlaylistFilter` 301146 onward.

**Proposed:** maintain a rule-by-rule parity matrix and deterministic fixtures.
Do not call basic genre/BPM filtering full smart-playlist parity. Fields tied
to services outside this scope, such as uploaded-cloud status, need an explicit
availability state; they must not silently produce misleading results.

Lexicon documents that unsupported destination rules become an ordinary
playlist containing the evaluated tracks. That is a destination limitation,
separate from Cuerithm's own rule support. Preserve the rule inside Cuerithm and
show the destination's native-smart/static status before direct sync.
[Smartlists manual](https://www.lexicondj.com/manual/smartlists).

## Direct rekordbox 6/7 sync

**Seen:** the installed implementation locates the configured rekordbox
database, uses an encrypted SQLite connection and has separate reader,
importer, writer and exporter boundaries. The writer manages foreign IDs,
local change sequence numbers, cues, beatgrid/analysis files, artwork,
MyTags, playlist hierarchy and intelligent-playlist definitions. Export
creates a backup before writing. This is considerably more than updating a
track table. Credentials and encryption material are omitted from this report.

Evidence: `Rekordbox6DatabaseWriter`, renderer 248837-250932;
`Rekordbox6Exporter`, 251390 onward; `Rekordbox6DatabaseBase`, 270973-271027;
`Rekordbox6Importer`, 271980 onward. Source naming groups versions 6 and 7 under
the same family; this does not prove every database revision is compatible.

**Proposed:** implement a Rust direct-sync adapter against copied fixtures
first, including database and associated analysis assets, and validate results
by reopening them in the actual target versions. No XML fallback is in scope.

Lexicon's manual requires rekordbox to be closed for direct sync and describes
limits on native smart rules, MyTags and active loops.
[Sync documentation](https://www.lexicondj.com/manual/sync-rekordbox-xml).
The word "XML" in that reference page's URL does not change our direct-only scope.

Rekordbox's published desktop operating environments are Windows and macOS.
Cuerithm's library and preparation features remain cross-platform; native local
rekordbox integration is targeted and validated on those two platforms.
[rekordbox system requirements](https://rekordbox.com/en/download/#system).

## Not established by this investigation

- Live Beatport/Traxsource success using this source revision.
- Runtime parity for the eleven fixes or all 58 smart operators.
- Successful writes to current rekordbox 6 and 7 databases.
- Round-trip cue, beatgrid, artwork, playlist and MyTag correctness in rekordbox.
- A finished Tauri user interface or a packaged desktop application.

These are implementation and acceptance work, not findings that can be inferred
from static inspection or the existing engine's green CI.

## Common-format clarification and decoder discovery

The user subsequently required common-format support in the first usable
delivery, explicitly rejecting a FLAC-only stage. The format set is MP3,
AAC/ADTS, AAC/M4A, ALAC/M4A, FLAC, WAV, AIFF, Ogg Vorbis, Opus and WMA.

A local FFmpeg 9.0.2 probe encoded and decoded original generated audio in each
of those formats. The initial Vorbis encoder was unavailable in that local
build; a follow-up using its native Vorbis encoder succeeded. All requested
formats decoded successfully. The lossless variants produced identical decoded
PCM hashes. Raw AAC, Vorbis and WMA produced different frame counts from the
generated source, demonstrating why padding and playable timelines need explicit
fixtures. This is decoder discovery, not packaged-app or analysis acceptance.

The proposed implementation therefore bundles a pinned offline native decoder
under Rust orchestration and generalizes timing/energy PCM inputs. It preserves
the existing validated FLAC path as a regression baseline and does not assume
that converting a container proves equivalent cues, key or energy results.
Users should not need a system FFmpeg installation.
