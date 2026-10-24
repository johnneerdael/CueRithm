# MIK facts register and modelling constraints

Revision 25 · 24 October 2026 CEST / UTC · Required engineering reference.

Revision 2 authorized private MIK cue-model research and superseded revision 1's
prohibition. Revisions 3–25 add fresh grouped-development comparisons, exact
whole-track pooling exclusions and independently verified later experiments. Existing fact IDs and evidence identities are
preserved; the product's validated Lexicon DSP remains in place.

Read this register before changing feature extraction, key/BPM/energy analysis,
training, evaluation or model promotion. Cite relevant F-numbers and the register
revision in the experiment record and PR. This is the canonical tracked aggregate
register; the private companion uses the same IDs and retains the underlying
audio, capture audits and identifying evidence. Historical reports can describe
earlier hypotheses; the scoped exclusions here govern current work.

The record covers controlled synthetic runs 01–10 (including rejected 06 and
replacement 06B), the bass/dither intervention in run 12, the staged cue
construction in run 13, the real-source suffix
experiment in run 11, local mechanism
investigations, and an independently recomputed assessment of the supplied mixed
track ledger. Each item distinguishes observations,
derivations and their scope. Project decisions and proposed techniques are
identified explicitly; they are not recovered vendor mechanisms. No predictive
model was trained while compiling revision 1. Later experimental fits are
recorded alongside input/data audits from F078 onward and retained in separate experiment records.

The frozen through-09 set has 295 captured synthetic fixture IDs and 277 distinct
retained requests; 281 IDs / 263 requests are qualified against local extraction.
Fourteen older 08 fixtures remain conditional observations with upstream failures.
Run 10 adds 12 IDs and eight new request identities: 307 IDs / 285 requests in the
through-run-10 ledger. The earlier 277-request benchmark is preserved. All 12 new files
match local numeric extraction, but none passes the original absolute-origin gate;
the reviewed scope is a grid-unqualified key diagnostic (F119). Run 12 has
seven files and five distinct numeric requests in that same lineage, including
two reused baselines and three explicit controls (F123–F124). A separate
real-recording test repeats 12 recordings three times; run 11 contains three
exact-prefix variants of one real recording. These are not added to the synthetic
benchmark or counted as independent songs (F121–F122). Run 13 adds one linked family of nine files, six musical conditions and
seven actual request identities. Its captured-input observations remain separate
from its eight exact native-key failures and six authored-bar-phase failures
(F127–F129); it is not added to the old 277-request benchmark. Run 14 adds one
linked ending family of twelve files, nine primary conditions and ten actual
numeric request identities (F142–F143). Its separately qualified captured
inputs do not repair the twelve native-key failures or original container
guards. It adds neither independent music nor requests to that benchmark.
There is no promoted
replacement model or completed 100% qualification.

The current register contains 147 scoped findings. Evidence references E001–E377 identify immutable private source snapshots by
SHA-256 in the provenance index below. They are identifiers, not missing public
links. The private companion maps them to files, reproduction scripts and capture
identities. Music, private library paths, real-track labels, credentials,
decompiled code and proprietary packaged assets are deliberately excluded.
An engineer without that evidence must request access before claiming to have
reproduced a finding. Public aggregates alone do not enable full reproduction.


## Latest completed development results

| Target | Exact agreement | Main remaining gap |
| --- | --- | --- |
| Key | 1,220/1,288 (94.72%) | Only 18/34 major responses match; 37 prior errors corrected and 33 introduced (F096). |
| Key unaccepted balanced diagnostic | 1,212/1,288 (94.10%) | Against the current control, corrects 18 but regresses 26; major matches rise 18→30/34 while minor matches fall 1,202→1,182/1,254 (F120). |
| Whole-track energy | 963/1,288 (74.77%) | Ratings 8–9 match only 16/50; inputs and learner both change against the earlier baseline (F094). |
| Window energy comparison control | 26,944/29,892 (90.14%) | 200/1,288 entire sequences match; retained as a reproducible control, not an exact recovered rule (F101). |
| Window energy unaccepted challenger | 27,711/29,892 (92.70%) | 307/1,288 entire sequences match, but 783 previously correct windows and 73 complete sequences regress; class 9 falls 11→10/18, class 10 remains 0/2 (F118). |
| Cue count comparison control | 1,091/1,288 (84.70%) | 175 over-counts and 22 under-counts; no matches among 44 cases with two to five cues (F132). |
| Complete ordered cue positions | 568/1,288 (44.10%) | Correct count coexists with early position misses in 140 requests; event recall is a separate diagnostic (F133). |
| Complete cue lists | 200/1,288 (15.53%) within 1 ns | 196 timestamp-bit-exact; full-list fixes/regressions 7/6. Selected placement falls 571→568; the prior position-only reference remains better (F106). |
| Frozen key model on prior synthetic diagnostics | 202/277 (72.92%) | Only 15/48 major responses match; 14 requests remain locally unqualified (F097). |

These are spent development comparisons using captured client features. They do
not measure unseen music or the complete decoded-audio path. The synthetic check
uses previously inspected diagnostics. Controls are reproducible comparisons, not
verified recovered rules. Earlier aggregate-only acceptance recommendations are
audited at case level; acoustic causes remain unresolved. No product-default replacement is promoted.

## Immediate exclusions

- Key input is 352 register/bank values, not interchangeable with 12 chroma values
  (F017–F019). Exact transposition equivariance is not established.
- Activity, spectral brightness and vocal presence are interventions, not measured
  energy labels or proof of a semantic detector (F013, F031–F040).
- Rounded window labels do not support a universal fixed monotone whole-track
  reducer (F041–F047). Label histograms have conflicting targets (F065), with a
  measured maximum of 98.81% on the larger observed cohort (F075).
- Uniform arithmetic pooling of hidden window values with any common rating
  thresholds is excluded by an exact observed contradiction (F089–F090).
  Other weights, nonlinear pooling and separate overall thresholds remain open.
- Hard selection among five compared key predictions is capped at 98.52% on
  the spent cohort (F098); this does not bound score fusion or new predictions.
- Correct counts alone do not repair the fixed cue selector (F099). Retained
  training examples include four additional occurrences at one position (F100).
- Captured-feature prediction and end-to-end PCM prediction have separate
  denominators (F001–F004). An upstream refusal is still an end-to-end failure.
- Prior guessed bundles and failed partial prototypes are not the starting
  weights for the new programme (F059–F063).
- Private MIK cue-model research is permitted by the owner. The product cue path
  remains the validated Lexicon DSP; research does not promote a replacement.

## Meaning and scope of a verified finding

Verified model-experiment outcomes belong in this register alongside measured
MIK outputs, source observations, data audits and mathematical exclusions. Each
states what its evidence establishes. A failed configuration is not a disproof
of its entire technique or a recovered server mechanism. Hypotheses and proposed
experiments remain separate. New analysis of retained captures can establish a
finding without new synthetic audio. Create new controlled audio when a specific
falsifiable intervention can resolve an outstanding gap.

## Findings

### Evidence and data

#### F001 — There are two separate parity problems

PCM-to-request reproduction and captured-request-to-response prediction are different stages. A correct response prediction from captured features does not repair a local extraction failure.

**Scope:** The inspected client and the case-qualified capture audits; no universal all-format qualification.

**Rules out / prevents:** Do not substitute captured inputs silently and call the result end-to-end accuracy.

**Evidence:** E002, E025, E028.

#### F002 — Current controlled counts are 295 files and 277 distinct requests

The accepted 01–09 collection has 295 captured fixture IDs and 277 distinct retained requests. Six original duration-rejected 06 IDs are excluded. Copies, repeats and related edits are not independent songs.

**Scope:** Current accepted manifests and genuine captured musical outputs, including the conditional 08 observations.

**Rules out / prevents:** Do not use file count as an independent sample count.

**Evidence:** E039.

#### F003 — Fourteen 08 inputs remain unqualified locally

Of the 295 IDs, 281 IDs / 263 distinct requests have exact local-request qualification. Fourteen 08 files have verified PCM and genuine complete captures but missing local timing/features.

**Scope:** All prior qualified counts plus 09’s 64 files / 58 new distinct requests; no guard was removed to fill the gaps.

**Rules out / prevents:** These fourteen remain upstream failures in an end-to-end denominator.

**Evidence:** E027, E033.

#### F004 — 09 is completely qualified

09 has 64/64 complete selected responses, exact retained local requests and unchanged post-scan decoded PCM; 58 distinct requests. Eight unrelated HAR entries were excluded.

**Scope:** Supplied 09 HAR only. Equality concerns retained analysis fields, not authentication, HTTP bytes or excluded metadata.

**Evidence:** E028, E033.

#### F005 — The observed profile is versioned

The audited captures use build 11.2.6.7446, key algorithm 94, energy algorithm 2 and cue algorithm 3.

**Scope:** The supplied captures and inspected installed build; not future revisions or other architectures.

**Rules out / prevents:** A new version requires a new qualification, not silent pooling.

**Evidence:** E019, E007.

#### F006 — Container mutation is not PCM mutation

Scan containers can change while decoded samples remain identical. This occurred in the first captures, 03/04, and all checked 09 files. Original finals and scan copies are separate.

**Scope:** Decoded little-endian stereo int 16 PCM hashes and recorded container hashes.

**Rules out / prevents:** Do not reject preserved audio solely for a tag/container change, or infer audio equality from filenames.

**Evidence:** E018, E020, E028.

#### F007 — Current class coverage is incomplete

Observed controlled MainKey labels cover 23/24, missing 6B. Window-energy labels cover {1,2,3,4,5,6,8}, missing {7,9,10}. 09 adds keys 11B/1B/7B and class 1.

**Scope:** Actual returned labels, not authored chords or expected activity ratings.

**Rules out / prevents:** A 24-construction design is not 24 observed classes. Missing labels must not be fabricated.

**Evidence:** E039, E032.

#### F008 — The 09 audio invariants were measured

Independent validation checked 64 decoded quantisations, 140 distinct float 64 sources, 53,048 events, 33 mode/brightness contrasts and nine activity contrasts. Three copies and three synthesis repeats were byte/PCM exact. Peak maximum was −2.412613208 dBFS; 4× estimated true peak −1.944827290 dBFS.

**Scope:** Pinned NumPy/SciPy environment. No audition, native REAPER bounce or arbitrary-machine identity certification.

**Rules out / prevents:** A successful session inspection or intended score alone is not decoded-audio evidence.

**Evidence:** E004, E028.

#### F009 — Scheduled and detected grids can disagree

09 tempo passes .01 BPM on 61/64; beat-equivalent origin plus tempo passes 54/64; bar-equivalent origin plus tempo passes 9/64 at 3 ms. All 64 local requests nevertheless equal captured client requests.

**Scope:** Scheduled origin remains sample zero. Detection agreement with the client is distinct from recovery of authored bar phase.

**Rules out / prevents:** Do not move audio to force a detection, or call equivalent beat phase correct bar phase.

**Evidence:** E004, E028.

#### F010 — The old pilot anomaly cannot currently be reproduced

The original 24-file pilot audio and source assets are no longer retained. The reported +25-cent pad anomaly cannot currently be independently reproduced.

**Scope:** A provenance limitation, not a confirmed tuning rule.

**Rules out / prevents:** Do not promote that anomaly into a modelling constraint without recoverable evidence.

**Evidence:** E014.

### Client mechanisms and interpretation

#### F011 — Normal analysis receives PCM through local DSP

Static tracing of the inspected normal file-analysis loop shows decoded PCM dispatched to key, timing, waveform, energy and volume analyzers, followed by a service-client operation.

**Scope:** Bounded static path inspection; not an exhaustive runtime trace.

**Evidence:** E002.

#### F012 — CoreML resources belong to resolved stem adapters

The inspected build contains two encrypted CoreML resources and resolved HTDemucs/MelBand stem-adapter loaders and prediction callers registered through the playback/stem controller.

**Scope:** Resolved static paths only. Opaque weights, optional runtime branches and inference speed were not measured.

**Rules out / prevents:** Do not infer that normal key/energy analysis uses stem separation, or that all possible CoreML use has been excluded.

**Evidence:** E002.

#### F013 — There is no demonstrated explicit vocal-category input

No named vocal-class bit was found in the inspected 183-feature energy path. The controlled vocal requests are reproduced by ordinary PCM/DSP extraction.

**Scope:** Does not prove that learned combinations cannot respond to vocal-like spectra.

**Rules out / prevents:** Vocal-on energy changes are not proof of a vocal detector. Speed alone is not evidence of its absence either.

**Evidence:** E002, E018.

#### F014 — The inspected duration rejection is 1,020 seconds

The arm 64 client path admits duration ≤1,020 seconds and labels the greater-duration branch “File too long”. Original 06 files were 1,219.047619 seconds; replacement 06B files are 975.238095 seconds.

**Scope:** Build 11.2.6.7446, inspected execution path; no runtime boundary scan or server-limit claim.

**Rules out / prevents:** Local audio QA does not establish application processability. Do not repair only one member of a duration/gain family.

**Evidence:** E003, E022.

#### F015 — SVM lineage evidence is stronger than kernel evidence

The client exposes EnergyFeatureExtractor::OutputToSvm and SegmentSvmInput; the energy patent describes SVM embodiments alongside alternatives.

**Scope:** A symbol/document observation supporting an SVM-lineage hypothesis, not identification of RBF, SVC versus SVR, current support vectors or server implementation.

**Rules out / prevents:** Do not label a proposed RBF classifier “recovered MIK”.

**Evidence:** E002, E010.

#### F016 — Patents describe alternatives, not current weights

The independent review’s patent reading includes key Bayes/category and multi-cluster correlation embodiments plus neural alternatives; energy includes several learner types and aggregation examples; the cue patent permits learned functions as well as signal rules.

**Scope:** Document-content evidence. Our earlier key-PCT first-page screening did not establish those details; the later OCR/text review expands that scope.

**Rules out / prevents:** A failed GaussianNB, fixed 24-template matcher or patent example cannot identify or exclude the whole described family.

**Evidence:** E015, E010.

### Key observations

#### F017 — The key input preserves register and periodic position

The vector is four banks of 88 absolute semitone positions. Position zero corresponds to MIDI 21; position 48 to MIDI 69. Bar index modulo four selects the bank in the inspected implementation.

**Scope:** Verified source geometry and captured vectors, not a twelve-value chroma representation.

**Rules out / prevents:** Collapsing to pitch classes or summing banks discards observed information and is an ablation, not a neutral preprocessing step.

**Evidence:** E036.

#### F018 — The original order tests did not isolate cadence

Order pairs have equal rational note inventories but bank-conditioned exposures differ by 41.26% relative L2. Captured 352-vector differences are about 40.29–41.55%; main keys stay unchanged but confidence changes.

**Scope:** The checked three order pairs. Acoustic/grid boundary effects also remain.

**Rules out / prevents:** Do not call this proof for or against long-range server chronology.

**Evidence:** E036.

#### F019 — Audio transposition is not an exact shifted feature vector

After nonwrapping pitch alignment, exploratory bank alignment and common scaling, measured transposition residuals span 8.16–40.72% in the checked family. Percussion stems were held fixed.

**Scope:** Audio interventions alter Stage A evidence; no direct server-input transposition was performed.

**Rules out / prevents:** Do not attribute the entire asymmetry to Stage B or impose exact equivariance without an explicit test.

**Evidence:** E036, E019.

#### F020 — The mixture weights have specific physical meanings

The C-major/A-minor chord family uses coherent linear amplitude weights (1−w,w). A separate progression/bass family uses equal-power coefficients. They are different treatments.

**Scope:** The declared families and their unchanged phase/gain policies.

**Rules out / prevents:** w is neither a perception probability nor a universal key-confidence value; the two weight definitions cannot be interchanged.

**Evidence:** E019.

#### F021 — The coarse key boundary was family-specific

In the scaffolded chord mixture, w=.125 returns 8B and w=.25 returns 8A; the observed intermediate outcomes motivated a narrower sweep.

**Scope:** This family, seeds, gain, register and dither only.

**Rules out / prevents:** Do not turn those endpoints into a universal threshold or assume monotonicity outside tested points.

**Evidence:** E019.

#### F022 — Dither alone can change a key label near a boundary

At w=.1875, two declared dither seeds applied to the same prequantisation waveform return 8A versus 8B. Decoded differences are ≤two PCM steps; 51 key-vector entries change, with relative L2 about .002995%. Copy/repeat controls preserve the primary result.

**Scope:** A changed-PCM counterfactual, not identical-input server randomness or a universal error rate.

**Rules out / prevents:** Do not deduplicate different dither inputs as exact requests, or infer stochastic inference from this pair.

**Evidence:** E020.

#### F023 — 07 did not yield every authored major label

All twelve minor constructions return their corresponding minor labels. Six major constructions return the same labels as their minor partners: E, F, F-sharp, A, B-flat and B.

**Scope:** 07’s fixed voicing, register and scaffold. The feature vectors differ and major-third evidence is nonzero.

**Rules out / prevents:** No universal tonic-based mode correction follows.

**Evidence:** E024.

#### F024 — 08 isolates a scaffold effect in four comparisons

Four of sixteen scaffold comparisons change MainKey: E-major and F-sharp-major, each at two pad registers, return major without the scaffold and parallel minor with it.

**Scope:** The complete audio treatment also changes detected timing/feature geometry; it is not an isolated manipulation of server fields.

**Rules out / prevents:** Do not conclude an explicit percussion detector or a universal major suppression rule.

**Evidence:** E026.

#### F025 — 08’s pad-register contrast had no MainKey changes

Zero of sixteen tested pad-register comparisons change MainKey; twelve of sixteen mode comparisons do change it.

**Scope:** A tested null in this matrix, not proof that register is irrelevant elsewhere.

**Evidence:** E026.

#### F026 — 09 authored-key correspondence is A23/24 and B16/24

All twelve authored minors match in each arrangement. A matches eleven of twelve majors; B matches four. These total 23/24 and 16/24.

**Scope:** Construction-label comparisons of MIK outputs, not scores for our model and not universal musical-key ground truth.

**Rules out / prevents:** Do not report these percentages as reconstruction accuracy.

**Evidence:** E029, E008.

#### F027 — 09 has eight cross-arrangement key changes

Eight of the 24 matched-tonic/mode A-versus-B cells change returned MainKey. Within B, eight major/minor pairs share the parallel-minor label. A’s B-flat-major construction returns G minor, not the parallel B-flat minor.

**Scope:** A/B jointly changes arrangement, voicing/register, timbre and tempo.

**Rules out / prevents:** These contrasts cannot isolate register or timbre as the cause.

**Evidence:** E032, E008.

#### F028 — 09 dither changes confidence without changing MainKey

The energy-family dither-only replicate keeps 8A, all window labels and overall energy, but MainKeyConfidence falls by .0047013163566589355.

**Scope:** One declared seed pair, not a confidence error bar or classifier-family fingerprint.

**Rules out / prevents:** Do not substitute an arbitrary model probability for MIK confidence.

**Evidence:** E030.

### Window energy and context

#### F029 — Energy inputs are 183 values plus separate RMS

The emitted window vector contains a tempo term and thirteen descriptors for each of fourteen bands: 1+14×13=183. VolumeRmsDb is separately supplied.

**Scope:** The inspected/reproduced profile. Emitted values already include feature scaling.

**Rules out / prevents:** Discarding RMS or changing scaling is a declared model ablation, not evidence that the server ignores it.

**Evidence:** E002, E005.

#### F030 — Energy feature tempo is independently estimated

In the traced vocal pair, the energy module uses its own tempo, about 122.00129/122.00137 BPM, rather than the request beat/downbeat to form these features.

**Scope:** This inspected local processing path and pair.

**Rules out / prevents:** Do not use request tempo alone to infer exact acoustic support.

**Evidence:** E034.

#### F031 — Whole-file normalization changes unchanged prefixes

The pair’s weighted-RMS 95 th percentile changes .3796841830→.5332594007, with reciprocal gain 2.6337678649→1.8752599554. First-window features change about 12.0716% although the early PCM prefix is identical.

**Scope:** Measured private local-DSP interventions, reproduced against all four captured feature windows.

**Rules out / prevents:** This does not require an early vocal event or server sequence state.

**Evidence:** E034.

#### F032 — Nominal windows are not isolated acoustic support

FFT frames use 2,048 samples/hop 682. The inspected phase search uses 256
quarter-beat evaluation positions on an approximately 64-beat grid. Reduced
families use 256 positions spaced one eighth of a beat on a 32-beat grid;
first-to-last sample centers span 31.875 beats. Selected phase offsets,
interpolation, filtering/smoothing and whole-file normalization still make the
nominal 32-beat interval an incomplete description of acoustic support.

**Correction, 23 October 2026 CEST (22 October 2026 UTC):** Revisions through 17 inherited an incorrect
half-beat/about-128-beat reduction description from the original report. The
current statement corrects that unit calculation (F125). The original report,
source identities, old register snapshots and F033's measured restoration
outcomes remain preserved. No audio, request, response or source algorithm changed.

**Scope:** The inspected source profile and exact captured-default/family-array
reproductions. A sampling-grid width is not a closed acoustic-support bound;
state, phase selection and normalization remain relevant.

**Rules out / prevents:** Do not use 128 beats as the reduced-family grid width
or transfer a stationary label solely because one 32-beat waveform interior is
unchanged.

**Evidence:** E034 (historical report containing the error), E148, E151, E152.

#### F033 — The prefix attribution has an exact diagnostic closure

The first unequal quantised PCM frame is 694083, after scheduled onset 694033. Holding normalization/tempo and restoring reference band timelines from first affected frame 1015 makes every feature exactly equal to the reference; restoring only from 1018 leaves a residual.

**Scope:** Diagnostic interventions on local intermediate data, never modified requests submitted to MIK. The percentages are not additive causal feature importance.

**Evidence:** E034.

#### F034 — Vocal-on contrasts also change ordinary acoustic features

In the first vocal pair, the active window’s VolumeRmsDb increases about .247717 dB and scaled feature-vector relative L2 changes about 38.46%, while MainKey stays 7A and overall rises 4→5.

**Scope:** A correlated waveform/spectrum/level intervention, not isolated semantic vocal evidence.

**Rules out / prevents:** Do not label this an identified vocal detector.

**Evidence:** E018.

#### F035 — The first activity templates failed calibration

The tested sparse/pad/hat 16/strong-bass/dense recipes in 02 all return homogeneous class 4 and overall 4 despite different stems/features.

**Scope:** Those recipes and context; absence of class separation is a measured negative result.

**Rules out / prevents:** Do not call A/B “low/high measured energy” from their intended construction.

**Evidence:** E019.

#### F036 — A gain sweep showed scoped stability

The reference and −3/−12/−24 dB treatments in 02 all return class 4/overall 4, while key confidence changes.

**Scope:** One baseline waveform, shared family gain and dither policy.

**Rules out / prevents:** This is not proof of universal loudness or gain invariance.

**Evidence:** E019.

#### F037 — Spectral calibration produced actual contrasting labels

All 31 calibration conditions in 03 provide three support-eligible equal-class windows;57 candidate pairs differ by at least two classes. Stationary 500–800 Hz gives 3;800–1200 Hz and broadband give 6 in those contexts.

**Scope:** Shared clock and declared component RMS. Leakage, settling and whole-file context remain part of the signal.

**Rules out / prevents:** A band name alone is not a portable energy label.

**Evidence:** E020.

#### F038 — More pulsing need not raise returned energy

800–1200 Hz and broadband stationary/pulsed contrasts return 6/4;500–800 Hz returns 3/4;375–500 Hz returns 4/5 in eligible runs.

**Scope:** The declared level/envelope/filter contrasts only.

**Rules out / prevents:** Do not impose a universal monotone density-to-energy rule.

**Evidence:** E020.

#### F039 — Template reuse did not preserve homogeneous B labels

In 06B, all-A returns 64 class 3 windows; all-B returns 60 class 6 and 4 class 4 windows. Mixed templates have their own measured label sequences.

**Scope:** Context, transition, endpoint and normalization effects remain; source-template names are not labels.

**Evidence:** E023.

#### F040 — 09’s energy grid mostly remains class 4

The sparse A/−18 dB cell returns homogeneous 3. The other eight A/B/C×texture cells return homogeneous 4. New class 1 windows come from the musical arrangements, not a calibrated stationary class 1 template.

