# Request JSON: calculation and serialization map

REA can trace how the installed client produces the analysis request. The
request builder is the end of that calculation: its inputs already contain
features, tempo, downbeat and cue scores. Reconstructing those producers is
necessary to generate the same request from audio.

This map concerns Mixed In Key 11.2.6.7446 ARM64. It combines retained static
REA evidence with bounded component comparisons; it does not establish full
independent audio analysis or recovery of the service's classifiers.

## Fields and producers

| Request field | Producer and transformation | Established boundary |
| --- | --- | --- |
| `Segments[].StartTime`, `EndTime` | Key-segment serializer; end is start plus length | Ordinary whole-file path creates one fresh segment: start +0.0, end is cumulative reader-returned frames / 44100; all five development requests match |
| `Segments[].PitchProbabilities` | Copies 12 float32 values from the segment's power array | Ordinary fresh whole-file path retains twelve zero float32 values; source-bound initialization/no-write proof and all five requests match |
| `Segments[].Features` | Audio decoding → scheduled FFTs → tracked peaks → tuning → 88-value cached frames → beat-window aggregation into banks | All 1,760 binary64 values and complete ordinary key-segment fields match on five development cases on Mac/Linux from independently computed PCM timing and key caches |
| `EnergySegmentData[]` | Energy-segment serializer copies start, end, RMS volume and feature array; absent features become JSON null | Independent PCM-derived bands/RMS now reproduce all five retained development arrays on Mac/Linux: 81 segments, 243 timing/volume values and 14,823 feature values; broader and integrated production validation remains open |
| `Tempo`, `DownbeatTime` | Main preserves the refined tempo, narrows it to float32, then promotes it to double for the builder; downbeat comes from the first adjusted pre-request grid point | Both scalars and all 2,498 grid words match on five Mac/Linux development cases using independently computed legacy and Main timing |
| `CuePointStartBeat` | Local cue calculation supplies an integer | All five capture-validated starts match on Mac/Linux using independently computed PCM timing, waveforms and duration |
| `CuePointData` | Local score calculation → float32 conversion → zero-prefix/suffix counts → retained little-endian float32 bytes → Base64 | All 2,816 scores and payload bytes match on five Mac/Linux development cases using independently computed PCM timing; these scores do not establish final selected cues |
| `DurationInSeconds` | Chronologically last key segment's start plus length | Ordinary path uses the analyzed reader prefix duration; all five request values match |
| `EliteData` | Optional supplied metadata object | Observed fields include names, genre, file type, duration and tempo; these are not prediction inputs |
| Product/platform and algorithm versions | Constants or analyzer version method | Recovered protocol metadata; key 94, energy string `2`, cue string `3` on the inspected path |

Authentication is outside this reconstruction. No credential values are needed
for an offline engine. The detailed [service contract](analysis-service-contract.md)
records the envelope, optional fields, response consumption and evidence IDs.

## Current independent boundary

The five development cases now form an independently computed chain: FLAC
reader-prefix decoding, legacy onset/initial-tempo/beat tracking, normal Main
waveforms, Main refinement and phase/grid handling, cue-score generation, key
FFT/peaks/tuning/cache and beat-window key-bank aggregation. Original legacy
marks and tempo are no longer supplied to predictions in this chain.

The legacy stage matches all 4,707 beat times and five BPM values. Its outputs
also reproduce the five Main request timing/cue fields and 1,760 key features.
The ordinary request segment now wraps those computed features with exact
boundaries and twelve zero pitch values, matching all five retained requests.
The selected Main path skips per-segment key processing and leaves the fresh
segment's calloc power storage unchanged; the field name does not make these
zeros normalized key probabilities.

The numerical PCM chain matches on macOS ARM64 and isolated Linux ARM64.
The latest Linux join uses independently Mac-decoded PCM; it is not a new Linux
FLAC decoder execution. Earlier decoder/component proofs retain their separate
scope. Whole-library, held-out, other-format and Windows accuracy are unproven.
A later workspace integration now recomputes this chain and energy fields from
a fresh file decode in one Rust process. All ten analysis fields match the five
captured requests on macOS/Linux ARM64. Its fresh whole-change review passed
the bounded integration gate; no whole-engine accuracy denominator changes
follow from these cases.

The reviewed classic composition also matches 13 synthetic streams on both
platforms, including a long silent introduction and a 6.5-minute stream. A
separate eight-second synthetic pulse success test still fails: its initial
histogram runs zero backward iterations, and the first forward iteration has
no accepted onset. No audited application store initializes the consumed phase
words; earlier native-provider stack contents remain unresolved. The engine
reports that unknown rather than supplying an invented zero. This is not proof
that the original application returns an error or behaves nondeterministically.

