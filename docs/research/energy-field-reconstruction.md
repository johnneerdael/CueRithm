# Independent energy request fields

Reference client: Mixed In Key 11.2.6.7446 on macOS.

The independent Rust finishing stage reproduces all five retained development
`EnergySegmentData` arrays exactly: **81 segments, 243 timing/volume values and
14,823 feature values**. The same computation matches on macOS ARM64 and Linux
ARM64. These are agreement measurements against the inspected client version,
not musical ground-truth labels or final energy classifications.

## Inputs and composition

The inputs are raw sustained/transient band timelines and weighted/time-domain
RMS arrays computed by our earlier independent PCM implementation. They retain
all 81,216 emitted frames across the same five development tracks. Transport
preserves 2,436,480 float32 words and selects the last complete snapshot before
global normalization. Expected requests and original feature arrays never enter
the prediction workers.

The finishing stage applies the source-derived smoothing, normalization,
autocorrelation, tempo refinement, overlapping segments, phase sampling and
183-value reduction. Result boundaries use the first result record's tempo:
`duration = (60 / tempo) * 32`, then `duration * index` and
`duration * (index + 1)`, with no clipping to file duration. Volume is
`20 * log10(max(raw_segment_rms, 1e-50))`. The Objective-C wrapper preserves
double precision and filters rejected results before serialization.

The profile supplies full decoded duration, zero skips and the complete useful
weighted-RMS slice. Its agreement with captured request fields corroborates
the profile on these tracks. Constructor arguments were not newly observed at
runtime. This composition reuses retained arrays from our own PCM pipeline;
it is not a new decoder run or a single integrated production execution.

## Portability correction and checks

The initial macOS run matched all fields. Linux matched every temporal and
feature value, but nine volume values differed by one or two floating-point
steps. A handwritten Rust implementation of the pinned runtime's double
`log10` calculation removes those differences. It preserves two arithmetic
paths, explicit fused operations and 129 table pairs. It receives numeric
arguments and contains no expected-output lookup or track-specific correction.

The primitive matches 8,496 scalar controls on both platforms, covering 8,415
analytic inputs and all 81 independently computed RMS arguments. Raw logarithms
and the scaled dB outputs both match: 16,992 double values. Its admitted domain
is finite positive inputs from `1e-50` through `1e9`; broader IEEE behavior is
outside this proof. Upstream double cosine, logarithm and exponential operations
retain their separate measured-profile limits.

The complete five-case projection matches 2,274,048 float32 words, 1,177,821
double words and 5,209 integer words on each platform. Linux runs in a container
without network access or mounts; expected values remain outside it and are read
after workers finish and the container is removed. The active composition and
dependencies pass 26 tests. Comparison controls preserve missing segments in
reference denominators and reject extras and one-step value changes. The first
failed Linux proof is retained separately.

## Remaining work

A later workspace integration now recomputes energy together with timing, key
and cue-score inputs from a fresh file decode in one process; see
[file-to-request integration](file-request-integration.md). This report retains
the earlier finishing-stage proof and its scope. Whole-library and held-out agreement, Windows,
other configurations and formats remain unverified. Final energy labels, key
classification and selected cues depend on additional service behavior. No
whole-engine benchmark improvement follows from this component proof; the
[last complete benchmark](engine-benchmarks.md) remains unchanged.

Private evidence is retained under `.analysis/experiments/energy-real-finish-*`,
`energy-result-fields-source-v1` and `energy-log10-*`. Static source inspection,
copied-component execution, scalar controls and independent workers have
separate records and scopes. No new server query was made for this proof.