**Scope:** Nine coarse parameter points, not a globally flat brightness/activity response.

**Evidence:** E032, E008.

### Overall-energy exclusions

#### F041 — Ordinary rounded mean/median already failed early

[4,6,6,6] returns 5, whereas nearest-integer mean is 6 and median is 6. [4,6,4,4] returns 4, whereas half-up arithmetic mean gives 5.

**Scope:** True returned integer window labels, not predicted labels.

**Rules out / prevents:** Do not describe a nearest-rounded mean/median as an established universal reducer.

**Evidence:** E018, E020.

#### F042 — Geometric half-up was refuted by 06B

Both 1/8-A mixtures have eight class 3, two class 4 and 54 class 6 windows; MIK returns 6 while geometric half-up returns 5.

**Scope:** Two requests containing the same histogram; not two independent aggregation mechanisms.

**Rules out / prevents:** Earlier 162/162 or 165/165 agreement is superseded, not continuing proof.

**Evidence:** E023.

#### F043 — The old square-root hypothesis was not identified uniquely

Before 09 it fitted 219/219 synthetic requests, largely uniform cases. Earlier nonuniform constraints admitted a broad exponent interval around .48≤p<1, not a unique p=.5.

**Scope:** Oracle-input formula checks only; no learned audio-to-overall predictor.

**Rules out / prevents:** A perfect related-data fit is not mechanism identification.

**Evidence:** E037, E041.

#### F044 — 09 refutes square-root pooling

Nine of 58 distinct new requests have [4,4,4,3,6,4]→5. floor(mean(sqrt(labels))²+.5) returns 4. Cumulative synthetic agreement is 268/277.

**Scope:** Nine instances of one rounded-vector failure pattern, not nine independent discoveries.

**Rules out / prevents:** The square-root reducer cannot be carried forward as a correct rule.

**Evidence:** E032, E038.

#### F045 — Any fixed monotone rounded-vector-only reducer is contradicted

The key-A G-major/minor pair gives [4,4,4,4,6,4]→4 versus [4,4,4,3,6,4]→5. Increasing one component 3→4 while holding the other integer components fixed decreases overall 5→4. A fixed coordinatewise-nondecreasing function of those labels alone cannot do this.

**Scope:** Same six positions; audio changes harmony/lead only, with kick/percussion/bass/texture hashes identical. Raw features and exact timings differ. Context-dependent weighting, continuous inputs and arbitrary nonmonotone label mappings are not excluded.

**Rules out / prevents:** Reject fixed positive-weight means and all ordinary monotone power/median/percentile reducers on only these rounded labels; do not overextend to every possible aggregator.

**Evidence:** E031.

#### F046 — No one fixed power-mean exponent fits the controlled data

The numerical[-16,16] feasibility interval is empty. More strongly, the fixed-monotone counterexample excludes the ordinary power-mean family on those rounded inputs regardless of exponent.

**Scope:** Fixed nonnegative weighting and ordinary monotone rounding assumptions; not variable feature-dependent selection.

**Evidence:** E038, E031.

#### F047 — There is no observed identical-vector contradiction in the synthetics

Across 277 distinct synthetic requests, no identical complete ordered rounded-label vector has different overall values.

**Scope:** A limited observed absence. Nonmonotone sequence lookup remains mathematically possible; this does not make it a good learned mechanism.

**Rules out / prevents:** Do not claim that every label-vector-only function has been disproven.

**Evidence:** E039.

### Observed cue evidence

#### F048 — Short cue gaps refute universal wide spacing

Capture 01 has 12-beat positive gaps;02 has four-beat gaps at the relevant fast-tempo convention.09 energy cases have 8/16 score-position gaps.

**Scope:** Returned coordinates under the observed tempo convention; below 100BPM an index is half a musical beat.

**Rules out / prevents:** Do not impose universal 24/32-musical-beat minimum spacing, or confuse score positions with beats.

**Evidence:** E018, E019, E008.

#### F049 — Duplicate and negative cue times exist

A controlled 02 fixture repeats a cue time; captured outputs also include negative times relative to audio sample zero.

**Scope:** Captured output representation; not negative scheduled audio samples.

**Rules out / prevents:** Deduplication, clipping negative timestamps or a uniqueness-only output space changes the research target.

**Evidence:** E019.

#### F050 — Authored transition location is not the selected cue

Some 02 off-grid constructions move to 65–71 while a returned main cue remains near authored 64; other offsets switch to later phrase positions. All 20 hat-step cases return rounded[0,32,64] despite distinct stems/payloads.

**Scope:** These interventions change the whole score shape and sometimes detected grid.

**Rules out / prevents:** No universal “loudest transition wins” or isolated phase-preference rule follows.

**Evidence:** E019.

#### F051 — Wire-score padding and origin must be respected

In the checked pinned profile, the two header bytes are omitted zero-prefix/suffix counts. Restore them before indexing; the score coordinate is shifted by 32 relative to downbeat coordinates.

**Scope:** Observed transport/profile mapping, including the below 100BPM half-beat convention.

**Rules out / prevents:** Do not index the transmitted float 32 payload as a complete unpadded timeline.

**Evidence:** E019, E017.

#### F052 — A hard single-phase filter cannot represent observed lists

In the 117-request phase audit,68 lists contain multiple residues. The local SDK prior matches at least one added cue in 112/117 but every added cue in only 46/117.

**Scope:** Phase/residue diagnostics, not complete position/count accuracy.

**Rules out / prevents:** A useful phase prior is not permission to exclude other residues.

**Evidence:** E017.

#### F053 — A real conditioning difference did not fix the phase failures

Python half-width flooring versus inspected Rust ceiling changes width 32 support from±15 to±16. The paired probe changes zero selected phases across 117 cases.

**Scope:** This paired implementation contrast and wire-float 32 inputs only.

**Rules out / prevents:** Do not treat a discovered code difference as the cause without an outcome change.

**Evidence:** E017.

#### F054 — Cue energy is not universal containing-window lookup

The 02 audit has fourteen mapped failures of direct interval containment. An early cue about 1.732 ms before a class 6 boundary is already labelled 6.

**Scope:** Observed musical cue labels; not a recovered rule.

**Rules out / prevents:** Cue-energy and cue-position prediction are separate targets.

**Evidence:** E018, E019.

#### F055 — The earlier beat-median cue rule is refuted

In 05, three final cues have 33 sampled class 6 versus 32 class 8 beats, predicting median 6; MIK returns 8. Exact rational arithmetic preserves the discrepancy, away from boundary ties.

**Scope:** The three vocal-boundary cases. The old oracle check had been 788/788 on covered cues before these observations.

**Rules out / prevents:** Do not reuse the earlier perfect-match claim or splice per-fixture exceptions.

**Evidence:** E021.

#### F056 — The tested repair candidates also have counterexamples

All 24 merged-run and 24 expanded-window cue-energy candidates tested after 05 fail somewhere. Through 08 the old beat candidate remains 1162/1165 covered, with five additional cues uncovered out of 1170.

**Scope:** Oracle returned labels, not predicted labels; these are historical through 08 denominators.

**Rules out / prevents:** Covered-cue/oracle agreement cannot be presented as complete cue reproduction.

**Evidence:** E021, E040.

#### F057 — 09 preserves structural positions across its key variants

All 48 non-control musical key constructions return relative coordinates[0,33,81,129]. Their downbeat is near authored beat 15, so later cues are near authored 48/96/144. Exact timestamps/energy labels can differ.

**Scope:** These shared-form arrangements only; not independence of key/energy and selection in general.

**Evidence:** E008.

#### F058 — 09’s dither cue difference is numerical, not a changed selected position

The dither control’s six absolute cue times differ by at most 4.62137e−11 seconds. Derived grid coordinates differ by at most 1.42109e−14; cue-energy lists agree.

**Scope:** Exact output equality fails, but there is no evidence here of different selected grid positions.

**Rules out / prevents:** Do not count tiny origin/time differences as a new cue-selection effect.

**Evidence:** E030.

### Model and evaluation history

#### F059 — The old guessed models are retired

The owner retired the previous model bundles. Their weights, transforms, predictions, ensembles and thresholds are not inputs to the new programme; historical observations are retained.

**Scope:** A project decision, not evidence that every former architecture is wrong.

**Evidence:** E001, E016.

#### F060 — The fresh partial prototype was deterministic but failed transfer

The fresh key prototype fits 188/191 training rows but gets 0/10 distinct exact-local 08 keys and 6/24 conditional captured-input keys. Four distinct targets are unsupported classes. It is frozen, not a default candidate.

**Scope:** Related-family development challenge, not independent real-music evaluation.

**Rules out / prevents:** Training fit does not establish transfer. This fitted MLP failure does not disprove all MLPs.

**Evidence:** E035, E026.

#### F061 — The energy prototypes were also partial diagnostics

On ten distinct exact-local 08 requests both energy prototypes match 54/58 windows and 9/10 overall labels using the then-hypothesized pooling rule.08 targets are homogeneous 4.

**Scope:** Bands-only and bands+RMS prototypes; no independent library accuracy or sufficiency proof.

**Rules out / prevents:** Equal aggregate results do not establish RMS irrelevance; the overall pooling rule is now refuted.

**Evidence:** E026.

#### F062 — Feature extrapolation was measured, not proved causal

The 08 key standardization diagnostic reaches|z|122.83 compared with 13.78 maximum in training, with many out-of-range coordinates.

**Scope:** Posthoc conditioning evidence from the failed partial prototype.

**Rules out / prevents:** Do not silently clip inputs or claim this alone explains the errors.

**Evidence:** E026.

#### F063 — Repeated fitting/export checks were exact in the pinned environment

The three fresh fits repeated with identical state/predictions, export/reload exactness, and independent NumPy forward reconstruction.

**Scope:** Only the recorded environment and models; not cross-hardware identity or validated predictive quality.

**Evidence:** E035, E006.

### Independently recomputed track-ledger facts

#### F064 — The supplied combined HAR is smaller and mixed

It has 3452 entries,3449 successful exchanges and 3444 distinct retained requests; it includes 112 known synthetic requests. With the 295 synthetic IDs/277 requests, the union has 3609 distinct numeric requests.

**Scope:** The source hash and exact request-retention definitions in the independent review; these are not counts of independent real songs.

**Rules out / prevents:** Do not call this an exclusively real-music held-out benchmark or use the owner’s approximate track count as the audited count.

**Evidence:** E013, E009.

#### F065 — Histogram-only overall energy is impossible on observed collisions

There are 27 groups with identical unweighted returned-label histograms but conflicting whole-track targets in the supplied combined ledger.

**Scope:** Exact histograms of mapped returned labels. Order, duration, RMS, continuous inputs and context differ or remain possible.

**Rules out / prevents:** Any deterministic histogram-only predictor must fail at least one member of each conflicting group. This is not an accuracy ceiling for richer inputs.

**Evidence:** E013.

#### F066 — Square-root pooling fails broadly on the track ledger

It matches 1320/3444 distinct combined requests (38.33%), compared with 268/277 synthetic requests after 09.

**Scope:** Oracle returned window classes; not a trained feature-based model score.

**Rules out / prevents:** Synthetic fit cannot stand in for real-track transfer.

**Evidence:** E013, E038.

#### F067 — Identical energy vectors have no observed repeat-label conflict here

65 repeated exact energy-feature groups have consistent labels in the combined ledger. No identical complete ordered label vector has differing overall output in that ledger.

**Scope:** A limited-repeat absence, not a sufficiency proof for 183 features or the ordered labels.

**Evidence:** E013.

#### F068 — Run volume follows the first constituent window in this ledger

All 33079 tested response runs match the first constituent input window’s RMS within 1e−8. Adjacent equal labels are represented as merged runs in the inspected records.

**Scope:** Recorded tolerance and interval mapping, not a run-average loudness definition.

**Evidence:** E013, E009.

#### F069 — Observed cue coordinates are integral but strict peaks are insufficient

All 26243 combined timestamps map to integer score coordinates within 1e−5 under the observed tempo convention. Only 12884/22799 later cues occupy strict local peaks of the submitted curve.

**Scope:** Representation/diagnostic checks, not a new selector. Integer positions still allow short gaps and duplicates.

**Rules out / prevents:** A strict-local-maximum-only output space excludes observed targets.

**Evidence:** E013.

#### F070 — The supplied track ledger is heavily class-imbalanced

Only 136/3444 distinct combined responses return a major MainKey; the other 3308 return minor labels.

**Scope:** This supplied mixed collection, not a universal music-key prevalence claim.

**Rules out / prevents:** Report per-class support; high aggregate agreement can hide failure on majors or rare energies.

**Evidence:** E013.

#### F071 — Stage timings show size trends, not a classifier fingerprint

Combined medians are key 27.781 ms, energy 22.634 ms and cues 4.6933 ms. Raw energy-time regression on windows has slope~1.000 ms and R²~.372; cue-time on score count~7.076µs and R²~.261. Key-time versus duration is nearly flat (r≈−.055).

**Scope:** Descriptive profiler observations with unknown measurement boundaries/runtime conditions. Binned-median R²≈.993/.996 is not raw per-request explanatory power.

**Rules out / prevents:** Do not infer kernel, support-vector count, FLOPs, neural size or exclusion of trees/templates from absolute milliseconds.

**Evidence:** E013, E009.

#### F072 — Identical 09 inputs have very different reported times

Key-A original/copy/repeat key times are 24.209/39.323/74.599 ms; key-B 38.834/100.351/40.064 ms, while retained requests and analytical outputs agree.

**Scope:** Profiler fields, excluded from analytical output equality. Future server revisions remain untested.

**Rules out / prevents:** Timing jitter is not changed analysis logic or evidence for a specific model family.

**Evidence:** E011, E008.

#### F073 — Repeated retained requests show no analytical conflicts in the union

The audited union has 157 groups of repeated complete retained requests and zero conflicts in the selected musical outputs.

**Scope:** This includes retained duplicate HAR entries as well as genuine copy/rerender controls. It is not 157 independent repeat experiments or a future-version guarantee.

**Rules out / prevents:** Do not infer server nondeterminism from profiling jitter or changed dither inputs.

**Evidence:** E013.

### Observed cue evidence

#### F074 — The old restricted output space excludes most observed lists

Only 104/277 synthetic lists satisfy the joint historical restrictions: additional positions divisible by 8, additional spacing≥24, at most 7 additional cues and unique times. Zero of 58 distinct 09 lists satisfy all of them.

**Scope:** A representability diagnostic in score units, not trained-model accuracy, and not a new cue inference implementation.

**Rules out / prevents:** Increasing scoring-model capacity cannot recover targets excluded by its candidate/output representation.

**Evidence:** E012.

### Fresh real-data audits

#### F075 — Histogram-only overall energy cannot reach 99% on this cohort

Among 6,386 selected requests, 76 groups with identical unweighted returned-label
histograms have conflicting whole-track targets. Their exact frequencies force
at least 76 errors: even an oracle lookup can match at most 6,310/6,386, or
**98.80989665%**.

**Scope:** Observed request rows in the fresh spent-development cohort. The
histogram includes class counts and therefore window count. This is not a ceiling
for order, duration, RMS, continuous features or an unseen population.

**Rules out / prevents:** A deterministic unweighted-histogram-only reducer
cannot meet 99% on this observed cohort. F065's earlier 27 conflicts alone did not
establish that numeric ceiling; this calculation uses actual group frequencies.

**Evidence:** E042, E043, E045.

#### F076 — Exact feature collisions do not explain the current errors

The 148,010 windows contain 52 repeated exact 183-feature groups, with zero label
conflicts. Adding RMS gives the same result. All 6,386 key vectors are distinct.
Complete ordered window-label vectors have 31 repeated groups and zero observed
whole-track target conflicts.

**Scope:** Observed exact repetitions only. No key-vector repetition exists here
to test key repeat-label consistency. Absence of collisions does not establish
input sufficiency or generalisation.

**Rules out / prevents:** Do not infer sufficiency or a recovered model solely
because an exact-collision audit passes.

**Evidence:** E042.

#### F077 — The complete cue representation covers all 6,386 selected lists

The fresh intake retains 49,410 cues. Every list fits the complete score timeline
plus its origin anchor. Two lists repeat positions; 219 include negative
coordinates relative to the detected grid. All first cues match the request
origin. Client-style timestamp reconstruction is bit-exact on 6,223 lists and
within 1e−9 seconds on all 6,386.

**Scope:** Oracle projection and formula diagnostics on spent development data,
not selector predictions. Negative grid coordinates are not necessarily negative
audio timestamps. Order, multiplicity and raw times are preserved.

**Rules out / prevents:** Representability and oracle timestamp reconstruction
are not cue-model accuracy. Do not replace ordered lists with sets or claim 100%
cue prediction from this check.

**Evidence:** E044.

#### F078 — The first fresh full-input key comparison remains below target

Thirteen declared models yield best exact MainKey agreement of 1,193/1,288, or
92.6242%, on grouped spent-development validation. The selected model gets 8/34
majors; the MLP gets 15/34 with lower aggregate accuracy. The selected independent
refit's predictions and serialized bytes match.

**Scope:** Fresh weights, train-only preprocessing and captured 352-value inputs.
The same spent data served model selection; this is not unseen, all-format or
end-to-end accuracy. There are 205 major requests overall: 171 training and 34
validation.

**Rules out / prevents:** Training fit near 100% does not establish
generalisation. This result does not exclude SVMs, MLPs, other preprocessing or
input sufficiency. No default is promoted.

**Evidence:** E045, E046.

#### F079 — The second fresh key comparison improves development agreement but remains incomplete

Twenty-two declared shape/magnitude/class-weight candidates select a full-register sqrt-profile SVC at 1216/1288=94.4099 percent, versus 1193/1288 in the first comparison. Major agreement is 15/34; minor 1201/1254. Paired predictions correct 45 old errors and introduce 22 new ones. Independent refit predictions and model bytes match.

**Scope:** Same spent grouped development split and captured 352-value inputs. No new independent sample. Fourteen of nineteen major errors choose relative or parallel minors; this is an error description, not its cause.

**Rules out / prevents:** Do not report development selection as unseen or PCM-to-output qualification, or infer universal improvement on every track. No candidate is promoted.

**Evidence:** E063, E055.


#### F080 — The real-trained R2 key candidate fails broad synthetic transfer

The frozen real-trained selected keyR2 candidate matches 174/277=62.8159 percent of existing distinct synthetic requests, including 0/48 major and 174/229 minor responses. There are no exact key-vector overlaps with the real intake.

**Scope:** Posthoc spent synthetic diagnostics, not independent music or training examples for this candidate. Includes 14 old 08 captured-feature-only cases with unqualified local extraction. The test uses captured inputs throughout.

**Rules out / prevents:** Its 94.41 percent real-development result is not evidence of broad controlled-stimulus parity. This failure does not exclude all SVMs or establish the cause as missing semantics, magnitude or register.

**Evidence:** E056.


#### F081 — The first fresh energy comparison leaves a large whole-track gap

Six input-only whole-track and eight sampled-window candidates yield best 886/1288=68.7888 percent overall and 25047/29892=83.7916 percent window agreement. Raw 183/C100 beats the tested standardized window candidates; RMS added to standardized candidates does not recover that loss. Fresh selected refits match predictions, decision values and serialized bytes.

**Scope:** Spent grouped captured-feature development;16000 training windows. Whole-track summaries are declared lossy baselines and use no oracle/predicted window labels. These fits do not identify the server mechanism.

**Rules out / prevents:** Do not claim RMS irrelevance, complete window-sequence accuracy, unseen accuracy or native/default qualification from this model comparison.

**Evidence:** E059, E058.


#### F082 — A narrower window kernel improves development agreement without solving track-level parity

Six fresh contrasts select raw 183/C100 with gamma four times the reference:26129/29892=87.4113 percent window agreement,1082 fewer mistakes. Only 123/1288 tracks have every window correct. Same 4855 training groups with 16000→32000 nested windows reaches 25476/29892=85.2268 percent at reference gamma. Tested weights on the appended RMS feature and balanced class weights do not improve the reference. Selected refit predictions, decisions and model bytes match.

**Scope:** Fixed grouped development split. Numeric gamma is held fixed across RMS/sample contrasts. The doubled sample changes within-group context coverage, not independent group count. These are separate one-factor comparisons; combinations were not tested. Whole-track overall remains 886/1288.

**Rules out / prevents:** A higher per-window score is not complete track/cue parity. These comparisons neither prove RMS unused nor establish the best architecture; no 99 percent or product-default claim follows.

**Evidence:** E060.


#### F083 — Combining the denser window sample with the narrower kernel improves agreement

The third fresh comparison selects 26675/29892=89.2379 percent window agreement with 32000 windows from the same 4855 training groups. Exactly 177/1288 complete window sequences match. Independent selected refits match predictions, decision values and model bytes.

**Scope:** Spent grouped captured-feature development. Compared with the second comparison winner, the winning configuration changes within-group window coverage while keeping its kernel setting. Whole-track energy remains 886/1288.

**Rules out / prevents:** Per-window agreement is not whole-track, full cue or unseen parity. No additional independent songs or product-default qualification are established.

**Evidence:** E061, E048.


#### F084 — The first fresh complete cue baseline has large selection and energy gaps

Fresh count, point-occurrence and cue-energy models produce 25/1288 complete timed and labelled lists. Count matches 1032/1288; entire positions match 224/1288. With true positions supplied, cue energy matches 6705/9922 labels. Export/reload checks pass; a complete independent three-model refit was not performed for this baseline.

**Scope:** Spent grouped captured-feature development. Cue-energy training uses true cue positions; operational evaluation uses predicted positions. The origin-anchor construction is an observed-cohort hypothesis, not a universal theorem.

**Rules out / prevents:** Oracle-position cue-energy accuracy is not complete cue parity. The score representation audit did not predict cues, and this poor baseline is not promoted.

**Evidence:** E050, E052.


#### F085 — A stronger repeat penalty repairs part of the cue decoder error

With all fitted models and predicted counts fixed, alpha 1 reproduces all original lists exactly. Alpha 8 changes exact positions from 224 to 401/1288 and complete lists within 1 ns from 25 to 33/1288; 32 are bit-exact. The observed duplicate-list count falls from 810 to zero in validation.

**Scope:** Posthoc spent-development decoder contrast. Selection still permits duplicate positions; the complete captured cohort includes two duplicate-position lists, both outside this validation subset.

**Rules out / prevents:** Do not infer a universal uniqueness rule, a recovered server penalty or an unseen improvement. Correcting decoder repetitions does not solve cue energy or count.

**Evidence:** E053.


#### F086 — The tested cue-count loss affects complete-list agreement

Holding the point model, energy model and alpha 8 decoder fixed, L1 count regression matches 1091/1288 counts versus Poisson 1032 and L2 1023. L1 yields 417 exact position lists and 36 complete lists within 1 ns, of which 35 are bit-exact. Fresh L1/L2 refit predictions and model bytes match. The constant training-mode count control matches 1080 counts.

**Scope:** Spent grouped captured-feature development; only the count objective/control changes. All 1288 requests remain in each denominator.

**Rules out / prevents:** The constant count is an explicitly restricted diagnostic, not a universal eight-cue rule. Better count agreement alone does not establish complete parity.

**Evidence:** E049.


#### F087 — Broader periodic score context improves the fresh cue-position candidate

Appending 30 request-derived score-context features to the original 74, with point training rows/targets/weights/parameters fixed, raises exact complete positions from 417 to 571/1288 and complete labelled/timed lists from 36 to 43/1288. All 43 are bit-exact. L1 count, cue-energy model and alpha 8 decoder remain fixed. The original-feature refit is byte-exact; new model refits and validation forward outputs are exact.

**Scope:** Spent grouped development. Periods are score-coordinate periods, including the slow-tempo half-beat convention; they do not prune candidates. No independent unseen or PCM-path qualification.

**Rules out / prevents:** This supports the tested candidate feature addition, not identification of the server feature set or architecture. Count and cue-energy gaps remain; 43/1288 is not close to the requested full parity.

**Evidence:** E051.


#### F088 — The bounded linear tree and neural window controls do not beat the kernel reference

