# Inspected analysis-service contract

This combines static observations from Mixed In Key 11.2.6.7446 ARM64 code with
user-authorized live mitmweb transactions from the licensed app. The first
transaction is described below; later targeted captures extend its coverage.
Analysis ran on disposable tuning-track copies; original library files were preserved.
Retained request/response copies redact license and authentication fields.
Field names do not establish mathematical meaning or calibration.

The [request field-to-calculation map](request-json-source-map.md) separates
serialization from the local producers and records the remaining audio-to-request
and request-to-result boundaries.

## Request envelope

`MIKAnalysisServiceClient::analyzeTrack:energy:eliteInfo:tempo:downbeatTime:cuePointStartBeat:preAnalysisScores:error:`
at main-image address `0x100091fbc` constructs a JSON request and an HTTP POST
to `https://online5.mixedinkey.com/Services/Analysis/AnalyzeSong10`.

REA Evidence: `ev_9752a3741b1eddca5d2754dd1a15ff3b20e93c5c643e0a5727d113685abe7425`.
The endpoint was also observed in a live HTTP 200 transaction on 2026-10-16.

| Field | Observed source/type | Interpretation boundary |
| --- | --- | --- |
| Platform | String, `MacOSX` | Protocol metadata |
| ProductName | String, live value `Mixed In Key` | Protocol metadata |
| ProductVersion | String, `11.2.6.7446` | Protocol metadata |
| VIPCode | Authentication string obtained by the original app | Its value was not inspected and is unnecessary for our app |
| Segments | Array of serialized key-feature segments | Described below |
| DurationInSeconds | Last chronologically sorted segment's start plus length | Segment-derived duration |
| EnergySegmentData | Serialized energy segments; added when nonempty | Described below |
| EliteData | Optional supplied object | Live fields listed below |
| Tempo | Number supplied to this method | Already available on the client |
| DownbeatTime | Number supplied to this method | Already available on the client |
| CuePointStartBeat | Integer supplied to this method | Cue context |
| CuePointData | Base64 of a packed float32 score array | Cached-feature score calculation reproduced below |
| KeyAlgorithmVersion | Number returned by MIKNewKeyAnalyzer; 94 in inspected framework | Versioned local feature/analysis contract |
| EnergyAlgorithmVersion | String `2` | Versioned contract |
| CuePointAlgorithmVersion | String `3` | Versioned contract |

The four `MIKService*Key` field-name strings were resolved statically from
`MIKToolkit.framework` constant pointers, independently of any credential value.
REA Evidence for key algorithm version: `ev_398f264cabc6e4e90ef406d74daef624dd2969876956244f3c78598140d09d9e`.

## Per-key-segment request

`MIKAnalysisSegment::JSONDict`, framework address `0x4c24`, serializes:

| Field | Observed content |
| --- | --- |
| StartTime | Segment start, numeric seconds |
| EndTime | Start plus segment length, numeric seconds |
| PitchProbabilities | Exactly 12 floats copied from the segment's `power` array |
| Features | The segment's numeric feature array |

REA Evidence: `ev_cbd4cc937c925c924f4c9d5acf6594e0e55d02399e3998068e8bb41e370ab2fb`.
The pseudocode omits some variadic dictionary arguments; the assembly and
referenced constant names establish the four keys.

The ordinary successful whole-file Main path creates one fresh segment,
initialized with `calloc(1, 48)`, and leaves its twelve power floats at positive
zero. The selected bank accumulation writes pitch/features in other fields;
per-segment key processing is skipped. This source-bound no-write result and
all five retained requests agree. Other segment-construction paths can differ.
The field therefore does not provide normalized key probabilities or final key
predictions on this ordinary path.

The inspected `MIKNewKeyAnalyzer` methods copy vectors of doubles from a local
feature extractor into `Features`. Time-range extraction can choose a raw or
processed vector; accumulation can add a raw vector at a supplied index offset.

- Raw range: `ev_ecda3c45c4c2cb6e53e83a740dab27520d019af66fd32a39aa98ec463a602c4e`.
- Full result: `ev_a1ea6c3edc13aae69779ee8fbee19c3b913d101ac2caaafbb7984986bded7b27`.
- Accumulation: `ev_2ad95cf0384dceb9c92190a6b706a3ad4bbe2307379965ff515684d331cf2e0e`.
- Processed range: `ev_fa4a9cd4fb1fc784afee2617068029011f8311a18a48337c29af3e9f8f5ca0e8`.

`KeyFeatureExtractor::GetResultsForTimeRangeHarmonicity` allocates two
88-element double vectors. `StaticProcessFeatures` accepts an 88-double vector,
applies weighting/compression and normalization. This establishes an inspected
88-bin path, not that every outgoing `Features` array always has 88 entries or
that its entries are standard chroma, MFCC or model logits.

