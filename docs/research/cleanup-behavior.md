# Cleanup behavior evidence

## Reference boundary

Observed source: licensed Lexicon 1.11.15, ASAR SHA256
`85fdbba4af97886ff1a626e6a4c96b4f9866dca3624fb2348a0332d7d6b63860`.
The local reference harness executes selected pure helper functions in an
isolated JavaScript context, without running application startup or operating a
library. An unused renderer-enum initialization is bypassed; actual string,
HTML and casing dependencies are retained. Independent scalar cases reset the
text-cleaner module's mutable regex state. The fixtures record the Node runtime
and formatted-source hash. This establishes helper outputs, not end-to-end UI
parity or behavior in every Lexicon build.

131 original synthetic metadata cases cover the following source behavior:

| Fix | Explicit options and observed boundary |
| --- | --- |
| Casing | Text field; uppercase/lowercase candidate switches; whole-value ignores. Mixed-case values are skipped. Common casing preserves acronym rules. |
| Characters to spaces | Literal characters in Title, replaced individually with trimmed edges. |
| Encoded characters | Title entity candidates, with one-level helper replacement. |
| Extract artist | Missing Artist only; literal separator; 1-based result number; the standard spaced hyphen also accepts a spaced en dash. |
| Extract remixer | Missing Remixer only; configured suffix word and optional Title update; generic mix/version names are excluded. |
| Garbage | Text field; repeated spaces, edge punctuation and empty brackets; human text is the target. |
| Number prefix | Title prefix; source candidate pattern is broader than digits. |
| URLs | Text field; URL/email cleanup or clear the selected field. |
| Mix parentheses | Title; trailing mix/version description, respecting an already-open parenthesis. |
| Common title text | Camelot prefix/suffix, Original Mix suffix, Camelot/energy prefix, or trailing numeric BPM-like suffix. |
| Promotional text | Selected text field; bracketed promotion, premiere prefix and promotional/provenance tails. |

The reference symbols are `SmartfixTitleCase`, `SmartfixReplaceWithSpace`,
`SmartfixReplaceEncoded`, `SmartfixExtractArtist`, `SmartfixExtractRemixer`,
`SmartfixRemoveGarbage`, `SmartfixRemoveNumberPrefix`, `SmartfixRemoveUrl`,
`SmartfixAddMixParentheses`, `SmartfixRemoveCommonText` and
`SmartfixRemovePromotionalText`. Shared string/casing helpers and the source
transformer catalog are part of the comparison. Extracted JavaScript and app
assets remain private and are not distributed.

## Deliberate data-preserving expectations

Raw source outputs remain in fixtures. Separate expected-policy fields identify
intentional changes: preserve `vs`; decode numeric/nested entities with bounded
work; retain Unicode words as words rather than numeric prefixes; replace Unicode
scalar characters intact; keep bare audio filenames during URL cleanup; remove
complete email addresses; normalize whitespace created by empty-bracket removal.
Do not call these changed cases exact reproductions. Unknown metadata and
protected contributors remain untouched.

## Genre and contributor cleanup

Seen source paths collect distinct values/counts, apply explicit replacements,
support locks and record undo. Contributor roles are artist, remixer, producer,
composer and lyricist. The source applies selected exact values; similarity is
not evidence of identity. Cuerithm preserves native multi-value boundaries,
provides unique affected-track and role counts, and retains provider/original
provenance while mapping to useful canonical values. Categorized custom tags
preserve source genre detail when requested.

Manual publication/undo, automatic pipeline integration, real format writes and
native UI checks are separate implementation acceptance. Reference helper
fixtures alone do not establish those outcomes.

## Structured file metadata boundary

The cleanup adapter selects the tag reader from verified media/container data,
including wrong filename suffixes. Native value arrays remain arrays; punctuation
inside a contributor name is not a delimiter. ASF retains a display Author and
repeated WM/Artist values; a documented TagLib build modification keeps repeated
attributes in one metadata object to preserve order.

Cleanup runs after selected catalog writes and their overwrite policy. The raw
catalog response and cleanup report remain separate. Manual patches pin every
field's before value. Staging verifies the resulting human metadata, full decoded
PCM and frame count before publication. Values the target tag format cannot
represent exactly fail rather than silently merging. A cleanup that changes no
physical fields does not serialize the tag, preserving file bytes. Cleanup-only
writes preserve the existing ID3 version and qualified/language-specific comments
outside the managed unqualified comment. Custom categorized tags are returned for
library persistence, not confused with physical genre fields.

Current adapter checks use generated audio in all fourteen fixture layouts,
including Unicode paths and renamed files. They establish file staging behavior;
automatic Incoming cleanup and the native UI are separate acceptance stages.

## Manual library backend acceptance

Persistent previews bind selected proposals to the source bytes, metadata,
categorized tags, current preparation ownership and cue history. Applying or
undoing a proposal preserves musical analysis and existing cue revisions.
Metadata journals share atomic file replacement, durable originals, destination
reservations, watch-event ownership and storage accounting with preparation.
Interrupted publication rolls back before workers start; later user edits and
recovery originals survive conflicts. Undo restores the exact original file
bytes when its post-apply identity still matches.

Generated-fixture checks cover all fourteen format layouts, partial selections,
source/archive/preparation/metadata/custom-tag races, database rollback, restart
before copying/after copying/after replacement, interrupted Undo and deferred
temporary cleanup. Custom-tag-only changes preserve file bytes. These are backend
checks; automatic pipeline and native UI acceptance are recorded separately.


## Automatic preparation and counted catalogs

Incoming uses the same persisted rule configuration as manual cleanup. Offline
cleanup does not contact a catalog provider. When auto-tagging is enabled,
selected provider tags are written before cleanup. One journal publishes the
physical metadata, cleanup report, categorized tags, native value cache and
analysis/cues together; stale settings or concurrent custom-tag changes cannot
publish an obsolete result. Retry retains valid audio analysis.

Genre and contributor catalogs count distinct ready tracks, including per-role
counts. Native contributor arrays preserve names containing commas. Legacy rows
can backfill their derived metadata cache from a guarded native read; unavailable
files are reported without inventing contributors from flattened display text.
Explicit protected names remain locked. Analysis-only preparation caches native
metadata when readable without requiring a metadata write.

Seven focused generated-fixture checks cover persisted rules/restart,
cleanup-only and provider-before-cleanup preparation, categorized tag retention,
protected/no-op metadata, concurrent custom-tag changes with cached-analysis
retry, superseded settings, native role counts and unavailable legacy files.
The complete workspace release suite passes 595 tests with six explicitly
optional tests ignored. Native cleanup UI acceptance remains pending.
