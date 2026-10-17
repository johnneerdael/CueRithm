# Engine measurements

Measurements through 2026-10-17 on Apple Silicon macOS. These are agreement tests
against supplied Mixed In Key outputs, not independently annotated musical
truth. The v7 analysis backend also passed Linux ARM64 container tests and real
model smoke checks. Windows execution and held-out agreement remain unverified.

## Current quality priority

The user has made musical/DJ usefulness more important than making an exact
copy. MiK agreement is a reference measurement, not musical ground truth or a
requirement to copy every cue. Exact numeric comparisons remain useful for
verifying reconstructed calculations. Keep the UI deferred until the offline
engine's key, BPM, energy and cues have credible quality evidence, clear
unknown/error behavior, and verified format/platform support. The historical
exact gates below describe earlier experiments rather than the current product
acceptance rule.

## Fresh captured-request verification, 2026-10-17

Passive licensed-app captures now pair requests and responses for434 physical
files, representing364 unique audio groups. The282 development representatives
are selected once per group;82 reserved groups remain excluded from development.
Credentials are redacted, while other numeric JSON lexemes are preserved.

The previously frozen audio-only Rust predictions match **all ten analysis
fields on282/282 real requests**:1,254,474 numeric values have exact binary64
agreement, and846 strings match, including all282 packed cue-score strings.
No parameters changed after capture. The normal-result timing backend separately
matches every request's BPM and first refined beat anchor. This establishes the
supported FLAC profile's audio-to-request calculations; it does not reproduce the
service's final key/energy classifiers or cue selector.

The fixed phase worker explains all1913 non-first response cues' four-beat
alignment. The282 candidate grids contain49932 bar anchors under an explicit
four-beat-meter assumption; this is not human validation of musical downbeats.
Four tracks need a request-coordinate origin other than32. Given the selected
integer beat positions, the existing two-anchor timestamp formula reproduces
all2195 wire cue timestamps exactly. Positions were supplied from responses in
that conditional test, so it does not establish cue selection. Wire `Time` is
seconds. Stored times differ from wire time at98 first-cue slots by at most
2.78e-17 seconds; binary64 storage equality remains a separate diagnostic.

Private reports: `fresh-request-feature-audit-v1`, `fresh-request-timing-audit-v1`,
`fresh-cue-grid-audit-v1`, and `fresh-cue-time-audit-v1`. No production code or
reserved results were changed/used by these audits.

## Quality-first cue experiment, 2026-10-17

A native Rust research worker compares three fixed policies using identical
retained PCM-derived structure features and the same candidate beat/bar grid.
Every one of282 tuning jobs succeeds; runtime has no reference-content or
network access. The existing core novelty function is unchanged. The CBM arm
uses eight acoustic means per complete bar and cosine similarity; it is not the
paper's within-bar spectral representation or a reproduction of the unlicensed
`automatic-cuepoints` wrapper. It follows the published/licensed CBM DP idea,
with a separate novelty-ranked maximum-eight cue policy.

| Research policy | Cues | Mean per track | MiK F1 at50ms | MiK F1 at one assumed bar |
| --- | ---: | ---: | ---: | ---: |
| Existing novelty on common timing |2251|7.98|53.94%|59.56%|
| Bar-constrained novelty |2251|7.98|54.21%|58.52%|
| Acoustic8/cosine CBM |1887|6.69|61.00%|64.28%|

These are secondary, one-to-one timestamp agreement diagnostics, not musical
quality scores. All policies allow at most eight cues; CBM does not pad sparse
candidates. A preselected24-group anonymous comparison includes the three
policies plus MiK as an unlabeled comparator. **No listening ratings have been
collected and no quality winner has been selected.** Selector-only median/p95
worker time is27/35ms on this host, excluding decoding and feature extraction;
this is not full-file analysis speed. Ten synthetic behavior/domain tests and
four matcher checks pass. The research worker links system C/iconv only, with no
Python or learned model at runtime. Production defaults remain unchanged; Linux
and Windows execution of this new selector have not been verified.

