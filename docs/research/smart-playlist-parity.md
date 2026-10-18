# Smart-playlist predicate validation

The pure `cuerithm-smartlists` crate implements the 58 local operators inventoried
in [the source research](lexicon-smart-playlists.md). This is predicate-level
validation. Persistent playlists, coherent refresh, native editor and direct
rekordbox sync require their separate integration gates.

## Original reference inputs

The committed JSON fixtures contain original synthetic tracks/rules and scalar
outputs from selected helpers in licensed Lexicon 1.11.15.0, ASAR SHA-256
`85fdbba4af97886ff1a626e6a4c96b4f9866dca3624fb2348a0332d7d6b63860`.
The shipped Electron32.3.3 / Node20.18.1 / V8 12.8 runtime evaluated them using its
already-enabled RunAsNode mode. The full application entry point, library,
authentication and network APIs were not exposed to the helper VM. Tag/category
catalogs and playlist membership are controlled synthetic fixtures. Extracted
functions, module factories and private runners are not published.

- `lexicon-scalars-v1.json`: all 637 initial scalar/combination cases.
- `lexicon-expanded-v1.json`: 883 full-dispatch cases covering all 58 operators
  with positive/negative results, empty input, numeric/array-count truthiness,
  reversed ranges, absent colors, unlabeled cues, loops/memory, unavailable IDs,
  Any/Every text and calendar boundaries. The 45 captured relative cutoffs cover
  months/leap years, Amsterdam's DST gap/fold and Apia's skipped calendar day.
- `lexicon-keys-v1.json`: all 1152 pairs of 24 Camelot keys in Harmonic/Fuzzy modes,
  plus 166 observed Camelot/Open Key/musical aliases.

Every one of the 2838 rows is compared or has an explicit policy assertion; none
is silently omitted. Raw source outputs are retained. These finite cases do not
prove arbitrary-input equivalence or complete application parity.

## Explicit policy differences

Eight malformed-regex rows are source false but become validation errors. Fourteen
expanded rows reference deleted tag/category IDs; source may ignore them, return
false or produce a vacuous category result, while Cuerithm reports unavailable
rules. One DateBetween row converts a missing/null date to the Unix epoch in
source; Cuerithm leaves the missing date unmatched. Four harmonic pairs are false
in source because its modulo boundary uses 0 against a 1–24 domain: 6A↔7A and
7A↔7B. Cuerithm uses the standard Camelot adjacency for those four pairs. These
27 rows account for every reference difference.

Rule values are typed, with explicit arity, finite-number, date-range and stable
ID checks. Lists support at most128 rules; IDs/references and text have bounded
sizes. Unknown fields, service-only fields and unavailable dependencies fail
validation/evaluation. Plays, Last Played and Danceability are excluded.

## Semantics preserved

Ordinary text lowercases and trims only the track value, using JavaScript's
whitespace set; query spaces remain significant. Regex sees raw metadata, while
key string comparisons use Cuerithm's Camelot display and file type uses its
actual display label. Any/Every text applies the predicate to each available
editable text field, including each negated predicate individually.

Numeric absence compares as zero except Between. Empty cue/tempo arrays have
count0 but remain present for Between/Exists, matching their source array
truthiness. BPM uses the observed decimal rounding before comparison; Exact
uses three decimals. Dates are integer milliseconds and local-day comparisons
in a captured IANA zone. Relative dates use calendar days/weeks/months, strict
cutoffs, clamped month ends, the earlier fold occurrence and forward gap
resolution. No host current time is read during matching.

Tags use known IDs without inferred artist/genre splitting. Cue loop, active-loop
and native memory facts are explicit fields; cue labels never manufacture them.
Playlist/folder/smart predicates consume complete supplied memberships and error
if they are unavailable. File availability distinguishes Present, Missing and
Unavailable. No metadata, file, audio or database access occurs inside matching.

Compiled lists own a shared immutable catalog and compiled bounded regexes;
evaluation receives a fixed clock, zone, mixing mode and membership/protected-
genre snapshot. All rules are evaluated before returning a combined match so
an evaluated error cannot be hidden by an earlier Any match. Archived records
require an explicit Archived=true rule. Source flat Any/All/multiple-OR semantics
are preserved rather than replaced with arbitrary Boolean nesting.

Chrono0.4.45 and chrono-tz0.10.4 supply checked dates and compiled IANA zones;
[Chrono](https://docs.rs/chrono/0.4.45/chrono/struct.DateTime.html) and
[Chrono-TZ gap information](https://docs.rs/chrono-tz/0.10.4/chrono_tz/struct.GapInfo.html)
are the pinned API references. The bounded ECMAScript adapter has its own
1798-case corpus and execution-limit tests from the preceding task.

## Library evaluation boundary

The library adapter now captures one transactional tree, metadata/custom-tag,
measured analysis, current cue-fact, preference and clock snapshot. It evaluates
dependencies before dependents and atomically publishes complete memberships
only while input revisions and the prior published-result baseline still match.
This compare-and-swap also permits a fresh evaluation after a clock correction,
while rejecting an older concurrently computed result. Failed lists retain
their last-good membership and expose errors; dependents do not consume partial
or failed results.

Relative and file-dependent lists have a one-minute deadline propagated through
smart-list and folder dependencies. The background service checks revisions,
definition/build signatures and deadlines once per second; it evaluates only
when needed and supports shutdown between tracks. Playlist preferences have
their own revision and never change preparation settings or enqueue analysis.
The build-time fingerprint covers evaluator, adapter, native tag and regex
sources, keeping old cached memberships from surviving an implementation change.

Previews pin file observations, time zone and expiry as well as database
revisions. Changed files or an expired clock reject publication. Waveform rules
inspect the actual cache and validate its complete playable clock, duration and
finite, ordered bins. Failed reads are observed too, so a repaired cache cannot
publish an obsolete error. Missing files and unavailable reads remain distinct.

Logical BPM/key/energy use measured analysis; raw tags remain separately
addressable metadata. Common date/year, fractional track-number and album-artist
tag aliases are normalized without rewriting tags. Native contributor values
retain their boundaries. Generated structural cues do not invent native memory
or variable-tempo facts; explicitly imported facts can supply those fields.

The current local five-crate regression run passes 299 tests with eight optional
tests ignored. Twenty library integration cases include actual preparation and
cleanup publication races, scoped paging/selection, previous-good retention,
clock corrections, dependency expiry and unchanged audio/analysis/cue histories.
Separate service/preview unit tests exercise controlled clocks and changed-file
publication. Embedded-artwork detection is checked before and after staged writes
across all 14 audio layouts; this exposed and corrected the upstream Ogg reader's
inverted `has_art` result, documented in the vendored adaptation notes.

These are library/predicate checks. Playlist tools, bulk generation, the native
playlist editor and final cross-platform desktop delivery remain separate work.