Evidence: `ev_ead45917a2a339097d1bd801cb7779d752b4373a894324f944030c3d2b20a506`,
`ev_2643c839d07fc63bae3db7411d40fe712646b38e84ccea5ba8b6f75867375a8c`.

## Per-energy-segment request

`MIKEnergySegment::JSONDict`, address `0x156ec`, sends `StartTime`, `EndTime`,
`VolumeRmsDb`, and `Features` (array or JSON null). Evidence:
`ev_ba51bb5eab010582fc6a7c3d0857c823503297cbc8a34c54db22cac8b85ad710`.

`MIKAnalysisCuePoint::JSONDict`, address `0x9fc4`, sends only `Time`.
Evidence: `ev_f05938c79063ae46dbcaa5a2228ec71348b9b3484e71f1fa7c85c1e32b82766a`.
This simple cue serializer is distinct from the request's packed `CuePointData`.

## CuePointData encoding

Main-image helper `0x1000933b0` converts a numeric score array to float32,
counts leading/trailing exact zeros (each count capped at255), and emits two
count bytes followed by retained little-endian float32 values. It Base64 encodes
that buffer. The POST builder assigns its returned string to `CuePointData`.
Empty/all-zero source behavior is recorded in the private recipe; the initial
five captures did not exercise it, but later short controls do. The cached-feature calculation and beat
coordinates are described below; final cue selection remains unresolved.

Helper pseudocode Evidence:
`ev_bf00d3b6c58e6c97323eccafd300ce8bd9008986805dd8f801cc9b3aab782a70`;
assembly: `ev_8ac331027edca8e7b25bdc6bfc95184a9b2c1dc50d2dfc3ef7feaaa7c32b8f18`.

## Live request and response

mitmweb12.2.3 captured the licensed app's own POST and response, without replay
or changing forwarded payloads. Process capture was restricted to the app PID
and TLS host `online5.mixedinkey.com`. Transparent mode exposed an IP address
as the parsed request host, so the exporter also checked the TLS server name.
No system-wide HTTP/HTTPS proxy was enabled. Live interception was stopped
after the successful transaction.

The request was132,847 transport-body bytes; response1018 bytes, HTTP200.
Retained JSON preserves original nonsensitive numeric lexemes. This is one
observation, not evidence that every track uses these dimensions or values.

| Request item | Live observation |
| --- | --- |
| Segments | One segment,12 PitchProbabilities values and352 Features values |
| EnergySegmentData | 22 segments; first has183 Features values |
| EliteData | SongName, Genre, FileType, AlbumName, ArtistName, LengthSeconds, FileName, Tempo |
| CuePointData | 3416 Base64 characters →2562 bytes: headers48/80 and640 finite float32 values |
| CuePointStartBeat | 32 |
| Versions | Key94, energy wire version`2`, cue wire version`3` |

Wire algorithm versions differ from embedded result-tag versions (energy13,
cue14); do not equate the two version namespaces.

The response supplied one key segment with confidence,11 energy segments,
eight cue points with `Time` and `EnergyLevel`, `OverallEnergy`, `Error`, and
key/energy/cue profile timing fields. `KeySummary` contained MainKey,
MainKeyIsSingleNote, MainKeyConfidence, SecondKey, SecondKeyIsSingleNote and
SecondKeyConfidence. One result cannot establish confidence calibration or
the behavior of secondary-key detection.

## Consumed response

The main client parser checks `IsLicenseValid`, then reads:

| Field | Observed consumption |
| --- | --- |
| KeySummary | Required summary object with non-null MainKey |
| Segments | Parsed into key-segment objects |
| EnergyLevelSegments | Parsed into energy-segment objects when present |
| CuePoints | Parsed into cue-point objects when present |
| OverallEnergy | Converted to a float |

The key-segment parser reads `StartTime`, `EndTime`, `Key`, `KeyConfidence`,
and `IsSingleNote`. Evidence:
`ev_2dbbd301c77ae2682575f3dcfd50498f3d2fbe14094893ecfe5a7b4cb8215ece`.
The energy parser reads timing and `EnergyLevel`; the cue parser reads `Time`
and `EnergyLevel` (independent static disassembly observations).

This is the consumed subset, not a claim that the server sends no other fields.
The live observation extends this consumed subset; optional response fields and
the server-side classifier remain unresolved.

## Exact request and response reconstruction

The user requires matching analysis values and outputs. Treat these as two
separately testable stages:

1. Reproduce the original numeric analysis JSON, including feature values,
   timing context, score-array packing and algorithm versions.
2. Reproduce key, energy and cue results from that JSON using offline code or
   models. Test this mapping without rereading audio.

Matching the numeric request establishes the original client's analysis inputs.
The request does not itself contain the service's implementation or learned
weights. Paired requests/responses provide evidence for reconstructing that
mapping; matching only the JSON field names would not establish either stage.

