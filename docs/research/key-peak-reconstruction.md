# Spectral peak reconstruction

An independent Rust component now reproduces the inspected client's default
spectral candidate and tracking stages on bounded synthetic inputs. It accepts
complex float32 spectra and uses no original SDK, audio files, service or labels.
This is a component result; the complete PCM-to-request pipeline remains unfinished.

## Recovered stages

The source trace follows `KeyFeatureExtractor` construction and harmonicity
processing in the pinned MIKAnalysis ARM64 artifact. It preserves sub-bin
correction, in-place magnitude updates, frequency-dependent local suppression,
three-frame nearest matching and fused float32 smoothing. Persistent candidate
queues are compared after correction and local suppression; the original
transient pre-suppression index list was not exported.

The first two retained frames are empty and there is no synthetic EOF flush.
Missing previous and next matches have different behavior because the original
comparison preserves a NaN asymmetry. Constant input frequencies can also move
by one float32 step during fused smoothing. Both details have regression tests.

Retained source evidence:
`ev_5d2d465d2501cef02bda0ef66f6b06f319027582fa8eb8348d7f2f03b6db5d2e`.
The private source gate also retains assembly witnesses and literal constants.
Recovered pseudocode is not original source; exceptional floating-point branches
require separate investigation.

## Exact component comparisons

| Check | macOS ARM64 | Linux ARM64 |
| --- | --- | --- |
| Synthetic sequences | 57/57 exact | 57/57 exact |
| Supplied spectral frames | 327 | 327 |
| Candidate scalar values in newest queue frames | 5,936 exact | 5,936 exact |
| Complete queued scalar observations | 14,796 exact | 14,796 exact |
| Retained peak scalar values | 3,904 exact | 3,904 exact |
| Fresh-peak-to-tuning score values | 5,232 exact | 5,232 exact |
| Fresh-peak-to-finalized-tuning values | 327 exact | 327 exact |

The initial 25 sequences exercise zero/isolated/flat/nearby peaks, scan limits,
phase/sign changes, moving peaks, amplitude ramps and missing neighbors. The
32-case extension adds independently computed FFT block outputs and seeded dense
complex spectra. On those extension cases, the original direct harmonicity and
default processing paths agree on candidate queues, retained peaks, scores and
finalized tuning. Numeric inputs were fixed before native reference acquisition.

The composed tuning worker receives only newly computed independent peaks.
Expected arrays are opened by the evaluator after worker exit. Linux uses the
same source, with networking disabled and native expected outputs outside the
container. All 19 component tests pass on both platforms. These counts describe
synthetic component coverage, not music-library accuracy or a production benchmark.

## Portable exponential calculation

The initial Linux tracker matched only 52/57 sequences: 54 amplitude observations
differed, while counts and frequencies matched. A separate diagnostic isolated
the source to `exp2f`. For 2,276 candidate math inputs, hypot matched throughout;
19 exponential results differed by one float32 step. Recomputing with those Linux
math results reproduced every Linux candidate amplitude.

A handwritten finite-argument exponential now follows the pinned runtime's
range reduction, mathematical table, double polynomial and FMA rounding. It
contains no fixture/output lookup and calls no host exponential routine.
It matches 308,459/308,459 native float32 values on each platform, including
reduction-grid boundaries, adjacent floats, subnormal inputs and finite extremes.
Four tests pass on both platforms. Substituting this component removes all
measured tracker differences without changing thresholds or tolerances.

The failed initial Linux proof remains retained. The runtime observations came
from an agent-owned numerical diagnostic process; the installed app was not attached.

## Limits and next boundary

The validated profile is 44,100 Hz, FFT size 4,096, hop 2,048, useful input count
513 and loudness processing disabled. Host `hypotf` remains a dependency; sampled
agreement does not establish every possible argument or Windows behavior.

A separate tiny-magnitude probe caused the native oracle to report a nonfinite
peak. It yielded no valid reference vector. The Rust prototype reports an error
for a nonfinite correction before mutating tracker state; equivalence of that
exceptional path is unresolved. It must not silently be counted as an empty,
successfully reproduced native frame.

A subsequent [real-audio reconstruction](key-real-audio-reconstruction.md) now
validates original-schedule PCM feeding on 40 synthetic cases and independent
decoding/peak/tuning/cache production on the five real development files. Those
later proofs remove original retained peaks from the key-feature chain while
still supplying the refined beat grid and downbeat. The spectral-input proofs
here retain their original scope; resampling, broader profiles and final
classifiers remain unproven.

Private proofs are retained under `.analysis/experiments/` in
`key-peak-tracker-rust-v1`, `key-peak-math-probe-v1`, `darwin-exp2f-runtime-v1`,
`darwin-exp2f-port-v1` and `key-peak-tracker-portable-v2`.
