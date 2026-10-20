# Current native smart-playlist export contract

Status: source-backed format research and implemented native export contract, 2026-10-20. Scope: current VirtualDJ MyLists, Engine DJ schema 3, current djay TSAF/YapDatabase objects, Serato 4 SQLite and current Pioneer USB. Do not add older database, crate or playlist compatibility paths.

## Evidence and verification boundary

The reference is the locally installed, hash-matched Lexicon 1.11.15 renderer. Private extraction, executable probes, provenance and results live under `.analysis/dj-software-sync/smart-rules/` and are excluded from publication. This document contains format facts, invented examples and original Rust interfaces, not application implementation code or collection data.

Run the private reference probe with `TZ=Europe/Amsterdam node .analysis/dj-software-sync/smart-rules/probe.cjs`. It executes the original conversion methods with their original constants, key converter and Moment date library, replacing logging and unrelated host services. Results cover 24 synthetic definitions, 35 field/operator matrices, ten reference assertions, twelve checks of actual Rust-generated native artifacts and the original Pioneer playlist serializer. The original djay reader also accepts the Rust date filter, including the exact representable predecessor used for an inclusive calendar-day end. This verifies serialization behavior; it does not establish that a target application accepted or evaluated an exported rule.

Reference renderer evidence:

| Concern | Line ranges / symbols |
| --- | --- |
| Field IDs/types, operators | 235854–236242; 230679–230795 |
| VirtualDJ condition compiler and constants | 276353–276537; 277087–277180 |
| Current MyLists native/materialized selection | 362713–362779 |
| Engine conversion and constants | 268953–269149; 269985–270120 |
| Engine Smartlist persistence/fallback | 358877–359020; 360377–360405 |
| djay conversion, filter classes and operators | 267458–267632; 268346–268487 |
| djay native object serialization | 364947–365040 |
| djay fallback and Windows override | 365846–365965 |
| Serato 4 conversion and operators | 274913–275245; 275394–275487 |
| Serato 4 persistence/fallback | 253393–253509; 254962–254971 |
| Pioneer materialization | 246184–246207 (`de`, `ue`, `pe`); 247915 |

## Shared model and option

Keep the existing typed smart model rather than inventing a second JSON rule language:

```rust
// Added to the portable playlist model. Keep cached membership for fallback.
#[serde(default, skip_serializing_if = "Option::is_none")]
pub smart: Option<cuerithm_smartlists::SmartDefinition>,

// Added to SyncOptions; false means prefer native when the whole definition fits.
#[serde(default)]
pub materialize_smartlists: bool,
```

`materialize_smartlists = true` always writes the captured `track_ids` as an ordinary playlist. With false, compile the complete definition to the target's native representation. If any leaf, group, value transformation or required metadata cannot be represented, materialize the entire list and return a specific warning. Never omit unsupported leaves or silently change AND/OR.

`pub fn smart_fallbacks(library: &SyncLibrary, options: &SyncOptions, target: DjSoftware) -> Vec<String>` supplies preview warnings. Use the same compiler for warnings and actual adapter writes. A compiler result must distinguish `Ordinary`, `Native(target_payload)` and `Materialized(reason)`; user-requested materialization is not an unsupported-rule warning. Native plans are crate-private typed values, not public opaque application JSON.

`SyncOptions.smart_context` now carries the explicit library timezone, native application timezone and key-mixing preference. Context-dependent predicates require that context; calendar rules materialize when the two timezones differ. Do not guess the machine timezone or reinterpret a relative date as a fixed cutoff.

## Meaning of the existing definition

`SmartDefinition` has `match_all` and a flat list of typed `SmartRule`s. It has no arbitrary recursive group type. Existing evaluation means:

- `match_all = false`: OR every rule.
- `match_all = true`, with at most one `or = true`: AND every rule.
- `match_all = true`, with two or more `or = true`: AND the unmarked rules with one OR group containing all marked rules.
- An empty definition matches nothing.

Normalize this once into an internal `False | All(leaves) | Any(leaves) | AllWithAny { required, alternatives }` expression. Do not interpret a single `or` flag as a separate group. The reference VirtualDJ, Engine and Serato capability checks reject multiple marked OR leaves. djay has a narrower set-valued representation, described below.

