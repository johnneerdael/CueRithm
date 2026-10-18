# Lexicon smart-playlist behavior inventory

Static inspection of licensed Lexicon 1.11.15, ASAR SHA-256
`85fdbba4af97886ff1a626e6a4c96b4f9866dca3624fb2348a0332d7d6b63860`.
Source remains in the private extraction. This inventory does not establish a
Rust implementation or runtime parity. Anchors refer to formatted renderer lines.
A private isolated helper probe now records 637 original synthetic scalar and
combination cases with a frozen UTC clock. It executes the shipped
rounding/date helpers and selected pure filters; assertions and known tag/category
lookups use controlled fixtures. No app entry point, filesystem or network API is
exposed. This does not yet cover every operator or native library behavior.

## Complete operator inventory

The enum at 230679 contains 58 operators. Every row needs source-derived
synthetic fixtures and an implementation disposition before a full support claim.

| Operator | Current port status |
| --- | --- |
| `StringContains` | Pending |
| `StringNotContains` | Pending |
| `StringEquals` | Pending |
| `StringNotEquals` | Pending |
| `StringStartsWith` | Pending |
| `StringNotStartsWith` | Pending |
| `StringEndsWith` | Pending |
| `StringNotEndsWith` | Pending |
| `StringRegExpMatch` | Pending |
| `StringNotRegExpMatch` | Pending |
| `NumberEquals` | Pending |
| `NumberNotEquals` | Pending |
| `NumberGreaterThan` | Pending |
| `NumberGreaterThanEquals` | Pending |
| `NumberLessThan` | Pending |
| `NumberLessThanEquals` | Pending |
| `NumberBetween` | Pending |
| `DateEquals` | Pending |
| `DateNotEquals` | Pending |
| `DateAfter` | Pending |
| `DateBefore` | Pending |
| `DateRecent` | Pending |
| `DateNotRecent` | Pending |
| `DateBetween` | Pending |
| `ColorEquals` | Pending |
| `ColorNotEquals` | Pending |
| `BooleanTrue` | Pending |
| `BooleanFalse` | Pending |
| `TagsIncludes` | Pending |
| `TagsNotIncludes` | Pending |
| `TagsAtleastOne` | Pending |
| `TagsExact` | Pending |
| `TagCategoryNone` | Pending |
| `TagCategoryAtleastOneAnyCat` | Pending |
| `TagCategoryAtleastOneAllCAt` | Pending |
| `Exists` | Pending |
| `ExistsNot` | Pending |
| `TempomarkerSameBpm` | Pending |
| `TempomarkerNotSameBpm` | Pending |
| `BpmExactNumber` | Pending |
| `BpmNotExactNumber` | Pending |
| `PlaylistInSpecificPlaylist` | Pending |
| `PlaylistNotInSpecificPlaylist` | Pending |
| `PlaylistInSpecificPlaylistFolder` | Pending |
| `PlaylistNotInSpecificPlaylistFolder` | Pending |
| `PlaylistInSpecificSmartlist` | Pending |
| `PlaylistNotInSpecificSmartlist` | Pending |
| `CuepointAnyLabelIncludes` | Pending |
| `CuepointAnyColorEquals` | Pending |
| `CuepointNoLabelIncludes` | Pending |
| `CuepointNoColorEquals` | Pending |
| `CuepointHasLoop` | Pending |
| `CuepointHasNoLoop` | Pending |
| `CuepointHasActiveLoop` | Pending |
| `CuepointHasNoActiveLoop` | Pending |
| `CuepointHasMemoryCue` | Pending |
| `CuepointHasNoMemoryCue` | Pending |
| `KeySimilar` | Pending |

## Combination and archive behavior

`SmartlistFilter.filterFunction`, 301300–301530, uses a flat rule list.
Any means at least one true rule. All normally requires a nonempty list and all
true. If more than one rule has its OR flag set, All requires every unflagged
rule plus at least one flagged rule. A single flagged rule does not create a
separate alternative group. Empty lists return false. Arbitrary nested Boolean
expressions are not an established equivalent.

Archived tracks are excluded unless the list explicitly contains Archived=true.
That exception does not itself make an archived track satisfy other rules.

## Scalar boundaries

`StringFilter`, 300996: ordinary comparison lowercases and trims the track value;
the rule value is lowercased without trimming. Empty contains/starts/ends queries
are false, while their negated versions are true. Regex uses the raw track field,
trims the rule pattern, rejects patterns longer than 100 JavaScript string units,
parses the last unescaped slash as flags, and catches syntax errors as no match.
The key/file-type string paths use their display conversions. Regex flags,
lookarounds, backreferences and UTF-16 cases need explicit comparison coverage.