The initial five captured exchanges cover different tuning tracks. Controlled original-component validation
matches every energy-request field on all five from audio alone, and all352 key
feature values when supplied with the original archived beat context. These are
private reference tools, not independently implemented Rust extractors.

Initial cue-only JSON probes matched26/36 returned cue beat locations with a
shared smoothed-score selector, including one complete ordered position list.
They did not reproduce every cue count, duplicate slot, energy label or
binary64 timestamp. The selector receives request JSON only; expected responses
are confined to evaluation. These five-pair development results are not a
whole-library or held-out agreement claim. Later component and selector results
are recorded below.

The cue payloads restore to768/928/160/384/576 scores. In these captures every
nonfirst returned cue maps into a positive score at restored index
`beat_offset + CuePointStartBeat`; the returned offsets lie on a four-beat
lattice. Keep the restored zero prefix when interpreting indices. One short
track returns four identical beat32 slots; do not discard those duplicates.
Exact timestamp arithmetic has since been reproduced with a two-point beat
construction, independently of integer cue-position selection; see below.

REA's default-pass trace distinguishes two client stages. Pass0 uses main-image
`0x1000c43ec`, granule helper`0x1000c5744` and score kernel`0x1000c37a8` to
choose timing phase. Pass1 calls the SDK's `DetectLikelyCuePointsV3` and its
`GranuleFeatures::WeightedAverage` / `DifferenceSum` helpers; that pass supplies
the final raw score array. Serializer`0x1000933b0` then converts it to the
packed field. The inspected local optimizers select one anchor, which is
distinct from the service's final cue-slot list.

A read-only decoder recovered324480 exact float32 amplitudes from the five
captured tuning records, with complete byte-identical re-encoding and no audio
analysis. Calling the original SDK curve generator with those cached numerical
waveforms and separately supplied stored beat context reproduces **every
CuePointData byte and CuePointStartBeat on all five requests**. The result
validates the traced final SDK path for these captures. The SDK's extra selected
anchor differs from the wire DownbeatTime and must not be substituted for it.

That reference tool reads no track or service response and makes no network
request. A separate independent Python implementation now reproduces the same
five captured payloads: **2816/2816 restored float32 scores, every packed byte,
every Base64 string and all five CuePointStartBeat values match exactly**.
The parent independently reran both the corpus evaluator and the two regression
tests successfully. The prediction worker receives cached waveform arrays and
explicit stored beat context; only the evaluator reads captured requests. It
does not load the purchased SDK, decode audio or contact the service.

The trace also distinguishes two score vectors. Full rescoring changes the
internal vector at+0x48, while the wire vector at+0x70 retains the original
guarded scores. Feature-derived onset indices32/32/31/32/32 explain the observed
48/48/47/48/48 zero prefixes; CuePointStartBeat remains32 after its separate
minimum-bound calculation. These values are computed, rather than copied from
captured headers.

A later complete waveform pipeline now computes these inputs from the five
development audio copies independently. On both macOS ARM64 and Linux ARM64,
all 324,480 waveform amplitude words and the raw numeric waveform serialization
match the retained app data. Feeding those generated waveforms and audio-derived
duration into the unchanged cue mathematics reproduces all five previously
capture-validated cue-request references: 2,816 score words, payload bytes,
Base64 and start-beat fields. That initial composition supplies original refined
timing. The latest composition now computes the full Main grid, tempo and
downbeat as well, matching all 2,498 timestamp words and outgoing cue fields
on both platforms. A subsequent independent classic PCM composition removes
that legacy-input condition on these five development cases, preserving all
4,707 legacy beat times, five BPM values and the downstream outputs. Details
and limits are in
[waveform reconstruction](beat-waveform-reconstruction.md).

The initial cached-feature tests establish the cue-request mechanism on five
development captures under supplied waveforms and timing. The later audio
compositions above progressively remove those intermediate inputs. Entire-request
reconstruction, the short-input initialization gap described below and the
service's final selection rules/model remain separate tasks. Private
initial implementation and provenance are retained in request-cue-recipe-v1;
its parent verification report is captured-curve-parity-parent-v1.json.

The same numerical waveform decoder recovered all282 tuning records without
audio access. Under identical explicitly supplied cached contexts, independent
Python and portable Rust implementations both match the original SDK on
**282/282 curves, all217408 float32 values, every payload and every start index**.
This is a broader component test, not282 captured request matches. Original
pre-request downbeat/duration values are unverified outside the five captures.
No held-out records were accessed. The Rust component passes the same exact
five-capture and282-case comparisons on macOS ARM64 and Linux ARM64. Linux
validation used a pinned existing image with networking disabled and expected
payloads outside the container. Windows execution remains unverified.