The implementation materializes mixed AND/OR groups unless the entire normalized expression has a demonstrated native representation. A flat native AND/OR list is insufficient for `A AND (B OR C)`. Empty definitions must materialize to an empty ordinary list, preventing native empty-filter behavior from matching the entire library.

Native rules evaluate in the target application's collection, including matching tracks added there later. Materialized lists retain the captured CueRithm membership. Explain that distinction beside the option.

## Observed native capability matrix

These are reference converter capabilities, not permission to reproduce known conversion defects. `albumTitle` is Lexicon's field ID; map CueRithm's `album` to it conceptually, without leaking that alias into the public model.

Operator groups:

- S6: Contains, NotContains, Equals, NotEquals, StartsWith, EndsWith.
- S4: Contains, NotContains, Equals, NotEquals.
- N6: Equals, NotEquals, GreaterThan, GreaterThanEquals, LessThan, LessThanEquals.
- N4: Equals, GreaterThan, LessThan, Between.
- D4: Equals, NotEquals, After, Before.
- D7: D4 plus Recent, NotRecent, Between.

| Target | Fields | Reference operators |
| --- | --- | --- |
| VirtualDJ | title, artist, album, key, genre, comment, remixer, mix, label, composer, grouping, location, extra1, extra2 | S6 |
| VirtualDJ | bpm, bitrate, playCount, duration, rating, year, trackNumber, sizeBytes | N6 |
| VirtualDJ | dateAdded; color; CustomFileMissing | D4; ColorEquals/ColorNotEquals; BooleanTrue/BooleanFalse |
| Engine | title, artist, album, genre, comment, label | S6; Exists/ExistsNot also emitted but unsuitable as observed |
| Engine | bpm, duration, rating, year | N4 |
| Engine | key | equality and KeySimilar intended; other inputs can incorrectly pass classification |
| Engine | fileType | string predicates intended; unsupported inputs can incorrectly pass classification |
| Engine | dateAdded; tempomarkers count | Equals/After/Before; special analyzed/not-analyzed conversion |
| djay | title, artist, album, comment, label, composer, grouping | S6 |
| djay | bpm, playCount, rating, year | Equals, NotEquals, GreaterThan, LessThan, Between |
| djay | key, genre; color; dateAdded | StringEquals; ColorEquals; D7 |
| Serato 4 | title, artist, album, key, genre, comment, remixer, label, composer, grouping | S4 |
| Serato 4 | bpm, playCount, rating, year | Equals, GreaterThan, GreaterThanEquals, LessThan, LessThanEquals, subject to strictness defects |
| Serato 4 | dateAdded | After, Before |
| Pioneer USB | all | Captured membership only; every non-folder becomes an ordinary playlist |

Regex, negated prefix/suffix, tags/categories, playlist references, cue predicates, archive/Incoming flags and arbitrary metadata property IDs have no demonstrated native equivalent in these current export paths. Materialize them.

Only enable a predicate when its source value is actually exported. `SyncTrack.attributes` carries additional stored text, numeric and date metadata. The projection includes prepared metadata and original added dates where available; adapters persist the corresponding supported native fields. Play count has no authoritative catalog source and remains a materialization reason. Never invent dates or substitute target defaults.

## Implemented subset

`smart_rules.rs` is shared by preview warnings and the four writers. It compiles every leaf before returning a native plan; failures retain the complete cached membership. The observed capability table above is broader than the safely compiled subset below. Prepared metadata attributes, context-dependent date rules and Engine key similarity are connected. Unsupported target fields or unproven comparison semantics still materialize the entire definition; the compiler and focused regressions are authoritative for eligibility.

| Predicate | VirtualDJ | Engine DJ | djay/macOS | Serato 4/root |
| --- | --- | --- | --- | --- |
| Exported text fields | S6 for supported exported fields | Materialize: trim/Unicode semantics differ | S6 for supported fields; genre equality only | S4 for supported exported fields |
| Canonical key equality | Musical query | Open Key sequence number | Lowercase Open Key set | Selected target notation |
| Canonical key inequality | Native | Native | Materialize | Native |
| Key patterns / KeySimilar | Materialize | Patterns materialize; KeySimilar uses captured harmonic/fuzzy mode | Materialize | Materialize |
| BPM | N6; range in AND | N6 and range, source rounding expression | Materialize: native two-decimal comparator semantics unproven | Equals, >=, <=; strict comparisons materialize |
| Rating/year | N6; range in AND | N6 and range; both rating bounds scaled | N6 and range | Exact integer lowering; range in AND; unsupported inequalities materialize |
| Duration | N6; range in AND | Materialize rounded native durations | Materialize | Materialize |
| Location | S6 | Materialize | Materialize | Materialize |
| Missing file | Native inverse Exists | Materialize | Materialize | Materialize |
| dateAdded with matching explicit IANA contexts | Equals, NotEquals, After, Before | Equals, After, Before | Equals, After, Before, Recent, NotRecent, Between | Before only; After inclusivity unproven |
| Raw color identity or fields not preserved by target | Materialize | Materialize | Materialize | Materialize |