`NumberFilter`, 300924: missing/falsy numeric values compare as zero for ordinary
comparisons; Between rejects a missing/falsy track value and accepts reversed
inclusive bounds. BPM comparisons round to two decimal places. Exact BPM rounds
to three places before testing for an integer; a missing/zero BPM satisfies
neither Exact nor NotExact.

`DateFilter`, 300880: equality uses local calendar days; After includes the chosen
day, Before excludes it, and Between includes both ends even when reversed.
Recent uses the current time minus a calendar duration and a strict comparison;
NotRecent negates it, including missing dates. Month-end, DST and time-zone cases
must be evaluated against a frozen clock and stated time zone.

`AnyFilter`, 300855, treats zero numeric fields and whitespace-only strings as
absent. Boolean filters use source truthiness. ColorEquals treats two absent
colors as equal. These details must not silently become SQL NULL semantics.

## Tags, cues, tempo and references

Tag operations distinguish every/none/at-least-one/exact sets. Unknown deleted
tag IDs are filtered; empty selections have explicit behavior. Category Any/All
operates over known selected categories (`TagCategoryFilter`, 301106).

Tempo Same requires at least one marker; NotSame requires more than one marker
with unequal BPM (`TempomarkerFilter`, 301132). Cue labels match trimmed,
case-insensitive substrings; an empty query finds unlabeled cues. Cue color is
exact. Loop, active-loop and memory-cue status are separate predicates, including
negated forms (`CuepointFilter`, 301219). Key similarity depends on the selected
harmonic/fuzzy mixing mode and key conversion (`KeyFilter`, 300987).

Playlist references use both stable IDs and typed paths, and repair renamed/moved
references (`PlaylistFilter`, 301146). Folder membership includes descendants.
The Rust store must validate recursive dependencies and reject cycles; dangling
references must be visible rather than silently changing a smart list's meaning.

## Ordinary lists and generation

`Playlist`, 300619, recursively copies folders/lists and membership while keeping
new identities. Folder children have explicit persisted positions. Smart results
are materialized separately and refreshed through the smart updater. User edits
must not masquerade as manual membership of a dynamic result.

`SmartlistGenerator`, 301573, includes per-tag/category generation, ten-year and
ten-BPM ranges, numeric buckets, and cartesian combinations of selected
collections. Generated range bounds are inclusive. The approved port excludes
Plays, Last Played and Danceability despite their presence in source generator
options. Cloud and streaming service predicates require explicit unsupported
state because Cuerithm has no such service, not fabricated values.

## Dependency research

[Chrono's date API](https://docs.rs/chrono/latest/chrono/struct.DateTime.html)
provides checked calendar-month arithmetic, including invalid local-time cases.
[regress](https://docs.rs/regress/latest/regress/) targets ECMAScript regex syntax
and offers UTF-16 matching, but does not guarantee linear matching time.
[fancy-regex's builder](https://docs.rs/fancy-regex/latest/fancy_regex/struct.RegexBuilder.html)
provides a backtracking limit. Neither documentation alone establishes exact
Lexicon regex equivalence. The downloaded official regress 0.12.0 archive has
SHA-256 `32eef8b209c3c1c15dbad02c1f30f9539f00dc7253e0cbcdaae442a50a09d7c1`.
Its current source includes Unicode properties/sets, UTF-16/UCS-2 paths and
lookaround/backreference execution despite older documentation wording. It has
no public execution budget. A port that uses it needs a small, licensed bounded
execution adaptation and dedicated differential tests; no product dependency or
modified regex engine has been added at this research stage.


## Reference runtime verification

The initial new probes used host Node 22.22.2. They were replayed through the
installed signed Electron 32.3.3 runtime (Node 20.18.1, V8 12.8.374.38-electron.0)
with its already-enabled RunAsNode fuse. This executes only the selected helper
probe; no application startup or private library is loaded. All 637 scalar/
combination outputs are identical. The regex corpus contains 1,798 original
synthetic cases; modifier-group syntax is accepted by shipped V8 and rejected
by host Node 22, so shipped-runtime expectations are authoritative. The published
131 cleanup helper outputs, originally recorded under Node 26.0.0, were also
replayed under shipped V8 without a source-output difference.
