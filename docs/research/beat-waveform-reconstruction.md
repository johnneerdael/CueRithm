# Beat and waveform reconstruction

The outgoing request's tempo, beat-aligned key features and cue scores depend on
upstream timing. Reconstructing JSON serialization alone does not reproduce
those calculations. This investigation concerns the ARM64 client version
11.2.6.7446; the independent engine remains incomplete.

## Normal waveform producer

Retained static evidence identifies the main executable's custom beat-analysis
transform as the preferred waveform supplier when its result is available.
It differs from the embedded SDK's fast waveform analyzer. In particular, the
normal stereo float32 path averages channels in double precision before
narrowing to float32. The key-analysis path instead sums channels.

The default waveform transform uses 4,096-point FFTs, a 1,024-sample hop and
Apple's Accelerate FFT backend. Its context filters produce six populated
feature arrays in an eight-array container. The source map records initialization,
framing, feature formulas and finalization. The complete independent pipeline
now reproduces five retained development waveforms, as described below.

Evidence anchors are `ev_24f60ed748a5cf054b133235571210ca3833e375bd91abd8865b441e3bb255f6`
(DSP), `ev_6d8d376626cd7f289b84972cbf1772c893026fb2a0b553412517e0bba3185339`
(wrappers) and `ev_bbb18b8d82213ea5eae86e86f04658b36049c273421fa13b8d6c04aad40ce003`
(PCM conversion). Private retained source evidence is under
`.analysis/experiments/main-waveform-source-v1/`.

## FFT numerical gate

A separate own-process diagnostic compares the existing independent key FFT
with the system FFT configuration used by the waveform transform. Both workers
receive the same fixed synthetic complex32 inputs. It loads no purchased-app
code, music or expected analysis labels.

In that initial comparison, only two of eleven raw FFT cases match every component bit. Across 90,112
components, 68,873 differ. Zero and origin-impulse controls match; more demanding
signals expose different rounding. This is evidence that the already verified
key FFT cannot simply replace the waveform backend for exact reproduction.
It does not measure final waveform error or real-music analysis accuracy.
Those failed comparisons remain retained. They concern the old key FFT;
the separate waveform implementation below addresses the identified profile.

## Portable waveform FFT

The system dispatcher selects different numerical paths according to buffer
alignment. An own-process test covers eleven synthetic signals at all sixteen
float32 displacements within a 64-byte boundary: 176 transforms. Nine signals
distinguish aligned from unaligned buffers. All tested nonzero displacements
agree with one another; the aligned outputs match the initial system reference.
Complete cached-waveform comparison below confirms this numerical profile for
the five development cases; other settings and formats remain unverified.

A new independent Rust implementation reproduces the aligned 4,096-point raw
complex FFT. Its coefficient generator computes all 16,096 required float32
table entries from bit-reversed indices and trigonometric formulas. Every table
word matches the system reference on macOS ARM64 and Linux ARM64; the worker
reads no captured table.

The first, middle and final stages preserve the source arithmetic order and
restore the final frequency order. Complete transforms now match every one of
90,112 component bits across all eleven synthetic cases on both measured
platforms. A subsequent forty-case extension fixes new synthetic overlapping
windows shaped like decoded 16-bit PCM before collecting any references. All
327,680 additional component words match on each platform, bringing the total
to 51 cases and 417,792 exact words per platform. These remain synthetic inputs.
First and middle stages also have separate exact comparisons. The
initial final-stage implementation had 16,209 differing words; correcting its
real-component cosine/sine accumulation order resolves them without changing
the bitwise gate. The failed source, binary and results remain retained.

The system references execute only a private process's numerical library.
Its identified three-stage pipeline separately matches the exported FFT API
for all eleven cases. Original application/SDK code and music are not executed
in these probes. Portable workers receive numeric inputs and independent Rust
source only; references stay outside them, including the Linux container.