Inclusive djay rating/year predicates use its native Between operator over the finite f64 domain; original Between predicates additionally exclude zero, matching CueRithm's missing-value rule. djay BPM rules materialize because native comparison has not been shown to reproduce the source evaluator's two-decimal quantization. A range inside a larger OR materializes when native flat syntax cannot retain its internal AND. Engine handles that range directly with BETWEEN unless an additional zero guard would introduce a nested group.

Serato strict integer lowering uses the known year/rating domains: `rating > 2` becomes native `>= 60` on its 0–100 rule scale, and `year >= 2020` becomes `AFTER "2019"`. Fractional BPM keeps its original decimal value for inclusive predicates. It is never adjusted by an approximate epsilon. Skipping beatgrids also forces BPM rules to materialize for VirtualDJ/Serato when the target tempo is being preserved; djay BPM already materializes independently.

Color rules currently materialize because portable RGB does not retain the evaluator's original case-sensitive color identity. djay additionally quantizes track colors. Enabling a color rule solely from a matching wire color index would therefore risk changing membership.

Native-record tests exercise real VDJ files and SQLite/TSAF records, both representation directions, Engine folder membership/order, djay old link removal, and Serato stable native rename/move identity. Engine's decimal SQL oracle compares six operators, four thresholds and ten values (240 comparisons), including half-cent ties. Target application import/evaluation remains a separate verification layer.

## Current wire formats

### VirtualDJ MyLists

Use the existing `.vdjfolder` location and ordering. A native list has XML root `FilterFolder` with `scope="database"` and a `filter` attribute; an ordinary list uses the existing `VirtualFolder` song membership. Replacing the same owned file changes its representation atomically.

The condition field names include `Title`, `Artist`, `Album`, `Genre`, `Comment`, `Label`, `Length`, `BPM`, `Year`, `Rating`, `Color`, `File Path` and `First Seen`. Operators include `is`, `is not`, `contains`, `doesn't contain`, `starts with`, `ends with`, `=`, `!=`, `<`, `<=`, `>`, `>=`. XML-escape the complete filter exactly once. Use quoted native string literals; reject a value containing both quote styles until an application-level escape form is demonstrated.

A numeric Between predicate can expand to two inclusive comparisons when the whole native conjunction remains AND (or the range is the only original rule). Do not flatten an AND range inside a larger OR. `CustomFileMissing` is the inverse of native `Exists`: true writes `Exists = 0`.

Key string comparisons in CueRithm operate on canonical Camelot display. VirtualDJ metadata is written in musical notation. Convert canonical key equality/inequality to the same target notation; do not send a Camelot prefix/substring predicate to a musical-name field.

### Engine DJ

Persist native lists in `Smartlist`, with `listUuid`, `title`, `parentPlaylistPath`, `nextPlaylistPath`, `nextListUuid`, `rules`, `lastEditTime`. These coexist with ordinary `Playlist` rows and must participate in sibling ordering. A rules document has `match: "all" | "one"`, `rv: 1`, and a rule array containing `col`, `con`, `param`, `v`. The rule-level `v: "2.20.0"` is a wire-format identifier retained by the current serializer, not a request to support an older database.

Examples of field expressions: `artist`, `album`, `comment`, `genre`, `label`, `title`, `length`, `year`, `key`, `IFNULL(rating, 0)`. Conditions are SQL-like strings (`=`, `!=`, `LIKE`, `NOT LIKE`, `BEGINSWITH`, `ENDSWITH`, `<`, `>`, `BETWEEN`, `IN`). String parameters are SQL string literals; double apostrophes rather than reproducing the reference's unescaped apostrophe output. Contains/prefix/suffix add `%` as shown by the native format. These describe observed wire syntax. Current Engine text rules materialize wholesale because native comparisons do not establish the source evaluator's trimming and Unicode case semantics, even for values without wildcards.

