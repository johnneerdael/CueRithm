# Lexicon timing alongside Lexicon cues

Date: 2026-10-18. Scope follows the user's request to use Lexicon's beatgrid
with the accepted Lexicon cue engine. Continue the existing automatic desktop
preparation workflow; do not add an Analysis page, model selector or download.
Source evidence and initial experiments: `docs/research/lexicon-beatgrid.md`.

Independently reproduce Lexicon 1.11.15.0's entire local raw-PCM timing path:
tempo voting/refinement, onset and attack primitives, intro/drop-based anchor
selection, walkback, subsequent-beat and grid correlation, half-beat and riser
checks, silent/weak-phase rescue, DP phase correction and final validation.
Do not substitute a histogram-only implementation for the full detector.

Use Rust and existing dependencies, keeping camelot-core's existing Rust floor.
No JavaScript runtime, proprietary binary, model file or app/service access is
permitted in the distributed analyzer. Original apps are read-only references;
only original synthetic inputs/numeric outputs and aggregate music comparisons
may be published. Preserve the current cue DSP calculations, key engine and
scalar energy meter. The currently selected MIK-style timing stays available
to research/regression APIs.

The timing input is the complete first-channel Float32 PCM at its declared
sample rate. Validate finite PCM, rates 8000..192000 and finite tempo bounds
50..300 with min < max. Defaults are the renderer's 98..195 BPM range. Preserve
Float32 storage versus Float64 arithmetic and stable candidate/peak order.
No source metadata tags or reference labels may enter prediction.

Provide separate tempo evidence and full timing APIs. The tempo helper returns
its actual best BPM, confidence and sample-index peaks. Its confidence remains
an uncalibrated heuristic. Complete timing returns the selected BPM, first-beat
anchor and confidence; generate the displayed constant grid from this anchor.
Do not invent a variable-tempo map or independent downbeat sequence. Use the
unrounded BPM for cue generation as the source worker does; round only display
or persisted DJ destination fields where their contracts require it.

Quality exceptions are explicit: silence, short/empty audio and a no-peak
fallback yield no timing, never a fabricated 120 BPM/zero anchor or an infinite
loop. Invalid PCM/options are errors. A nonempty successful result must contain
finite BPM/anchor/confidence and a supported BPM and playable anchor. Bound all
searches; do not preserve source hangs. Record any further necessary deviation
before implementing it and pin it with a test.

Wire a mutually exclusive `EngineSettings.lexicon_timing` backend alongside
the legacy source-profile and optional learned-beat backends. Require complete
input. Desktop preparation selects the complete Lexicon backend only after
full reference/integration validation; old completed results stay intact until
explicit reanalysis, while unfinished/settings snapshots migrate with engine
identity. Capture source identity across decoding and include algorithm/options
in cache identity. Key and energy downmix behavior remains unchanged.

Native decoding must supply the first channel from the same canonical playable
clock used by playback and cues. Current canonical desktop PCM is 44100 Hz
stereo; preserve its delay/trimming and do not silently replace left channel
with a stereo average. Direct file/API decoding must explicitly retain a
first-channel buffer too. Compare source-rate and canonical-rate paths before
claiming WebAudio equivalence; existing media-clock guards still apply.

Validation requires original tempo/anchor reference outputs, guard cases,
noninteger BPM, range/half-double decisions, offset/quiet intros, stereo channel
asymmetry, nonfinite input and source-replacement races. Exercise every common
audio-format layout already supported by desktop analysis. Compare a larger
owned complete-track corpus without modifying music or consuming labels as
inputs; report disagreements, failures and elapsed time separately. Synthetic
parity and source parity are not musical ground truth.

Deliver a reviewed PR with Linux, Windows, macOS ARM and Intel checks. Do not
claim the new desktop timing is shipped before that integration is merged.

Delivery ruling: preserve cue semantics while allowing minimal corrections of
demonstrated source-port defects, each with original numeric regressions. These
are Float64 IIR feedback with Float32 stores, the quieter-before earlier-drop
condition and the fractional transient-count threshold. Do not fit cue positions
to songs or change the key/scalar-energy calculations.