On exactly the same 32000 training windows and 29892 validation windows, two linear SVM controls match 18599 and 18637 windows, LightGBM matches 24647, raw-input MLP 24312 and standardized MLP 24099. The R3 kernel reference remains best at 26675. Export/reload predictions match for all five new candidates; no convergence warnings are recorded.

**Scope:** Spent grouped captured-feature comparison of five declared configurations. No new candidate wins selection, so no new selected-model independent refit is required; the retained R3 reference already has its pinned accepted refit.

**Rules out / prevents:** Do not infer that all neural or tree models are excluded, or that the server uses an RBF SVM. This test changes declared model configurations, not training groups, labels or validation denominators.

**Evidence:** E062.


#### F089 — A shared equally spaced latent quantizer does not rescue arithmetic overall pooling

Under a common clamped unit-width quantizer for window and overall labels, 1564/6386 rows have strictly disjoint feasible intervals for uniform arithmetic pooling of the window latent values. Another 190 are boundary-only: they fail the declared half-up rule but are not tie-independent exclusions. Captured binary 64 duration weighting retains all 1564 strict witnesses; its 85 additional strict gaps are only 6.04e-18 to 4.10e-16 label units and are precision-sensitive boundary perturbations.

**Scope:** Necessary per-row interval audit with unbounded end-class bins; independently recomputed with exact rational arithmetic. The tested hypothesis pools exactly the reported windows using the same unit-width thresholds for window and overall ratings.

**Rules out / prevents:** This excludes the specified arithmetic-pooling/common-quantizer mechanism, not all continuous aggregation. Unequal thresholds, different/context-dependent weights, nonlinear pooling, different acoustic support or a separate overall classifier remain possible. Do not count microscopic duration perturbations as additional robust contradictions.

**Evidence:** E065, E064.


#### F090 — Uneven common thresholds cannot rescue uniform latent arithmetic pooling

Eight observed responses supply an exact contradiction to uniform arithmetic pooling of unobserved window scores under any common strictly increasing finite rating thresholds shared by windows and whole-track output. Reconstructed positive rational multiples of their valid closed-bin inequalities imply `(139123129 / 7355852275) × (t_9 - t_1) ≤ 0`, contradicting `t_9 > t_1`. The certificate is checked in exact fractions and independently reconstructed from the captured labels.

**Scope:** Necessary closed-bin relaxation of the 6386-row spent-development cohort, permitting every row its own latent values and unbounded clamped end bins. Positive affine normalization is optional: the independent proof also holds before normalization. Initial numerical-only attempts remain historical evidence; the R3 exact certificate establishes this exclusion.

**Rules out / prevents:** This excludes uniform mean with a common quantizer, not duration/context weighting, nonlinear pooling, distinct overall thresholds or a separate whole-track model. A numerical infeasible status alone would not establish the fact, and the proof does not identify the actual mechanism.

**Evidence:** E067, E068.


#### F091 — Cue-to-next-cue acoustic summaries improve the energy-label candidate

With the latest 571/1288 exact position lists held fixed, appending 189 request-only acoustic span features raises complete labelled/timed lists from 43 to 66/1288 within 1 ns; 64 are timestamp-bit-exact. At true positions, cue-energy agreement rises from 6705 to 7190/9922. Original 757-column training inputs match exactly; the original energy refit is byte-exact. The winning new independent refit has identical model bytes and validation numerical outputs.

**Scope:** Spent grouped development with all 1288 requests retained. New inputs are overlap-duration means of the 183 acoustic features and separate RMSdB, plus explicit span/coverage metadata. Training uses true positions; operational validation uses the fixed predicted positions. Oracle-position accuracy is reported separately.

**Rules out / prevents:** The 72.4652 percent conditional energy score is not complete cue parity; complete lists reach only 5.1242 percent. This does not identify the server aggregation interval or feature set, repair wrong positions, or establish unseen/PCM/default qualification.

**Evidence:** E054, E047.


#### F092 — The bounded whole-track scaling comparison does not improve the reference

Six fresh SVC configurations on the same 5098 training and 1288 validation requests achieve 872,806,866,871,841 and 865 exact overall matches. None beats the retained 886/1288 reference. All 6386 stored input-only summaries are independently reconstructed exactly; each new exported model reproduces predictions and decision values. The retained reference already has its accepted independent refit.

**Scope:** Spent grouped captured-feature development. Controls test raw 183-value means and raw/hybrid 1476-value summaries. Hybrid preserves 1464 acoustic values and standardizes 12 RMS/count/scalar values on training only; numeric gamma follows declared transformed-training variance and factors. Tail-weight changes also change that automatic gamma, so they are geometry/configuration contrasts.

**Rules out / prevents:** These negative configurations do not exclude richer summaries, window-model outputs, sequence models or all SVCs. No oracle window labels enter these predictors, no new candidate is selected and no whole-track accuracy improvement is established.

**Evidence:** E057, E066.

#### F093 — Grouped cross-fitting supplies honest window evidence for whole-track learning

Three fresh window models predict all 118118 training windows while excluding the complete held track groups before sampling or fitting. All 29892 development windows use the pinned full-training reference and reproduce its accepted labels. Every one of 148010 windows is covered exactly once. Fold 0 independently refits with identical bytes and outputs; folds 1/2 have exact export/reload checks but were not independently refitted. All 6386 semantic summaries reproduce exactly.

**Scope:** Same spent 5098/1288 request split. The builder never loads overall targets. Each fold uses 32000 training windows with cap 16/group and a training-only rare-class reserve. Ten fixed class slots and explicit availability flags preserve missing/binary cases. The 150 derived features are lossy summaries, not an identified server representation.

**Rules out / prevents:** Cross-fitting removes the specific in-sample window-prediction shortcut; it does not make this historically spent development cohort unseen or prove input sufficiency. Do not claim all three folds were independently refitted.

**Evidence:** E074, E073, E077.


#### F094 — Honest window outputs improve the bounded whole-track candidate

The four declared overall configurations match 926,963,959 and 830 of 1288 responses, versus the retained 886. The selected 31-leaf tree with original 1476 plus 150 window-output features reaches 963/1288=74.7671 percent. Selected independent refits reproduce labels, numerical decision values and model bytes exactly. Paired outputs correct 161 old errors and introduce 84 new ones. Ratings 8/9 match 16/50, including 0/3 rating 9.

**Scope:** Spent grouped captured-feature development with group-excluded window outputs for training and the full-training window reference for validation. Comparing the winning tree against the retained SVC changes learner and inputs together; the matched SVC with combined inputs reaches 926, versus 830 using summaries of window-model outputs alone. The selected tree fits all 5098 training targets, which does not establish generalisation.

**Rules out / prevents:** The improvement is not unseen or PCM-to-output accuracy, a recovered pooling formula, or a default promotion. Sparse high-rating coverage and large remaining errors prevent a 99 percent claim. No oracle window labels enter overall predictors.

**Evidence:** E075, E076, E077.


#### F095 — Cue-label sequence errors remain even at correct positions

With true positions supplied to the fixed cue-energy models, entire energy-label lists match 99/1288 before the span addition and 142/1288 after it. The new model corrects 1019 individual labels and regresses 534, net 485, reproducing the 6705 to 7190 of 9922 label change. The unchanged selector has only 571 exact position lists; perfect energy labels cannot repair its other 717 lists.

**Scope:** Posthoc saved-output development slices at explicitly oracle positions. First-cue agreement changes 896 to 936 of 1288; later cues 5809 to 6254 of 8634. All counts include the same fixed requests/cues.

**Rules out / prevents:** Oracle energy-sequence agreement is not operational cue parity, and error counts do not identify their cause. The new model improves aggregate counts without improving every label or track; both placement and labelling remain bottlenecks.

**Evidence:** E069.


#### F096 — Shared harmonic candidate evidence improves the new key comparison modestly

The four declared fresh configurations match 1191,1172,1220 and 1212 of 1288 development responses. The selected harmonic candidate model reaches 1220/1288=94.7205 percent versus 1216 for the retained R2 reference; it corrects 37 previous errors and regresses 33. Major agreement improves 15 to 18 of 34; minor agreement 1201 to 1202 of 1254. Independent selected refits match predictions, raw decisions and serialized bytes.

**Scope:** Spent grouped captured-feature development: 5098 training requests and 1288 validation requests. All 352 absolute register/bank slots remain in the transformed 353 feature prefix; a declared 96 feature relative-pitch branch augments it. The balanced harmonic contrast reaches 30/34 majors but 1212 overall, exposing a tradeoff. The raw binary candidate without this branch reaches 1172; direct multiclass tree 1191. No target transpositions or authored-key substitutions are used.

**Rules out / prevents:** The four-track net improvement is not universal per-track improvement, unseen or PCM-to-output accuracy, recovered server architecture, or default promotion. Major coverage is sparse and class balancing does not solve total agreement.

**Evidence:** E070, E071.


#### F097 — The frozen harmonic key candidate transfers better to spent synthetic diagnostics

Without fitting to synthetic targets after selection, the R3 candidate matches 202/277=72.9242 percent distinct captured synthetic requests versus 174/277 for R2. Major responses improve 0 to 15 of 48; minor 174 to 187 of 229. The 295 fixture IDs deduplicate to 277 requests and 277 key vectors; exact key-vector overlap with the real intake is zero.

**Scope:** The same previously inspected synthetic stress set, with 14 old 08 captured-feature-only requests whose local extraction remains unqualified. Selection and independent refit precede this diagnostic. These are model-versus-captured-label comparisons, separate from authored-key-versus-MIK construction comparisons.

**Rules out / prevents:** This is stronger transfer on spent constructions, not independent unseen evaluation or native PCM-path validation. The remaining 33 major errors prevent claiming that harmonic features solved major-key behaviour; no causal or architecture identification follows.

**Evidence:** E072.

#### F098 — Choosing among the compared hard key predictions cannot reach 99 percent on this cohort

On the same 1288 development requests, at least one of the retained R2 and selected R3 models is correct on 1253 requests; adding balanced R3 raises this to 1263. Across all five compared saved model predictions, at least one is correct on 1269/1288=98.5248 percent. Nineteen requests are wrong in every member, including 11 where all five agree on the same wrong key. The five-model set contains a correct major answer on 30/34 major responses.

**Scope:** Posthoc saved-output diversity diagnostic on spent grouped development. The reported union selects a member using the true target and is explicitly an oracle diagnostic, not a fitted or operational selector. Every saved validation row aligns before comparison; no new model, inference, data or service request is used.

**Rules out / prevents:** No hard selection rule restricted to these exact five already computed answers can meet 99 percent on this cohort. This does not exclude score fusion, retraining, a new representation or a different model that can produce different answers. Do not report the oracle union as attained prediction accuracy.

**Evidence:** E083, E082.


#### F099 — Correct cue counts alone do not repair the frozen position selector

The frozen point/count models and alpha 8 decoder reproduce 571/1288 exact position lists and 1091/1288 correct counts. Giving only the decoder the true additional-cue count raises exact position lists to 660/1288: 89 lists are corrected, none regress, and 628 remain wrong. The oracle-count selections match 7652 of 8634 additional target occurrences as multisets.

**Scope:** Explicit oracle-count diagnostic on the same spent grouped captured-feature development requests. Point inputs/rates, legal coordinates, anchor construction and decoder penalty stay fixed. Target counts never enter point features or a fitted model; no fitting occurs. The unchanged selector is tested, not an optimal position predictor.

**Rules out / prevents:** 660 is not operational accuracy, complete timed/energy-labelled cue parity, an unseen result or a universal selection ceiling. Count repair helps only part of this fixed pipeline; position ranking/context remains a separate gap. Multiset event overlap must not replace complete ordered-list agreement.

**Evidence:** E079, E078.


#### F100 — Retained cue-point training targets include multiplicity four

The unchanged 360656-row cue-point training sample contains 326272 zero-occurrence rows, 34382 single-occurrence rows and two rows with four additional cue occurrences. The forced first anchor has already been subtracted. These targets sum to 34390 additional occurrences, exactly 39488 training cues minus 5098 forced anchors. The original 74 feature hash and all 5098 query samples/weights reproduce exactly.

**Scope:** No-fit prospective ranking preflight and independent reconstruction of every retained training query on spent captured-feature development. The initial max 2 assumption was rejected before any real model fitting; corrected linear relevance gains [0,1,2,3,4] support all observed counts. Neither training targets nor sampled coordinates were collapsed, capped or removed.

**Rules out / prevents:** Two duplicate-position lists do not imply maximum multiplicity two. Do not binarise or cap these occurrence targets without declaring a different experiment. This metadata audit is not a fitted model result, universal server repeat rule or proof that a ranking objective will improve predictions.

**Evidence:** E081, E080.

#### F101 — The bounded kernel comparison improves window agreement but leaves rare ratings unresolved

All six raw-183-feature kernel candidates converge. The selected RBF candidate matches 26944/29892 development windows (90.1378 percent), versus 26675/29892 (89.2379 percent) for the retained R3 reference. It corrects 1113 previous window errors and introduces 844. Complete window sequences rise from 177 to 200 of 1288 requests, with 96 corrected and 73 regressed sequences. Independent selected refits reproduce labels, raw decisions and serialized model bytes exactly.

**Scope:** Spent grouped captured-feature development, identical 32000 training-window indices and 29892 validation windows. Polynomial degree-two/three alternatives are worse in this matrix. Macro recall slightly declines from 74.7601 to 74.6764 percent; class 8 falls from 495 to 479 correct of 647, and class 10 remains 0/2. The failed original R5 reporting run and three capped/ineligible R5B fits remain separate; only completed uncapped R5C is accepted.

**Rules out / prevents:** A gain on common window ratings is not a solution for rare ratings, whole-track energy, complete cue parity, unseen music or native PCM prediction. Existing honest OOF feature caches remain their explicitly pinned R3 branch; replacing only validation outputs with R5C would invalidate the matched training representation.

**Evidence:** E088, E087, E086.


#### F102 — The two tested query-ranking replacements regress complete cue agreement

After byte-exact reproduction of the frozen 1288-request selector baseline, ranking truncation 8 and 16 produce 558 and 564 exact position lists versus 571 for the reference. Complete timed/energy-labelled lists within 1 ns fall from 66 to 62 for both; bit-exact lists fall from 64 to 61. Positional fixes/regressions are 101/114 and 105/112; complete-list fixes/regressions are 7/11 and 8/12. The predeclared full-list selection retains the reference.

**Scope:** Spent grouped captured-feature development. Both fresh rankers use the same 104 features, 360656 point samples, 5098 queries, occurrence targets including multiplicity four, weights, fixed L1 count model, span-energy classifier and alpha-8 repeat-capable decoder. The changed ranking objective plus raw-score utility link is an explicitly designed combination. Both model export/reload checks pass; no new winning independent refit is claimed because neither candidate wins.

**Rules out / prevents:** These two objective/link configurations are not improvements. This does not exclude other ranking methods, different features, a structured decoder or a new model. Do not present partial event overlap, position-only changes or the occurrence of repeated predictions as complete cue parity or a recovered server rule.

**Evidence:** E085, E084.

#### F103 — Complete 24-key query ranking does not improve the accepted harmonic candidate model

The two full-query ranking configurations match 1172 and 1157 of 1288 development keys, compared with 1220 for the accepted harmonic binary reference. Both match 8/34 major responses versus 18/34; minor matches are 1164 and 1149 of 1254 versus 1202. Each corrects 16 reference errors while introducing 64 and 79. The frozen exact-key/macro-recall selection retains the reference.

**Scope:** Spent grouped captured-feature development, identical 5098 training requests and complete 24-candidate queries (122352 paired rows), 462 harmonic candidate features, one captured positive per query and retained positive/negative point weights 1 and 1/23. Truncations 1 and 4 change the ranking objective/group normalization as a declared combination. Both exports reload exactly. Each actual booster contains 85 trees/iterations; 500 is the configured maximum, not an executed-round count. The exact reason for fewer fitted rounds is not established by the review. No new selected independent refit is claimed.

**Rules out / prevents:** These two complete-key ranking configurations regress total and major agreement. Neither failure proves that every ranking method fails, the 352 inputs are insufficient, the vendor uses a particular architecture, or extra features/observations cannot help. Preserve actual fitted-tree counts and do not relabel configured maxima as completed iterations. No unseen, PCM, native or default qualification follows.

**Evidence:** E093, E092.


#### F104 — Predicted window context improves cue labels while positions stay fixed

With the selector, count model, positions and timestamps fixed, adding 150 cue-to-next-cue predicted-window summaries raises complete timed and labelled lists within 1 ns from 66 to 187 of 1288. Adding all 390 local/whole-track/span features reaches 199/1288 (15.4503 percent), with 195 timestamp-bit-exact lists versus 64. The selected all390 model corrects 153 old complete-list failures and regresses 20. Count agreement remains 1091 and exact position lists 571 of 1288.

**Scope:** Spent grouped captured-feature development with the original 946 cue-energy inputs, learner, 39488 training cues and targets retained. Training window outputs exclude each entire track group and validation uses the matched accepted R3 branch. At true cue positions, individual label agreement rises 7190 to 8515 of 9922, correcting 1796 labels and regressing 471; complete oracle label sequences rise 142 to 435, with 331 corrected and 38 regressed. All390 class8 is 107/150 versus 33, class9 stays 0/3, and validation has no class10. All four executed fits have 300 boosting rounds and 3000 native trees. The selected independent repeat matches raw joblib bytes, all-tree canonical bytes, numerical outputs and both prediction files. Historical reference canonical state and exhaustive outputs match; its raw pickle container differs, explicitly reported under the frozen C2 contract.

**Rules out / prevents:** Useful predicted context in this fitted cue-label pipeline does not identify the vendor architecture, improve its unchanged placement model, isolate the effects of all individual context blocks or establish unseen/PCM/default parity. Oracle-position agreement is not operational complete cue parity. Aggregate gains do not erase individual or rare-class failures; preserve the historical serialization limitation.

**Evidence:** E090, E089, E091.


#### F105 — The tested duration-weighted distribution expansion does not improve whole-track energy

Appending 970 duration-weighted quantiles to the same 1626 inputs with the same accepted learner matches 958/1288 development overall ratings, below the retained reference 963. It corrects 31 errors and regresses 36. Ratings8-9 remain 16/50 in aggregate, with one corrected and one regressed track; rating9 remains 0/3. The predeclared selection retains the reference.

**Scope:** Spent grouped captured-feature development, identical 5098 training requests, 1288 development requests, targets, groups, cache branch, parameters and seed. Added quantiles cover 183 raw acoustic fields, separate RMS and ten aligned window-model margins with duration weights. Existing inputs already include unweighted RMS and margin quantiles; the new branch is the declared combined expansion. The original prefix and full OOF cache invariants validate. The candidate has 500 executed boosting rounds and 4500 native trees; it reports 5098/5098 training labels, which is not generalisation. Export/reload labels, raw decisions and canonical bytes match. The accepted reference is reused with its pinned prior repeat receipt and runtime full label forward; no new selected refit is claimed because the candidate loses.

**Rules out / prevents:** This combined feature expansion is not an improvement for this frozen learner/cohort. It does not show that distributions are irrelevant, exclude other representations or learners, or recover a universal overall-energy formula. Keep the prior model; do not promote perfect training agreement as unseen or PCM accuracy.

**Evidence:** E096, E095, E094.

#### F106 — Nearby predicted-window inputs give a nominal full-cue gain but regress placement

Appending 90 local predicted-window fields to the same 104 point inputs changes complete timed/labelled lists within 1 ns from 199 to 200 of 1288: seven old failures are corrected and six formerly correct lists regress. Timestamp-bit-exact lists rise 195 to 196. Exact position lists fall 571 to 568, with 19 corrected and 22 regressed. Counts remain 1091/1288. The frozen full-list-first rule selects the new model; the prior model remains better for position-only agreement.

**Scope:** Spent grouped captured-feature development with identical 360656 training point samples, 5098 training queries, targets including multiplicity four, weights, Poisson parameters, L1 count model, alpha8 decoder and legal coordinates. Only the 90 current/previous/following matched R3 OOF outputs are appended. The C2 all390 energy state is fixed, but new coordinates legitimately alter its spans and labels. Of 1288 requests, 116 position lists change and 85 of those change their energy-label vectors; all 1172 unchanged-position lists retain identical times and labels. Runtime reproduces all baseline coordinates/labels/timestamp bytes before fitting. Both candidate and independent repeat execute 300 boosting rounds/300 native trees, with raw model, all-tree canonical, rates/raw-score and prediction bytes identical. Only cache fold0 has an independent refit; other cache folds have reload checks.

**Rules out / prevents:** The one-track net full-list gain is not evidence of a substantial or statistically established improvement, and this configuration does not improve placement overall. Do not generalise the earlier cue-label context gain into a placement gain, report the retained 571-placement reference as the selected new pipeline, or claim unseen/PCM/native/default parity. Preserve full-list and position-only tradeoffs.

**Evidence:** E100, E099, E098, E097.

### Verified later experiment and input-audit findings

The following entries add independently checked results from retained captures.
No new synthetic audio was created for these findings. Model outcomes, input
observations and QA results are factual research evidence; their exclusions apply
only to the stated experiment. They do not identify a hidden server architecture.

#### F107 — The bounded direct-summary search does not improve overall energy

**Type:** Model experiment result.

A 24-trial Optuna search completed 21 trials and pruned three, with none failed.
Grouped training-only selection chose a CatBoost regressor scoring 888/1,288,
versus the retained 963: 87 errors corrected and 162 introduced. Synthetic
transfer is 179/277. The same-input historical reference scores 886; the new
model corrects 128 of its errors and introduces 126, a descriptive net two.

**Scope:** Four model families and two direct summary representations on the
same spent split. Selected labels, numbers and canonical state repeat; raw
CatBoost containers differ in verified GUID/finish-time metadata.

**Rules out / prevents:** Reject this selected replacement. Do not exclude
CatBoost, tuning or richer representations generally, or present net two as
established real-music improvement.

**Evidence:** E101, E102.

#### F108 — The first bounded M4 symbolic equation fails transfer

**Type:** Model experiment result.

Eight input columns selected on 2,549 inner-fit tracks support a size-15 equation
selected on the other 2,549. It matches 777/1,288 spent development outputs,
versus 963: 92 corrections and 278 regressions. Ratings 8–9 match 2/50. Synthetic
transfer is 98/277; a constant-four imbalance control matches 238/277. Synthetic
macro recall is 34.45%, versus 25% for that control. The equation misses one
member of the retained monotone-reducer witness.

**Scope:** Serial 64-bit Julia/SymbolicRegression search with pinned M4 packages.
500,000 evaluations and 180 seconds are configured maxima, not verified executed
counts. Independent formula/scaler/selection/transfer checks passed. No full
training refit or repeat search was performed; 14 old 08 inputs remain conditional.

**Rules out / prevents:** Reject this equation/feature-selection/search recipe.
Do not exclude all symbolic regression or continuous/context-dependent rules.
The constant-four control is not a replacement model.

**Evidence:** E103–E106.

#### F109 — The first fixed sequence recipe does not improve overall energy

**Type:** Model experiment result.

Position-aware pooling at epoch 14 wins inner selection, 1,784/2,549 versus
1,773 for the GRU at epoch 12. The full-training refit scores 904/1,288 versus
963, with 96 corrections and 155 regressions. Ratings 8–9 match 8/50. Synthetic
transfer is 190/277 versus the constant-four control's 238/277.

**Scope:** All 148,010 windows; 183 fields, RMS, relative position/duration and
track context. Repeated full fitting, reloads and independent pooled inference
match. Unselected epoch metrics have no saved per-epoch states to recount.
Both models receive positions, so this is recurrent versus position-aware
pooling, not order versus no order.

**Rules out / prevents:** Reject this fixed recipe. Do not exclude all sequence,
neural or pooling methods, or claim unseen/PCM/native qualification.

**Evidence:** E107, E108.

#### F110 — The bounded multitask sequence search does not beat the reference

**Type:** Model experiment result.

The planned 48-trial search reaches its 1,800-second between-trial limit after
32 trials start: 29 complete, three prune and none fail. Grouped two-fold
selection chooses a width-16 GRU with auxiliary window supervision. Epochs
39/25 select 32 full-refit epochs. The fixed primary seed scores 907/1,288
versus 963: 85 corrections and 141 regressions. Ratings 8–9 match 14/50 versus
16/50. Prespecified diagnostic seeds score 903 and 918; neither is selected.
Synthetic transfer reaches 211/277 versus constant four's 238/277; macro recall
is 43.41% versus 25%.