Instruction meanings were checked against the pinned
[AMX research source and tests](https://github.com/corsix/amx/tree/483714bb051da088d08a66724b22dd08a5db3c99).
The Rust engine component expresses scalar arithmetic and does not ship an
instruction interpreter or native code extracted from the library.
A source-only check additionally matches all 8,192 final-stage expression
components and the complete output scatter without reassociating operations.
Independent review confirms that source binding and the original platform
comparisons.

## Geometric filter power

The waveform's geometric context filters also need a specific double-precision
power calculation. A separate source-derived Rust helper reproduces its fixed
exponent profile using compensated arithmetic and small numerical tables,
without host power/logarithm/exponential calls. All 9,075 synthetic positive
finite/positive-zero inputs match both binary64 results and narrowed binary32
results on macOS ARM64 and Linux ARM64.

A strict external audit pins the reference identities and requires complete
counts and matching input identities before comparison. Its twelve guard tests
reject truncated, extra or misaligned references and outputs. This supersedes
the earlier permissive evaluator while preserving its original files. The
reported 9,075-case comparisons remain exact under the strict checks.

## Independent complete waveforms

The independent pipeline now joins guarded FLAC decoding, the packet-derived
analyzed prefix, double-precision stereo averaging, paired FFT windows,
source-specific reconstruction, spectral windowing, weighting, geometric
rebinning, context filters and finalization. It reads only the audio copy and
fixed configuration. Original waveform arrays, timing, tags and labels are
absent from its prediction inputs.

On macOS ARM64 and Linux ARM64, all five development tracks match every one of
324,480 waveform amplitude words across 54,080 frames and six populated
channels. All eight array shapes, stored scalar metadata and binary64 duration
also match. Intermediate magnitudes agree across platforms for all 11,194,560
values. Encoding the independently computed values and audio-derived frame
counts into the raw V2 format reproduces all 1,298,420 original numeric payload
bytes per platform.

This covers the inner numeric waveform format. The native outer archive
wrapper and in-memory frame validity/padding bytes are separate and were not
compared. The six channels are analysis features; agreement does not establish
final classified energy labels or selected DJ cue points.

The context port preserves the thirteen-magnitude/three-geometric history,
six-frame delay, six-zero finalization, distinct frequency-edge arithmetic,
fixed melody taper and per-term band weighting. Eleven context tests include
nonzero onset/energy and temporal melody support; twelve frontend tests cover
stereo conversion, scheduling and reconstruction. Source review and retained
real-output comparisons establish this bounded pipeline. No original app/SDK
execution or held-out evaluation occurred during this composition work.

The cue-request generator now consumes these independently computed waveforms
and audio-derived duration. On both platforms, all five curves match the
previously capture-validated references: 2,816 float32 scores, packed payload
bytes, Base64 and start-beat fields. Original refined beat grid, tempo and
downbeat remain explicit supplied inputs. Full independent timing, final
classification/selection, broader formats, Windows and production adoption
remain unfinished.

## Legacy beats and refinement

The embedded legacy beat library contains both classic and newer detector
implementations. The traced file-analysis route explicitly selects the classic
implementation, overriding the library default. Detailed API and DSP bodies
have been retained, including onset tracking, initial tempo estimation and
backward/forward beat tracking. This source recovery is not an independent
implementation or a numerical parity result.

Classic API evidence is
`ev_5a508f7777b54513e18479f7022a21e5634d9aeb54bf1a32e28dccdc33382f6b`;
the deeper DSP batch is
`ev_75f13314f1d500c9155bad2b25a6364d1ffb6a1990b203efb199fe5c4070ed17`.
The ordinary single-pass wrapper creates fresh first/second-pass objects and
executes the first pass with refinement enabled and version 4. Its result path
therefore selects the SDK's `FixBeatGrid`, rather than the different Main-image
preview implementation. Static tracing follows the constructor through lazy
result dispatch without an intervening settings-application call. This establishes
the selected algorithm for that source route; the five captured requests do not
record these flags and were not individually branch-traced at runtime.

The source anchors are Main `0x10008f378` → `0x100075e3c` → `0x10006d120`,
constructor `0x100059f84` and result consumer `0x10005bcc0`. The private
`beat-job-config-source-v1` report retains exact instructions, virtual-table
slots and the distinction from preview entry `0x10005b98c`. Registered parameter
identifiers are not established preference keys.

A complete independent Rust SDK-refinement component now matches all five
retained numeric fixtures on macOS ARM64: 2,468 binary64 beat timestamps,
overall tempo, SDK downbeat and normalized input counts are exact. Its inputs
still include the original legacy marks and SDK-fast waveform features.
A subsequent composition replaces those features and duration with the
independently computed normal waveform and audio-derived duration. At the same
SDK boundary, all five cases and 2,463 timestamp words match a numeric-only
reference call, together with tempo, SDK downbeat and normalized counts.
Original legacy marks/tempo remain supplied. The SDK downbeat is distinct from
the later outgoing `DownbeatTime`.
Those legacy values come from separate reference runs; matching the captured
app jobs' actual legacy configuration and inputs remains a further condition.

The port covers fit/rebuild, constructor octave normalization, beginning
extension, delayed transient detection, melody-based fallback and final trim.
The SDK's half-beat condition is strictly `0 < tempo < 100`; other components'
different threshold comparisons must not be substituted. Retained coordinate
and tail evidence includes
`ev_dc4be32ad3a0cf9d582317d8cd91e1c512510ad2c1152c73857207513210d7d6`
and `ev_1f58b45cfb46bf8f985ecbf6eae9fa03dd173bdf3f4bed69c87b1a2861860c0d`.

The current bounded revision adds allocation preflight, a fixed-five fail-closed
verifier and complete bounded JSON reads. Its 33 Rust tests, 16 verifier tests
and seven end-to-end rejection controls pass, alongside Mac format/Clippy/release.
The initial supplied-feature port used retained references. The subsequent
normal-waveform composition executes only numeric functions in a separate copy
of the signed SDK under network/write restrictions; it runs no application
process or audio decoder. The later full composition below adds Linux validation
of exported timing/cue fields; Windows remains unverified.

The initial direct comparison of these SDK-stage results to the complete saved
application grids matches zero of five whole grids. All five tempi narrow to the wire
value as float32, initially a diagnostic of the remaining conversion boundary.
Main-specific phase selection, tiny default shifts and tail extension run after
SDK refinement and were missing from that initial gate. Component parity does not substitute
for that complete timing gate.

## Main processing after SDK refinement

The ordinary-route source map now covers the additional pre-request stages:

| Stage | Source-established behavior |
| --- | --- |
| Pass 1 after SDK refinement | Reconstructs the grid, then may select a half-beat shift by comparing weighted onset scores; marks and normalized origin both change |
| Pass 2 initialization | Adds its reset-time shift of approximately −2.77555756156289e−17 seconds separately to every mark, then aligns using the Pass 1 SDK anchor |
| Quarter/half/whole processing | Uses source patterns and weighted windows to select discrete phase changes; each reconstructs the beat object |
| Intermediate cue pass | Main's custom scorer selects an anchor and changes the grid origin |
| Final cue pass | SDK V3 supplies the outgoing score curve while retaining that earlier grid origin |
| Final grid construction | Samples the coordinate transform from the chosen downbeat through the full duration, potentially extending past the SDK's last retained mark |
| Request conversion | Applies the waveform offset; narrows the preserved tempo to float32 and promotes it to binary64 for JSON |

These are static source conclusions for the ordinary fresh-wrapper defaults,
not individually observed execution traces of every captured track. The
private eighteen-file `main-beat-pass2-source-v1` freeze is
f63479fa81d7ca642488ddf5b53e1d9bcd2afd975101d01066b944d8e2f92c74.
It retains exact literals, effective reset defaults, source bodies and call
ordering. A new Rust Main custom-anchor component now implements this source
path separately from the SDK curve/phase port.

The tempo narrowing is explicit at Main `0x10008f9f8` (`FCVT s8,d10`), followed
by promotion at `0x100090074` (`FCVT d0,s8`). Thus the five narrowed-tempo matches
above now have a source-backed conversion rule. Grid timestamp interpolation
continues to use double precision and actual marks.

## Complete Main timing composition

The new worker composes SDK refinement, Pass1 half-shift/SDK anchor, Pass2 tiny
shift/alignment/patterns, the custom Main anchor/origin update, the final SDK
curve and final grid resampling in source order. It reads independent waveform
arrays and audio-derived duration, plus reference-run original legacy marks
and tempo. No original refined grid, final tempo/downbeat or phase is supplied.

On macOS ARM64 and Linux ARM64, all five development cases now match every one
of 2,498 binary64 grid timestamps, wire tempo/downbeat values, 2,816 float32
cue scores, packed payloads, Base64 and start indices. Expected numeric data
stays outside workers and Linux containers; every worker exits before host
comparison. The strict verifier retains five cases on every failure and rejects
truncated/extra/malformed results. The Linux source package runs 96 tests and
five workers offline with no host mounts, then removes its owned container.

The new SDK-anchor port also matches all 2,816 raw binary64 scores and scalar
anchor values in five same-input Mac SDK references. Four additional controls
cover below/exactly/above 100 BPM and constructor density normalization. Review
corrected the strict half-beat boundary and both SDK/Main short-range guards:
when the rounded last coordinate is below16, curve/anchor construction is
skipped and the default empty/zero result is preserved. The Main custom port
has source/analytic review; no native Main function replay is claimed.

Exported timing/cue fields are exact on both measured platforms for these five
cases. Internal diagnostic numbers have 314 differing binary64 occurrences;
this count covers numeric-float positions, not every raw bit-string field.
Source-faithful platform math and wider cases still need validation before
claiming general internal equivalence or universal output parity.

The key-bank aggregator now uses these computed grids/downbeats and independent
audio-derived caches on both platforms. All 1,760 outgoing binary64 key-feature
values remain exact. Original legacy timing is the remaining supplied timing
producer. Final key classification, energy-request features and final service
cue selection remain separate requirements; production predictions and held-out
measurements are unchanged.

## Classic real-FFT numerical gate

The classic detector uses a separate 2,048-point real ZRIP transform. Its onset
and initial-tempo windows differ in denominator and arithmetic precision;
channel averaging/summing, mean removal and finalization also differ. The
source gate records the exact packing, scale and Nyquist handling.

A system-only probe now retains sixteen fixed synthetic inputs across two
window profiles and sixteen shared buffer displacements: 512 complete jobs,
1,536 outputs and 2,621,440 float32 words. Inputs were frozen before oracle
execution. Shapes, hashes, finiteness, nonnegative magnitudes and measured
alignment pass their bounded checks. No purchased app/SDK or music executes.

Against the zero-displacement profile, all 480 other windows match, but only
60 packed-FFT and 90 magnitude comparisons are exact. The largest absolute
difference is approximately 3.05e−5. These are combined-pipeline observations
with all buffers sharing a displacement; they do not isolate individual APIs
or measure every possible buffer-alignment combination. No tolerance replaces
the exactness gate. Binding the original allocation profile and implementing
the corresponding portable real FFT, cosine and magnitude arithmetic remain
necessary before claiming independent legacy beats.
An independent read-only audit rechecked all 1,561 frozen files and recomputed
these displacement statistics without rerunning the oracle; no material
finding remains in this bounded baseline.

Independent legacy timing remains necessary before claiming complete audio-only
generation of these fields. Server classification and final cue selection are
subsequent boundaries.

## Classic source and default-wrapper follow-up

The expanded classic source map identifies additional real forward/inverse
transforms for initial-tempo autocorrelation. Its default candidate domain
requires powers of two from 16 through 16,384, alongside the 2,048-point
windowed spectra. Histogram preanalysis invokes adaptive tracking even when
final tracking adaptation is zero. Its reset has three floating-point
arguments; internal 16-byte beat marks export a 12-byte projection. Source
review also preserves reciprocal multiplication and divide-before-multiply
rounding. The ordinary application's actual job adaptation remains unknown.

A new bounded reference uses actual FFT factory objects in a complete signed
framework copy. Thirteen configurations, sixteen synthetic inputs and four
heap layouts produce 832 jobs and 5,109,760 finite output words. All native
forward/inverse outputs agree with source-expression system-call replay on
the same allocated buffers. Default float buffers are observed 16-byte aligned
with different addresses modulo 64; the four tested layouts produce no output
differences. All 384 window/packed/magnitude vectors compared against the
earlier 2,048-point displacement-zero reference match exactly. These
observations do not establish arbitrary heap/callback or live-app behavior.

Portable source-derived cosine/window code separately matches 7,211 distinct
cosine inputs and 4,096 coefficient positions on macOS ARM64 and Linux ARM64.
The onset coefficients also match actual native factory output. The periodic
coefficients match the source expression installed with native SetWindow;
the original CDaLoopa constructor is not exercised in that comparison.

Source, saved-output and platform reviews close without material findings.
The numerical reference executes copied low-level framework routines with
network and music access denied. It supplies exact implementation targets;
the independent variable real FFT, autocorrelation math, onsets and tracker
remain to be ported. These synthetic gates do not expand real-file agreement
or change production accuracy. The [request source map](request-json-source-map.md)
keeps the remaining audio-to-JSON and JSON-to-result boundaries explicit.

## Independent real-transform stages

The subsequent Rust implementation now computes all 16,376 setup coefficient
words independently, with exact Mac actual-setup comparison and identical
Linux output. The positive float32 cosine and promoted-angle double sine
have source-derived arithmetic; captured tables are never prediction inputs.
Every required stride-one packing stage is also ported: 352 synthetic jobs
and 1,048,064 float32 words match on Mac and Linux ARM64. Four separate products,
DC scaling, source lane reversal and the duplicated central-bin store order
are preserved.

The complete real8192 transform now composes those stages with the previously
reconstructed aligned complex4096 DAG. Sixteen synthetic forward/inverse
comparisons match all 262,144 float32 words on each platform. Native default
split buffers at this size were all observed aligned64, binding the reused
dispatch profile. The inverse uses swapped input/output planes and no extra
normalization; its fixtures are roundtrips of independently computed forward
spectra. This does not establish arbitrary inverse spectra, other complex
sizes or live-app allocation behavior.

Packed vector math separately matches 139 bounded cases/283,365 words on both
platforms. The magnitude port follows source reciprocal-square-root estimate
and refinement instructions. An independent source review exposed a missing
eight-bin-tail unfused-square rule; the repaired implementation and expanded
reference cases preserve that distinction. Beat-mark container/lookup rules
also have 27 source/analytic tests on both platforms. Neither gate constitutes
the complete onset or tempo tracker.

## Fixed-frame spectrum composition

The independent complex1024 transform now matches 48 synthetic forward,
inverse and initial-stage jobs: 98,304 float32 component words and 4,192
generated setup words. The source-bound radix4 stage retains zero-coefficient
FMAs; the full final-stage symbolic check preserves all 2,048 output expressions
and their in-place addressing. This is a selected aligned system profile.

Its composition with windows, real packing, forward half-scaling and source
magnitude now matches all 32 prepared 2048-point frames and 98,304 words across
both window profiles. Window4 has an actual factory reference. Window7 retains
the source-expression/native-setter reference scope; an original CDaLoopa
constructor was not used. Caller mixing, mean removal, stream scheduling and
lower-threshold processing remain outside this fixed-frame gate.

Small complex8/16/32/64/128, special complex256 and initial radix2/radix8
stages have separate Mac numerical checks. Together with complex1024 and the
fixed-frame composition, all 314 jobs and 492,290 payload words also match on
Linux ARM64. The Linux predictions are compared to independently validated Mac
worker bytes only after all workers exit and the isolated container is removed.
This does not establish Windows or arbitrary floating-point environments.

The two-block middle4096 arithmetic separately matches 16 Mac cases and
262,144 words with explicit numeric coefficients. Full source-only final-stage
checks cover complex512/2048/8192, but complete numerical compositions at those
sizes remain pending. Current prototype guards need reviewed extensions for
valid larger inverse/autocorrelation spectra; existing bounded checks do not
authorize silently widening those domains.

Next, complete those transforms and join the fixed-frame spectrum with onset
extraction and CDaLoopa, then the adaptive tracker. The supplied-legacy condition
on existing Main timing/key/cue proofs remains in place. Production accuracy
and held-out measurements remain unchanged.

## Complete required real-FFT sizes

The remaining complex512/2048/8192 compositions now match 144 Mac synthetic
forward/inverse jobs and 1,032,192 words, plus 42,336 generated setup words.
The8192 dispatcher reuses the coefficient base for its4096 and256 middle
levels; the implementation preserves that reset and the reviewed final scatter.

The resulting real-FFT provider covers all powers of two16..16384. It matches
176 window0 forward/own-forward-inverse cases against retained original-wrapper
references (1,048,064 words), and440 larger own-system-reference jobs
(1,703,104 words), including raw inverse/DC/power controls. The new version
uses separately recorded own-source copies to extend prototype guards without
editing frozen versions. An observed large-roundtrip guard failure was repaired
before reference collection. Forward PCM and inverse packed-spectrum bounds
are separate; finite outputs are checked, without an all-input IEEE claim.

All763 jobs and3,825,696 words match on Linux ARM64, including setup tables.
Forty-nine Rust tests pass in the isolated source-only package, with expected
values retained outside the container until its workers exit and it is removed.
Final source/saved-proof review closed with no material findings, including49
fresh analytic tests and16 rejection controls. This establishes the FFT dependency
for onset and autocorrelation in the bounded profiles, rather than a complete
onset stream, tempo tracker, request or final musical result.

## Complete default onset lifecycle

The independent44100Hz classic mode0/band1 onset stream now composes the checked
window/realFFT/magnitude with source framing, mean removal, three-spectrum Goto
distance, FIR filtering, history statistics and scalar peak picking. Finish
preserves the original zero-padding overshoot, strongest-first clustering and
divide-before-multiply exponential normalization. Sine and exponential use
separate source-bound arithmetic, without host sine/exponential calls.

All16 synthetic schedules match392 initialized-state snapshots and33 final
onset marks against the actual copied component. With67,316 separate math
controls, all2,028,139 float32 and19,890 integer words match on Mac and Linux
ARM64. The state projection includes buffered PCM, all three useful spectra,
histories, statistics, coordinates and initialized mark records. Unused native
padding/tails and exception flags are excluded. This is not a real-music or
full beat-detector result.

Review identified a missing nonnegative-strength premise in an exception for
an ambiguous source suppression read. The corrected prototype rejects that
negative proposal explicitly. Old source/worker/reference evidence stays
separate; new workers preserve every measured numerical output. No new native
reference execution was needed for the correction. The final review has no
unresolved material findings within the stated domain. Mac14 and isolated
Linux101 Rust tests pass; Windows remains unverified.

Next, implement CDaLoopa initial tempo and adaptive tracking, then join their
independent legacy marks/tempo to the existing Main timing chain. Supplying
those original values remains necessary today; complete requests, server
mapping and whole-library accuracy remain separate gates.

## Initial-tempo composition

The default initial-tempo stage now computes its three spectral band histories,
onset curves, unnormalized autocorrelations, two-pass harmonic comparison and
selected phase from PCM. It preserves reused peak/harmonic tails, inclusive
fold cycles, mixed-precision quality arithmetic and separate stored-period,
raw-BPM and normalized-BPM behavior. It uses the previously verified portable
FFT/vector/window components, including every autocorrelation size16..16384.

All18 synthetic cases match the copied original constructor and full component
on Mac, including665,418 float32 and100,050 integer words. Linux ARM64 matches
every packet and passes67 Rust tests; expected answers and derived output
shapes remain outside the isolated package. Cases reach24seconds and select
each of the three bands. Fresh review closed with no unresolved material findings; these bounded results
do not establish Windows or real-music agreement.

The remaining legacy boundary is the seven-hypothesis tracker, its mandatory
adaptive histogram and buffering/finalization lifecycle. The existing five-track
Main timing/key/cue proof still supplies original legacy marks/tempo until that
boundary is replaced. No whole-engine benchmark or holdout result changed.

## Seven-hypothesis tracker

The tracker composition now matches18 synthetic operation sequences against
the actual copied component, including both reset modes, two-stage preprocessing,
backward/forward tracking and reuse of the saved state around histogram tracking.
All148 initialized snapshots and the1,110 final physical marks across cases match
on Mac and Linux ARM64. The checked projection contains25,645 float32 and8,517
integer words. Unwritten source fields are masked; later defined scratch reuse
is preserved, while undefined first-use values produce an explicit error.

Separate variable-power/confidence/decimal-exponential controls match112,060
float32 and41,927 integer results. The combined isolated Linux package passes
all21 jobs and47 Rust tests. Expected answers and derived output shapes stay
outside it; all workers and the container exit before comparison. Fresh review
closed with no material findings, and Windows remains unverified.

This does not yet execute the original classic histogram entry point or implement
the surrounding PCM candidate buffering and finalization lifecycle. Those stages,
the ordinary feed/configuration binding and their composition must replace the
supplied legacy timing before the existing five-track Main/key/cue proof becomes
an independent audio-to-request result. Whole-library metrics are unchanged.

The ordinary Main configuration is now statically traced to detector0, fresh
analyzer adaptation+0.0f and512-frame read requests. Actual returned frame counts
are preserved; a fresh unbounded segment adds no finite-boundary shortening.
The mandatory histogram pass independently uses1.1f. These source findings
identify the lifecycle to compose, without establishing new runtime parity.