Rating is stored on a 0–100 scale: multiply scalar and both range bounds by 20. The reference only scales scalar bounds, so copying its range output would be wrong. Normalize reversed range endpoints as CueRithm does.

Do not use the reference's integer `ROUND(bpmAnalyzed)` for decimal rules. The implementation emits `CAST(CAST(printf('%.14e',IFNULL(bpmAnalyzed,0)*100) AS REAL)+0.5 AS INTEGER)/100.0` in the trusted SQL-expression slot. BPM is nonnegative; the scientific conversion reproduces the source evaluator's 15-significant-digit repair before rounding. A plain SQLite ROUND(...,2) differs at tested half-cent values. Synthetic SQLite evaluation establishes source parity across the comparison matrix. Acceptance of this expression by the target application is an explicit inference from its expression-based wire format and remains a runtime verification item; do not claim that the application accepted it from the SQL tests alone.

Key equality uses Engine's Open Key sequence index (Open 1D = 0, 1M = 1), not djay's chromatic key-signature index. Only compile equality with a valid canonical query. Do not reproduce empty parameters emitted for key substring predicates. KeySimilar is compiled to the compatible Open Key index set using the stored harmonic/fuzzy key-mixing preference; missing context materializes the whole list. Do not reproduce `Exists` parameters such as `IS NOT 'false'`, which do not express CueRithm's truthiness rule.

### djay Pro

On macOS, use the same TSAF playlist object with type 2 and a `filterSet` containing `ADCMediaItemFilterSet`. Omit ordinary `itemUUIDs`. Native filter classes are `ADCMediaItemStringFilter`, `ADCMediaItemNumberFilter`, `ADCMediaItemDateFilter`, `ADCMediaItemSetFilter` and `ADCMediaItemSetFilterComponent`.

The properties are `enabled`, `name`, `keyPath`, `keyPathContainsMultipleValues`, and class-specific value fields. String operator IDs are Contains=0, NotContains=1, Equals=2, NotEquals=3, StartsWith=4, EndsWith=5. Number IDs are Equals=0, NotEquals=1, GreaterThan=2, LessThan=3, Between=4. Numeric scalar/range values use aligned doubles in `startValue`/`endValue`. Set components use operatorType 0 and a string or small-number value. Key values use lowercase Open Key, e.g. canonical 8A becomes `1m`; genre uses text; color uses the native color index.

String key paths include `title`, `artist`, `album`, `comments`, `label`, `composer`, `grouping`; numeric paths include `bpm`, `rating`, `year`, `playCount`; special paths are `key`, `genre`, `colorIndex`, `addedDate`. Artist, album, genre, label, composer, key and color paths set `keyPathContainsMultipleValues = true`.

The current writer does not serialize a general conjunction flag. Compile plain AND (and single-rule OR) only. Set components can encode an OR among values of one key/genre/color field, but the current compiler materializes such groups because its supported native subset does not retain that normalized expression. The reference capability check rejects a folded group because filter count shrinks; it also incorrectly accepts general OR without persisting its meaning. Do not copy either behavior.

The reference Windows writer forces materialization even when the global option is false. Keep that platform restriction until native Windows rule support is separately demonstrated.

Date operator IDs are Equals=0, NotEquals=1, After=2, Before=3, Recent=4, NotRecent=5, Between=6; relative periods are day=0/week=1/month=2. Dates use the 2001 epoch. `startDate` and `endDate` use native DATE double tags; `periodValue` uses a double and `period` a small-number tag. Calendar-day equality uses Between from day start through the exact representable predecessor of the next day. Strict After uses the predecessor of day start; Before uses day start. Relative day/week/month rules retain their native period instead of freezing a cutoff. The explicit timezone context controls eligibility; DateNotEquals materializes.

### Serato 4

The current reference places native smart crates in the local root database and filters them out of external/network/streaming location playlist writes (254962–255066). The root definition spans connected target spaces. External-only exports materialize and must report that location restriction. The master aggregate must copy native rules to its mapped container IDs; a smart-looking container without its rule row is not a working export.