**Scope:** Auxiliary labels enter fit loss only. Both inner folds select epochs;
these are selection scores, not an independent test. A trial may finish after
the timeout. Primary state, containers, logits, ratings and reloads repeat
exactly. Independent GRU arithmetic reproduces all final labels, with maximum
logit differences below 2e-6. Fourteen old 08 cases remain conditional.

**Rules out / prevents:** Retain 963. Reject this selected configuration, not all
neural, auxiliary, ordinal or tuned models. Transfer measurements on existing
synthetic captures are new evaluation evidence, without new audio/capture
generation or established real-music parity.

**Evidence:** E109, E110.

#### F111 — The key-score cache meets its group and identity guards

**Type:** Reproducibility and data audit.

Ten 500-tree fits construct scores shaped [6,386, 2, 24]. All 5,098 training
rows receive group-excluded scores from both harmonic bases; 1,288 development
rows use matched full models. Fresh full fits reproduce current canonical
exports and all 6,386 historical-object scores/labels exactly. Historical
pickle bytes differ; their cause is not fully attributed. Both fold-zero fresh
repeats match raw bytes, canonical state, scores and labels. All six fold
exports reload exactly. Every fit fold observes all 24 keys, with sparse majors.

**Scope:** Independently checked inputs, scalers, outputs and ledgers. Only fold
zero has independent fold refits; full-reference guards are separate. Canonical
comparison uses current-environment exports, not arbitrary-machine identity.
This cache alone establishes no meta-model accuracy gain.

**Rules out / prevents:** Do not train fusion on full-base in-sample outputs.
Row-wise OOF does not guarantee safe selection on arbitrary later inner folds;
matched/nested group exclusions remain necessary.

**Evidence:** E111, E112.

#### F112 — Request-only linkage leaves 664 groups quarantined

**Type:** Capture identity and exposure audit.

A supplied 4,135-entry capture contains 4,129 distinct selected-input hashes.
Request-only linkage finds 664 unmatched entries/groups; 3,315 spent-overlap
entries/3,313 groups; 40 sealed entries/39 groups; 115 diagnostic entries/112
groups; and one prior identity with uncertain exposure. No new bridge between
old groups is found. All 664 unmatched groups remain quarantined, with no labels
admitted.

**Scope:** Exact selected-input, key-vector, normalized-name and known-group
linkage against 10,176 prior identity rows, the 6,386-row cohort and 79 sealed
hashes. The request-only code/canary test supports no nested response-JSON
decoding; independent component reconstruction verifies classification and
quarantine without opening response labels.

**Rules out / prevents:** Neither an owner-reported addition count nor 664
nonmatches proves independent recordings, successful responses, class coverage
or final-test eligibility. Keep recording/alias qualification and label blinding.

**Evidence:** E113, E114.

#### F113 — Fixed key fusion improves some majors but loses exact agreement

**Type:** Model experiment result.

Appending 48 full base margins and two own-candidate margins to the unchanged
462-column harmonic prefix gives 1,218/1,288 versus 1,220: 15 corrections and
17 regressions. Major matches rise 18→22/34 while minor matches fall
1,202→1,196/1,254. Synthetic aggregate agreement rises 202→205/277, with 11
corrections and eight regressions, but major matches fall 15→9/48; minors rise
187→196/229. The exact-first rule retains 1,220.

**Scope:** One fixed matched learner, group-excluded training scores and matched
full bases for validation. The raw 352 fields remain; 400 external inputs form
512 candidate columns, with all 24 possible answers. No meta-CV/tuning is claimed.
Repeated bytes/state/margins/labels and the copied-base→meta inference chain match.
Independent reconstruction shares the documented immutable harmonic helper and
native boosting engine. Fourteen old 08 inputs remain conditional.

**Rules out / prevents:** Do not hide the synthetic-major regression behind the
aggregate increase, promote training fit as qualification, or exclude all fusion.

**Evidence:** E115, E116.

#### F114 — Adding the global acoustic vector does not improve this energy fit

**Type:** Model experiment result.

Appending 352 raw key-path acoustic values to the unchanged 1,626 energy inputs,
with the same 31-leaf learner, gives 954/1,288 versus 963: 32 corrections and
41 regressions. Ratings 8–9 match 13/50 versus 16/50. The candidate executes 500
rounds/4,500 native trees and is rejected.

**Scope:** Identical targets, groups, seed and R3 window-cache branch; no added
scaler, key labels/predictions or tuning. All ownership, prefixes, semantic
summaries, reference outputs and exported native inference were verified.
The losing candidate has reload evidence only, not an independent fresh refit.
No new synthetic audio/capture or transfer test was performed for this contrast.

**Rules out / prevents:** Retain 963. This does not prove global acoustics are
irrelevant or identify which request fields the server uses.

**Evidence:** E117, E118.

#### F115 — Every frozen request has one key segment and twelve zero pitch values

**Type:** Request-input/schema observation.

All 6,386 frozen requests have one key segment, beginning at zero and ending at
request duration, with twelve finite zero PitchProbabilities values. Every first
352-value vector reproduces its stored input. CuePointData is a string throughout,
with lengths 176–12,120 characters. The twelve constant values provide no varying
omitted signal in this cohort.

**Scope:** Independent request-only rereading of 6,371 pinned JSON files and
exactly 15 selected HAR request bodies, without response labels. Those bodies
were resolved by the frozen source hash because the original capture filename
now refers to updated bytes. One segment still contains the known periodic
352-value bank representation.

**Rules out / prevents:** Do not generalize to other versions/requests, discard
nonzero pitch values elsewhere, infer input sufficiency or claim chronology is
unused. Resolve immutable capture versions and verify semantic input identity.

**Evidence:** E119, E120.


#### F116 — The raw-feature SVR anchor loses to the matched classifier

**Type:** Model experiment result.

A fixed raw-183-field RBF SVR matches 20,948/29,892 window labels versus 26,944:
996 corrections and 6,992 regressions. Complete window sequences fall 200→9 of
1,288 tracks, with two corrected and 193 regressed. Both new fits converge in
83,483 iterations with 9,267 support vectors. Repeated bytes/state/raw scores,
decoded labels and reloads match. Independent full RBF calculations reproduce
every decoded label, with maximum raw-score differences below 2.6e-12 against
the predeclared 1e-7 tolerance.

**Scope:** Same 32,000-window sample/4,855 groups and raw inputs as the reference;
C100, gamma .18616444995021475, epsilon .49, an uncapped solver and declared
1 GiB cache versus the reference's 256 MiB. The explicit model decoder is
floor(score+.5), clamped to 1–10; it is not an identified MIK quantizer. Input, support-vector and raw-score arrays, and reference calculations, use
float64; native solver internals are not independently certified all-double
arithmetic.

**Rules out / prevents:** Retain SVC. Reject this particular SVR/decoder recipe,
not all regression or ordinal methods. Equal numeric C does not establish equal
regularization across objectives. These are window results, not whole-track
energy or complete-cue parity; matched existing OOF caches remain unchanged.

**Evidence:** E121–E123.


#### F117 — The bounded count-only search does not improve complete cues

**Type:** Model experiment result.

All 48 Optuna trials complete, with none failed or pruned. Two-fold training-group
selection chooses a 600-tree depth-five CatBoost classifier on the original 22
inputs: 4,391/5,098 counts, versus L1's 4,368 and the constant mode's 4,372.
On 1,288 spent development requests it matches 1,088 counts versus 1,091, with
18 corrections and 21 regressions. Complete positions fall 568→563, with eight
corrections and 13 regressions. Complete timed and labelled lists fall 200→198,
with four corrections and six regressions; timestamp-bit-exact lists fall
196→194. Sixty-three lists change and 1,225 remain exactly unchanged.

**Scope:** Two input-only representations and three techniques; no inner learned
window outputs or target-bearing features. Fixed point, energy, decoder and
matched window-cache states. Changed counts legitimately alter cue spans and
labels. All 96 fold models, two control models, selection and every final list
were independently recounted. Selected refits/reloads, canonical state, numeric
outputs and labels match. Raw CatBoost containers differ only in verified GUID
and finish-time metadata; normalizing only those fields restores byte identity.
No new audio/capture, synthetic-transfer or unseen/PCM/native qualification.

**Rules out / prevents:** Retain 200. Reject this selected configuration, not all
count models, CatBoost or geometry/activity features. Better count selection
alone would not establish complete-list parity.

**Evidence:** E124, E125.

#### F118 — Full training-window coverage gains aggregate matches but regresses known cases

**Type:** Model experiment result.

Using all 118,118 training windows from 5,098 groups instead of 32,000 from 4,855
raises exact window matches from 26,944 to 27,711 of 29,892: 1,550 corrections
and 783 regressions. Complete ordered window-label sequences rise from 200 to
307 of 1,288, with 180 corrections and 73 regressions. Rating 9 falls from
11 to 10 of 18; rating 10 remains 0/2. Both converged fits have 31,040 support
vectors. Model bytes, typed learned arrays, canonical state, scores, labels and
reloads repeat exactly.

**Scope:** One fixed raw-183-field RBF SVC, C100 and gamma .18616444995021475,
with an uncapped solver and cache 8,192 MiB versus 256. Full coverage adds 86,118
windows and 243 previously unsampled training groups; unit-window loss mass by
track and class also changes. This is a combined training-design contrast.
Independent full RBF, vote and OVR calculations match every label, with maximum
score differences below 9e-12 against the declared 1e-7 tolerance. Exact-zero
prediction and displayed-score tie conventions are tested separately. Fits take
225.058 and 224.692 seconds; total run time is 1,000.006 seconds. No new audio,
capture, synthetic transfer, unseen/PCM/native or default qualification.

**Rules out / prevents:** The earlier research-reference acceptance is withdrawn
under the owner's case-level regression gate; the measured counts are unchanged.
Keep 26,944 as a comparison control and 27,711 as an unaccepted challenger.
Investigate all 783 window regressions and the 73 regressed complete sequences
before treating a correction or combination as a replacement. Do not hide rare
failures, attribute the gain solely to density, infer overall/cue gains or choose
models using target labels. Group-excluded outputs may support diagnostics, but
arbitrary later inner selection still requires matched or nested exclusions.

**Evidence:** E126, E127.


#### F119 — Tested tempo, pad voicing and lead register retain E major in one controlled context

**Type:** Controlled MIK observation.

All eight major cells in a complete 2×2×2 matrix return E major (12B); the minor
control returns E minor (9A). None of the 12 one-factor edges changes MainKey.
The exact copy and fresh rerender have identical analytical results; variable
profiling durations are separate. A dither-only replicate retains MainKey while
confidence and small timing values change. The historical major/minor baselines
reproduce PCM, all ten analysis-input fields and all five common analytical
response fields exactly. Major confidence varies from about 0.4040 to 0.6523.

**Scope:** One E-tonic A-arrangement context: tempo 126/140, compact versus
root/fifth-down-an-octave pad voicing, and the original versus octave-lowered lead.
There are 12 files, ten distinct requests and nine musical construction conditions;
eight request identities are additional to the frozen 277. Parsed numeric equality
does not imply identical JSON encodings of 0 and 0.0. F057 already records the
detected downbeat near authored beat 15: 0/12 pass the original 3 ms absolute-origin
gate. The failed qualification is preserved; independent review permits only a
key diagnostic of the total audio→extractor→MIK intervention, including possible
timing mediation. Tempo also changes duration, density and envelope overlaps;
lead transposition changes which harmonics fall below the declared ceiling.

**Rules out / prevents:** These changes, individually and in the tested
combinations, are insufficient to reproduce the earlier B-arrangement major-
construction→minor result in this A context. This is not general register/tempo
invariance or evidence that those inputs are ignored: confidences change. Do not
claim direct register causality, cue/grid qualification, twelve independent
recordings, server architecture or broad real-music accuracy.

**Evidence:** E128, E129.

#### F120 — A frozen key alternative solves the targeted synthetics but regresses known minor tracks

**Type:** Model experiment result.

Predictions saved before run 10 give the current key control 1/12 file matches
(1/10 distinct requests), versus 12/12 (10/10) for an already fitted class-balanced
variant. No model was refitted on these clips. On the same 1,288 spent real
requests, the alternative has 1,194 cases correct in both models, 18 corrections,
26 regressions and 50 failures in both. All 26 regressions have MIK minor targets:
eight parallel-mode errors, seven relative-mode errors and eleven other-tonic
errors. Major matches rise 18→30/34; minor matches fall 1,202→1,182/1,254.

**Scope:** Frozen captured-feature key classifiers; the alternative weights the
24 target classes during training. The comparison is 1,220→1,212 against the
current control. F096's 40 corrections/44 regressions instead compare the same
alternative with the older 1,216 control; those historical figures remain valid.
The corrected case audit requires opposite modes for a relative-major/minor error;
a same-mode shift of three semitones is a different-tonic error. Three earlier
diagnostic annotations were corrected, with raw predictions and totals unchanged.
The new synthetic family is linked to its historical parents for future splitting.

**Rules out / prevents:** A perfect small controlled batch is not acceptance of
the alternative. Preserve its 26 real regressions and the other historical known-
correct witnesses alongside the synthetic correction. Do not mix comparison
baselines, infer exact MIK class priors or choose models using reference answers.
An input-derived correction still needs the declared regression, unseen-music,
PCM, native and supported-format gates.

**Evidence:** E128–E132.

#### F121 — Three passes preserve analytical results for twelve real recordings

**Type:** Measured MIK repeatability observation.

All 36 captured analyses succeed: each of twelve real recordings appears once in
three scan blocks. All 36 within-recording pairings have byte-identical
HAR-exposed request bodies and exactly identical parsed analytical results,
including confidence and complete cues. Full response bytes and full parsed JSON
are identical in zero pairings; the only changing fields are the three key,
energy and cue profiling times.

**Scope:** These recordings and three passes in one owner-supplied capture.
Body-byte equality concerns HAR-exposed content after any HAR base64 decoding,
not headers, compressed wire bytes, packets or encrypted traffic. No new model
fit, agent-controlled vendor call or training-label admission is involved.

**Rules out / prevents:** Separate analytical repeatability from variable runtime
reporting. The observed stability is compatible with both fresh deterministic
computation and caching. It does not establish a cache hit, cache key, universal
absence of randomness or the server's model architecture.

**Evidence:** E133, E134.

#### F122 — One real recording retains final decisions under one-second suffix edits

**Type:** Controlled real-source MIK observation.

Three files preserve every original stereo PCM sample and gain: unchanged,
appended one second of exact digital zeros, and appended one second of seeded
noise. All three captured analysis inputs match local extraction exactly; the
unchanged baseline also matches all analytical results from its three earlier
passes. MainKey, overall energy and the complete window-energy sequence stay
unchanged. All eight ordered cues retain their count, order and non-time fields.
Silence preserves their timestamp bits exactly; noise changes timestamp bits by
at most 5.6844×10⁻¹⁴ seconds, retaining complete agreement within 1 ns.

Confidence responds: the silence and noise changes are respectively
+0.0001953840 and −0.0065430999. The returned whole-track segment endpoint increases
one second in both variants. The actual key input changes 21 of 352 values for
silence and 288 for noise; relative L2 differences are about 0.01896% and 0.16676%.
Duration and energy-input fields also change, so the suffixes are not identical
analysis inputs despite exact preservation of the original audio prefix.

**Scope:** One underlying spent real recording, not three independent songs or a
replacement-model accuracy measurement. Original 44.1 kHz stereo PCM16 is kept
exactly; the noise is −30 dBFS RMS before quantisation with declared 5 ms edges
and seeded TPDF. The original mastered level is an explicit inherited-source
exception, not a pass of the −1 dBFS controlled-synthesis contract. Postscan PCM
and FLAC audio-frame bytes match protected masters despite changed metadata.

**Rules out / prevents:** “100%” here describes consistency of the predeclared
final decisions across this contrast under the 1 ns criterion. It is not full
response byte identity, universal robustness, unseen-music qualification or
accuracy of our replacement. Confidence remains a separate responsive field.
Changed confidence contradicts blind reuse of the unchanged complete original
analysis for these edits; it does not identify cache bypass, fresh computation,
cache keys, server architecture or the request field causing the change.

**Evidence:** E135–E138.


#### F123 — Lowering the sine bass alone preserves keys in the tested A context

**Type:** Controlled MIK observation.

Lowering the same 108 sine-bass events from MIDI 40 to 28 preserves MainKey in
both authored modes: major remains 12B and minor remains 9A. Confidence rises by
0.0091878772 and 0.0062176585 respectively. Within each mode, window-energy labels
and overall energy also stay unchanged. The exact copy and fresh six-stem rerender
produce identical analytical responses, and both reused baselines reproduce their
earlier analytical results exactly. Profiling durations remain separate.

**Scope:** Seven files, four musical conditions and five distinct numeric requests
from one linked E-tonic A-context family. Five unaffected stems, event schedules,
gain, content initialization and primary dither are fixed. The bass edit also
changes its finite-envelope waveform, interaction with the fixed kick, whole-mix
RMS by about −0.25635 dB, and extracted features/timing. All seven actual inputs
match local extraction. Protected masters and postscan PCM/audio-frame bytes are
exact despite metadata changes. The inherited absolute-origin gate still fails.
Cue count/order/energy labels agree within each bass pair, but cue times move up
to 0.730635/0.680308 ms and exceed the cross-treatment 1 ns identity criterion.

The frozen key control matches 2/7 files (2/5 distinct requests); the balanced
candidate matches 7/7 (5/5). Its 26 known real-track regressions remain (F120).
No model was fitted, accepted or promoted.

**Rules out / prevents:** This particular bass-octave edit is insufficient to
reproduce the earlier A/B key contrast in the fixed A context. It does not exclude
bass/register effects generally, isolate a direct server-input cause, qualify the
cue grid or establish independent real-music accuracy from related fixtures.

**Evidence:** E139, E140, E143–E145, E147.

#### F124 — One dither-only PCM contrast changes energy and a cue label

**Type:** Controlled MIK observation with independently checked local inputs.

Changing only the quantisation dither realization from 990901 to 990912 on the
same authoritative float64 mix preserves MainKey 12B but changes overall energy
5→4, the fourth nominal window 3→4, and the third ordered cue's EnergyLevel 3→4.
The six-window sequence changes [4,4,4,3,6,4]→[4,4,4,4,6,4]. Copy/rerender controls
with identical PCM and inputs retain exact analytical responses.

Independent TPDF reconstruction reproduces all 4,032,000 mono PCM samples in both
files. Their maximum difference is two 16-bit steps and difference RMS is
−93.3177823 dBFS. In the authored pad-interlude mask at beats 96–128, PCM RMS changes
only −0.0000053421 dB. The nearly aligned fourth captured window's separate
VolumeRmsDb changes −0.0000055089 dB, while its emitted 183-value feature vector
changes 8.31546486% in relative L2 norm. Scaled transient-count fields account for
99.9999789% of squared coordinate change under the inspected reducer mapping.
These are derived signal counts; authored events are identical.

Fresh repeated local extraction and independent checks reproduce actual captured
inputs exactly per PCM. Both frozen window classifiers predict 2 for both
interludes, where MIK returns 3 and 4. Relative cue beats [0,33,81,129] stay fixed;
the actual tempo/downbeat fields reconstruct both returned timestamp lists within
5.69×10⁻¹⁴ seconds, although the clock fields and returned times themselves change.

**Scope:** One declared dither-only physical-PCM contrast, with different numeric
analysis inputs. The nominal interval is not a complete acoustic support mask
(F032). The coordinate partition uses existing emitted scaling and is not causal
feature importance. The initial capture audit left the intermediate route
unresolved; F126 supplies a later bounded local trace. The server's decision/overall
mechanism remains unresolved. No new fit, training admission,
audition, product-default change or agent vendor call occurred.

**Rules out / prevents:** Do not automatically copy energy/cue labels across tiny
PCM or dither augmentations. This counterexample does not imply universal dither
fragility, identical-input randomness, cache behaviour or server architecture.
Preserve exact extraction instead of smoothing it to impose an unverified
invariance. The identical-input repeatability in F121–F122 remains distinct.

**Evidence:** E139–E144, E146, E147.


#### F125 — The reduced-family unit correction is independently corroborated

**Type:** Source/unit correction and reproduction audit.

The original inspected source and the qualified private phase probe both use
one-eighth-beat reduced-family spacing, a 32-beat grid of 256 positions. Independent
reconstruction from saved raw band timelines reproduces all 43,008 family values
across three interlude cases exactly. This confirms the correction in F032 and
identifies the earlier half-beat/about-128-beat description as an inherited
interpretation error.

**Scope:** This local processing profile and the captured-default/family-array
qualification. The separate phase search still evaluates a roughly 64-beat grid
(63.75 beats between first and last centers, with zero endpoint weights), plus
selected offsets, interpolation and signal-state support. These grid sizes do not
define an isolated rectangular acoustic support mask.

**Rules out / prevents:** Preserve the original report, source hashes and old fact
versions. F033's observed restoration from frame 1015 and residual from 1018 remain
recorded; the historical vocal pair was not rerendered here. Correct the unit
interpretation explicitly instead of silently rewriting measurement history.

**Evidence:** E148, E151, E152, E155.

#### F126 — Local threshold-count arithmetic explains the large feature change

**Type:** Local DSP intervention and independent reference-calculation result.

Holding the dithered file at baseline whole-file normalization and independently
estimated energy tempo leaves the interlude's feature discrepancy essentially
unchanged at 8.31546%. All 84 naturally selected phases are exactly equal between
the two files; forcing baseline phases leaves the dithered feature file
byte-identical. Holding normalization, tempo and phases together still leaves
64 of 70 interlude transient-count coordinates different.

Independent calculations reproduce all 2,100 count coordinates exactly across
five qualified runs, six windows and 70 count fields. The calculation includes
reduction gain, cyclic geometric means, the 90th percentile, an inclusive threshold
at half that percentile, integer counts and recorded scaling. In the 2,500–3,700 Hz
even-transient field, the raw count changes 45→62: 24 of 128 sampled positions stop
passing the threshold and 41 start passing it. This reconstructs the large
count-coordinate change while the authored musical events stay fixed.

**Scope:** Private local data-flow interventions with exact captured default
features, a joint own-settings no-op, verified realized overrides and unchanged
pre-normalization source timelines. Internal transient units/counts are distinct
from PCM dBFS and authored event counts. Forcing a phase affects the next band's
search center; the phase-only call retains the dithered file's energy tempo and
does not freeze absolute sampling positions. The normalization-only intervention
could affect the recomputed tempo generally; that tempo happens to remain exact
in this pair.

**Rules out / prevents:** The measured normalization/tempo differences and a
phase-selection switch are insufficient explanations for this pair's large
feature discrepancy. This closes the specific local count-coordinate arithmetic;
it does not identify which server inputs cause the changed window, overall or
cue labels. Intervened numeric inputs were never submitted to MIK. Preserve the
matching extractor behaviour instead of imposing an unverified smoothing rule.
No model was fitted or promoted, and no new audio or vendor capture was needed.

**Evidence:** E149–E155.


#### F127 — Staged group removals change cues while reported energy stays fixed

**Type:** Controlled audio and captured-output observation.

Six musical conditions, each 192 beats at 126 BPM, return key 8A, overall energy 4,
six window labels of 4 and cue-energy labels of 4. Their cue counts are 6, 3, 6, 5,
2 and 3. The removable lower group is kick plus bass; the upper group is percussion
plus lead. Harmony and texture stay fixed. These element groups overlap in
spectrum and level; they are not isolated frequency bands.

In the lower-early pair, physical PCM before beat 128 is identical. Without the
later upper-group removal, a cue occurs near authored beat 120. With that removal
and subsequent restoration, cues occur near 116, 124 and 132. Neither upper-early
pair member has a cue near 120. The late baseline and upper-early/lower-late
conditions also have a reference cue cluster near authored beats 32, 36, 40 and 44.