Ten further actual captures exercise all-zero curves and a nondefault start
index31. The unchanged Rust scorer matches every payload/start value and all1568
scores on macOS ARM64 and Linux ARM64. Four subsequent captures from other tuning
sources also match on macOS ARM64 and Linux ARM64, giving19 actual capture cases and5024/5024 exact
score values. These include related derivatives, not19 independent tracks or a
held-out benchmark. The14 additional waveforms are decoded from only their named
disposable cache records. Actual captured D/T/duration are explicit component
inputs; no audio or service is accessed by this cache-only validation.

## Request timing lifecycle and JSON-only phase

The request caller obtains DownbeatTime from the first refined beat, after
subtracting the codec waveform time offset. After receiving the response, the
client overwrites that property with the first returned cue, then rephases the
saved beat grid using a ceil operation. The saved grid is therefore a later
state than the grid used to construct the request. Equal fields in five captures
must not be generalized to all cached tracks. The full lifecycle trace is
retained privately in wire-downbeat-context-v1.

SDK helper0x35108 multiplies five interpolated modular-distance factors for
periods32/16/8/4/2. With its final zero weights for periods4/2, positions outside
the four-beat lattice receive zero weight. Hanning helper0x35200 uses ceil when
constructing its half-window; width32 therefore includes offset16. Reading only
pseudocode's inferred casts would miss this boundary.

REA Evidence: ev_60846cf9a25d87ea690e8062ecafd5b407384c8425736256a75cd2f9a8b1900d
and ev_4b5c9b5d6234e09c14e4609e54874a19d5f2556e73045ade55f192bddeb8e1de.
The full SDK tail assembly is retained as
ev_0a77db7ba340cdba5538f68f726fe1dc109104c3433d348dcf81d1ed6554ba59.

A separate worker now reconstructs that local single-phase calculation using
only CuePointData and CuePointStartBeat. Its integer phase matches the original
SDK on282/282 cached cases, despite receiving serialized float32 scores instead
of the SDK's internal doubles. The phase also explains the modulo-four alignment
of all1913 nonfirst reference cues, including27 positions on four tracks that
previously appeared off the common lattice. The strengthened evaluation uses a
computed Start−32 origin, without taking its origin from the expected first cue.
All native integer-phase targets have zero inversion residual; reference cue
coordinates have a maximum1.903e−7beat residual from integers. This establishes alignment for this
development corpus, not final cue positions or exact timestamps, and does not
prove that the server uses the same local optimizer. Private source, evidence,
red/green reports and boundary tests are retained in json-cue-phase-v1.

Finite selector probes with corrected modular weights, peak filtering and
spacing constraints still reach at most26/36 cue beat locations on the five
actual captures. No such selector has been adopted. Counts, duplicate slots,
exact positions and binary64 timestamps remain required work. Conditional
cue-energy annotation has since been reconstructed as described below.

## Conditional cue-energy annotation

The older SDK cue-list routines0x42078/0x447ac do not reproduce any complete
current captured cue list; they are not substitutes for the service picker.
Their energy annotation tail nevertheless exposes a separately testable rule:
sample the classified energy timeline at beat-granule centers, then take the
integer median from a selected cue to the next strictly greater selected cue
index. Repeated cue slots therefore share the same interval. For an even count,
average the middle pair with integer truncation, and clamp the result to1..10.

Given the original selected times and original classified energy timeline, an
independent implementation matches all36 cue energy labels in the five captures.
Using the library's saved timeline directly initially matches2156/2195 labels
across282 tracks. Those saved boundaries have undergone client processing.

Main duringOperation0x10008dd58 applies two distinct roundings: choose the nearest
individual beat, then round the integer index relative to its cue-derived anchor
onto a16-beat lattice. The nearest-beat lookup chooses the earlier beat on exact
ties; the16-beat stage uses round-away-from-zero. Combining these operations into
one rounding changes results near boundaries. The last saved energy interval is
extended to the full track duration. The first saved energy boundary identifies
an anchor grid point, not a universally verified original request DownbeatTime.

REA Evidence: ev_e33d2b8168c28c02f22c58bbe27658488c2e1347b63b3e18f3439210ddc2d90f.
The legacy picker/annotation source is retained as
ev_e9facb6ebba05b141adc5014212acf99e4216c7828c5eef58fcb0fa7f8d87ec0 and
ev_db42564451d3765ef15477dcf827318628c42ccc0beb7e35addd8ce05f0da856.

An inverse using original-component energy-feature interval boundaries and the
saved classified segments recovers unambiguous raw transition starts on282/282
tuning tracks. All2536 internal ends also invert uniquely using exact stored
length subtraction; reconstructing end as start+length alone introduces eight
one-ULP differences. Original raw final ends are lost to the duration override
and remain unverified outside the five captures. Recovery receives no cue times
or cue-energy labels. All46 captured
segment StartTime/EndTime/EnergyLevel fields match exactly; other response fields
such as Volume are outside this recovery. Feeding the recovered timelines and
already selected cue times into the median annotator then matches **2195/2195
cue energy labels on282/282 tracks**, including duplicate slots. No fallback,
per-track correction or excluded failures is used. Six boundary tests cover the
two roundings, tie directions, ambiguous inverse rejection and duplicate windows.