Private experiment: `cue-quality-acoustic-cbm-v1`. Original CBM provenance:
[author's BSD-3-Clause implementation](https://github.com/ax-le/autosimilarity_segmentation).

## Integrated source-profile pipeline, 2026-10-17

The independent `analyze-features` Rust path now completes fresh file analysis
for all282 tuning representatives, using the same file identities as the older
full run. It reads audio only and computes every stage inside one offline
process. These are macOSARM64 measurements; broader Linux/Windows execution
and the82-group holdout remain unverified.

| Measurement | Exact result | Reference |
| --- | ---: | --- |
| Full-precision BPM | 282/282 | Stored original library BPM, with binary64 equality |
| Displayed BPM | 282/282 | Two-decimal diagnostic only |
| Reader-returned analyzed frame count | 282/282 | Retained original-reader extraction |
| Complete source frame count | 282/282 | Retained complete-file extraction |
| Request duration | 282/282 | Reader-prefix frames /44100 |
| Complete energy-feature arrays | 282/282 tracks | Retained native feature extraction |
| Energy segments | 6182/6182 | Same segment count/order and all fields |
| Energy timing, volume and feature values | 1149852/1149852 | Exact binary64 values, including1131306 features |

All source hashes remain unchanged. Fifteen initial workers could not open
Unicode filenames because the harness incorrectly used JSON ASCII escapes in
Sandbox Profile literals. A synthetic same-audio filename control reproduced
the failure, verified the corrected quoting and preserved denial of unlisted
files. The15 pre-analysis failures remain retained; all15 retries succeeded
using the identical frozen Rust binary. All282 tracks remain in the denominator.
Expected values were opened only after both prediction barriers. A separate
root check reverified every BPM and energy-field bit without the shared grader.

This is exact tempo plus energy-*feature* reproduction on the tuning set.
It does not classify energy levels, select cues or establish final key results.
The default `analyze` path retains its earlier timing backend. The explicit
source-timing option below is now validated through normal engine results.
The frozen combined-model results below remain historical evidence for their
separate configuration. The five-file macOS/Linux request comparison remains
the broader-platform real-music boundary for full request features.

Private run: `engine-file-request-tuning-v1`; aggregate report SHA256
`85f11d6654be64df077a0283a4cf64d474f74d168a025aac24737970ff44aad5`.

## Source timing in normal engine results, 2026-10-17

A fresh `analyze --source-profile-timing` run succeeds on all282 tuning
representatives. Every full-precision BPM matches the stored library binary64
value exactly; all full/prefix frame counts also match. All source hashes remain
unchanged, each complete FLAC verifies, and all282 private encoded snapshots are
removed. An independent regrade verifies every prediction hash and all282 BPM
bit patterns. Failures remain in the denominator; no holdout was used.

Both existing decoders consume one private encoded copy, so a transient original
pathname replacement cannot combine timing from one file with key audio from
another. The original is checked against the captured content/metadata identity.
A fresh review found that the initial pathname-only guard did not establish
this invariant; its deterministic replacement regression passes after the fix.
The earlier validation run was interrupted by host disk exhaustion and retains
no completed score. It and its logs remain preserved; the fixed-binary run above
starts afresh across all282 identities.

The final workspace passes380 default tests (2ignored) and386 all-feature tests
(5ignored) on both macOS ARM64 and Linux ARM64. Release builds pass. Linux's
generated-fixture timing/profile/code identity matches Mac. The five captured
feature requests still reproduce16921numbers/15strings and complete JSON bytes.
Linux ran offline without mounts and its container was removed. Windows
execution and broader real-music grid/classifier equivalence remain unverified.

This run establishes the selected timing backend, not a combined key/energy/cue
score. Unknown strength stays absent; the single source anchor is separate from
a complete downbeat sequence. Private run: `source-timing-engine-integration-v1/main-bpm-v2`;
report SHA256 `98f67abde2e443ad33312901b74f4b718b27713018009a69f63c7afb99fe3827`.

## Earlier final-output pipeline

A separate unchanged upstream OpenKeyScan comparison now matches274/282 keys
(97.16%) against the same tuning identities, compared with254/282 below. This
Python research result is not yet integrated into the Rust engine. See the
[pinned comparison and disagreement analysis](openkeyscan-comparison.md).

The completed `combined-skey-native-v1` run analyzed and integrity-verified all
282 tuning groups with no failures:

| Output | Last full-run result | Exact gate |
| --- | ---: | ---: |
| Key | 254/282 | 282/282 |
| Internal BPM, full precision | 0/282 | 282/282 |
| Displayed BPM, two decimals (diagnostic) | 275/282 | 282/282 |
| Overall energy | No production estimator | 282/282 |
| Cue count | 248/282 | 282/282 |
| Exact persisted cue timestamps | 0/2195 | 2195/2195 |
| Ordered cue times and names together | 0/282 tracks | 282/282 |

BPM within0.5 matched281/282 with no half/double-time mistakes. Cue timing
within50ms matched1028/2195 (45.7% precision,46.8% recall). These relaxed
metrics do not replace the exact gate. Cue names remain `Candidate`.
Elapsed4183.3s overlapped other work; it is not a controlled speed measurement.
The older upsampled beat run matched271/282 displayed BPM values,280/282
within0.5 and had no metrical errors, with all282 decodes verified.

Timestamp fidelity uses the original stored milliseconds, retaining negative
values, repeated positions and slot order. A read-only extraction from the original analysis library now supplies
full-precision internal BPM for all282 tuning identities. The strengthened
private `exact-fidelity-v2` assessment passes0/282 complete outputs. All internal
BPM values differ from the frozen engine output, despite275 rounded matches.
Explicit per-cue energy levels are also required; candidate names cannot pass.
Beat-grid and dense energy-timeline equality are reported separately as
diagnostics, currently0/282 each.

Four difficult tuning tracks were reanalyzed by the licensed app from tag-free
copies. All reproduced the saved key, displayed BPM, overall energy, cue counts,
names and exact stored milliseconds. One short track legitimately repeats the
same timestamp in four cue slots. This remains in the target data. A numerical
seconds↔milliseconds round trip changed one value by2.84e-14 seconds while its
stored milliseconds remained exactly equal; comparisons retain the source unit.

## Numerical fidelity work

A private original-SDK reference run now reconstructs internal BPM from audio
alone on 281/282 tuning tracks; only one legacy, pre-refinement BPM matched.
The remaining refined value differs by one float32 step and remains a failure.
The SDK waveform frontend also differs from the main application's beat
frontend, so this result does not establish exact reproduction of that pipeline.

Independent Rust mathematical components have exact native-reference parity:

| Component | Verified cases | Platforms |
| --- | ---: | --- |
| Key FFT | 56; all 74,340 float32 components | macOS and Linux ARM64 |
| Beat score and per-beat buckets | 72 | macOS and Linux ARM64 |
| Fixed 2x interpolation | 13; all 108,740 binary64 values | macOS and Linux ARM64 |
| Tempo-refinement stage | 5 exact binary64 BPM values | macOS ARM64 |

The tempo-stage inputs are original waveform features and legacy beat detections,
with expected results supplied only to the verifier. These private prototypes do
not independently extract those inputs from PCM, supply offline classifiers or
reproduce complete cue output. They have not replaced the production algorithms.
The custom key FFT parity also does not cover the separate waveform frontend's
Accelerate FFT.

A production JSON precision defect was reproduced with generated finite values:
loading a saved fractional timestamp could change its binary64 value by one step.
The workspace now enables exact float parsing. Two new Cue/Analysis round-trip
regressions pass, preserving negative and repeated cue slots as well as precision.
The current source passes 59 all-feature tests on both macOS and Linux ARM64,
with five explicit model/private-data tests ignored; both release builds pass.
Formatting and Clippy pass on macOS. The cached Linux test image lacks those
tools, and Windows execution remains unverified. No new whole-engine music
agreement score is claimed for this dependency change; the frozen full-run
results above remain the agreement baseline.

## Complete tuning runs

Both runs select one representative from each of the same 282 tuning audio
groups. Failures stay in the denominator. Copied files do not increase scores.

| Measurement | Original DSP baseline | Balanced key profile + classical 79–192 BPM search |
| --- | ---: | ---: |
| Exact key | 99/282 (35.1%) | 223/282 (79.1%) |
| BPM within 0.5 | 181/282 (64.2%) | 269/282 (95.4%) |
| Exact BPM at two decimals | 38/282 | 58/282 |
| Half/double-time BPM errors | 93 | 0 |
| Decode failures | 5 | 0 |
| Complete, integrity-verified FLACs | 277 | 282 |
| Sum of analysis time | 540.5 s | 639.1 s |
| Predicted cues | None | None |

The original baseline's five failures reproduced an empty resampler-tail bug.
The candidate includes its fix. Timing is indicative: other local experiments
overlapped part of the candidate run, so this is not a controlled speed comparison.

The key candidate's 223/282 score is **training fit**, because its shared
profiles were learned from tuning labels. Five-fold cross-validation inside
that partition gave 221/282 including the five unavailable baseline feature
records as failures. Major-key recall was 75%, based on only eight major-key
examples; minor-key recall was 79.9%. This is insufficient major-key coverage.
An aggregate-accuracy candidate with zero major recall was rejected.

The tempo range is an explicit classical-search configuration. It substantially
reduces metrical mistakes here; only 58 exact displayed BPM values means there
is still a large precision gap. Do not infer accuracy outside this library's
104–144 BPM reference range.

## Learned beat candidate

A separate twelve-track tuning pilot exercised the small Beat This! model
through Rust inference, with no decode failures. Its first BPM postprocessor
mistakenly advanced beat indices for extra detections. After reproducing that
failure, the corrected periodic-lattice fitter matched all twelve reference
BPM values at two decimals when applied to the saved detections. That is
component evidence, not a fresh full-corpus score.

A fresh full-track inference on the worst original tempo error also completed,
verified FLAC integrity, and returned the correct key and BPM after the fix.
The complete combined run above now measures this path more broadly. Its
remaining seven displayed-BPM errors prevent satisfying the exact gate.

The facade now decodes once into independent DSP/key-feature and 22050 Hz
learned-model buffers. Previously it upsampled the 11025 Hz analysis PCM for
the beat model, after losing the upper frequency band. A new complete twelve-
track tuning pilot used direct 22050 Hz input: all decodes verified integrity,
all key features/results were identical to the prior path, and 12/12 displayed
BPM values matched. Cue timing matched 45/95 references at 50 ms versus 44/95
on those same groups using the older input. Nearest-model-beat coverage was
93/95 at 50 ms (previously 92/95), and median nearest-beat error fell from 25.7
to 19.9 ms. Coverage at 100 ms was 93/95 versus 94/95: this small pilot does
not establish uniform improvement. Its 313.0 s total overlapped other work.

## Neural key candidate

The pinned community ONNX export of [Deezer S-KEY](https://github.com/deezer/skey)
ran through Rust RTEN on all 282 tuning representatives, using direct 22050 Hz
audio and no local training/calibration. It matched **254/282 keys (90.1%)**,
including 5/8 major and 249/274 minor keys. All decodes completed and passed
FLAC integrity verification. Summed decode/inference time was 457.4 s during
concurrent experiments. These are tuning comparisons; pretrained training-data
overlap has not been audited, and the local 82-group holdout remains unused.

A shared DSP+neural calibration head was compared against a DSP-only head on
identical whole-group folds, preserving assignments for the original 277 valid
groups and assigning the five recovered decodes deterministically. Selected
fusion CV gave 253/282, with major recall 75% and minor recall 90.1%. This offers
a mode tradeoff versus raw S-KEY, rather than a clear overall gain. The extra
head remains a private research artifact; the simpler raw model is available
only as the experimental `--key-neural-model` option.

The original checkpoint SHA was verified against the conversion manifest.
On identical generated/noise/music PCM, RTEN and ONNX Runtime selected the same
class in three cases. Score error was below 1.4e-6 for noise and a 25-second
music excerpt, but reached 3.8e-4 on a sparse harmonic fixture. A strict 1e-5
gate therefore failed; do not claim universal numerical equivalence. The
checkpoint/export revision, hash, mapping and code notices are pinned under
`models/skey/`; no weights have been included in the source project.

An additional PCM-only temporal probe ran on all282 tuning groups:30/60-second
windows centered at20/50/80% of duration, with arithmetic/logarithmic pooling
and voting. None improved whole-track254/282 agreement; variants achieved
246–253. The temporal change was rejected, with no holdout used.

## Cue work and evidence boundaries

The tuning set contains 2,195 reference cues. Most tracks have eight, but counts
vary from three to eight. Many cue gaps correspond to 32/64/96/128 beats, with
other lengths also present. A fixed 64-beat grid cannot reconstruct them.
In the twelve-track pilot, 92/95 reference cues were within 50 ms of a model
beat; that says little about selecting the correct beats or naming their energy.

An opt-in PCM feature timeline now supplies local loudness, six-band energy
distribution and spectral-change features for cue/energy experiments. Full
extraction completed for all 282 tuning groups with no failures and all FLAC
integrity checks passing; key and tempo predictions retained the preceding
candidate's scores. Summed analysis time was 734.7 s with concurrent experiments.

The first timing heuristic matched 61/139 reference cues within 50 ms on 18
tuning tracks (42.4% precision, 43.9% recall). A refined version interprets
minimum separation in rounded beat intervals to tolerate timestamp quantization;
its 38-track partial tuning diagnostic matched 129/296 (42.4% precision, 43.6%
recall). These are selection baselines, not sufficient cue quality. The Rust
engine exposes the refined heuristic only through `--cue-suggestions`, disabled
by default. Cue names remain `Candidate`, and the eight-cue cap cannot reproduce
every observed count. A fresh complete 373-second track passed combined Rust
key/beat/cue inference and FLAC integrity verification; key and displayed BPM
matched, and three of eight cue times matched within 50 ms. Its cue array also
matched the independent prototype on the fresh features. This verifies pipeline
integration, not sufficient accuracy. No Rekordbox export behavior is verified.

A separate five-fold, audio-group-separated energy-name experiment used all
2,195 tuning cues at their **reference positions**, so it did not test selection.
Its selected class-balanced ridge model achieved 41.8% exact names versus 44.3%
for the median/majority-label baseline. It improved agreement within one energy
level to 87.7% from 81.2%, but remains unadopted. Local loudness and broad-band
features alone are insufficient for the requested exact energy names.

Complete cue-selection research on the older beat path matched1006/2195 at50ms
with the heuristic. A separate tree selector using294 PCM-only beat-window and
phase-context features reached1246/2195 (55.4% precision,56.8% recall) in
whole-audio-group five-fold cross-validation. Its configuration was selected on
these tuning folds. It remains unadopted and does not establish exact timing.
A fitted PCM beat grid offers2157/2195 candidate positions within50ms versus
2115/2195 raw detections; this is a timing ceiling, not cue-selection accuracy.

Shallow boosting on the same2195 oracle-position energy cues reached51.4% exact
names with24.4% macro recall; a balanced variant traded aggregate accuracy for
33.9% macro recall. An overall-energy probe using282 whole-track feature
summaries reached59.9% exact versus51.1% for the majority baseline, with28.6%
macro recall. None meets the exact gate or has been adopted into production.

A private native feature oracle now provides an intermediate validation target.
The original audio reader returns fewer frames than the declared FLAC length
on148/282 tuning tracks. Composing that reader with the original energy analyzer
reproduces every energy-request field exactly on all five captured cases, using
only the audio path as input. This resolves the earlier three energy-input
mismatches caused by feeding additional end frames. A generic4096-output-frame batching simulation reproduces analyzed length
on all282 tuning references and95 generated boundary cases across six FLAC
block sizes. An independent Rust packet-based probe now also matches282/282 tuning
lengths and95/95 generated cases, with zero failures and full-source FLAC
integrity verified. It uses no original framework or reference label as an input.
This is a private prototype, outside the production analysis pipeline; it does
not establish musical-output reproduction or cross-platform runtime validation.

Intermediate extraction completed for282 tuning tracks without failures,
producing6182 native183-value energy feature vectors. A read-only, exact-file-
identity join to the original library supplies2818 merged energy spans and
199371 beat timestamps; all282 extracted keys, displayed BPM values, overall
energies, cue counts, names and original cue milliseconds agree with the frozen
references. The held-out musical outputs remain unread.

All6182 native feature segments map unambiguously by midpoint to one merged
reference energy span. Five-fold cross-validation separated by whole audio
group gives4657/6182 exact segment labels (75.3%) for shallow boosting, with44.8%
macro recall and only4/282 tracks matching every segment label. An SVM gives
72.1% with49.6% macro recall. These are component studies using original native
inputs and inferred segment labels; they are not independent Rust or end-to-end
predictions, and no classifier has been adopted.

A controlled key-input reconstruction now reproduces all352 feature values
exactly on each of the five captured cases, including the nonuniform-grid case.
This requires the original archived beat grid and installed key analyzer, and
preserves the observed edge-interval time mapping. It resolves the earlier
aggregation mismatch but does not establish an independent key extractor,
independent BPM/grid prediction or offline classifier equivalence.

The private oracle calls installed frameworks for validation. It is not an
independently implemented Rust extractor or a product runtime dependency.

Shared quadratic pitch interactions improved same-fold key CV by only three
tracks (224/282 versus 221/282), with major recall still 75%. The simpler linear
profile remains selected while richer features are investigated.

## Additional upstream candidate

The unchanged [Stratum DSP 1.0.0 crate](https://docs.rs/stratum-dsp/1.0.0/stratum_dsp/)
was run with its default configuration on the same deterministic twelve tuning
groups. All decodes completed and verified integrity. It matched 5/12 keys and
10/12 BPM values within 0.5; none matched displayed BPM at two decimals. Our
untrained baseline matched 3/12 keys and trained profile 11/12 on those groups.
This compares untrained upstream defaults with a locally trained profile, and
does not establish broad algorithm superiority. Stratum was not adopted.
Its tonic/mode values were mapped through our notation implementation rather
than using its differing numeric labels. Private results and exact source/
binary provenance are in `stratum-default-pilot-v1/`.

Private executable snapshots, model coefficients, manifests and per-track
predictions remain under `.analysis/`. The complete-run directories are
`baseline-v1` and `profile-balanced-tempo-range-v1` under its `benchmarks/`.
Each contains code/binary/config/model/corpus identities in `run.json` and
aggregate metrics in `report.json`. The 82-group holdout remains unused.