An exact copy and independent resynthesis match all five musical analysis fields;
runtime profiling differs. The independent dither control keeps labels and cue
coordinates but changes confidence by about -0.00037688 and cue times by up to
3.906724 ns, exceeding the 1 ns cross-file identity tolerance.

**Scope:** One linked family: nine files, six conditions and seven actual request
identities. Shared float64 sources, gain and dither policy were checked; matched
musical-construction pairs have unchanged PCM outside their declared edit supports. Six of nine authored-bar-phase checks
fail; actual origins and submitted curves change. Beat positions above are display
approximations from physical timestamps, not replacements for detected coordinates.

**Rules out / prevents:** Do not call this a pure frequency, phase or isolated
server-feature experiment. The upper-early comparison does not demonstrate precursor
suppression. Changed dither/PCM is distinct from randomness on identical inputs.
These observations do not identify server architecture or a universal cue rule.

**Evidence:** E157, E159, E161, E162, E174.

#### F128 — The frozen cue selector fails despite correct standalone energy

**Type:** Frozen-model experiment and exact failure diagnosis.

Using actual captured inputs, the unchanged controls match all nine key and
overall responses and all 54 newer standalone window-energy labels. The older
window branch actually used by the cue pipeline matches only 41/54. Cue count
matches 1/9; complete positions and complete ordered timed/labelled lists match
0/9. Across the six unique conditions, these are 6/6 key, 6/6 overall, 36/36
standalone windows, 29/36 cue-branch windows, 1/6 counts and 0/6 complete cues.

Supplying only the true count to the fixed decoder repairs one position list,
the simultaneous-early condition. In the upper-only condition, MIK selects near
authored beats 0 and 34, while the model selects near 0, 32, 64, 80, 96, 112 and
128. False beat 64 receives point rate 0.842608, rank 2; true beat 34 receives
0.004509, rank 19. No section, gain or element change was designed at 64; ordinary
note/noise events still occur. Baseline true cues near authored beats 36, 40 and
44 are not strict local peaks of the submitted curve. Their detected coordinates
are 25, 29 and 33; clocks and coordinate systems remain separate.

**Scope:** Seven models were frozen before labels. Exact known-case replay,
repeat/reload and model-state checks passed. Point rates are occurrence intensities,
not probabilities. The full-list tolerance is 1 ns, with bit identity reported
separately. This one-key/one-energy family is not a broad music benchmark.

**Rules out / prevents:** Correct count alone, a uniform cue offset or a strict
peak restriction cannot repair these fixed lists. Do not silently substitute the
newer standalone window model for the older cue context. This is no accepted model
improvement, real-corpus gain or method-wide ceiling; no model was fitted here.

**Evidence:** E163, E165, E160, E167, E168, E174.

#### F129 — Eight new files fail exact native key-vector reproduction

**Type:** Request-only extraction and guarded captured-input audit.

Eight of nine files differ between the client request and the pinned local
extractor at exactly one of 352 key coordinates: 36, 124 or 300, each congruent to
36 modulo 88. None is the global maximum. Captured-minus-local differences are
approximately -0.05028486 to -0.05028557 feature units, about 4.72–4.74 parts per
billion of the affected coordinate. The independent dither file is exact. All
other nine analysis fields and non-feature segment fields match in all nine files.
Post-scan PCM, encoded FLAC frames and protected masters also match.

The original exact guard rejected the batch before responses were opened. A
separately reviewed actual-input branch preserved the eight failures, verified no
actual key-vector overlap with the 79-hash sealed guard, and froze predictions
using captured vectors before labels. Only eight key-input arrays change; every
raw score, label and cue-time bit from the unchanged models remains identical.

**Scope:** One nine-file family on the current M4 / 44.1 kHz stereo PCM16 FLAC
profile. The original pre-HAR forecast and later post-capture/pre-label forecast
are separate records. Guard tripwire tests verify that rejected inputs cause no
response callback. Unrelated or sealed response labels were not admitted.

**Rules out / prevents:** The numerical cause remains unresolved. Do not claim
rounding, tuning, a global maximum, caching or randomness as the cause. Tiny
relative error and unchanged predictions do not repair exact native qualification
or justify a hidden correction. Captured-input prediction and PCM-to-request
reproduction remain separate acceptance stages.

**Evidence:** E170, E171, E166, E173, E174.


#### F130 — Removing explicit modulo fields fails the complete-cue regression gate

**Type:** Controlled model-experiment result; no new MIK observation.

Removing seven explicit raw-coordinate/tempo-adjusted modulo fields from the same
194 point inputs reduces complete positions from 568 to 384 of 1,288: 59 lists
are corrected and 243 regress. Complete ordered timed/labelled lists within 1 ns
fall from 200 to 144, with 25 corrected and 81 regressed. Timestamp-bit-exact
complete lists fall from 196 to 142, with 24 corrected and 78 regressed. Count
stays 1,091/1,288. All six unique run 13 complete lists remain wrong; count stays
correct in one of six conditions. Targeted point rankings can improve while the
complete list loses formerly correct cues.

**Scope:** Three fixed 300-tree fits: fresh 194-input baseline, 187-input candidate
and independent repeat. The original 360,656 point samples, 5,098 training queries,
multiplicity targets, weights, split, older window-context branch, count/energy
models, legal coordinates and repeat-capable decoder stay fixed. Fresh baseline
canonical state and all numerical/operational outputs match the historical
control; historical pickle bytes differ without an established cause. Candidate
repeat pickle, canonical state, reloads and full outputs agree exactly. Independent
review replays all 1,288 comparison requests and nine synthetic files. Runtime is
308.01 seconds.

**Rules out / prevents:** Reject this candidate as a replacement. It does not prove
each removed field universally necessary or justify a hard phase, spacing or peak
restriction; absolute/relative coordinates and 30 pooled-phase fields remain.
Model attributions are not ablation effects. This spent bank is not the exhaustive
historical protected union or unseen validation. No server mechanism, new audio,
PCM/native qualification or product change follows.

**Evidence:** E175, E178, E180, E184.

#### F131 — Origin-relative additions and a predeclared branch combination also fail

**Type:** Controlled model-experiment result; no new MIK observation.

Keeping all 194 original point inputs and adding seven origin-relative modulo
fields changes complete positions from 568 to 571 of 1,288, with 42 corrected and
39 regressed. Complete ordered timed/labelled lists within 1 ns stay at 200, with
11 corrected and 11 regressed. Timestamp-bit-exact complete lists stay at 196,
also with 11 corrections and 11 regressions. Count remains 1,091. All six unique
run 13 complete lists still fail, with one of six correct counts in every arm.

A separate input-only branch rule was frozen before fitting: use the whole
194-input output when all seven new columns duplicate their old absolute
counterparts, otherwise use the whole 201-input output. This applies to 1,232
duplicate-field queries and 56 others. Copying the old branch prevents ten
complete-list regressions but discards eleven improvements. Among the 56 other
queries, complete lists fall from one to zero; positions fall from five to three
(two corrected, four regressed). The combined total is 199 complete lists within
1 ns, 195 bit-exact lists and 566 complete position lists. It is also rejected.

**Scope:** Three fixed 300-tree fits: fresh baseline, 201-input candidate and
independent repeat; runtime 289.69 seconds. Samples, queries, targets including
multiplicity, weights, split, window cache, count/energy states, legal coordinates
and decoder stay fixed. New columns re-encode existing detected-coordinate,
forced-anchor and tempo inputs; they add no acoustic information. All 249 changed
position lists are retained; all 1,039 unchanged-position lists keep identical
times and labels. Fresh baseline canonical/numerical/operational results match;
historical pickle differences remain unattributed. Candidate repeat pickle,
canonical state, numerical reloads and complete outputs match. Independent review
replays all 1,288 real requests, nine dependent synthetic files and 1,297 input
gates. Primary and combined outcomes remain separate.

**Rules out / prevents:** Reject these interventions as replacements. Improved
point ratios or recovering one missing cue do not establish a complete-list
repair. Copying outputs preserves them by construction, not by learning a better
rule, and can discard improvements. Saved whole-branch merging does not qualify
a runtime/native router. Do not select branches from labels or infer a vendor
feature, method-wide exclusion or general failure of relative coordinates. This
is spent development, not all-history or unseen qualification; upstream failures
and PCM/native/format gates remain unresolved. No product defaults change.

**Evidence:** E186, E187, E192, E196, E194, E200, E201.


#### F132 — Rare counts fail, and nondecreasing scalar calibration cannot repair one local pair

**Type:** Frozen local-model diagnostic and mathematical exclusion; no new MIK observation.

The retained count predictor matches 1,091 of 1,288 spent requests: 175 are
over-counted and 22 under-counted. It matches none of 44 requests with two to five
total cues. Training contains 4,372 of 5,098 requests with eight total cues.
Relative to the constant-eight training-mode control (1,080 matches), the predictor
corrects 28 non-eight-count cases and regresses 17 eight-count cases.

In one existing synthetic family, its upper-only raw additional-count estimate
5.924926874015134 needs target one, while the lower-only estimate
5.476725344581922 needs target two. The greater local estimate needs the smaller
target. Therefore no nondecreasing unary calibration of this frozen local scalar
can fit both. This is not an exclusion about the MIK server's architecture.

**Scope:** No model fits, loads or forwards. The audit reconstructs all 6,386
admitted real rows and nine dependent run-13 files. All 140,690 raw-22 and 447,650
expanded-70 values match independent reconstruction. No conflicting-target
bucket exists for either representation; the one three-record duplicate bucket
is baseline/copy/rerender. The synthetic duration is inside the training min/max
but below its first percentile. Marginal range checks do not establish joint
support, acoustic causes or representation sufficiency. Only saved synthetic raw
estimates are used; this is not a real-bank raw-residual audit.

**Rules out / prevents:** Do not repair this pair with a nondecreasing map of
that single local output, infer a universal eight-cue cap, treat count estimates as
probabilities or use target-based routing. Nonmonotone and richer input-dependent
models remain open. Class imbalance does not prove that reweighting will work.
Correct count is not complete cue parity. No model gain, new audio, vendor run,
unseen/native/format qualification or product-default change follows.

**Evidence:** E202, E205, E213, E216, E239.

#### F133 — Correct counts coexist with early position misses in the retained control

**Type:** Frozen local-model error diagnostic; no newly recovered vendor rule.

The retained 194-input cue pipeline misses 950 of 8,634 additional target
occurrences and adds 1,233 spurious occurrences. Among 7,684 matched additional
positions, 1,334 have incorrect energy labels. The first forced anchor is counted
separately: 200 of 1,288 anchor labels are wrong; none fails the 1 ns clock gate.

Define early as nonnegative file time satisfying either fewer than 128 elapsed
beats at captured tempo or the first quarter of captured duration. The union has
2,012 target occurrences; 208 are missed across 169 of 1,136 requests with early
targets. Among 1,091 correct-count requests, 140 still have early misses: 172 of
1,779 early targets. Correct total count cannot guarantee the intended events.

First-quarter recall is 1,768/1,957 (90.34%); last-quarter recall is 855/1,013
(84.40%). The 32–64 elapsed-beat bin is 204/250 (81.60%). These unequal-support
event strata do not establish a general early-only weakness or identify any
missed event as a breakdown.

**Scope:** No fits, loads, forwards, new response reads or PCM decoding. Independent
reconstruction covers all 1,288 requests in 1,287 recording groups, both occurrence
streams, 1,288 anchors and 58 table rows, including float bits. FIFO coordinate
occurrences preserve multiplicity and original indices; ordered whole-list flags
remain separate. Target and predicted exposures are binned on their respective
times; zero denominators are null. File/group means retain their support. Nine
configured historically qualified audio editions have no eligible early position
miss, so the initial passage selection explicitly uses three energy-error fill-ins.
Those fill-ins are not missing-breakdown witnesses.

**Rules out / prevents:** Do not substitute correct count for position/label
agreement, call a matched cue with wrong energy a missing position, inflate early
recall with the guaranteed anchor or repair an exact miss with a nearby false
positive. This is spent development and posthoc triage, not a causal early-bias
finding, an OpenKeyscan benchmark or unseen/native/product qualification. Retained
scores remain 1,091 correct counts, 568 complete position lists and 200 complete
timed/labelled lists within 1 ns; no improvement is accepted.

**Evidence:** E217, E219, E223, E226, E240.

#### F134 — Qualified early misses already exist in the candidate universe

**Type:** Current PCM-to-captured-input bridge and frozen local-model diagnostic.

Three posthoc failure editions reproduce all ten retained captured analysis
fields; all four cue scalars per file and all 9,483 independently checked numeric
leaves are bit-equal. All three model outputs have the correct eight total cues
but omit a legal early candidate. Two missed candidates rank eighth and one
thirteenth for seven additional slots, without ties; one case also misses a
candidate ranked ninth. In two cases a selected false point has lower submitted
cue-score amplitude but higher local model intensity than the missed true point.

The complete cache contains 1,057,216 candidate rows. Its ordered numeric digest
matches the independently verified 194-input reference, and an independent decoder
reproduces all 1,288 lists and timestamp bits using their own saved predicted
counts. Replacing count with truth cannot change these three lists because those
counts already agree. Candidate priority must change to repair their full lists.

**Scope:** Three pinned offline extraction calls; no new MIK analysis, decision
fit, load, forward or response admission. Current PCM hashes and frame counts are
recorded; no capture-time PCM hash proves historical original PCM byte identity.
Analysed-duration tails remain present and timelines are not rescaled. The passage
audit independently verifies 24 unclipped before/after masks, 24 full Welch PSDs,
12 level differences and three figures. Two early markers show an RMS rise and
one a fall in the declared two-quarter-beat masks. No audition or independent
section annotation was performed. Same-file correct controls are contextual
comparisons, not one-variable counterfactuals.

**Rules out / prevents:** These cases fail at priority in this frozen predictor;
the decoder follows its declared selection. Do not infer the learner's causal
feature error, a MIK server mechanism, a breakdown label or a universal score-peak
rule. Local intensities are not probabilities or MIK scores. Do not force a cue
position, increase count per track or select models from targets. PCM-input parity
for three spent cases is not unseen, all-format or native product qualification,
and no new model accuracy gain or default replacement follows.

**Evidence:** E229, E232, E235, E230, E233, E236, E241.


#### F135 — Added raw acoustic and window-edge context do not repair this cue control

**Type:** Controlled local-model experiment; no new MIK observation or recovered server rule.

The experiment holds the existing 194-input point representation and learner
fixed while appending two declared blocks: 552 raw acoustic values, six explicit
window-edge offsets, or both. The raw block contains all 183 submitted energy
features plus RMS for the current, previous and following windows. The offsets
preserve the historical client clock and window mapping, with offsets expressed
in captured quarter beats.

| Condition | Complete timed/labelled lists within 1 ns / 1,288 | Complete fixes / regressions | Complete positions | Position-list fixes / regressions |
|---|---:|---:|---:|---:|
| Retained 194-input control | 200 | — | 568 | — |
| Added raw acoustic block | 194 | 6 / 12 | 557 | 24 / 35 |
| Added window-edge timing | 200 | 5 / 5 | 570 | 17 / 15 |
| Both additions | 196 | 9 / 13 | 555 | 33 / 46 |

The timing condition's two-list position gain is a valid partial observation,
while its unchanged complete-list total hides five corrections and five
regressions. Additional target positions recovered/lost are 75/90, 54/45 and
83/107 respectively. Cue-count agreement remains 1,091/1,288. All three qualified
early misses from F134 persist. Their legal candidates still rank below the
seven-slot budget. None of these configurations is an accepted replacement.

All nine dependent run-13 files, representing six conditions, retain zero correct
complete lists. Previously matched additional occurrences fall from 15 to 9, 10
and 10; none is newly recovered. Each candidate improves some anchor labels but
still loses known positions. Partial attribute gains do not repair full lists.

**Scope:** Exactly seven 300-tree, single-thread Poisson fits: a fresh control and
three candidates with independent repeats. The original 194-input prefix,
360,656 samples from 5,098 training groups, multiplicity targets including four,
weights, matched R3 window branch, count model, cue-energy state, legal candidates
and alpha-8 decoder remain fixed. Changes in selected positions legitimately
change downstream spans and labels. Joint presence-and-correctness outcomes are
reported separately from label/clock changes at common matched positions.

Independent checks cover all 6,386 query contexts, training matrix/target/weight
hashes and the 79-key exclusion guard. Historical and fresh-control complete
numeric outputs match before candidate fits. All candidate raw/canonical states,
reloads, numeric outputs and complete lists repeat exactly in the pinned
environment. The completed review replays all seven states on 1,288 spent
requests in 1,287 recording groups and nine dependent synthetic files. It also
checks 9,600 independent tree paths with exact leaf IDs and raw scores. All 32
inspected witness scores are untied. Runtime is about 23 minutes, reported peak
process memory about 6.41 GiB and retained output about 60 MB.

**Rules out / prevents:** Reject these three replacements and retain every
correction and regression. Do not erase the measured position-only gain or call
an unchanged aggregate a regression-free fix. This is one fixed learner and
budget; it does not exclude acoustic information, timing, other objectives,
capacities, sequence models or frameworks. F102 already records two failed
104-input ranking configurations; they must not be relabelled as untried.
The underlying acoustic/learning cause of the demonstrated misses remains open.
No accepted accuracy gain, new MIK run, audio render, response admission,
unseen/PCM/native/format qualification or product-default change follows.

**Evidence:** E242, E244, E245, E250, E254, E255.


#### F136 — Equal query weighting produces repairs and regressions, with a remaining label gap

**Type:** Controlled local-model experiment and descriptive audio evidence.

Multiplying each original sampled point weight in query i by mean(L)/L_i, where
L_i is its legal candidate count, yields 198 complete timed and labelled lists
within 1 ns versus the retained 200/1,288. It fixes 14 and regresses 16. Complete
positions become 560 versus 568, with 62 fixes and 70 regressions. Counts stay
1,091. Additional target positions recovered/lost are 149/152. The candidate is
rejected as a replacement.

**Scope:** Three fixed 300-tree single-thread Poisson fits: fresh control,
treatment and independent repeat. All 194 inputs, 360,656 sampled rows from
5,098 groups, multiplicity targets, original inverse-inclusion ratios, frozen
downstream heads and repeat-capable decoder stay fixed. Total weight mass stays
4,177,472; target-weight mass and initialization may legitimately change.
Data, complete states, reloads, repeats, full numeric and operational outputs,
paired occurrences and subgroups pass independent review.

One qualified case now has all eight positions and clock bits right but two
cue-label errors remain. The other two selected early misses persist. Dependent
run-13 complete lists stay zero; two matched additional targets are lost and
none recovered. A separate read-only audio audit verifies 16 masks, 16 full
spectra, 240 band integrals, eight RMS deltas and two viewed figures. The direction
of one measured RMS change reverses between two- and eight-beat support.

**Prevents:** Do not call a position repair a complete fix, double-correct the
original sampling weights, infer calibrated intensities or assert that equal
query weighting cannot work with other models. Same-file audio controls are
descriptive; neither a universal RMS rule nor causal propagation from window
labels follows. Current PCM qualification does not establish capture-time PCM
identity. No new MIK run, accepted model, unseen/native/format or default
qualification follows.

**Evidence:** E256, E257, E258, E259, E260, E261, E262, E263, E264, E265, E266, E267, E268, E269, E270, E271, E272, E273, E274.

#### F137 — Capacity search improves some cue results while losing known matches

**Type:** Controlled local-model search and complete operational comparison.

A fixed 24-trial grouped three-fold search compares LightGBM and CatBoost Poisson
learners on direct 104- and 662-input representations. Six trials belong to each
cell. Its true-count position reference is 2,455/5,098. The four predeclared
diagnostic choices achieve 2,679, 2,590, 2,835 and 2,709 exact position lists.
None has a positive gain that protects all known complete lists and matched
target occurrences.

Eight subsequent full-training fits, comprising four chosen candidates and
independent repeats, use each model's own positions with unchanged operational
count and cue-label heads:

| Candidate | Complete timed/labelled lists within 1 ns / 1,288 | Complete fixes / regressions | Complete position lists |
|---|---:|---:|---:|
| Retained 194-input control | 200 | — | 568 |
| LightGBM, direct 104 | 210 | 26 / 16 | 615 |
| LightGBM, direct 662 | 207 | 30 / 23 | 581 |
| CatBoost, direct 104 | 214 | 37 / 23 | 610 |
| CatBoost, direct 662 | 208 | 33 / 25 | 592 |

Timestamp-bit-exact complete totals are 208/204/211/206 versus 196. Counts remain
1,091. All real-corpus regression gates fail against both historical controls;
all run-13 gates against the retained 194-input control fail. LightGBM/direct104
passes the run-13 regression gate against historical104, an exception that
does not qualify it. All nine dependent files, representing six primary
constructions, still have zero complete correct lists. None is accepted.

**Scope:** Exactly 72 CV fits and eight final 600-iteration fits, CPU eight
threads, fixed seeds and versions. The same 5,098 groups, sampled coordinates,
targets and weights remain. Direct662 adds three windows of all 183 acoustic
features plus RMS and six window-edge offsets. Candidate point models exclude
the 90 learned R3 point fields during inner CV and final refits. Historical194
retains those fields; the unchanged downstream label head uses the matched R3
window branch. Choices are frozen before any candidate development forecast.
Historical104 is separately preserved at 199 complete lists and 571 positions.

Independent checks replay all 72 CV states and 100,259,328 candidate rows, all
24 sampler checkpoints and all ten final point models over 10,595,200 rows.
They verify full downstream spans, clocks, classifier class maps, numerical
archives, omitted-input hashes, independent repeats, paired occurrences,
anchors, rare counts and location support. Independent tree checks cover
81,600 CV paths and 43,200 final paths with exact leaves and zero observed raw
error. All requested tree budgets are reached. Native CatBoost repeat JSON
differences are precisely GUID and finish-time metadata; opaque serialized
container differences remain separately scoped.

Larger or different models recover complete positions in two of the three
qualified real witnesses where the fixed F135 budget did not. Label errors
remain, and the third early miss persists with other exchanged markers.

**Prevents:** Preserve these partial gains and every regression. The 214 total
is spent-development evidence, not an accepted or significant improvement.
True-count CV positions and operational complete cues have different targets
and cohorts. Independently tuned cells do not establish that a feature block,
framework, objective or model class is useless. Fixed counts and correct-position
label failures cannot be repaired by position tuning alone. No new MIK mechanism,
response admission, product change or unseen/PCM/native/format qualification
follows.

**Evidence:** E275, E276, E277, E278, E279, E280, E281, E282, E283, E284, E285, E286, E287, E288, E289, E290, E291, E292, E293.

#### F138 — Correct local cue counts do not require every window label to be right

**Type:** Saved local-prediction contingency and descriptive qualified-audio evidence.

On the same 1,288 spent requests in 1,287 groups, the retained count head paired
with the standalone R5c window predictor gives the following outcomes:

| Cue count | Every window label correct | At least one window label wrong |
|---|---:|---:|
| Correct | 165 | 926 |
| Wrong | 35 | 162 |

The frozen R3 branch used by the cue pipeline instead gives 148/943/29/168 in
the corresponding cells. R5c has 26,944/29,892 correct window labels and 200 files
with every window right; R3 has 26,675 and 177. These are distinct saved models.
Their figures and case sets must not be substituted. Of the retained 200 complete
cue matches, 146 still contain a standalone window error; 136 do so under R3.
These compare the same retained cue pipeline with two window predictors; they
are not results from swapping the cue pipeline's window model.

Retained requests partition into 197 wrong counts, 523 correct counts with wrong
positions, 368 correct positions with wrong cue labels and 200 complete lists
within 1 ns. Independent review reconstructs all 2,576 branch ledger rows and
the four-way hierarchy directly from saved targets and outputs.

A separate qualified case audit examines a remaining label error after positions
are repaired. Its cue label differs from its containing-window label, and both
local predictions differ from their respective MIK targets. Eight masks, eight
full spectra, 120 band integrals, four RMS deltas and one viewed figure reproduce
independently. RMS-change signs depend on two- versus eight-beat support.
No audition is claimed. Preserved scope addenda correct inherited historical
case descriptions; actual selection, source audio and numeric arrays are
unchanged.