This closes the annotation calculation on this development corpus. It does not
implement the energy classifier that produces those timelines, infer the cue
positions supplied as inputs, recover every original request context, or establish
held-out performance. Private scripts, provenance and reports are retained in
raw-energy-timeline-recovery-v4 and json-cue-energy-median-v1. Earlier failed
single-rounding and saved-timeline probes remain preserved.

Both the JSON-only phase calculation and the conditional median annotator have
also been implemented in a private portable Rust component. On macOS ARM64 and
Linux ARM64, all282 native phase targets and all2195 cue energy labels match,
along with the five actual captured cases. Twelve boundary/protocol tests pass
on each platform. Windows remains unverified. These tests supply original
classified transitions and selected times; they do not establish a complete
offline service replacement.

## Exact timestamp conversion from selected integer positions

A source-grounded two-point construction reproduces all108 cue timestamp
binary64 values across19 captured exchanges. Given an already selected integer
beat offset b, construct the values with separate binary64 operations:

```
p = 60 / Tempo
t0 = DownbeatTime
t1 = t0 + p
step = t1 - t0
time = t1 + (b - 1) * step   if b >= 1
time = t0 + b * step         otherwise
```

Do not fuse the final multiplication/addition. Direct `D+b*(60/T)` matches
only35/76 values in the initial15-case test, and a fused two-point expression
matches67/76. The unfused construction matches76/76 there and32/32 in four
subsequent cases without changing the worker. It also explains the nondefault
start31 case's four-ULP difference from the direct expression.

SDK0xc6070 GetTimeAtNormalizedBeatPositionInternal supplies the two-entry
extrapolation branch structure; wrapper0xc6060 applies normalization. Retained
Evidence: ev_dc4be32ad3a0cf9d582317d8cd91e1c512510ad2c1152c73857207513210d7d6.
The inspected ARM64 implementation uses fused instructions. A synthetic grid
`[D,D+60/T]` and unfused evaluation are therefore an observational reconstruction
of the service result, not proof of its backend runtime or a Windows build.
The tested published legacy number parser preserves all examined request/time
bits and does not explain the discrepancies.

The first returned score coordinate equals CuePointStartBeat in all19 captures,
so its beat offset is Start−32. A16-second control with Start31 returns a first
cue one beat before D. It is incorrect to generalize the initial five captures'
first-cue-equals-D observation to every request. This also gives an actual case
where the saved first beat/energy anchor differs from outgoing D.

An isolated Rust V2 component reproduces the initial15/76 timestamp test on
macOS ARM64 and Linux ARM64, preserving the original phase/energy tests. Its
unchanged binaries also pass the separate four-case prospective extension:
32/32 additional timestamp bits, for19 cases and108/108 bits on both platforms.
The original frozen proof remains unchanged. Conversion still receives selected integer positions;
it does not solve their selection.

## Independent cached key-feature aggregation

An independent private Rust component now reproduces all352 outgoing key-feature
values on each of the original five captured requests: **1,760/1,760 binary64
values on both macOS ARM64 and Linux ARM64**. It receives previously exported original88-value
float32 frames, the archived refined beat grid, downbeat and decoded duration.
The worker reads no audio, loads no original framework and contacts no service.
Expected requests are opened by the evaluator only after the worker exits.

The recovered range calculation uses three-frame medians, fractional endpoint
weights, full-span normalization even when cache boundaries clip the interval,
and fused binary64 accumulation. Full four-beat windows accumulate raw values
into four banks at `(bar & 3) * 88`. The source's unusual time-to-beat comparison
direction and separate rounding operations are preserved. The component handles
the observed full-file segment path; general multisegment and nonfinite edge
behavior are not established. Source inspection does not require every output
array to have352 elements; its length depends on which banks are visited.

Retained range and accumulation Evidence:
`ev_ead45917a2a339097d1bd801cb7779d752b4373a894324f944030c3d2b20a506`,
`ev_2ad95cf0384dceb9c92190a6b706a3ad4bbe2307379965ff515684d331cf2e0e`.
Eight synthetic/contract tests pass, including irregular-grid extrapolation and
FMA cancellation, on both platforms. Linux used the frozen source with cached
dependencies and networking disabled; expected outputs stayed outside its worker.
Windows execution remains unverified. This is a cache-conditioned component result. Independent
PCM-to-cache reconstruction and the service's final key classifier remain open;
the production key benchmark has not been rerun or improved by this experiment.

## Independent spectral-peak-to-cache reconstruction

