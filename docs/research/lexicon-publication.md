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

## Default preparation and player integration

The library's Lexicon timing path now derives publication on every preparation
attempt, including raw-analysis cache hits. It runs after final genre-aware cue
refresh; configured tag writes receive rounded measured BPM. Raw key, tempo,
structural markers and energy remain unchanged in `FileAnalysis.analysis`.
The optional `FileAnalysis.publication` records the saved grid, typed cues and
implementation/input hashes. Legacy timing paths keep their prior behavior.

Track BPM and versioned cue revisions use the published values. Smart-playlist
facts resolve their labels, colors, loop/active/memory flags and tempo. The player
uses the published constant grid, cue labels/slots and canonical cue colors;
bar jumps use its saved tempo. Older analyses fall back to raw values and remain
unchanged until explicit reanalysis. No Analysis page or model selector was added.

A generated background-preparation test covers genre cleanup before publication.
Fractional persistence retains raw128.345 while saving128.35. Player and smart
rule regressions cover the published data rather than reconstructing it from
raw cue names. A private reanalysis of a copy of the user-reported track agrees
with the captured fresh Lexicon GUI on BPM, anchor and all five cue timestamps,
names, colors, types and positions. Original-source SHA-256 was unchanged.
A browser preview verifies the visible five-cue strip and breakdown navigation.
The packaged macOS app was also tested with a separate application identity and
private one-track library containing the copied audio. It decoded the waveform,
displayed all five published cues, sought to Breakdown at 1:50.9 and moved one
bar forward with Right using the saved 130 BPM. Playback was muted. This is a
native acceptance check for the reported track, not a full-library parity claim.

## Remaining gates

The current application path uses factory template rows, with the configured
emergency-loop detector flag. A complete custom-template/settings editor and
custom-anchor extraction still need delivery; the pure publication API alone
does not implement those caller/UI stages. Direct native rekordbox transport
must consume published cue kinds. Windows/Linux runtime acceptance and broader
music-corpus GUI agreement remain separate verification gates.

Tempo-bound settings also need equal-input comparison: the factory range is
98–195, while the observed user GUI selection was78–155. That difference is
separate from the rounding and template publication demonstrated here. Preserve
the user's unnumbered Settings pages and built-in analysis components when
adding meaningful preparation options.