**Prevents:** Perfect window labels are not a prerequisite for correct counts
from these local predictors. This does not establish MIK server independence,
exclude continuous energy or shared upstream information, or show that improving
windows cannot help cues. Cue labels and window labels remain different targets;
no general lookup or aggregation mechanism follows from the containing-window
comparison. Do not turn same-file spectral/RMS observations into causal physical
contrasts, semantic section labels or a universal energy formula. No new fitted
model, MIK run, accepted gain or final qualification follows.

**Evidence:** E294, E295, E296, E297, E298, E299, E300, E301, E302, E303, E304, E305, E306, E307, E308, E309.

#### F139 — Oracle window-evidence edits isolate two cue-label input sensitivities

**Type:** Frozen local-model input intervention and descriptive current-PCM audit; no new MIK observation.

Thirteen distinct input views cover three remaining correct-position label errors
in two spent contexts. Supplying reference hard window labels repairs two selected
labels in one fixed eight-cue list without losing another label. The other late
label remains wrong in all five tested views, including replacement of every hard
window label. Margin-only swaps repair none of the three selected labels, although logits
change. An unchanged argmax does not make those margins unused.

Positions, counts, timestamp bits, head state and the first 946 raw/acoustic/span
columns of the 1,336-input head stay fixed. Only 390 derived window-evidence fields
may change. Exactly 52 producer and 52 independent-review head API calls are
recorded, with zero fits or point/count forwards. Reference labels deliberately
enter these oracle inputs; the resulting combinations can be off-manifold.

A separate read-only audit of the already-qualified current PCM verifies five
time masks, five fixed-Hann 16,384-point Welch spectra and 75 piecewise-linear
band integrals. The last-cue span's mean power equals its three-piece partition
with zero observed residual. Complete-mask RMS and spectral estimates have
different supports: Welch omits incomplete trailing frames. The source is
preserved, not treated as a newly exported controlled fixture. The figure was
viewed; audio was not auditioned.

**Rules out / prevents:** These edits establish sensitivity of the tested software
path, not a repaired upstream predictor, an operational gain, an upper bound or a
server mechanism. Residual error with oracle hard labels does not isolate the
head: continuous upstream margins are not replaced by known-correct scores.
The descriptive PCM differences establish no single acoustic cause, semantic
section label, universal energy rule or capture-time PCM identity. Missing final
nominal window records do not imply absent acoustic support (F032). Preserve
F054–F056 and all known regressions. No model or default is promoted.

**Evidence:** E310, E311, E312, E313, E314, E315, E316, E317, E318, E319, E320, E321, E322, E323, E324, E325, E326, E327, E328, E329, E350.

#### F140 — Added training coverage causes the selected calibrated energy regression

**Type:** Controlled local-model experiment; no recovered server rule.

Seven serial binary fits reproduce the retained raw-183 class-pair parent states,
cross base/full training coverage with base/full class-loss mass, test a kernel
cache change and independently repeat the two weighted cells. The selected
previously correct window flips to the wrong class when coverage expands at
either tested mass setting. Changing loss mass on the old examples does not flip
it. Increasing the old model's tested cache from 256 to 8,192 MiB reproduces its
active state and margins exactly.

Pair training windows grow from 5,705 to 21,160, across 3,000 to 4,723 groups.
Of the 1,723 groups newly represented in this pair, 1,502 already occur in the old
multiclass sample and 221 do not. These are not 1,723 newly independent songs.
Coverage changes both represented contexts and groups. The mass intervention
jointly changes effective regularization and relative class weighting; its effect
is not globally zero. It changes 26 pair signs at full coverage without changing
the two tested complete hybrid label vectors.

All 29,892 development margins and the 45-pair decision arithmetic are independently
corroborated. Expanded multiclass coverage repairs 1,550 windows but regresses 783;
it remains unaccepted. The review reconstructs 209,244 margins and 239,136 hybrid
outputs without another fit.

**Rules out / prevents:** Coverage is a demonstrated cause within this calibrated
experiment and selected regression. It does not identify a bad training row,
the cause of all regressions, the decisive acoustic feature or a server mechanism.
The cache exclusion applies only to the tested base-cache path. Subtracting a
fixed support-vector contribution is arithmetic attribution, not the outcome of
deleting that example and retraining. No aggregate gain is accepted as a repair.

**Evidence:** E330, E331, E332, E333, E350.

#### F141 — A single class-pair bias cannot repair the preserved two-case tradeoff

**Type:** Mathematical exclusion for saved local scores and descriptive qualified-PCM comparison.

Two passages from one qualified recording need different reference labels. The
old class pair assigns the same class to both; the expanded pair assigns the
other class to both. Holding the remaining 44 pair decisions fixed, repairing the
regression requires a constant bias above a lower bound that exceeds the upper
bound needed to preserve the repair. The constraints are incompatible. Positive
affine rescaling preserves that wrong ordering. Restoring the old pair repairs
four windows while losing four others, including the preserved same-recording
repair. It remains an unaccepted diagnostic.

Independent checks verify current full-PCM identity and three fixed passage masks.
The contrasted raw-183 vectors differ, and measured spectral/time-envelope
structure varies. This is not an identical-input collision or a one-factor acoustic
counterfactual. No audio audition was performed.

**Rules out / prevents:** A constant shift or positive affine transform of this
saved pair score cannot repair both cases in the frozen contexts. This excludes
neither richer inputs, different learners nor nonmonotone transforms. The measured
PCM differences do not establish a causal RMS/spectral feature or general rule.
No MIK implementation claim or model promotion follows.

**Evidence:** E334, E335, E336, E333, E350.

#### F142 — The tested ending sweep preserves cue choices while continuous inputs and clocks vary

**Type:** New controlled MIK observations with separately qualified captured inputs.

One linked family contains nine primary 200-beat, 126 BPM conditions and three
reproducibility controls. The eight edits restore upper components at beats 176,
184, 192 or 196; remove rhythmic components after beat 192; or reduce the last
eight beats by 12 dB, 24 dB or a complete mute. All eight edited decoded waveforms
match the baseline exactly before their declared onset. There are ten distinct
submitted numeric requests among the twelve files, not twelve independent songs.

All nine primary constructions receive key 8A, overall rating 4, six window labels
of 4 and three cues at detected-grid coordinates 0, 32 and 128, each labelled 4.
Cue choices stay fixed while raw cue times change. Seven contrasts exceed 1 ns;
the largest time change is about 51.64 microseconds. The beat-196 restoration
changes a cue time by about 9.116 picoseconds, within 1 ns but not bit-exact.
Every cue clock reconstructs bit-exactly from its own actual submitted clock.

The beat-196 restoration leaves all six submitted window RMS values unchanged
but changes 112 features in the first nominal window, with maximum absolute
change about 0.006485. Its three changed cue scores and the other arms' changes
prevent interpreting unchanged early PCM as unchanged analysis input. This does
not uniquely identify normalization, phase search or server-side look-ahead.

Copy and repeat-render controls preserve all musical response fields, including
confidences and cue time bits; profiles and complete response-body hashes differ.
The independently seeded dither control preserves CuePoints bit for bit while
changing four other musical numeric fields: one energy-segment endpoint, its
Volume and two key-confidence fields. Changed dither changes PCM and request;
this is not identical-input randomness.

All twelve scan containers change during the MIK scan despite disabled tag/export
UI controls. The particular writer is not established. Protected masters,
STREAMINFO, compressed audio payloads and decoded PCM remain exact. The original
container guard remains failed. All twelve local key vectors also fail exact
equality at coordinate 36 alone; the remaining 351 coordinates and nine other
analysis fields match. A separate reviewed captured-input branch admits only the
twelve selected responses and excludes all 79 sealed hashes. Original failures
are preserved, not waived. No audition occurred.

**Rules out / prevents:** These observations apply to this ending sweep. They do
not establish that endings never affect cues, isolated server look-ahead, native
input parity, architecture, unseen performance or broad key/energy coverage.
Earlier run-13 counterexamples remain valid (F127–F129). This family is not added
to the earlier 277-request benchmark. No replacement model is promoted.

**Evidence:** E337, E338, E339, E340, E341, E342, E343, E344, E350.

#### F143 — The ending-family cue failure is localized to the supplied count

**Type:** Frozen local-pipeline diagnosis against the new MIK observations.

Seven unchanged component models match all twelve keys, all twelve overall
ratings and all 72 window labels in both the standalone and cue-context branches.
They match zero counts or complete ordered/timed/labelled lists. Every prediction
has coordinates 0, 32, 64, 80, 96, 112 and 128, whereas MIK selects 0, 32 and 128.
All three reference cues already occur as a correctly labelled, timestamp-bit-exact
subsequence. Four extra cues cause each complete-list failure. The nine primary
conditions alone give nine key/overall matches, 54 window matches per branch and
zero complete cue matches.

The baseline count regressor outputs 5.924926874 additional cues, rounded to six
plus the forced first anchor. The true added coordinates, 128 and 32, already have
the highest point intensities, approximately 1.40933 and 1.03049. These are
occurrence intensities, not probabilities. Supplying only the true additional
count to the unchanged alpha-8 selector recovers all twelve position lists.
The label head is not rerun on the shorter selected spans: this is a positions-only
oracle diagnostic, not a demonstrated operational full-list repair.

The actual count input consists of 22 cue-curve/timing summaries, not predicted
window labels. Correcting already-correct window labels cannot change this count
decision. Saved local and captured-input forecasts differ only at key-input
coordinate 36; model scores, labels and clock bits match exactly. Across the two
branches, 74 producer full-forward rounds and 26 independent framework-replay
rounds use zero fits. Known real-case controls, repeats and fresh reloads pass.

**Rules out / prevents:** Investigate why this count estimate overpredicts before
another broad fitting search. Sparse support, objective bias and representation
loss remain hypotheses, not demonstrated causes. F128 already contradicts a
universal point-intensity cutoff with a true low-intensity cue and a false
higher-intensity cue. Count does not explain every cue error (F138). Exact native
key extraction remains failed. No broad accuracy gain, accepted model, unseen
qualification or product-default change follows.

**Evidence:** E338, E339, E345, E346, E347, E348, E349, E340, E350.


#### F144 — The frozen count output collapses cases requiring different answers

**Type:** Frozen-model arithmetic diagnosis and scoped mathematical exclusion.

All twelve ending-family inputs share all 300 leaf indices and values with the
run-13 upper-only input. Their additional-count output is exactly
5.924926874015134. The run-13 upper-only input and the ending-family baseline
require one and two additional cues, respectively; those two 22-field vectors
differ in 15 coordinates. No deterministic function of that scalar alone,
including a nonmonotone calibration, can fit both.

The audit covers 6,407 already-spent rows: 6,386 real requests and 21 synthetic
files in one linked family. Independent reconstruction verifies 1,922,100 tree
leaf indices/values and all 4,200 stored splits; retained development counts remain
1,091/1,288. No training request shares that complete 300-leaf signature; the
nearest differs in 20 trees under the declared leaf-Hamming distance.

**Rules out / prevents:** Reject a scalar-only repair for this pair. Shared paths
are not identical raw inputs, proof that the 22-field representation is
insufficient, a training-cause explanation or evidence that no separating
threshold exists elsewhere in the model. Support and leaf contributions describe
the frozen software; they are not deletion/retraining effects or a recovered
vendor mechanism. No model improvement, new MIK run or promotion follows.

**Evidence:** E351, E352, E353, E354, E355, E356, E357, E358, E359, E360, E361, E362, E363, E364, E365, E366.


#### F145 — The tested fixed superlevel-region count rules have exact counterexamples

**Type:** Exact logical reanalysis of existing MIK observations.

A two-request real-music witness and a two-condition controlled witness each
contradict eight precisely declared rules. Each rule counts contiguous regions of
the complete restored submitted curve above a threshold, then includes the forced
anchor. The variants use one shared absolute threshold or one shared fraction of
each file's positive maximum; count the anchor independently or count its active
region only once; and leave counts uncapped or apply a separately labelled
hypothetical eight-cue limit. No threshold fits either witness pair.

All 23 examined curves reproduce the retained 22 count inputs byte for byte.
Independent rational masks verify all 3,729 critical threshold cells, including
strict-greater boundaries and equal-height plateaus. Only two real rows were
examined: the real scan stopped once every rule had a finite contradiction. All
21 synthetic files are one linked family, including copies, repeats and dither.
The anchor is inactive at every nonnegative threshold on these witnesses, so the
two anchor conventions coincide here; these are not eight independent mechanisms.
Each request is individually feasible. Their shared-threshold intersections are
empty.

**Rules out / prevents:** Exclude only these fixed, complete-curve,
superlevel-component count rules. Adaptive or contextual thresholds, prominence,
smoothing, different domains or spacing, timing, continuous energy and richer
learners remain open. The hypothetical cap is not a discovered universal limit.
This is not a predictor, a 6,386-row accuracy measurement, proof of input
sufficiency or an identification of server architecture. No new MIK query or
model fit was used.

**Evidence:** E367, E368, E369, E370, E371.


#### F146 — Identical returned key and energy labels do not determine cue count

**Type:** Exact counterexample from existing controlled responses.

The simultaneous-late and simultaneous-early constructions both return key 8A,
overall energy 4 and the complete ordered six-window label sequence
[4, 4, 4, 4, 4, 4]. Their complete cue-list lengths are six and three, including
the leading anchor. A deterministic cue-count function whose only inputs are that
returned discrete tuple cannot fit both.

**Scope:** Two files in one linked synthetic family. All original timestamps,
negative leading anchors and six/three cue occurrences remain preserved. These
are identical returned labels, not identical continuous features, RMS, energy
segment times, cue curves or detected clocks. Mute onset and duration differ, and
the original detected bar-phase confound remains. Current protected PCM and float
sources were independently verified; no new response was collected.

**Rules out / prevents:** Perfect prediction of these other discrete labels alone
cannot identify the correct cue count for this pair. This does not show that
energy information is irrelevant, exclude continuous or contextual energy inputs,
identify separate server models, or prohibit adding predicted energy to a richer
predictor. Returned labels remain oracle observations, not deployable input.

**Evidence:** E372, E373, E374, E375.


#### F147 — Unchanged early PCM can accompany different earlier selected cues

**Type:** Verified physical localisation and scoped prefix-only exclusion.

The controlled pair is PCM-bit-identical outside samples [2,520,000, 2,688,220)
at 44.1 kHz. Exactly 136,626 frame positions differ, all from 2,520,006 through
2,688,218. Harmony and texture float64 stems remain identical throughout; the four
muted stems and the authoritative mix are bit-identical outside the edit support.
Within the unchanged nonnegative audio prefix ending at 57.142857 seconds, the
captured lists contain four versus one cue occurrences. Three earlier cues near
17.142791, 19.047585 and 20.952379 seconds occur in only one construction.

The mute begins at quarter-beat 128 versus 120 and restores at 160 in both, so its
duration also changes. The detected bar origins and anchor indices differ. This
is one full audio-to-output construction contrast, not an isolated server-input
or constant-bar-phase experiment. All ten context masks, 732 quarter-second RMS
bins, original PCM/float hashes and the physically aligned plot were independently
checked. No resynthesis, native analysis or new MIK call was required.

**Rules out / prevents:** The unchanged prefix PCM alone, without other context,
cannot determine both earlier cue-presence results. Global preprocessing,
normalisation, grid/timing or other metadata and decision context may differ;
this does not attribute lookahead or a causal field specifically to the server,
or prove local submitted features equal. A nearest-sample correspondence is only
a descriptive cross-condition aid: the shared positive cue's timestamps differ
by about 2.793 microseconds. It is not 1 ns parity, timestamp rewriting or removal
of the preserved negative anchors and full six/three lists. No audition, model
improvement or product-default change follows.

**Evidence:** E376, E377, E375, E372, E373, E374.


## Current owner direction: case-level correctness before acceptance

The research target is 100% agreement on each final output: key, whole-track energy
and complete ordered cue lists. Unseen-music, native inference, PCM extraction and
supported-format qualification remain separate requirements.

Begin each proposed change by identifying exactly what the component gets wrong.
Record the same-case four-way split: both correct, corrected, regressed known
matches and both wrong. Describe classes, musical/acoustic contexts and timing,
then distinguish measured associations from hypotheses and experimentally tested
causes. A higher aggregate score is insufficient to call a change a verified fix.

Preserve known-correct cases as explicit regression witnesses. A regressing model
may remain useful evidence or contribute to a tested combination; it does not
silently replace the control. Any combination must use deployable audio-derived
inputs, not the reference answer. Prior experiment counts and counterexamples
remain factual; aggregate-only selection is not a verified fix under this policy.
The F118 acceptance recommendation is explicitly withdrawn without erasing data.
Historical controls have their own regressions: F101 corrects 1,113 windows but
breaks 844 prior matches; F094 corrects 161 overall ratings but breaks 84; F096
corrects 37 keys but breaks 33. Preserve those earlier known-correct cases too.
Do not portray restoring a predecessor as recovering an exact rule. A genuine
zero-regression result must also retain its narrow measured scope: F085 improves
eight complete lists without observed regressions on its saved development set.

Select models or algorithms independently for key, window energy, overall energy,
cue placement and cue labels where their failure modes differ. Response-stage
timings do not identify the hidden model families (F071–F072).

## Working method decision — proposed techniques

This menu records proposed techniques, not an execution queue or a fixed final
architecture. The case-level gate above takes precedence. A different method is
permitted when the observed failure and a discriminating experiment justify it.

| Target | First technique | Controlled challenger / diagnostic | Why this order |
|---|---|---|---|
| Key MainKey | Fresh multiclass RBF SVC over all 352 emitted values | One compact full-bank MLP; patent-motivated categorical/profile Bayes or multi-cluster correlation as bounded controls | Nonlinear feature classification is supported; the hidden architecture is unidentified. Preserve absolute register/banks. RBF is a reference choice, not a fact about MIK. |
| Window energy | Fresh multiclass kernel SVM on the 183 emitted, already scaled values | Add separate RMS as an ablation; compare ordinal/SVR-plus-quantisation explicitly; one compact tree/MLP challenger only if useful | Best-supported SVM lineage. Kernel, objective and sufficiency of 183 remain hypotheses. Do not automatically overwrite shipped scaling. |
| Overall energy | Constrained system identification of retained information/selection, then a bounded feature-based whole-track baseline | Ordered/duration/RMS/raw-feature summaries versus continuous window scores; small tree model or sequence model as competing implementations | Histogram-only and fixed monotone rounded-label reducers are already impossible on witnesses. Symbolic/program search is useful only on inputs that can distinguish those witnesses. |
| MIK confidence | Separate target after MainKey/energy classification is usable | Compare calibrated classifier output with a separately fitted confidence regressor | A probability-like number does not prove its construction. Do not rename model probabilities “MIK confidence”. |
| Private cue research | Fresh structured selection from the complete observed score timeline; cue-energy prediction is a separate target | Compare rule/program hypotheses with a bounded learned sequence selector; preserve headers, origin, duplicate/negative outputs and observed spacing | Owner-authorized research. F048–F058, F069 and F074 constrain representation; no hidden architecture is identified. |
| Product cues | Preserve current validated Lexicon DSP | Research results remain separate until product review | Private research authorization does not promote a replacement or attach energy labels to product structural cues. |

The generic decision is a hybrid: keep deterministic PCM-to-request DSP, learn the classifications from actual request/response pairs, and identify whole-track aggregation separately. Do not begin with an end-to-end waveform transformer or one neural model for every output merely because compute is available. Neural alternatives are not excluded; they must earn selection in the same frozen comparison.

For overall energy, the first task is input sufficiency, not searching for a prettier mean. Retain the complete ordered windows, boundaries, RMS and 183-value features. Test any simplified representation against the histogram and monotonicity witnesses before fitting. An arbitrary nonmonotone lookup could fit known label vectors, but supplies no evidence of transfer. Do not force feature-level monotonic constraints just because integer labels are ordinal.

Before fitting: record usable versions/classes/errors; group original tracks with alternate formats, copies, transpositions, dither and stem descendants; fit preprocessing on training only; freeze a limited comparison and split ledger. Captured window labels are oracle diagnostics, not deployable overall-model inputs: use raw request inputs or predicted labels/margins. If predicted window outputs feed an overall learner, generate its training inputs out of fold with entire track groups held together. All currently inspected synthetic and combined observations are development evidence. Independent-review aggregates were used for method choice, so this combined corpus is not an untouched final test. Existing sealed identities must be audited before any claim of fresh evaluation.

Use nested learning-curve sizes 500/1000/2000/4000 track groups only when feasible. Score exact MainKey and overall energy, per-class support and conditional versus end-to-end failures separately. Model selection uses development validation only; reserve truly unexamined track groups for a frozen final evaluation. A new capture date or a clean restart of weights does not make known music unseen.

Native Rust/ONNX numerical and label parity, supported-format checks, resource measurements and product review follow a successful frozen candidate. Do not port/promote the failed partial prototype or introduce Python in the installed engine. Current product Lexicon calculations remain unchanged.

## Open questions — deliberately not facts

1. Exact current key classifier, confidence mapping and training distribution.
2. Energy classifier kernel, multiclass/ordinal/regression objective, weights and whether separate RMS/context is necessary.
3. Which continuous/order/duration/selection inputs determine whole-track energy. No surviving universal formula has been identified.
4. Whether fixed transposition-equivariant server-input treatment would hold. Audio transposition tests do not answer it.
5. How remaining upstream timing refusals are resolved without changing the verified feature profile. Removing a guard is not a verified fix.
6. Rare-class support in a clean, overlap-audited training ledger, especially 6B and energy 7/9/10. No more synthetic batch is a prerequisite.
7. Independent real-music generalisation and a defensible unseen evaluation after extensive historical development exposure.
8. Exact cue-selection/energy mechanisms remain unresolved; private cue-model research is now authorized, with product Lexicon DSP preserved.
9. Runtime optional CoreML/stem paths, model internals and future versions beyond bounded static evidence.
10. A validated deployable default meeting the current 100% research target across the requested domains. None exists in this evidence.


## Mandatory workflow and maintenance

1. Before proposing a model or changing extraction, identify the inputs, targets,
   preprocessing, assumed invariances and output space. Cite the relevant fact IDs
   and register revision; state which existing witnesses could falsify the idea.
   For unrelated changes, explain that the register is not applicable.
2. A representation that cannot distinguish an observed conflicting group cannot
   claim complete reproduction. Reject that sufficiency claim before tuning, or
   label the representation as an intentionally lossy baseline. The 27 histogram
   conflict groups alone do not prove that 99% is unreachable: any numeric ceiling
   needs the actual group sizes and target frequencies. F075 supplies those
   frequencies for the larger cohort and gives a 98.81% bound for histogram-only
   predictors; it does not bound richer inputs.
3. Preserve the counterexamples as development diagnostics. Do not add per-file
   exceptions, remove a failing witness or silently relabel it to rescue a model.
   Keep observation, hypothesis, project policy and modelling decision distinct.
4. New contradictory evidence requires version/build, PCM/request identity,
   denominator, field scope, tolerance and reproduction attempt. Add a finding or
   explicitly supersede an existing one with a reason. Preserve old dated evidence
   and IDs; never silently erase or renumber a disproof. Update the private source
   map and tracked aggregate together, without publishing identifying material.
5. Freeze a grouped split before fitting. Original tracks and their copies,
   formats, transpositions, dither and stem descendants stay together. New capture
   dates and renamed files do not create unseen songs. Record prior development
   exposure. Reserve genuinely new, overlap-checked music before inspecting its
   targets if it is intended as a final test; fit preprocessing only on training.
6. Use development validation for model choice, then freeze model/configuration
   before opening the final evaluation. Report exact agreement, per-class support,
   unknowns and upstream failures; do not equate synthetic construction labels or
   oracle-input diagnostics with model accuracy. Predictive confidence, if claimed,
   needs its own validation.
7. If an overall learner consumes predicted window outputs, create its training
   inputs out of fold with entire track groups held together. Returned window
   labels are oracle diagnostics, not available inference inputs.
8. Native Rust/ONNX numerical parity, supported-format validation, resource
   measurements and product review are required before promoting a default. Keep
   Python out of the installed engine and preserve the Lexicon cue calculations.

9. Preserve immutable fact, source and cache snapshots pinned by completed
   experiments. Advance the current register/index through explicit versions;
   do not rewrite a pinned artifact to make an old experiment appear reproducible.
   Update tracked aggregates and the private evidence map together.
