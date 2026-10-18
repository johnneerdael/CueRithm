# Playlist tools and generation

This Rust library layer follows the licensed Lexicon 1.11.15.0 renderer
inspection recorded in `lexicon-smart-playlists.md`. The installed archive was
rechecked against SHA-256
`85fdbba4af97886ff1a626e6a4c96b4f9866dca3624fb2348a0332d7d6b63860`.
No proprietary code/assets, music or private library data are published.

## Tools

Tools preview their complete changes before one SQLite transaction publishes
them with a guarded Undo record. Successful application consumes the preview;
an error rolls back both the tree and consumption so the preview remains
retryable. They preserve source playlists, file bytes, analysis and cue history.

- Ordered union and intersection preserve first occurrence/source order.
- Cross-reference also supports tracks absent from every selected source, using
  the active prepared library as the universe.
- Sibling sorting supports names in both directions and descending direct-link
  counts. Name keys use JavaScript trim/lowercase and UTF-16 lexical order;
  descending order reverses ties. Folders have zero direct links as in source.
- Number/text prefixes and uppercase names retain source text behavior. Prefix
  numbering follows tree order; numeric replacement recognizes the source's
  one-to-three ASCII-digit prefix grammar.
- Partial sibling/member reorders replace selected slots while keeping hidden,
  archived and unselected members in their existing relative order.
- Recursive snapshots create independent ordinary playlists from complete smart
  memberships without modifying the original definitions.

Pure name/tree operations bind the playlist revision. Operations consuming smart
results or library membership also pin data/preferences, time zone, file
observations and dynamic expiry. Unrelated metadata publication therefore does
not prevent a name-only operation. Shared publication retains the original
operation identity needed by removal/restore/Undo.

## Generation

Generation discovers observed local scalar values, tags/categories, decades,
ten-BPM ranges and bitrate/size/duration buckets. Full-domain energy/rating
filters are an explicit option; observed values are the default. Separate
dimensions use separate folders; cartesian combinations concatenate names and
AND the rules. Empty dimensions are ignored in a combination as in source;
all-empty generation makes no empty destination folder.

Every item previews its name, complete rules and matching count. Candidate
membership is evaluated once per primitive group; combinations intersect those
sets. Node limits include newly required folders. Exactly 5,000 nodes can be
created in one transaction, while excess folder overhead or products fail
without a partial tree. Existing equivalent generated lists preserve their
IDs, spelling and rule IDs; conflicting meanings or ambiguous names are errors.
Excluded metrics and unavailable service fields are not generation options.

Logical ratings use native 0–5-star readers, including ASF, rather than treating
the raw Vorbis `RATING=80` as 80 stars. Raw tags remain separately addressable.
Rating predicates and previews pin the file observations and refresh deadline.
The format test writes four stars and clears the rating in authored copies of
all 14 supported layouts, then verifies decoded PCM/frame identity.

## Evidence and policy boundaries

The fixture bank retains 104 original positive prefix outputs and 20 original
generator-helper cases producing 65 items. Controlled descriptors/catalogs are
declared; these are helper measurements rather than installed-library/schema or
UI equivalence. Every raw result is compared in Rust.

The public generator uses two-decimal BPM predicate precision for both bucket
discovery and inclusive upper bounds ending in `.99`. This deliberately fixes
the source's fractional gaps at integer upper bounds, including `139.995`
rounding into the next bucket. Original integer-bound output remains in the
fixture bank. Observed text is trimmed/case-deduplicated by predicate semantics;
empty labels are skipped. Explicit unknown tags, conflicting definitions and
duplicate field modes are errors rather than silently omitted or overwritten.

Integration tests exercise actual materialization/counts, repeat generation,
contributor boundaries, stable identities, stale previews and an injected
failure after two node insertions. Rollback restores the original tree and
allows retry. Native editor wiring, whole-branch review and final platform
delivery remain subsequent plan tasks.