Energy uses a separate stereo analysis path. Its default 44.1 kHz constructor
setup now matches the copied component and Linux Rust: fourteen band limits,
1,025 weighting values and 2,048 periodic-Hann values, with the observed
2,048-sample block and 682-sample hop. The subsequent stereo frame and mono/stereo PCM stream ports match 17 bounded
synthetic cases on both platforms: 365,749 float32 values, 147 float64 values and
409 integer words. The stream comparisons cover 49 snapshots with buffer
trimming, 512-frame feeding, complete-frame thresholds, signed zero and skip
boundaries. Raw and weighted RMS use the original mixed-precision accumulation;
spectral magnitudes preserve the source fused-operation order.

A separate controlled run of the five existing independently decoded development
PCM streams also matches on macOS and Linux: 25 projected snapshots, including
complete raw/weighted RMS timelines through 81,216 emitted frames and current
spectra at selected snapshots. It does not compare every per-frame spectrum.
The explicit duration comes from independently decoded full FLAC frame counts,
with zero skip fractions; normal-app runtime constructor arguments were not
newly observed. The new real-data cohort is separate from the synthetic review.

The next independent Rust stage now reproduces the fourteen sustained and
fourteen transient band timelines, eight smoothing-history vectors and default
global normalization. Nine synthetic streams and their normalized variants,
plus percentile/coefficient controls, match on macOS and isolated Linux.
Twenty fresh own-source replays and 26 tests passed independent review. This
includes the source's signed sustained remainder and mixed-precision percentile
interpolation; per-segment normalization in the alternative mode is outside
the validated profile.

A separate controlled check of the same five development PCM packets matches
the copied component on macOS and the same Rust computation on isolated Linux:
30 snapshots containing 5,339,971 float32 words,
100 float64 words and 815 integer words, including complete band timelines and
the final globally normalized state. It retains the explicit full-duration,
zero-skip profile described above and is separate from the synthetic review.

The temporal and 183-value stages now have independent Rust implementations.
Ten synthetic band/RMS controls match copied-component macOS output and isolated
Linux ARM64 output, including segment tempo, raw RMS, aggregates and four families
of fourteen 256-value phase arrays. The composed reducer also matches all 2,379
feature values from thirteen accepted segments; two rejected segments retain
empty feature records. Twenty tests pass across the two crates. These controls
supply numeric band timelines and a declared full weighted-RMS normalization
slice. A subsequent real-track composition is described below.

The implementation preserves direct 1,024-lag autocorrelation, the separate
energy tempo search, coupled phase alignment, and the source's two different
normalization stages. Segment starts use floor(span*i), while nominal lengths
use ceil(span), allowing one-frame overlap. CountBeats returns 128 on a zero
256-value vector with step two; zero feature inputs therefore do not imply an
all-zero 183-value result.

The finishing stage now consumes the previously retained raw band/RMS arrays
computed by our own PCM pipeline for the five development tracks. All 81
segments match the captured `EnergySegmentData` arrays: 81 starts, 81 ends,
81 `VolumeRmsDb` values and 14,823 features. Complete temporal/reducer/result
projections also match the copied component on macOS and the independent Rust
computation on Linux ARM64. This uses the explicit full-duration, zero-skip,
full weighted-RMS-slice profile; matching captured fields corroborates that
profile on these cases without newly observing runtime constructor arguments.

The first real Linux run differed in nine dB values while every feature and
other projected value matched. A source-derived double `log10` implementation
removes those differences. Its two paths and 129 table rows match 8,496 scalar
controls on Mac/Linux, including all 81 independently computed RMS arguments.
No tolerance, per-track correction or supplied expected value was added.

This composition reuses our earlier PCM-derived arrays; it is not a fresh
decoder run or an integrated production analysis from files. Alternative modes,
whole-library/held-out/Windows agreement, the full request object and final
energy labels remain unverified. [Energy-field reconstruction](energy-field-reconstruction.md)
records the proof and its limits.

See [real-audio reconstruction](key-real-audio-reconstruction.md),
[peak reconstruction](key-peak-reconstruction.md),
[tuning reconstruction](key-tuning-reconstruction.md),
[cache reconstruction](key-cache-reconstruction.md) and
[timing reconstruction](beat-waveform-reconstruction.md) for component evidence
and the original boundaries of earlier conditional comparisons. A separate
314 internal floating-point diagnostic occurrences differ between platforms;
matching exported values is not full internal bitwise equivalence.