10. For meta-model inner selection, rebuild base outputs and learned transforms
    within matched/nested group exclusions. An existing OOF cache alone is not
    automatically honest for a new validation split (F111).

AGENTS.md makes this workflow mandatory for agents; the PR template makes the
evidence review visible. These instructions do not mechanically prove scientific
compliance. Existing review and merge checks still apply; no approval is implied
by a citation alone.

## Independent review disposition

The supplied review was checked against retained controlled captures and the
combined ledger. Its 3,444 distinct combined requests, 27 histogram-conflict
groups, 1,320 square-root-pooling matches, 136 major labels and repeat-consistency
counts were independently recomputed. Its updated union has 3,609 distinct
requests, including synthetics; older fields in its reports describe a pre-09
snapshot and must not be read as current counts (F064–F073).

The proposed full-bank key classification and SVM energy baseline are reasonable
experiments. RBF, a particular multiclass objective and confidence mapping remain
choices to test. Patent embodiments and symbol names do not establish current
server weights or architecture; binned timing correlations and absolute runtimes
do not identify a kernel (F015–F016, F071–F072). The proposed cue selector may now be
investigated privately; it does not override the current product boundary. The failed partial MLP is evidence about
that fit, not an exclusion of neural methods in general (F060).

## Private evidence provenance index

E001–E100 retain revision 13 snapshots; E101–E350 add the reviewed later experiments and input audits. These hashes
attest snapshot identity, not correctness by themselves. The private companion
provides the file map and original reproduction context. Future revisions must
retain prior IDs or explicitly version replacement evidence.