The preceding aggregation stage now also accepts independently reconstructed
cached frames. A second private Rust component receives original tracked
spectral peaks and finalized tuning, then reproduces **2,379,872/2,379,872 exact
float32 cache values across27,044 frames** on the five original development
tracks on macOS ARM64. It loads no original SDK and reads no audio or service.
Nine synthetic/contract tests, formatting, Clippy and release build pass.

The source uses fractional FFT-bin peak coordinates, wrapped tuning compensation,
integer and half-semitone raised-cosine grids, A-weighting, compression and
neighbor suppression. Both grids have89 cells. Only cells0..87 are compressed;
the raw guard cells can affect the highest retained output bin. The precise
float32 suppression order, double cosine/FMA kernel, and initial empty frames
are retained. Evidence for constructor0x1a578 and finalizer0x1bab8:
`ev_5d2d465d2501cef02bda0ef66f6b06f319027582fa8eb8348d7f2f03b6db5d2e`.

Running this new component and the bank-aggregation component in sequence
matches all1,760 outgoing key-feature values on macOS. The second worker receives
only the first worker's newly computed cache and the archived beat context.
Original cached88 values are therefore no longer required as worker inputs for
that two-stage proof. It still supplied original tracked peaks, finalized tuning
and beat context. The subsequent tuning reconstruction below removes one of
those supplied inputs; independent PCM-to-peak extraction and service key
classification remain unresolved.

The first Linux run of this cache component failed exact parity:1,206 of
2,379,872 values differed. Bounded helper tests and100/200-digit arithmetic
checks isolated platform logarithm/power rounding. Simply promoting power to
double made its arithmetic closer to nearest binary32 but changed the reference
macOS approximation; four diagnostic variants were retained without accepting
any of them.

A separate handwritten Rust implementation now reproduces the pinned runtime's
positive finite power approximation, including its rounding errors. It matches
22,729/22,729 synthetic native results on macOS, including subnormal inputs.
The revised cache component combines this power implementation with the measured
double-logarithm-to-float32 path and again matches all2,379,872 original cache
values on both macOS ARM64 and Linux ARM64. All17 tests pass on both platforms.
Its composed key-request output also matches all1,760 feature values on both
platforms, using newly computed caches and the unchanged bank aggregator.
Windows and full PCM inference remain unverified. The
measured corpus does not establish every possible floating-point input.

## Independent finalized tuning

A separate Rust implementation now computes tuning directly from retained
spectral peaks. On macOS ARM64 it exactly matches all432,704 original float32
score values and27,044 original binary64 tuning values on the five development
cases. Six empty-peak controls additionally match1,104 score values and69
tuning values. Nonempty zero-score curves are nonzero because of the optimizer's
deterministic first-tie rule; they must not be replaced with zero tuning.

The composed three-stage chain computes tuning, then cached frames, then key
request banks. All1,760 outgoing feature values match exactly on both macOS ARM64
and Linux ARM64 using tuning V2, cache V4 and the unchanged request aggregator.
Workers receive retained peaks and projected archived beat context; no original
finalized tuning or cache values are inputs. Original-derived retained peaks
remain required, so this is not independent PCM-to-request reproduction.

The source trace corrects the decompiler's three-double HFA return and negative
phase rounding, and preserves search ordering, fixed search seeds, strict winner
updates and sequential overlap blending. The initial Linux run matched final
tuning but differed in15,007 raw scores. A source-derived portable logarithm
removes that discrepancy: all432,704 real scores and27,044 tuning values now
match on both platforms, as do all controls. Thirteen tuning tests and23 cache
tests pass on both. The fresh Linux chain also matches all27,044 tuning values
and2,379,872 cache values before its final1,760-value request comparison.
Double pow/cos remain tuning dependencies, and cache uses cos/exp2f.
[Mechanisms and verification](key-tuning-reconstruction.md)
record the exact source/profile scope. Full engine agreement remains unproven.

## Independent spectral candidate and tracking stages

An independent Rust spectral-input component now matches every candidate queue
and retained peak on 57 synthetic sequences / 327 frames on macOS ARM64 and
Linux ARM64. Its newly computed peaks reproduce all 5,232 original score values
and 327 finalized tuning values through the unchanged independent tuning stage.
Original peak or tuning vectors are not worker inputs for this synthetic chain.
The separate five-real-track feature proof above retains its original-peak and
archived-grid input conditions.

The first Linux tracker run failed on five dense-spectrum cases. A pinned
exponential approximation, independently checked on 308,459 native inputs on
each platform, removes all measured differences. The failed proof remains
retained; no tolerance or analysis threshold changed. The
[peak reconstruction report](key-peak-reconstruction.md) records the native
nonfinite edge failure and remaining decoder/streaming/real-track gates.

## Independent real-audio key-feature frontend

