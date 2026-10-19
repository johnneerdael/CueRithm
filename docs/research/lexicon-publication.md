# Lexicon cue and grid publication

Inspected Lexicon 1.11.15, ASAR SHA-256
`85fdbba4af97886ff1a626e6a4c96b4f9866dca3624fb2348a0332d7d6b63860`.
The retained private renderer extraction separates DSP detection from publication.
No proprietary source, user track labels or audio are included here.

## Seen in the shipped renderer

`AudioAnalyzer`, renderer 238705 onward, saves one tempo marker with first-beat
anchor unchanged and BPM rounded to two decimal places. The actual rounding
helper, module 2994 via 83841.Jw, corrects the scaled value to 15 significant
decimal digits before rounding. Simply displaying a raw BPM to two decimals
does not implement this saved-grid boundary.

The subsequent cue publication function, renderer 238721–238979, assigns template
names, colors, hot/memory/loop kinds and positions. Module 45988.Q defines six
enabled default rows: Start/yellow, Drop/red, Breakdown/blue, Drop #2/red,
Breakdown #2/blue and End / Fade-out/violet. Each is a hot cue. The UI's disabled
emergency-loop row is separate from this six-row factory object. Module 36864
provides marker, cue-kind and start-behavior enums; module 93814 distinguishes
saved cue type 1 from loop type 5. Memory variants carry the rekordbox
`originalNum: -1` discriminator.

Disabled structural points can leave gaps in output positions. Fixed positions
refer to the matching template row; otherwise positions follow the expanded
marker list. Custom before/after offsets use the track's saved BPM. Start-cue
zero/existing behavior is applied after custom offsets, so offsets retain their
original structural anchor even when the displayed Start cue moves. Legacy
`useFirstCueAsStart` is a fallback when the newer setting is absent. End/fade-out
forces zero loop duration. Emergency-loop positioning has its own type/time
filter. These details are not covered by worker-only marker comparisons.

`overwrite` gating and extraction of enabled custom anchors occur outside this
publication function. A port of this function alone does not implement those
caller stages or the complete application workflow.

## Independent Rust checkpoint

`camelot_core::lexicon_publication` implements this template stage and the
measured-analysis save order: round BPM, then publish beat-relative offsets.
It preserves the raw marker inputs and records implementation/input identities
for derived publication. Ordinary cue-only publication can retain existing
track timing instead. The original function and factories were executed on
229 authored publication cases plus 203 rounding cases, in installed Electron
Node 20.18.1. All recorded output fields match. Reference hashes and the precise
isolation boundary are in `tests/fixtures/lexicon-publication/reference.json`.

Validation deliberately rejects nonfinite timing and the source emergency-only
negative-infinite position rather than serializing apparently valid nulls.
These are explicit invalid-input policies. The tests do not establish audio
accuracy, complete custom-anchor extraction or native DJ export behavior.

## Integration still required

Raw engine output and published app state must stay distinct. Preparation must
apply the template after final genre-aware cues, preserve the rounded saved grid
for app playback/sync/tagging, record publication provenance and refresh it after
configured tagging changes the cue input. Saved cue kinds must reach smart
rules and direct rekordbox transport. Old completed results need explicit
reanalysis; updating code must not rewrite the user's completed collection.

The app currently consumes raw structural cues. This module has not yet been
wired into persisted analysis, the player or native sync. Full published-grid
and default-template parity therefore remains unproved. Tempo-bound settings
also need equal-input comparison: the factory range is 98–195, while the observed
user GUI selection was 78–155. That difference is separate from the rounding
and template publication demonstrated here. Preserve the user's unnumbered
Settings pages and built-in analysis components when adding meaningful options.