| Evidence ID | SHA-256 |
|---|---|
| E001 | `173cd66188822ad937243b5864106522a4c3cfa20fd34d179ec5c0a91dabe845` |
| E002 | `e307ea3a448364ae9e7509375c98c08ab1dae867420661e227cfe1a9ec12ecc3` |
| E003 | `691de6f7ca52c9fdd84601f5a851a86a719bc73324fd73e7abc3f2737779a615` |
| E004 | `36798664df1882bc5d4bc7621913e60cbadadab0bdf24c15c03ea1c159f45282` |
| E005 | `d726bed56fb5e726d3dbab0bf6c6d2b2da3f2c700a46b778c3421695fc64c928` |
| E006 | `8d02815c8f28c95b5591b79305f15dcebc37107ad717544b441377b661d7cc3b` |
| E007 | `f8e36fe8f75dae6084ecab2d7c4974626e55eb6eec2e52e22819ad77a92631d7` |
| E008 | `1ff84bee4f3e75ecd1c4463be8ffa72624262cc33b17e57b1f691367fd1f4e98` |
| E009 | `831c9d28970629494017c42b64ebed7432574dbd9b9783c9e25d51c0d0f10e8a` |
| E010 | `d90fe35b02b9eec08f87ba68e86e843dfc15cb469e6950c7756819ea46800b2c` |
| E011 | `53b00baea30aef5c656797aa9da4393269353d252671194f4bc20358f8624954` |
| E012 | `ba415b8cfb949f1d1eaf67e85decc91dee6d4238d54a4b898c3fed91d522fb58` |
| E013 | `18b859dc948a0cc74c97aa1ab65700c0ce76c92aa1408ef20bffe4ed3a11af4a` |
| E014 | `ff73010607f6d4010aa6f2a9fb6f57092759300a67d6becb121a630e1fb0a11b` |
| E015 | `237ab378f1e8de415ff06f15873ee5ab7548fa10fb93accdfaa8c52333ccbf41` |
| E016 | `f937fe31404396aec8679f095a6f82e6a69cd93be150f43b77b20ac6a59faaae` |
| E017 | `27ea457330f76dd27d61eae1e8c911138197d4985b89a048bff7c7040c62d4a8` |
| E018 | `b489aa8405e5ae21e108632dc03b4df37cc66eccf25ba698df0aff1f259313b3` |
| E019 | `3eb187fd3979e9f24a190013978e269c841e6b6bec098d35c80c7f31c8d9c898` |
| E020 | `2a22acf5b3dcc27ac6e76b9593adc1b9a055915fd8f9e25d25a4b2765936358c` |
| E021 | `6c12ed7acc8ac6e67ad2e503b753fa6316b819001ab268c464506c97b5af9d76` |
| E022 | `b69169cc8726071d31c0651ee196ca21a8e098317add232e791b7f76b40c41f3` |
| E023 | `ca1769cf6a9285feae579dec565d04e60de66fb5fed3c2b24536b9e2e37fc461` |
| E024 | `c4866174528aca538d9d37c3bd45c051e570af5fce4b82069cc347e5bf9919b3` |
| E025 | `e1d3c3fe56aa0d0f6f11373f15211a75b8e64788ce5e65636a779c43cee6db78` |
| E026 | `0f46b234c0c3f17141e730a86e0f9b98215bbc0604bdab76ebf532f9c16ee0ef` |
| E027 | `ff1125c84cecb130ac18d259218861f16e0f0431940c12788d296b8e8baa4a82` |
| E028 | `14725a61e3d9ced9f1c2625bc153d04498a3e0e686d60a4e40cd1a25065f9b14` |
| E029 | `c0ea1cd61703ebd682b45d33f4f9abbf7821f3dead7489a6e3abd3dbd4bf6f0d` |
| E030 | `36349fa8883f06e6e8600ecbe25ae299cc819fa398eef59659c5099eb05b544f` |
| E031 | `44cc76d9ee45bb9f07b9cc4b4a179985106826a8a0ce8fa10b3fcc304b84fcb9` |
| E032 | `a1ce3e2c0338497eda121821900043d3394c341557dd32eab0b63d290062ceb8` |
| E033 | `8c7e4f197819b5a6f5799383a62dfe7dc29087004c086ef7b9601d073d0ef582` |
| E034 | `831fcf01b59dd2cff9322f277859794ac5d7191521c7d88b8b227f9441ba091d` |
| E035 | `43e690a26ca93b3b805066599836db81fe33060e0e74f2b0e84011d40914ee50` |
| E036 | `4de83f746505b09f761e276662e153ca1a39cc96fda68a033615097368c1c6b3` |
| E037 | `78afa6f64459cdf582106df7a34de0be1629ec8658024843df9c698bd58fbb11` |
| E038 | `49ff9576ac353ba2a11ee14d8dcf21275356850c63f31b8c1a5037e27b850163` |
| E039 | `be91503809ef913c04182b29db346d6ad55c4b27ff64b9220d4d16c13f43ae46` |
| E040 | `bc40f9688434479230c8e16d1bad01ce3db7f645cb202b6da7aeffd4b57e75de` |
| E041 | `7c8d01427a8c89df65f5927c42e63c3aeb7ff40ed336c759adb1a2fe7414f72d` |
| E042 | `3db66aa6055df285df31f26f3c2c2959a26961fe59fe2b728ddd87ffb1de02d2` |
| E043 | `05a6d0253bdfdff385fad9cd1577b3ce0fa4d2adb06ea8dff4c39376321b287f` |
| E044 | `ab086ba24e16b64154d40ce926286ca8f293b76a70b222139d41fcb788ad68be` |
| E045 | `df0bf2ff832614e99bd7b373a8fdd1e839ef9341f47f41b62b9b1ef84e6f0faf` |
| E046 | `d9c920e0670fc965d2ddf4b38e26419c83d89f6242fd05c1f78e6fe3ab934466` |
| E047 | `909c28c1b1c05902d67520b2b0f2f7dab1420e8bfb613a329c1c238a18e5a98f` |
| E048 | `2223082c277615e334e149e7bee7e6a67ec2dc974a622a302948202eeb3843d6` |
| E049 | `35e7385e968e5ac99aa7c8a0b2026b11bfdabe320505eebd9b480a0ac83c225f` |
| E050 | `45fd759228bf9866f9889a392b79be1b263fc2bbcbc7dabf06ad7a35b5aad50e` |
| E051 | `0b450c6041b7e538d21b0c21c7a4d870fd4ec06ad63b9574554a3308dd2004be` |
| E052 | `9f44e489782ca65fcf9d9c30662ab5d5e112134ae986502b14872de83713bd91` |
| E053 | `ed3773957e429f746a9f85a384549d6b9d16975d6fbda1460432d9b72c93f1c1` |
| E054 | `d01928b6ce7c07460bb4f8842638752600243f27b32fc6df90d3bd2913559e57` |
| E055 | `39d07fa6460de9590a108df94fbf7ca664575fadd8f7c7bf1999f80f0b204c87` |
| E056 | `0a3e7a13259dfda389bcfaba261112e82d1e033ad7fafae106a47cdaff748610` |
| E057 | `1de6cabe53eac3b6dd8aa32739b526b7526b535fc58a3de44f0834fc79416005` |
| E058 | `ccb1443b5ce4336eb3042d332433c44fed7b637b1b4b880ee47ca6f595db08cd` |
| E059 | `79c95325446c3daf456032711620d8b9e62325611a201237addf7cd759e48d41` |
| E060 | `fa56d5a9c022c0433e1e1edbb5cc10fe776224983e67611bf23adecaa8b9f8ba` |
| E061 | `3ef80d4d34f92a97fc6816e5fe93d51b24d6358173d218df44c696d677bf86a0` |
| E062 | `94ac3e655d25c8fb84a51bbc8e2b99cfd36efe97ca3c99a3b4d373205c19d912` |
| E063 | `f732f1f33072e767b9604319d9937648877b4253037c7af4f4b729d66ca0f972` |
| E064 | `a5c84d5a663966780506002186cb44f1b99c5f321ca77491836b3f149182dd20` |
| E065 | `38ca7f263a7aca739cac56a078bec9064f84b0b27ecc0bdbbb3e605540eff7c2` |
| E066 | `c5e9aa4cb057f4c44142e11e916ad6c5e53a67f0461d6b2005a640462d487581` |
| E067 | `0aca3cb9487c2c5703b82cca4c56b35f20b20cad75d576cb10d8cc43d25ef0ba` |
| E068 | `80b2c4ee9a463975d4a351fa297ba03a1ce1e89d6028aba6300479625ee885b3` |
| E069 | `a36fb967596643382bc3109bd642316dde7d0a9adab7181d127f5ac4d2954f02` |
| E070 | `aca343433c433fcb8e7d106c8129920036638eac3aa6b838dce86ac76a731149` |
| E071 | `3af456b8c2fb684d9c21db2da44fe0cd7eec962e21d1a2bd4fb98b9aed358428` |
| E072 | `d16e55fdaf19f9ad4e20bdbe2872b376a84f6db7deca91d4b118b4137f128982` |
| E073 | `2744fe66cd384c49040e909a57cc570ae3455ee9b7e1a0fa23397622ae6a6f03` |
| E074 | `e744940190908aa2319c77f19d9c0560dbd3771d38cb8ab82a81d42813c4130b` |
| E075 | `67e1ff84efa22d7dc53e8769e92f0931c36cde611b237b30ce0aeb0ee0351771` |
| E076 | `c57d6631587dba89478e127b4f71da9c61166253d7a89d85f5863d77b1d8b1bd` |
| E077 | `101113d7021ef75970f1ce22ae7a63efa6ebb05e4cf98a432a71560c3692989b` |
| E078 | `7b5b3b8b9a7ea7216aedb734a33e55cb90271262f148a746e9e86fc0a4b03d3c` |
| E079 | `9517b340c3967399500be031673fd771f308ccb64497988c5a531649d63fc771` |
| E080 | `0bd84b3493c0f5d9308a97727a4f5c6fadfb6e92c3fe81a1b6e9c04dded210bf` |
| E081 | `cd9f172323e609399a920d6c10aa91266b056cd373debc87efa29d0b01386aeb` |
| E082 | `e985214014d8df17422fae38739688153886f579c2e24d4fc6991b30c7285278` |
| E083 | `9ec104afa85a5153669f571d75e561dba561bf99c37353e1bb92e88b82063a00` |
| E084 | `56c53f5a65566caec34710b5dded7669d7dda9d5bad46c4090a697ba800c8cdf` |
| E085 | `7e8046939aaf67ff6bc378b2151d7695d493e862a1e7240323e4019a67ea5a90` |
| E086 | `cd32d979409d6ba0c127aca6ac9593c1b398910be66df42886186198a8f95637` |
| E087 | `aa851b5834243cc4134ae46f23a8978d681910624442f43f53ef7be19ba17040` |
| E088 | `5822cb7283091cd9bde8a1b18ebbcb09acd2aea3f6d8c8bc3d9b007372f63332` |
| E089 | `756938d8df0888f21ceb464a317e37bbcbb75aef7e649e645bfc3f13b34eef4a` |
| E090 | `0dbbf3a3acc8ad9c552eea4c7da5193a643ae0cd19bb580c9e7de07b36f26930` |
| E091 | `f78f97dc8a5218f202c9c945574f4951bfb9322d201532a269a450ea699f07d2` |
| E092 | `5d0751048a6cdc8049958d2fc7c11e2685b81b696f6fdbec94b2100268f2b133` |
| E093 | `b59b5924cc8c92ce978ed45227655669c476c6fc85e0ee7fdabdca8ebbd2f95e` |
| E094 | `816139959a8488098e139d71f30239dc1c5d9f8f4d9ddfc7c98b21a2fa96139a` |
| E095 | `07c240e0576990f3c157ab434c4404d33e10f5be39328078c0f409b28596d1c8` |
| E096 | `104bacae0de7a2a3aaaba8e5dc8178c4688df75de634857c5531bae4f75a7ff6` |
| E097 | `9094d975be7cfcf756f773d185093521c89d02ff45c9091c74ed843c491b9ad8` |
| E098 | `a3516c14fe8a3381c4385d56ee53eb14564ed68a0878f8435b3728a07f789559` |
| E099 | `93f48f765fe04d6dc6d0953cdefa9998cb254bae9dce0e65225025525b208634` |
| E100 | `3fbdf6ffa2ee4ce46d78bbacff6a70910caec21573007c9cdf7d591fbcd1846c` |
| E101 | `6b1db5f2c733f3325f0a20ee4a0d1d445dfb5bf6e7872c9d54cba37e7a26e7e9` |
| E102 | `5af1147851fc9fb3e41704dda6346849b6666ad938fc5f7f592b6cc726c2dac5` |
| E103 | `086ff3b6128835ccf17dbfc8dd0a1b230e284e72a50d093aa8cb91c39939285e` |
| E104 | `ff970424bbfa78b49ebbf4a549393809060b775cef443e823d739cf7f9ce23a7` |
| E105 | `a2c31f52e067bdca72328b64d801f86e6fa2a46dfa6485f294a767d33f274665` |
| E106 | `326333d549994250e9751edd7b431e6f58f29981382235b41b7b24f768b90a9b` |
| E107 | `059acda53094a293d78b763678823beb012c4394c23806e3cc938a2d48081d57` |
| E108 | `571252e4fdfa0a248f717b5481a1d382439a5da7bc8cf663c5ae44c812af2f3c` |
| E109 | `a965e91fec73b3fc218c0d6825d006b2d35a38c0d2b1172db2e1926fa431e77c` |
| E110 | `a59f3dc46f07bee85ed448ab1d8947d94e1148bd3f75165cba6f92111b173494` |
| E111 | `b327355538008a8fdef0915c098a972dbc574d47932d1fef8a75994215982bee` |
| E112 | `4b7252a4d5571aac3dce742986d04c2a27e19c89345ed1bb9c9b175cd373b0a0` |
| E113 | `ab4d0d5d6747aa02938392c2dc8faf941134ee8df1bf873bba72e8f4b5fc2a3c` |
| E114 | `23947b3d330253c99b8bd3a927043accbbbf94f3288a93190ba4c527ead7dd70` |
| E115 | `f443adf6c27a8483d1806a4b61ebdd2c78f96d5376c349fb1d83b829ff20d882` |
| E116 | `105b63555905dc363ad4f4b4eb8844b5a778cbe1fabb727255b65ce5e758adce` |
| E117 | `112e797da75d1e84d46a56031da98c2d5bef425a0dcd64ae405ed18bbc647930` |
| E118 | `2d58548c6e28b866ef2d1442c9d1298f3be02168efd515ffa34451fc88eefbbb` |
| E119 | `c0398382602b139729032cf8ebb484006a5c77f19cd962173156d1c963fbdbf1` |
| E120 | `1bcf01eab127c4bb5bd9bc22a7a0a94569aa1fbd69c7e831eab437141370b29b` |
| E121 | `f7c6040f9ce91c65179000211bcb35c1fadea64596483eb3254c0eccba9d2954` |
| E122 | `d327b7c8d04b49e91c838bc5288160f19de51e75546c3eaf9a152e9f8f433d21` |
| E123 | `aee518ed5c3bb1b2d603760ce8fabe0f25ae7d537f74376bde1d307f4f6ea91a` |
| E124 | `de7e54262a26a2001d6cc551401ef73da181596e1c322646ca9edb23245acbb0` |
| E125 | `1d423db0e11f65548aac2a392a4765fc7c3cdc5445e4e0a28caeb409b30ba93d` |
| E126 | `6c7efc74b19c93b865a79009c295629f47df197ae3531a46e90117f6ec3151e2` |
| E127 | `1bb16ed05019925ac1109e8d15e4482d06f6e99a163a73e9185a7d890ea04272` |
| E128 | `064018b014cdc7b066d0ef2644e667f7c1f4a2cb9bb2caa2a4e5d8323ab0dab1` |
| E129 | `1c6cdaa97709944f51f70ad9cb7123f2059ebe35ea54a4b2be5173adfe429cbb` |
| E130 | `bdd555d09b8a4acdc939c0af0355c689ba7c198150e79708919e1e8a19c7a0d1` |
| E131 | `08ee7056dc4de8f89b383135c0f65409d6ce0cc4c3e002d3529ff213503afb1b` |
| E132 | `3f3f09fbb0003ff7266edae6cd15d5678b75eae837698a5ab9331573498268fc` |
| E133 | `7c72a3ce7e91d621a3f95f9dbbe6c790b7e49cd6792780976f9230ff1cc21588` |
| E134 | `a11074b3a33a53af90bdfd5ad736eea08d726ee136d9a76cb6710ad3d8c69c1c` |
| E135 | `08c918cd96f1a7a08aaca411db6b8245a2f5619bac5d0b515d981abf4087f8f6` |
| E136 | `509ffce9bf8978ca5df9d339af8b3aef1d9c6708e9da9f9c934604a583905739` |
| E137 | `2b66f7ff7d3d746bc9153cb9a1d3973c8ab040f335403168439fca19b0f9dce7` |
| E138 | `7cd0bad6654ba2161a60a1f7199ec3f14f8ae84647aa93cf93ad9f9b9009e547` |
| E139 | `8f58b337dee2f35b34b9ae2114306422d14f75dd1fab78855cee73e7c4e2e081` |
| E140 | `59dc0872e75d2205cd36d07e64d90e4797d308c5b34725295518744bc50da7fc` |
| E141 | `967902fb0651feb9e880c7d1715496526150890f6de3a8951158fa014f1174b1` |
| E142 | `c4d2ac5eff7434c7696fe9ae70ecd2fd3abb490bc8fc95a457263506fbdc2061` |
| E143 | `3f9a8c03d09a0d73186fa1bbb4a30a1c4e8a240199596b84e1f735f2bb1b62f4` |
| E144 | `b3da8d6e551ab0944044fb045a56a7dfb79d8b6d3d2a457c1a829bab4721db84` |
| E145 | `f7f11b6f7512a141e5c428661e4fb3f4092bdb99725e1df2a7aefd2435a4fe7c` |
| E146 | `b30d67749bf79a635326ad3ad0693bbffa3e1486d578e3bd0b0c369984a0e7b3` |
| E147 | `431de77f594f53c61d5bcd2e9abaa27ed30c3c18aed4bd9ea02b9326dd48c998` |
| E148 | `1e9ac740d50caf6ecbb6f1868d1a21cc1661e8d4aec39f8d5128635d6d6c9df6` |
| E149 | `1ede2c121cc62176595b096fc505309481ada4d78d9a18a7a4bfbca2f508a321` |
| E150 | `6634182b081dc8dc41ec6133b79aeef5c5b2dd3007bfb6ed6769f88801ec737e` |
| E151 | `a185001f1cbcbda35854ebcfc4eb726d702bff59875363800e011cacdba71ee2` |
| E152 | `eff6f810534baec688f610a6bd9fc5e79bb53f23c8367512d55361c792d3ae5e` |
| E153 | `ce8a8d4b92cedda3a93551d9b81c4c7a19530358d946aea63824db895d54fb43` |
| E154 | `1054addf33c21e369468b1df643256152edb50b68b5f51098c15c5d7d173a6c2` |
| E155 | `a1e28b3dec9f93a669a5ac519ac02fbd10e24b1fd97393b77580a0ae5c78f926` |
| E156 | `6a4e1a30c11413758447de107185ab3fef1477bfe60d7613c296e4b1938e69d0` |
| E157 | `dca6ddebd0205a851153c7ace48185ed0426c3d47fa7e3fbd399ccbf47be8ffa` |
| E158 | `08bb741a4dc819d2ddf17b7a89c48393a5bb092c9cb5b418d84b3a9523bf9c65` |
| E159 | `f2138fc050c7fa1dc6acc2f213881fedf6aa708ac8f7d7a38b0cab267b579b40` |
| E160 | `fff250217b55f0d8beb163bde81f5af5ee1f34d2a4d655b64487e540c7f08eca` |
| E161 | `14ad6dfd84e7ff590c23790fc990e4f03bf49331f746945e42756dd07ee73001` |
| E162 | `38d9427edc47601cac405551bb20495562d7430be82e48f1133c79dec0bb37d2` |
| E163 | `60f74f2f317dbb159248dd9959b9829c0adcb927860c7752102a7e4a649fe3c5` |
| E164 | `b1512285f3abbac11392626a65de7c4bf937d43c52efccc8a9485f0cf4593af7` |
| E165 | `851566ad3798243694962946db1a4100d09c5c8f25c561264cc6577ebbc130f7` |
| E166 | `58cd29851632aca579a515e2c44c333d49210eae79180975d4ab1f8492be388f` |
| E167 | `58dce4f704144a587380670b0cc4623bf434d8a007b3861c44cce25909562aa0` |
| E168 | `eb8bdcfbc377330f978f10293f2f48130fa6f9919e85809ae932216ce4aa9283` |
| E169 | `71d7d016d6b920b309d618c7380e2a2d8028abd5f83acedc9e1c1397b8c7d712` |
| E170 | `444bc4d85f28506b61ef2c71ed82f9fe01635f82bc5670854ac55eb6b5f31d56` |
| E171 | `6404ed929733a7cd08c8b15ead1eb4155a4474f44e66a98783d4cc505a03eb33` |
| E172 | `239c461cb47aa42e48b3bbffd4631aebfd742265e95f6f63b313474794c42a2b` |
| E173 | `ff9b1b8c2f34622c0ad07c1571a0c0c4501cd73b7ae110c7aaeffa3a24f7eda5` |
| E174 | `794fa707a57550a6eb92fefaa3d04fd736bee15e2fa705b5cc8a13564823c4af` |
| E175 | `a1530595efc069541b20ac51840278ba9c635e2e954c32d362d5a6516b45d5f7` |
| E176 | `7215779ee0e638e38eb85037fe96f1fdccc3ed1131f7e48ebfe71f05de4bae2c` |
| E177 | `6becf97ecc0718c913b50bed2dca686169e55e06507f432dff813218df9d7e54` |
| E178 | `f92f303d2793649d4d0d805994ed7a759471a4ed5c54b24ba0f531069f886f96` |
| E179 | `3ca26528dbb242a8c1a2cef0a2eee4884df0294c50b61cdcf555ee018f05ed48` |
| E180 | `e6c87fae9acde2e81012273799e0a5b563bd51a7d7124de8a8ed222b21e6dc65` |
| E181 | `d99494b592573a7f52e4246ac34136a177d8ab8ac8ce9f81ca0c340996c6f3ec` |
| E182 | `8b87702b8ff48afb9f6d1f8b5a08d2599bcd74476a8dde393ea8f867439de335` |
| E183 | `6b5249052710fb895b01606501eab0b5a67fc843ff50d576b8d0eaf5a55c0a28` |
| E184 | `8faa4c155368a58e256a219597c078f1bbf15412eeccd19263befc9db5b6c19a` |
| E185 | `07a64fdbdadebeac768d004c4adb7d6c55266d414db02c060da1235714d481f0` |
| E186 | `3be60c9c80610d900d474be05ad76a24ea94983ec76a8d2d39eeddfa4e8ab640` |
| E187 | `c9a9ad87a08c2865d5cbe7246b252016c4831215cec9882fc8bbe2c1cfc7357c` |
| E188 | `12f942b7e111326904c0ed4ede25a63eb5b85e10abc5523a289cf609e5ba7845` |
| E189 | `c60229d0375ff706b33c03131b54751c98c4007233fb5150dc31c029a4d85ef5` |
| E190 | `3df04ab72cc2da412e30b1a065b32679ddf0f21cc24ae5c3a6bddc1df6274739` |
| E191 | `96e58b1b31da166da1457e5280928fe678a19a44bad1bb84df7746915f11b4fc` |
| E192 | `1ffb700ed0da42d34a615b64d0c0563a87f8033edda3793a4b83d85e267b6b7b` |
| E193 | `299b37d1d00faa9d69be2cfed2b65cc1e703cbacc29f247e1c9cdbbea18e918f` |
| E194 | `ba683e48cc7358c75e59d60e6657d3dce4e5d54a37ebc4d53a91fe6abda922a7` |
| E195 | `8b0d0ca3e3f05910568c98331be53726a2324f327ebcdc54cc727d6d681bed7c` |
| E196 | `66358ffcde695a82bd7765eaa8a44d59179ad64e6aa1b4e378a80e1a71bdb865` |
| E197 | `69b8e7088a0c6ab565c41837b6e3d9de313472bb62f438a06b3f4900d491b564` |
| E198 | `e1b78b46ea543e154cf3a00281b77dc3d62dbcc318ace8dfad0c290e97f8786e` |
| E199 | `2874b474d50eafc6ef371868ee0f13d58157edc585dfa5755a7abed1bf6947bb` |
| E200 | `ceecafb31d0823d0a61e1bb5b946ed66476e0913eea266b7187550676606e1d2` |
| E201 | `6d83baf3f7bd91a7ee364ba35bf6e84ea33bd3102e3530ea04dc211005c16f68` |
| E202 | `745dc72c716d217275e4d89472fe24e3cb3189cc9198f8facba131cfd717ea00` |
| E203 | `548149335a287b73897f81ce657b0f0d4f18861ce99967a9cab70ec2fcfed88a` |
| E204 | `0eba9bb5a71832a033d154f1d5b371ac2982b8fab3c4996bfea7b5fdadf9fe4a` |
| E205 | `55a6f527cb5c1e5a8dede345ce6706829c54c08054924f69fcf6b94a50fdf945` |
| E206 | `c79e301e3a38ca617b959ca788769835954b8debb6f458d79a5adcc57eb654b3` |
| E207 | `3ce728169a46a23c50f1930bd8405f1c720a87d36c269289711568ddbf7815e4` |
| E208 | `36a8ae0befe6ddadc2a775412055f6088cfe282fd44ecd63069c3d70f20e8a07` |
| E209 | `f8ebf06718463da53874cf04a15fe445657ce7019c0fbb3c09063dbcc16281ac` |
| E210 | `92a2f53712a61cdd93dba8c07c7e3f1d75d1a7484c03b776309f99751cc56ee8` |
| E211 | `9bac24ea133e2ffcf45e524f0347c65657c18ddca8b82f8c85b05412d291b44b` |
| E212 | `765c8e35b3950fc3b0648f42f6f3b03473e1a3b54d3d8d9f6e75c798881d8c5d` |
| E213 | `80f54d18e7880f9d3a10a3f6755461e9e4f839af9a88f4119f18c557f519698b` |
| E214 | `eb134027fd5b9422a3b2d2620c36228674507a524585a2f993aec0d81333adf1` |
| E215 | `6b66bfb6c043eac54e9f14efe4dfff0740ebdad71c33e259fdfc894d805f09f9` |
| E216 | `3b6db777228f1ba5fd0ddf7b8aa6bb326385a39b7ccfafb34333eb76dd19ad6b` |
| E217 | `4226b6fd02539bfb7ceb6c4ede87587f26383dce72b04e482ec17b9185a9eb39` |
| E218 | `276a0938ecbe58d0898f5502237ae90a1a76a929a1a4e59b63fb54739e1efab6` |
| E219 | `420f1d2dc9f1097ae794de6b0ad1b30a1f8e3d2fcad468109cb1e92cd91b8b6d` |
| E220 | `839c43adbddd7887701c8f4fd861a0f678731b8f787b96ae6726ea3225b10e73` |
| E221 | `3d700fc6e4b6df0c62802ca1ca60c04f2bdd1aa1db5b2d8a4d72ace1fdfd0231` |
| E222 | `6fd3d328b4fb79eacf6f5b2287efe7d3f731f63612f2d0a43a263822e7c99253` |
| E223 | `fe3f244cd9e53e0c9f199ec47ad8c67303f7ac33037bfbbf65200cdb92876013` |
| E224 | `7f0217af21d746d00ec584c7ee40c77a4d44e992e2a6f05cded6bd86fad08dac` |
| E225 | `8f05941354ec74ae73b3832f04108407dd4564f19a337a5d85445b33408a321f` |
| E226 | `1363d471e150622bada1d5c910de66bdbfe683d9f4c78c6518ff8b352b12a132` |
| E227 | `4e4a36cde06427a50a5db75a043421916b59b85c3b926d9cdc85eb1f18b4e77e` |
| E228 | `c9a7a14503ab2c74f3012f9da3992a29dbc4d689fccf54650801130c22ca5a30` |
| E229 | `d4aa433757ec0680bdb62f885fef872c392fb36aea5d3f4984b22cdaeae1ee29` |
| E230 | `6ece844187fa6a953f3b959d8283ae521fb6a7605138dba2f4a2ef0160f3769b` |
| E231 | `b329d098a7d0b8dac2add5a74a2044ec1c2afce32e93f5275d76c736269e1627` |
| E232 | `2ce6c2216e1248f40c39707d2e768859b8a2cc27b28d93c6522e88b6f761cfa7` |
| E233 | `edc206b582748ed414df280c1d73f977515167bfbc45f6dbf9c013d9a1f7b494` |
| E234 | `dc2692a303088b729086c0ca335887a51b89a5334d2f5e9afafbdce877028827` |
| E235 | `f78bd8ffe4d4312faeb21b15783dc0b96993a4d66be05cdf4f53a87d5af9685e` |
| E236 | `baf3588ce359609d65a697da51ce075c219810bf8e2ecf07a345eaeae622d787` |
| E237 | `565630cd8a79e396038495f96acc2323b5789ef84949905d60e982de5df6bf67` |
| E238 | `58b41ccaa8c7a3dcbafd50b32adb98c8006ad2bb28dce8295acc74e729d5af93` |
| E239 | `1cf34b8d8970dccc64e2f9b74066e4f954aeb2932b418deaaff3419a369ce89f` |
| E240 | `bc1b5a8ef6bbb6912972c49bf89541eea7450589cb296e3e15399d80f5dd918b` |
| E241 | `00ea93c04c832b9922c4c45443e965b0c11a0fd4496d3075b06086e65d8d96cb` |
| E242 | `87cb846c42393f2002074e2c5375958601772d1adf7ff02197acbcff2b4f3c8a` |
| E243 | `f642e1917d96de2a6af28ec583463ab437481e78fe735b6c020156efff13c50b` |
| E244 | `9e05a9f6bf2db48ecbca4d1fa1835bd7536421310d92a1b5f92bee7cfdab3620` |
| E245 | `a28a3ff9d858f2ca158d2a8a6a49c4cdacdd97ad9e9c9b7f45616f84a9375d70` |
| E246 | `df9c9064d5f78c2ca805dd1f9bd09e6922ace797fd820c723ab859a8baab9810` |
| E247 | `48aab92d5c84888601e1483b86d1a782b7d41e390dadd1b4cb56dd46fd728856` |
| E248 | `43ad71ebb999451b2eb7f46cc23fc00f37cf414f7dc912edc05c68cc53c13a3e` |
| E249 | `49ff67bdda21be4ccb17c8d4f7ca5cf9bbda6909f2b6e33b147ef73e9a4b7591` |
| E250 | `356bcdeac7a4167d3f507da7968407b82e65708e42b84f48bb134a6e427f738c` |
| E251 | `20321c8dae644d9796345f42e997fe8feaebe628ba2823ee559d478ec1000974` |
| E252 | `65f77233c9a31ec92d34482bfb67cb08b93dfe05b3f5e52580631ec2c08f6e9f` |
| E253 | `03861aebfc2afc9c13324195026c12f51ead26560bcbc3197211f32d15879a97` |
| E254 | `b1a2c6b8d0c58462fac9da3c489b6b50c1be098a860cb298e5923baf8a2ac78c` |
| E255 | `3b3b19108cd4cf5d58bd7697505ab1007599534215925b7247a9df5dceff8bcc` |
| E256 | `3740cbafca4eaa9ed6bae259bb937aab796e9c852eb65cc1317130c3332d23bb` |
| E257 | `7125e9e6329c115fd96e3905dd0f16d86265f5e05ae36ec8428b3a01b16a54c2` |
| E258 | `d37eaeeee77926ce9ebe808b498e4f32b8fdac0c1b078a518791cffde9449717` |
| E259 | `f45e8660d82b8bf74aaad7293e91be05a4f2114109bdc2df8e62a4744889719b` |
| E260 | `2522bca07f9e27bad96fd9abde22de81a8ef472f4efb20f230eb622e5fb4eb1f` |
| E261 | `28a618c9293bad82e64cdf8bca37ff5bb394142264bf3a68affefe33883a670c` |
| E262 | `600c336d73bd7903ce73a00ce821b198a807962bf507029fd8c9b6545dd0473e` |
| E263 | `d21f02fe0c7d1edaac60c6c0457e127e24dbaeed21aac9618990fdf2fd4bc4d5` |
| E264 | `44672d5b79799076cede5fff64575d3de05a1803ce01e00993d8c9e0382d5a97` |
| E265 | `4d9a694cd34f2c68b6faee824e8d518f0ad6de034b5edd34567ddd6cd56c2f18` |
| E266 | `807a81faa629f492205ce8e69dc48dabfbfc31456689577d1a12d4fe93210875` |
| E267 | `66f5622d6ed1c35ea90b96e089f3e38624caaa092d2adcc93811ce0da9b9183c` |
| E268 | `ae1019d801b32326dfdc5234956fd08928357b777111d57e9e7d65a6a6fa8c8f` |
| E269 | `8a765bb8519eb56697e5f97d42b7757050c694b43652ba5807b862c1c1815bc2` |
| E270 | `7b1bde2ce53fd1c3989bb321452da926640c83222717c498c88f950a96527e71` |
| E271 | `2b70ff1bb23ce82d2e940be9b807767d897f7fcfb0f42bbe082ca854b13f7420` |
| E272 | `a11bd6d30d10795b4c2c02ff5327a242f62906d2d4dac37737776333e5b93ce0` |
| E273 | `a821434894eff06b37e16d3dcdcb2d57ddbf85c5dbb64a928738d8991e03e6e2` |
| E274 | `08b20970c4e4662a70a4d2ad117c53d3bfd8fb36a967f62e32838b9af00c7115` |
| E275 | `1b7e63ac79f2f8574f4e78303888214576c083bf652039a7ba40a95efc5d0b53` |
| E276 | `fb5188edf054faed28e9337208e2fdb6506b7f37ca225b953e3ebb0c08b21d94` |
| E277 | `d03104fff59347aa1181f6228f4aa1b5c6d8d1ec2db5ab035c014284ebce940a` |
| E278 | `7001520a5b4b0bf5be6287ed828db28979cd123ad044da89b7c9aa369408001c` |
| E279 | `4f9e49b9503884b6c7fe020e0bb07d59af7b535bfb66f260a7e1b2a1ae420ed3` |
| E280 | `886d0252bcff4ef637a9806a5802f35976892e684877013f19efa34b824da4a3` |
| E281 | `a0f150b1c6238f2f142d7b92ebf5795ab3a9440d4e0c366c5dd988d67a99ba51` |
| E282 | `b1ee1ae146a346c533bb04bd1de8de1a7852eb80a9fe08c413a27039e818fbd5` |
| E283 | `14fdd806644d19b3e937854a03bd19591982f52c02597843263fa8b86ade981a` |
| E284 | `4cb2f149027b2628417443feab6f1a3ef167ef35be6bb14b4c55d1264e41706a` |
| E285 | `bcee6fcd95aedfef93963d8af49e6d71cde6df8554c538891e2bafdcc72d4c93` |
| E286 | `55c6ba09e4721ab8a6ba62805bed117099c6c1c4ab06c8655aedde7cb9472bad` |
| E287 | `188235c6d956e2d61a29709571da5d271730fb0d8334960ab04b02511759f59a` |
| E288 | `52dd76dfb89c966789e4c2c342be8407b84469360373002bb8723faf0e243ddd` |
| E289 | `c92880ce6e4b765171eaf62303b86db33a40d298f8566417a9a39964b7f03ae0` |
| E290 | `acd4a5adeb07f71d9b40fab72e4f72711cfcb18c7397a134dc73c06c330540e2` |
| E291 | `389f8e9794dc75e633c150fde3121ed0f1276cff55275da9567d8cfc5b2f0ccf` |
| E292 | `11bb1db53333c6cef1c8f667c5b3a4ef88cc46695ac3b971cdb86b913410c3d9` |
| E293 | `f0e63fb15dbafac33700bdaf119061cb5246f3ea292f43a5d1287f3f69e8ebbf` |
| E294 | `e62e5a9784ddbe38788d36865b76e938add7479a898233e98f0f80be7bc05c62` |
| E295 | `d08845bccab764aa526a242c7b74cd1c2669183f5e886e87720ad5e9c4dcf503` |
| E296 | `a0fe7a01f9658796c1b8484b6f8e9f1f8cb2bab5a87d547b6f21a5875e71a7a8` |
| E297 | `acb3cdbf744b147dc00a7ee1a2e63ac78710f8ba4c236c28892794955d4415be` |
| E298 | `e532eab88579e6bbff05ebb0d09c6432db780c8c3003a341e3fc45183ac956ce` |
| E299 | `ec3f867cca0288fda5994d0e83cbda6848a98906d9eb7e0670c41781cc564164` |
| E300 | `b56b307d545c413c7a128b91c3e1f40b71ead9e0b9451df886d470d5662516b5` |
| E301 | `17c4d63e24d6b530e0df89cc1ce7da7a35f35027bad3473be0c6f138d33796a7` |
| E302 | `0a1f48e048f0ddeec53f28522d20fa04f14a890ce3554f2491b439003fa4ab74` |
| E303 | `94336528303e0d53f4cf3bd903fb781cb3f19f50ae4e7742bd7869a466f53699` |
| E304 | `a619374ec522ccdc858d13eede5cf6ec0d61564a9dcbe9d75967a7d2f0086a3b` |
| E305 | `cedcc0c4e31fca6c5f9dece95390f1866ae4abc179eee895d2431ae86a6508b1` |
| E306 | `92ea753932de61370337348b619a0dbff79ddd760dcf300d61df3f4b815b9dd8` |
| E307 | `99290375cf291816bfe13172c25187803f8ae801db69bfaa7fd276923e270488` |
| E308 | `8054cd668b530190055d7a91623e6527b4db2d6976645dd5547e4d1e972aed18` |
| E309 | `360540b4d3106eb7dfb2f28cd64ebbc46c903553a0cbf9f61c471b2415ea8fdb` |
| E310 | `9654d06cd00c9052bc6a29e10b97c024c4b66cb7fdb6ee4325383a81712dd255` |
| E311 | `dac8b953c86977df2dd3b752b92c243cba9e38c7c6c1e73116cbb4d7eaa0ef33` |
| E312 | `7b4cf1c846647b82a67ff3404867b31f5d09bf791f221cf8ffdbf6e08508c311` |
| E313 | `aa4da0669b917e5f717f6f968a4c8c2dfc7b342429565e64f86c02a99f70b3e0` |
| E314 | `d4ef58da9c1aca8061ea7d8affdf37aee6599c1580d418643f14e2a3b405d261` |
| E315 | `53326c9135f33e007f27985beea37831ba0fae2a89d539fa2427a58f956d5308` |
| E316 | `2f0c3aec7c69e4bde4ef8a3d0ad2e339000e8689d4476925f6aca4ca5f10f0db` |
| E317 | `318bf1f1c24e96baafea071c812cb43c225d1301619fba368d7afdf6f616faf6` |
| E318 | `fc1014e2f69a1315e6f94fc2474884d6b1d4d342f6e66665b9ef49f62ede76f9` |
| E319 | `a72717404a1bfb99a12ca2a6bea9eb5c3fd439cf8174aaca3010f79d5c1eaabe` |
| E320 | `bcfbb226302d9dafcff8db873b6e75b6596677f657d8a6798f08740569b966d8` |
| E321 | `619cb4f0e9e59a7764d8c439f437b1aa043188f9a8b9c6c935b328de5e0ad6bc` |
| E322 | `ef5c6ab6c0096f296ac1c812e9c9a032e31e4609b18058baa7927ff475cbde71` |
| E323 | `4b18407a95b876ec388b1495f12807bc9acfdc12152705469a867042559ea877` |
| E324 | `91b89acf2d464f04ac3672fd598647a73c09cfe9bf61311a9c629254d3bc4a4a` |
| E325 | `ca443fcb4fe3b6450b75b5629628c676abf404fee2907497fad1277cd0f44fb0` |
| E326 | `1d114dc1bf90eaa036a6e1a9ca52d23c2ec0aa2f3d0c2f0b45faad634c1be6b6` |
| E327 | `15858823e6328bba2bb574d592ae3f98bd35d51205e59c048a1fc69c41220d98` |
| E328 | `cc4460f77eb4917097ec9d1eb364c08a69b0bcc7c1123de75cf1ff8b3c91ee5e` |
| E329 | `d88f4bbb7f1b4ad3092266e67b276a250eac0ac97ff899471f55507721e4a567` |
| E330 | `c1dfb5251578341d91bd53f5eba49e3264760f898c358fa01e080d03987fb490` |
| E331 | `64f5bcd9c435ccc112ac30162a5ab11e07618a183c6918045769dbee32dcbb18` |
| E332 | `2f6867e4881803057ba8bd5ca42610b31f4fb6977e98a00be0ff39fe0b302dc9` |
| E333 | `f6cd1aa7001e671de4acb5e25fc5566c30500017aa1c0e6027f5b7fa0b8aba16` |
| E334 | `1823dfa43ccece013e18d0961e96bc269b097b70a352efc945032ad1bea14ea7` |
| E335 | `e1d2b3810d3d0e9c817e0ef3bf5e0c189b4fbc01794015b4dd2efde0e6f5f125` |
| E336 | `3833320828bdf2a7b0845272c0d835a1559014eff80cb91241a141c43f68b1c2` |
| E337 | `0b42fbb1e6244d37ffa23e0e82180c4a96c1401ababf31a5a393c0493f830616` |
| E338 | `88b2f562723a0ae31b06c427aaed0fc2c910ab7163a7fc5d27e4dc0b44f86f47` |
| E339 | `c054442bab5ca8dd6a9b2b944cbe7464b5fb7afaf82457b2160dcc0d0925320d` |
| E340 | `01d1903a9629dd0173c71f8b632ceb4c17a39b162ace254966d4b208777d0432` |
| E341 | `dbd1f3548332952b122ff87f6815ed00f8dcf3f6a3d332f905244cab6ac7a571` |
| E342 | `5b249a8524b6c3e98f8f9db498b1871a792d4ca128274cc42b04eb6e80180680` |
| E343 | `87fe9884b4264f34546b1d8fba620372d3eed2afe57560bac428f251b3cc64e3` |
| E344 | `5d6fc6bb86ab272d555dc07e9e21ff1712c296948d84ce8764417364dd168317` |
| E345 | `8d930d72a6a63aa7aa058326679d10d3738d95d211b280d711460e9f25d44eec` |
| E346 | `ed4b833aa2e8dfbf31624d5460f603b99b012fe71144fa298953a319ef32bab0` |
| E347 | `fe58606e1b5b1450657313610d95be5d99b777b6cd40dc13ff65db9130dfc4f8` |
| E348 | `a411d5324818152705b29ac172527b8535c3f7b49c2de0ae02c30a3b575b69f5` |
| E349 | `dd95277c3f57aa9c760c109c860080f51613fdfce0adc408637cf9c6c4f826dd` |
| E350 | `fe66576be435d84206c83c9b2120863a81556a929ee7a23b7af89acc9c37f9d3` |
| E351 | `33174c50a2e946bd387cc7b02127b145893863c0f6e8a4ccc5799beab714973c` |
| E352 | `eb5c96abc8086eba83ce79d28abb6dd0b286a48c18d53b196eeec95778e379c5` |
| E353 | `4c69c55bc5a9fcca67543abfdd432ddf2ec35dbc958194ccbe1b8aed57267ab0` |
| E354 | `bca34ec1b4381652847686546104c6b8d29fbc9a03aed3a4c4c1f5f5a920e5ff` |
| E355 | `a29b259bca6e341f94e09365f89fb427adc73bf92a6c3285e0997f3e93ead32f` |
| E356 | `e8ec5b233b8730b464aa0626052fabd324935b3fda15cd81638cc898eed2e2cc` |
| E357 | `10e59bd80d3aae2bc1ed9e34427a8a480a44a31095c2e37414650c3049f0be50` |
| E358 | `612ba285c4789511a794421f53f5f764c72669ef44973875dbe0b5d19a8c8cb8` |
| E359 | `53ea05b9c85411a7fdc2349d97b426d31951a1e26ad17ec9f80b381c75af8b57` |
| E360 | `ec83f286c27055677ce777abca80f446cd07208829bc56cc167ae8b6a4f80370` |
| E361 | `58fda476e2a8cd6b491a9136b629ade2726b05d7edd7dee551d4ee79d4d09aa7` |
| E362 | `8190c94277367022248f9aa1dc8cfc5f3252148254f194c5c073273a437f23d1` |
| E363 | `239bdde9e78997249af63fefdf4847f03ad68014b11b13be625a30c1ff700ec5` |
| E364 | `065e1ef7c5100523f33b8860f7d90c80438c9b811a78f0832e5f06b8efcdd7b3` |
| E365 | `8e65e6a697093dbcd5e5f715adf9877566a44ae7943f772eca61af911f42dfc4` |
| E366 | `11e99bafe34e1b8891bcfa2b956ca3305e48f797649287a40d68f0fdc005c0e3` |
| E367 | `0aacc4af163a64452de008a4b28c5086fb0a9f66fa36bb861e04fee2d2a88815` |
| E368 | `a19653e25a3c421e94d773d250377e7909f26895eafeeba50eae5cadf8b60f6e` |
| E369 | `440ae38edc7fc65b5e18842d9529b2168265a49139611e0ed37243a75cfa199b` |
| E370 | `6c0141bc976b0e0502fb9fad12a8df01e5666f343d59f9c7bd934c317c8f0c62` |
| E371 | `05c93efdd51ae46f1c3f01cc23a9bec51b0ac1b52b7210d4e6edfc5a9445c37b` |
| E372 | `510c1b28ffdf56a8e6d78c32293ec0f25e48f0ff4a160e996dae9be038444894` |
| E373 | `73c489a262c10695a580e68f7a7cac1b4c0119a909e2b0b30db7f0eb849c5e06` |
| E374 | `86bc9bd5c08040f5e753c696e3d69957332625900539104819ffb12df84c4c89` |
| E375 | `5efb41c77cdcad4a7085c1bc3041aa86cf21f7d95e6388387923f9d57e4af4e2` |
| E376 | `bafc0bdf5c30664b7862586b65563fc4c0d15e5be83a7a2742fa8ced1cf3c589` |
| E377 | `72306810d126ca4a4f73c0488133aa07f590404c6fd00409a0ec63edb1c15dd4` |
