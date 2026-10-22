# MIK facts register and modelling constraints

Revision 3 · 22 October 2026 · Required engineering reference.

Revision 2 authorized private MIK cue-model research and superseded revision 1's
prohibition. Revision 3 adds F075–F078 from fresh development-data audits and the
first fresh key comparison. Existing fact IDs and evidence identities are
preserved; the product's validated Lexicon DSP remains in place.

Read this register before changing feature extraction, key/BPM/energy analysis,
training, evaluation or model promotion. Cite relevant F-numbers and the register
revision in the experiment record and PR. This is the canonical tracked aggregate
register; the private companion uses the same IDs and retains the underlying
audio, capture audits and identifying evidence. Historical reports can describe
earlier hypotheses; the scoped exclusions here govern current work.

The record covers controlled runs 01–09 (including rejected 06 and replacement
06B), local mechanism investigations, and an independently recomputed assessment
of the supplied mixed track ledger. Each item distinguishes observations,
derivations and their scope. Project decisions and proposed techniques are
identified explicitly; they are not recovered vendor mechanisms. No predictive
model was trained while compiling revision 1. Later experimental fits are
summarized in F078 and retained in their separate experiment records.

There are 295 captured synthetic fixture IDs and 277 distinct retained requests;
281 IDs / 263 requests are qualified against local extraction. Fourteen older
08 fixtures remain conditional observations with upstream failures. All 64 new
09 fixtures pass local-request and decoded-audio checks. There is no promoted
replacement model and no established 99% agreement result.

Evidence references E001–E046 identify immutable private source snapshots by
SHA-256 in the provenance index below. They are identifiers, not missing public
links. The private companion maps them to files, reproduction scripts and capture
identities. Music, private library paths, real-track labels, credentials,
decompiled code and proprietary packaged assets are deliberately excluded.
An engineer without that evidence must request access before claiming to have
reproduced a finding. Public aggregates alone do not enable full reproduction.

## Immediate exclusions

- Key input is 352 register/bank values, not interchangeable with 12 chroma values
  (F017–F019). Exact transposition equivariance is not established.
- Activity, spectral brightness and vocal presence are interventions, not measured
  energy labels or proof of a semantic detector (F013, F031–F040).
- Rounded window labels do not support a universal fixed monotone whole-track
  reducer (F041–F047). Label histograms have conflicting targets (F065), with a
  measured maximum of 98.81% on the larger observed cohort (F075).
- Captured-feature prediction and end-to-end PCM prediction have separate
  denominators (F001–F004). An upstream refusal is still an end-to-end failure.
- Prior guessed bundles and failed partial prototypes are not the starting
  weights for the new programme (F059–F063).
- Private MIK cue-model research is permitted by the owner. The product cue path
  remains the validated Lexicon DSP; research does not promote a replacement.

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

FFT frames use 2048 samples/hop 682. Inspected phase searches extend about 64 beats and reduced families about 128 beats forward, with interpolation/filter/smoothing support beyond nominal 32-beat windows.

**Scope:** The inspected feature implementation; exact support also depends on state and phase.

**Rules out / prevents:** Do not transfer a stationary template label solely because one 32-beat waveform interior is unchanged.

**Evidence:** E034.

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

## Working method decision — proposed techniques


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
10. A validated deployable default meeting 99% across the requested domains. None exists in this evidence.


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

E001–E041 retain the revision 1 snapshots; E042–E046 add revision 3 snapshots. These hashes
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