The original-peak input condition of the earlier five-track proof is now removed
by a subsequent independent real-file pipeline. On macOS ARM64 and Linux ARM64,
guarded FLAC decoding, source-derived packet boundaries, channel summation,
scheduled FFTs, peak tracking, tuning and cache generation reproduce all five
development cases exactly. Complete PCM stream digests cover 110,795,692 float32
samples; all 749,737 peak pairs, 432,704 raw scores, 27,044 tuning values and
2,379,872 cache values match their native references.

Fresh caches produce all 1,760 outgoing key-feature values exactly on both
platforms. Frame counts and duration are audio-derived. The first composition
supplied the original refined grid/downbeat; a later bridge now uses computed
Main timing and preserves all 1,760 exact feature values on both platforms.
A subsequent five-development-track PCM composition also computes the legacy
marks/tempo independently; all Main timing/cue and key-bank outputs still match.
No original peak, tuning, cache,
duration or expected feature arrays enter the workers. The validated decoder
profile is source-rate 44,100 Hz/stereo/16-bit FLAC, including the original
packet-dependent tail behavior. This is not full independent request generation
or final classifier parity. The [real-audio reconstruction report](key-real-audio-reconstruction.md)
records the synthetic scheduling gate, real-file digests and remaining work.

## Short-window cue selection and remaining ranking gap

Four8/16/24/32-second controls send all-zero curves and receive one cue. A
64-second loop sends a positive192-score array and receives three cues. The
initial short160-score case instead returns eight slots, including four copies
of the same later position. A PCM-identical full control repeats all inspected
analysis request/response fields exactly;40/48-second loops reproduce that
eight-slot pattern with different score curves. This is repeatable behavior.

The SDK taper's geometry is identically zero for short effective windows below
160 positions even when raw scores are positive. A rule inferred from the
observations was frozen before four additional controls on other tuning sources
and a shifted loop. It predicted all four complete eight-slot lists exactly.
The validated positive-score cell remains N160/Start32/effective128; positive
nondefault starts and other range edges remain unverified. The server's fallback
implementation has not been obtained. Full phase modulo32 is also contradicted
by a loop with computed phase16 but cues at0/32/64; only modulo4 alignment is
supported by that result.

A private hybrid combines the frozen zero/short-window rules with the previous
mean9/gap32 baseline for ordinary curves and the exact timestamp converter.
On19 actual development captures it matches103/108 timestamp slots, with109
predictions, five missing references and six extras. Fifteen complete ordered
lists and17 counts match. The initial five cases improve from26/36 to32/36
matched positions; this is not a comparison of independent held-out tracks.

The broader282-track check matches1808/2195 integer positions and51/282 complete
lists under explicitly provisional cached request contexts. Exact outgoing
timestamps cannot be graded from those contexts. Ordinary ranking/counts,
classification and the complete independent PCM pipeline remain unfinished;
no experimental picker has been adopted into the production engine.

All three targeted capture sessions have stopped. Their28 new sanitized JSON
files pass credential-redaction checks, the local web listener is absent, and
no replay or forwarded-payload modification was used. Analysis and tag/export
settings, the installed bundle and original music were preserved; the reference app
contains14 newly imported disposable test records.

## Latest ordinary request and energy boundary

The ordinary fresh whole-file key segment is now independently assembled from
the computed PCM frame count and key banks. Its start/end, twelve zero power
values and 352 features match all five retained development requests, along with
DurationInSeconds. The same fields match on macOS ARM64 and isolated Linux
ARM64. The latest Linux composed PCM run uses independently Mac-decoded audio;
it is not a new Linux FLAC decoder execution. These new real joins still need
fresh combined review and production integration, and do not establish broader
library, held-out, other-format or Windows fidelity.

The default energy constructor's fourteen frequency-band limits, 1,025
weighting values and 2,048 periodic-Hann values also match copied-component and
Linux Rust checks. The following stereo FFT/RMS frame and PCM stream stages also match 17 bounded
synthetic controls on macOS and Linux, including 49 streaming snapshots.
The unchanged numeric stream also matches a controlled five-development-PCM
cohort on macOS/Linux, through 81,216 emitted RMS frames and selected current
spectra. That run supplies explicit audio-derived full-frame duration and zero
skips; it does not newly observe ordinary-app runtime arguments or compare every
frame's spectrum.

A subsequent Rust stage matches the fourteen sustained/transient band pairs,
eight smoothing-history vectors and default global normalization. All twenty
synthetic/scalar jobs match on macOS and isolated Linux; independent review
replayed them exactly and ran 26 tests. The same five development PCM packets
also match copied-component macOS projections and isolated Linux computation,
including all band timelines
and final normalized state: 30 snapshots and 5,339,971 float32 words. That
separate real-data check retains explicit audio-derived full duration and zero
skips.