Use container type 2 for a smart crate and one `smart_crate_rules` record keyed by `container_id`, with `revision`, `version=1` and JSON `rules`. Do not fill ordinary `container_asset` membership for a native crate. The current reference name includes its visible name followed by a private smart-crate delimiter and an identifier; preserve that convention for newly created native crates and reuse an existing owned container on representation changes.

The JSON contains `spaces: ["Serato Library", "iTunes"]`, `conjunction: "AND" | "OR"`, `live_updates: true`, and rule objects with `attribute`, `operation`, `type`, `version: 1`, and `value` or `partial_date`. Attributes include `name`, `artist`, `album`, `genre`, `label`, `comments`, `key`, `rating`, `year`, `bpm`, `dj_play_count`, `time_added`. Types are `STRING`, `UINT32`, `TIME` (BOOLEAN exists in the schema but the current exporter does not emit boolean rules).

String operations are CONTAINS, DOES_NOT_CONTAIN, IS, IS_NOT. Numeric operations are IS, GREATER_THAN_OR_EQUAL_TO and LESS_THAN_OR_EQUAL_TO. Year uses STRING with IS/AFTER/BEFORE. Rating values are multiplied by 20. Dates use `partial_date: "YYYYMMDD"` with AFTER/BEFORE in the reference format. The current compiler emits Before only; DateAfter materializes because native AFTER calendar-day inclusivity is not established.

The reference maps strict numeric `>` to `>=` and `<` to `<=`; it similarly conflates strict/inclusive year comparisons. Preserve strictness: lower comparisons exactly for discrete integer domains (rating/year), otherwise materialize strict fractional BPM predicates rather than inventing an epsilon. Inclusive BPM predicates retain their original decimal value. Numeric ranges can expand only when conjunction grouping is preserved. Current source output uses UINT32 as its BPM rule descriptor while accepting fractional JSON values; application interpretation of that descriptor remains a runtime verification item.

### Pioneer USB

The current reference gathers `getTrackIds()` for every non-folder and serializes ordinary playlist type 2, folder type 1. No smart definition is passed to the USB writer. Materialize regardless of the option, with an explicit preview reason when native preference was requested. Do not create invented smart-rule fields in PDB/Device Library Plus.

## Compiler invariants and integration

1. Validate field, operator, value shape and finite/range constraints before constructing a native plan. Track original rule IDs through expansions; every rule must be covered exactly, even if one rule becomes several native comparisons.
2. Respect simultaneous field mappings. A predicate on a source field is valid only where that exact source value is exported. Either map it to an equivalent supported destination field or materialize; never test the target's overwritten field. Key conversion is a separate transformation, not ordinary text renaming.
3. Keep a complete `track_ids` fallback for every smartlist. Never require native rule conversion to obtain membership, recompute DSP or read source tags during compilation.
4. Resolve native/materialized representation before persistence. Change representations atomically on the staged copy; remove the old owned rule/membership representation rather than creating duplicate lists.
5. Include `smart` in portable playlist baseline hashes. A rule edit must trigger Changed export even when the current member IDs stay identical. The materialize option already participates in the options hash, so toggling it must re-export the complete affected hierarchy.
6. Preserve parent references, sibling ordering, unrelated lists and stable identity. Engine's separate smart/ordinary tables and Serato's smart-name suffix need explicit representation-transition tests.
7. Rebuild/invalidate native derived indexes using the existing adapter path. Apply and restore continue to publish only reviewed staged bytes and acknowledge only verified transactions.
8. Keep missing context or schema support visible. A missing current smart-rule table is a preflight/schema issue; do not synthesize an older layout. A valid definition outside the compiler subset materializes with a precise reason.

Regression coverage includes each target's native record, requested materialization, unsupported whole-list fallback, mixed group preservation, empty definition, decimal thresholds, rating scaling/ranges, reversed range endpoints, unsafe literal characters, key equality notation, field mapping, ordinary↔native conversion, Changed rule-only edits, Windows djay materialization, and Pioneer materialization. Compare synthetic wire output to the private reference except where this document identifies a deliberate semantic correction. Keep target-application acceptance distinct from serialization and SQLite tests.

Final converter review also preserves raw Engine BPM in exported track fields; the
source's decimal rounding belongs in the smart comparator expression, not in stored
analysis. Current-head checks and integration evidence are recorded in
[PR 12](https://github.com/johnneerdael/CueRithm/pull/12). Native application/hardware
acceptance remains a separate verification step.