## Exactness checks

Validate each producer before joining it to the serializer. Keep expected values
outside prediction workers and compare array lengths, float bits, segment times,
cue bytes and Base64 independently. Whole JSON text also contains object ordering,
whitespace and number formatting; matching numeric analysis values does not by
itself prove identical HTTP body bytes.

After the complete numeric request matches, evaluate request-to-result mapping
separately. The response's final key/confidence, classified energy and selected
cue list are outputs of the service. The request exposes their inputs, but does
not supply the server implementation or weights. Repeated request/response pairs
can constrain that mapping; perfect reproduction still requires verification.

## Source anchors and retained proofs

The request builder is main-image `0x100091fbc`, retained REA evidence
`ev_9752a3741b1eddca5d2754dd1a15ff3b20e93c5c643e0a5727d113685abe7425`.
Its static evidence identifies JSON validation/serialization and the POST to
`AnalyzeSong10`; earlier live captures independently observed the exchange.

Key-segment serialization is framework `0x4c24`, evidence
`ev_cbd4cc937c925c924f4c9d5acf6594e0e55d02399e3998068e8bb41e370ab2fb`.
Energy serialization is `0x156ec`, evidence
`ev_ba51bb5eab010582fc6a7c3d0857c823503297cbc8a34c54db22cac8b85ad710`.
The ordinary fresh-segment caller is main `0x10008f378`, retained evidence
`ev_8b6695cfd9933134e1c2da2b8d4a8cd8ddefbad9b9e69439043f2ab5fd3bfa96`;
its lifetime/no-power-write proof also uses copied-image assembly.
The energy constructor is framework `0xb2f4`, evidence
`ev_f1f24fb8babf922ba7040c7669a301637467abd7d4861c3895c1391e476712ca`;
its coefficient instruction order is retained as
`ev_7924be2520f0f3360589e348cd4faabef0dd31586da484ad2fbb7d1fb0500ac5`.
The key frontend source gate uses evidence
`ev_5d2d465d2501cef02bda0ef66f6b06f319027582fa8eb8348d7f2f03b6db5d2e`.
Decompiler output is recovered pseudocode, not original source; indirect
Objective-C calls and some value-flow coverage have explicit limitations.

Private proof directories are `key-tuning-request-composition-portable-v2`,
`key-peak-tracker-source-v1`, `native-key-peaks-v1` and
`native-key-block-spectra-v1` under `.analysis/experiments/`.
Current ordinary-segment/setup proofs are `request-key-segment-source-v2`,
`request-key-segment-rust-v1`, `energy-pcm-source-v1`, `energy-setup-rust-v1`
and `request-energy-setup-linux-v1`. Frame/stream proofs are
`energy-frame-rust-v1`, `energy-stream-rust-v1` and
`energy-frame-stream-linux-v1`. Band/normalization proofs are
`energy-band-rust-v1`, `energy-band-reference-v1`, `energy-band-linux-v1`,
`energy-band-review-v1` and `energy-band-real-pcm-v1`. The latest real PCM chain is retained in
`pcm-to-main-timing-v1` and `pcm-to-main-timing-linux-v1`.
The temporal/reducer proofs are `energy-temporal-rust-v1`,
`energy-temporal-reference-v1`, `energy-temporal-linux-v1`,
`energy-feature-rust-v1`, `energy-feature-reference-v1` and
`energy-feature-linux-v1`; the corresponding source maps retain their separate
static and numerical evidence boundaries.
The real finishing and portability records are `energy-real-finish-v1`,
`energy-real-finish-rust-v1`, `energy-real-finish-rust-v2`,
`energy-real-finish-linux-v1`, `energy-real-finish-linux-v2` and
`energy-log10-rust-v1`.
No production benchmark, held-out evaluation or full-engine improvement is
claimed by this map.

## Integrated file-to-request command

`camelot analyze-features PATH` and the camelot-core feature APIs now compute
all ten analysis fields from a fresh supported FLAC decode in one process. The
workspace source contains the independent arithmetic; it has no runtime
dependency on the private experiment directory or original application. Five
captured development requests match all 16,921 numbers and 15 strings on macOS, and
fresh Linux outputs match the complete macOS JSON bytes. Windows, broader inputs,
finalclassification and whole-library exactness remain open. See
[file-to-request integration](file-request-integration.md) for profile and proof.