Temporal descriptions and the default 183-value producer have now also been
ported to independent Rust. Ten synthetic band/RMS controls match the copied
component on macOS and the Rust computation on isolated Linux ARM64. This covers
the direct autocorrelation and energy-tempo path, overlapping segment geometry,
raw segment RMS, four phase families and all 2,379 feature values from thirteen
accepted segments. Two rejected segments emit empty records. Twenty tests pass
across the temporal/reducer crates. These are numeric timeline controls with a
declared full weighted-RMS slice.

A subsequent composition now feeds the previously retained independently
PCM-computed raw band/RMS arrays through the finishing stage. All five retained
development `EnergySegmentData` arrays match: 81 segment starts, 81 ends,
81 RMS dB values and 14,823 features. Complete component projections match on
macOS and Linux ARM64. The first real Linux run's nine dB discrepancies were
isolated to host `log10`; the source-derived primitive closes them and matches
8,496 scalar controls on both platforms. The failed run remains retained.
No tolerance or per-track adjustment was introduced.

This uses explicit full decoded duration, zero skips and a full weighted-RMS
slice. Captured-field agreement supports that profile on these cases; runtime
arguments were not newly observed. It reuses our PCM-derived arrays rather than
performing a fresh decoder/production pipeline run. Alternative modes,
broader/held-out/Windows/full-request agreement and final energy classification
remain unfinished. [The energy-field report](energy-field-reconstruction.md)
details the result.

A separate eight-second synthetic pulse success test still fails during the
mandatory legacy histogram pass: zero backward iterations leave no proven phase
values for the first forward iteration. Audited application callers supply no
initializing store; earlier native-provider stack contents remain unresolved.
The independent engine preserves an explicit unknown error. This does not prove
that the original application errors or is nondeterministic, and the successful
five real tracks do not remove the gap.

The [current field map](request-json-source-map.md) records these latest producer
boundaries. Complete independent numeric request generation and offline final
key/energy/cue decisions remain unverified. Matching client inputs alone does
not recover service classifiers or weights.

## Retained records

Complete local records are stored outside distributable source:

- `.analysis/mixed-in-key-11.2.6/rea-main-evidence-complete.json`.
- `.analysis/mixed-in-key-11.2.6/rea-framework-evidence.json`.
- `.analysis/mixed-in-key-11.2.6/rea-cue-followup-evidence-v4.json`.
- `.analysis/mitmweb-analysis-v1/`: sanitized request/response JSON, metadata,
  a sanitized mitmproxy flow, capture addon and synthetic safeguard test result.

These include recovered pseudocode and provider limitations. High p-code value
flow is truncated for some large methods; the conclusions above rely on the
retained pseudocode, assembly, constants and explicit parser calls, not on a
claim of complete interprocedural flow. Runtime claims are limited to the live
transaction described above; no server model or weights were obtained.

## Integrated independent feature command

The workspace now has an offline single-process `analyze-features` command. A
fresh supported FLAC decode feeds the independent legacy/Main/key/energy stages;
all ten numeric analysis fields match the five captured development requests.
Fresh Linux ARM64 runs also reproduce the complete macOS output JSON bytes. No
authentication, EliteData, full HTTP body or final service decision is recreated
by this command. [Integration scope](file-request-integration.md) records its
validated input profile, source provenance, tests and remaining requirements.

## Fresh full-corpus request comparison, 2026-10-17

The actual licensed-app request/response pairs now cover434 physical files and
364 unique audio groups. Development uses the original282 representatives only;
82 reserved groups remain sealed from interpretation and scoring. The previously
frozen independent Rust audio-to-request outputs match all ten fields on282/282
fresh requests:1,254,474 binary64 numbers and846 strings. This includes every
packed cue-score string, full key/energy feature arrays, timing and algorithm
versions. No numerical parameters changed after capture. This removes the prior
cached-context uncertainty for these development representatives; it does not
establish the server's final classifier or cue-picking implementation.

The independent phase worker, given audio-derived cue-score data, explains the
four-beat lattice of all1913 non-first response cues. Four tracks have a request
start index other than32, so the beat-index origin must be applied explicitly.
This yields a candidate four-beat grid, not a human-validated musical meter.

Actual response `CuePoints[].Time` values are seconds. A fixed two-anchor
converter conditioned on the response-selected integer beat positions matches
all2195 wire timestamps bit-for-bit. This is a conditional timestamp test, not
independent cue selection. Stored library timestamps match2097/2195 wire values;
98 first-cue slots differ by at most2.78e-17 seconds. Keep strict wire/stored
transport comparisons separate from DJ-quality tolerance measurements.

The user now prioritizes useful musical results over exact cloning. See
[cue-quality comparison](cue-quality-comparison.md) for a fixed native experiment
and the remaining listening requirement. Reference agreement cannot by itself
choose the best cues.
