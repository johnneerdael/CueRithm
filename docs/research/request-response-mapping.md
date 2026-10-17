# Request → response mapping (energy and cues)

Aggregate findings from 2,500+ request/response pairs (Mixed In Key
11.2.6.7446, key 94, energy "2", cue "3"). The request is the only input the
service sees, so every response field is a function of request fields. Request
production is already reproduced; this note covers the decision stage. Scripts
and per-track output stay under `.analysis/` directories.

## Exact rules (282/282 or 2818/2818, no exceptions)

| Rule | Evidence |
| --- | --- |
| Each `EnergyLevelSegments[].Volume` equals `VolumeRmsDb` of the **first** raw window inside the interval (not mean, power mean or min) | 2818/2818 |
| Intervals are runs of equal window level; adjacent response intervals never share a level | 0 violations |
| Interval bounds are raw-window bounds (not clipped to the track end) | handoff `KNOWN_FACTS.md` |
| Cue time from integer beat `b` uses the handoff's two-step anchor formula | 2195/2195 given `b` |
| Score peak is +16 beats ahead of the cue position: `cue_beat = sb + score_index − 16`; shift −16 gives 9759 matches on 2608 pairs versus ≤1717 for any other shift | empirical sweep on 2608 pairs |
| The first cue is always the origin beat (`CuePointStartBeat − 32`, normally 0) | 282/282 |

## Cue selection

### ML approach (best: OOF F1 = 0.933, 60.7% exact sets on 2,608 pairs)

`cue_learn.py` — HistGBM classifier on 44 features per 8-beat candidate
position.  5-fold GroupKFold OOF evaluation (no track leakage). Uses oracle
`n_cues` (86.8% of tracks have exactly 8 cues; always-8 gets 86.8%).

1. Decode scores, compute 44 features per candidate on 8-beat grid:
   32-beat window sums/max/std, position/ramp, phase context, neighbor
   windows, individual score values, local peak strength, relative rank.
2. Classify: HistGBM (800 iter, depth 7, lr 0.03). OOF AUC = 0.993.
3. Blend predicted probability (90%) with normalized raw score × ramp (10%).
4. Greedy: pick top blended score, suppress `|Δ| < 32` (score-index
   distance), repeat `n_cues − 1` times. DP gives identical results.
5. Map: `cue_beat = CuePointStartBeat + score_index − 16`.
6. Prepend origin cue (beat 0).

Error analysis (998 non-exact tracks):
- 80% of errors are single-cue swaps (FP=1, FN=1)
- Most common FN distance to nearest predicted: 32 beats (31%),
  then 16 (20%), 64 (15%), 8 (10%)
- In 78% of FN cases, the picked position has a genuinely higher
  raw score than the true position — the server sometimes picks
  lower-scoring positions, suggesting structural constraints (phase
  alignment or rhythmic templates) not captured by per-position features
- Spacing penalties (DP with gap costs) do not help: best penalty = 0
- 85.5% of true inter-cue gaps are multiples of 32 beats

### Rule-based approach (F1 = 0.873 on 2,608 pairs; 95.8% recall on 282)

Original 282-track rule (`predict_response.py`):

1. Decode scores `s` and get phase `ph`, start `st`, end from `select_phase`; `origin = st − 32`.
2. Ramp: `ramp(i) = min(1, ((i − st)/32)², ((end − 32 − i)/32)²)`, zero below `st`.
3. Score each position on the 4-beat lattice as
   `s[p] · ramp[p] · phrase_weight(p − origin − ph, [0.7, 0.8, 0.5, 0, 0])`.
4. Greedy: take the best position, discard all positions closer than 32 beats
   (`|Δ| < 32`; neighbours exactly 32 apart remain allowed), repeat 7 times.
5. Cue beats are `p − 32` for each pick plus the origin cue `origin`.

On 282 tracks: OOF recall 95.8%, precision 93.3%. On 2,608 tracks: the
phrase weights hurt (F1 drops from 0.86 to 0.76); best rule-based uses
no phrase weights, step=8, ramp exponent 1, shift −12, min_dist=28 →
F1 = 0.873. The 282-track weights were overfit to that subset.

Not explained: 333 tracks (13.2%) have fewer than 8 cues and score thresholds
do not cleanly separate dropped from kept picks. True window energy levels
add nothing to a learned ranker, so cue choice does not depend on the energy
stage.

## Cue energy level (near-exact given window levels)

A cue's `EnergyLevel` is not the level of the window it falls in. It is the
median level over the beats from that cue to the next cue (the `known_math`
annotation recipe, confirmed): beat `i` has centre `downbeat + (−32 + i + 0.5) ·
60/Tempo` and takes the level of the window that contains it; a cue at beat `b`
covers indices `b + 32` up to the next cue's index (or the beat count); the
level is `(lower median + upper median) div 2`. Beat count =
`floor((DurationInSeconds − DownbeatTime) / period) + 66`. With the true window
levels this matches 2189/2195 cues (99.7%; counts above ~64 beats saturate). With predicted
levels, matched-cue energy accuracy is 67.7%, bounded by the window model.

## End-to-end results

### Full dataset (2,608 pairs, all OOF)

| Output | Result |
| --- | --- |
| Raw-window energy level exact / within one | **91.7%** / 97.7% (RBF SVC, C=10, γ=0.1; 62K windows) |
| Cue F1 (±4 beats) | **93.3%** (P=93.2%, R=93.3%; HistGBM, oracle n_cues) |
| Cue exact-set match | 60.7% (1527/2516 valid tracks) |
| OverallEnergy tag | 100% deterministic (level with longest total duration) |
| Server OverallEnergy (deprioritized) | ~79% (ridge on 18 features) |

### Original 282-track subset

| Output | Result |
| --- | --- |
| Raw-window level exact / within one | 85.47% / 96.72% (SVM; boosting gave 67.97% / 97.06%) |
| OverallEnergy exact / within one | 74.82% / 99.65% (boosting gave 69.86%) |
| Cue F1 at 50 ms | 94.5% (precision 93.3%, recall 95.7%) |
| Cue energy on time-matched cues | 83.1% (boosting gave 67.7%) |

Window model: RBF SVC, C=10, gamma=0.1. On 282 tracks (6K windows): 85.5%.
On 2,608 tracks (62K windows, 5-fold GroupKFold): **91.7%** exact, 97.7% within one.

## Energy level: a multi-class RBF SVM on the 183 features

Static analysis of the client's `MIKAnalysis` framework (Ghidra 12.1.4 on an
`.analysis` copy of the 11.2.6 thin binary) shows `EnergyFeatureExtractor::OutputToSvm`
and `SegmentSvmInput`: the 183 values per window are the input vector of a
support vector machine. The model itself is not shipped (only extractor code), so it
is the server's. Fitting the same family reproduces it far better than every other
model class tried:

| Model (out of fold by manifest `cv_fold`, 183 raw features) | Window level exact |
| --- | ---: |
| Gradient boosting / forest / extra trees (regress + round) | 67-69% |
| Single tree, 8 / 128 leaves | 62% / 64% |
| RBF SVR (regression), best C, gamma | 67% |
| RBF **SVC** (multi-class, one-vs-one), C=100, gamma=0.011 (sklearn `scale`) | 80.4% |
| RBF **SVC**, C=10, gamma=0.1 (282 tracks, 6K windows) | 85.5% (within one: 96.7%) |
| RBF **SVC**, C=10, gamma=0.1 (2,608 tracks, 62K windows) | **91.7%** (within one: 97.7%) |

- Use the features as supplied: the per-feature scaling is already the SVM's.
  Standardising drops it to 71%; adding `VolumeRmsDb` raw drops it to 82% (scaled
  by 1/20 it is neutral); dropping the tempo feature is neutral.
- Hyperparameters plateau around gamma 0.06-0.2 and C 3-30 (84.7-85.5% at 282
  tracks); the original server hyperparameters and training set are unknown, so
  this is a fit of the same model family, not a copy. Level 10 never occurs in
  the data (3 windows).
- The decision function is explicit: one-vs-one voting over
  `f_ij(x) = sum_k a_k exp(-gamma * ||x - sv_k||^2) + b_ij` (libsvm layout).
  `svm_export.py` exports the fitted model as arrays. A Rust port is the same
  arithmetic with no Python.
- Label runs are mostly one window long and smoothing lowers agreement, so the
  windows are classified independently.
- Learning curve confirmed: 6K → 62K training windows lifted accuracy from
  85.5% to 91.7% (5-fold GroupKFold, no track leakage). Per-level accuracy:
  levels 4-7 at 90-94%, levels 1/3/8 at 76-85%, level 9 at 24% (42 windows).
  Curve may still be rising; more captured data should help further.

## Overall energy

**Tag energy (client-side, solved)**: The client's `recalculateOverallEnergyLevel`
is deterministic: build a histogram of total segment duration per energy level
(0–10), then pick the level whose duration strictly exceeds all higher levels'
total durations (highest level wins ties). This is the value written to file tags
and shown in MiK's UI. It agrees with the server's `OverallEnergy` response
field only 57.3% of the time (server is systematically +0.39 higher).

**Server `OverallEnergy` (response field, deprioritized)**: Not needed for cue
replication (cue positions depend only on CuePointData scores; cue energy labels
depend on per-window segment levels). Best approximation: Ridge regression on
18 features (level stats + volume + vol-weighted mean) gives 79.2% exact OOF
(2,608 tracks). Ensemble with HGB continuous window predictions: 79.5%. The
remaining gap appears to require the server's continuous SVM decision values,
which are not in the response. The 352-value `Segments.Features` vector in the
request is unrelated to energy (54.9% accuracy). The request `EnergySegmentData`
raw 183 features alone predict at 69.9%; adding them to level stats does not
improve beyond 79.5%.

## Negative results (do not repeat)

- Before the SVM finding: window level, out-of-fold exact: additive stump model 58%; 4-leaf trees 64%;
  8-leaf 66%; deep boosting 69%; random forest 68%; MLP ≤50%. Interactions
  matter, so there is no additive or short closed form.
- Learning curve (train on 1, 2, 3, 4 folds): 62.5, 65.3, 66.3, 66.6% — flattening.
- Adding track-level summaries, neighbouring-window features, key vector, cue
  statistics, window position, or the true OverallEnergy: no gain (≤69.0%).
  Even the true neighbouring levels give only 70%.
- A single classification tree reaches 61.8% out of fold with 8 leaves and
  64.4% with 128; training accuracy only reaches 100% at about 1000 leaves, so
  the level is not a small hidden decision tree. Smooth models (quantile-scaled
  MLP, 44% at the first setting tried) did worse than trees.
- Feature-group ablation (gradient boosting, out of fold): any single quantity
  family reaches 41-59% (best: zero-lag transient amplitude 59%, lag-8 pair 58%),
  and any single band 44-56% (high bands best); the signal is spread across
  families rather than concentrated.
- Cue count: after the greedy picks, the drop-off ratio of the first omitted
  pick to the previous pick (median 0.66) overlaps the kept picks' ratios (5th
  percentile 0.57), so no ratio, cumulative-mass or absolute threshold gives the
  stopping rule.
- No two windows share identical features, so determinism of the level as a
  function of these inputs cannot be tested directly.
- Overall energy from the true window levels: best simple rule is
  `round(wmean + 0.7*wstd)` at 73.5%; percentile rules cap at ~71.6%.
  Ridge on 18 features (level stats + volume + vol-weighted mean) reaches
  79.2%. Adding raw 183-feature aggregates does not improve. Temporal
  pattern features (half-comparison, max-run, sustained peak) do not help.
  The 352-value `Segments.Features` vector predicts at only 54.9%.
  Track-level SVM on aggregate raw features: 68.3%. Standardizing helps
  for track-level SVM (67.1%) but not for window SVM. Ensemble of
  Ridge+SVR+HGB reaches 79.5% — hard ceiling without server's continuous
  SVM values.
- Older macOS 10.3.3 and Windows 8.5 builds call the same remote service; no
  local energy classifier exists to recover.

## Binary analysis findings (Ghidra 12.1.4 on DanceabilityAudioEngine + main binary)

### CuePointData scoring (client-side, `MIK::flow::DetectLikelyCuePointsV3`)

The CuePointData float32 array sent in the request is the output of `CalculateScore`,
computed locally from audio. Each value is a per-beat "cue-worthiness" score based on:

- `DifferenceSum`: measures forward energy changes at 32/16/8 beat scales (weights
  0.8/0.8/0.9, scaled by 96.0) plus changes in features 0 and 2 at 32-beat scale
  (weight 0.4, scaled by 98.304 and 245.76). A penalty term (negative weight -1.0)
  for backward decreases at 32/16/8 scales (3.2/1.6/0.8), clamped at zero.
- `HanningSum`: standard Hann-windowed sum, `cos(2πt)*(-0.5)+0.5`.
- After scoring, a band-pass smoothing: `1.5*H(w=1) - 0.5*H(w=3) - H(w=32)`.
- Ramp: `min(1, (min(dist_left, dist_right)/32)²)`, zero within 32 beats of edges.

### Local cue selection (client-side, also in `DetectLikelyCuePointsV3`)

After ramp and smoothing, the local client selects one cue per 32-beat phase:
- For each of 32 candidate offsets (0..31), sum `score[beat]^1.1 * PhraseDistanceWeight(beat_offset)` across all matching beats.
- Multiply by `PhraseDistanceWeight(candidate_offset, w_outer)²` where `w_outer = [0.9, 0.924, 0.96, 0.973, 0.971]^1.15`.
- `CalculatePhraseDistanceWeighting(offset, weights[5])`: hierarchical at 32/16/8/4/2 beat boundaries. Product of `(w[k]-1)*d_k + 1` for k=0..4, where d_k is the normalized distance to the nearest boundary at each scale.

### Server-side cue selection (decompiled config schema)

The server receives CuePointData scores and returns CuePoints. Decompilation of the
`CuePointDetectionPass1TransformData` constructor (`FUN_10006fdb4` in main binary)
and `CuePointDetectionV2Pass2Transform` constructor (`FUN_1000735da`) reveals the
full config parameter schema and client-side default values.

**Pass1 (`CuePointDetectionV2Pass1Transform`)** — beat grid fix + phase scoring:

| Parameter | Type | Default | Range |
| --- | --- | --- | --- |
| `_fixBeatGridVersion` | int | 4 | 4–5 |
| `_shiftEnable` | bool | true | — |
| `_shiftDir` | int | 1 | 0–1 |
| `_weightWindowSize` | double | 0.1 | — |
| `_shiftLikelihood` | double | 1.0 | — |
| `_shiftAmount` | double | 0.5 | — |
| `_timeShift` | double | 0.0 | — |
| `_phaseFeatureSelect` | int | 0 | 0–7 |
| `_phaseScoreQ` | double | 0 | — |
| `_phaseScorePow` | double | 0 | — |
| `_phaseSectionSize` | int | — | 8–32 |
| `_phaseSectionStep` | int | — | 1–16 |
| `_phaseResolution` | int | — | 32–128 |
| `_pathSlopePenaltyLin` | double | 0 | — |
| `_pathSlopePenalty` | double | 0 | — |
| `_pathSlopePenaltyPow` | double | 0 | — |
| `_pathSecondSlopePenaltyLin` | double | 0 | — |
| `_pathSecondSlopePenalty` | double | 0 | — |
| `_pathSecondSlopePenaltyPow` | double | 0 | — |
| `_pathCullThreshold1` | double | 0 | — |
| `_pathCullThreshold2` | double | 0 | — |
| `_pathCullCount` | int | — | 100–2000 |

All DP/phase-score parameters default to 0 in the client; the server sets them
via config. The DP "path" parameters are under `_beatGridFixV5._phaseScoreParameters`,
suggesting the DP optimizes beat-grid alignment (finding the best phase offset),
not cue selection directly. Cue selection after alignment is likely simpler
(greedy or top-k).

**Pass2 (`CuePointDetectionV2Pass2Transform`)** — shift refinement:

| Parameter | Type | Default | Range |
| --- | --- | --- | --- |
| `_shiftEnable[0..2]` | bool | all true | — |
| `_shiftDir[0..2]` | int | — | 0–1 |
| `_shiftFeatureSelect[0..2]` | int | — | 0–7 |
| `_offWeight` | double | 0.5 | — |
| `_onThreshold` | double | 0.688 | — |
| `_shiftModulo` | int | 8 | 2–64 |
| `_lockPhase` | bool | false | — |
| `_weightWindowSize` | double | 0.1 | — |
| `_shiftLikelihood` | double | 1.0 | — |
| `_shiftAmount` | double | 0.5 | — |
| `_timeShift` | double | 0.0 | — |

Three shift channels (1/4, 1/2, 1 beat) with per-channel enable, direction and
feature selection. Constants in the constructor: 1.07 (≈ the score^1.1 exponent),
2.636, 0.201, 0.259. Also contains 16 hardcoded binary beat-magnitude patterns
(rhythmic templates at 16 and 32 beat lengths, plus hierarchical phase patterns
with wildcards).

Empirically:
- 81.9% of response cues fall at 32-beat boundaries (phase offset 0)
- 12.7% at offset 16, 5.4% at other offsets
- 86.9% of tracks have exactly 8 cues

### Client-side `recalculateOverallEnergyLevel` (decompiled)

The client recalculates overall energy after receiving server results:
1. Build histogram: total segment duration per energy level (0-10)
2. Iterate from level 10 down to 0, tracking running max duration
3. Pick the level whose duration strictly exceeds all higher levels' durations
4. This is effectively "energy level with the longest total duration" (highest wins ties)

This gives only 57.5% agreement with the server's `OverallEnergy`, confirming the
server uses a different formula. Best empirical server approximation: `round(percentile_73)`
of duration-expanded levels → 72.2% exact, 99.3% within-1 (on 2,300+ pairs).

### Overall energy: expanded investigation (2,300+ pairs)

Rule search exhaustively tested 92 percentiles (50–95 × round/floor),
cumulative thresholds, mode, volume-weighted percentiles, and wmean+bias
combinations. Key findings:

| Rule | Exact match |
| --- | ---: |
| `round(p73)` of duration-expanded integer levels | 72.2% |
| `round(wmean + 0.7 * wstd)` | 73.5% |
| `round(0.15*wmean + 0.85*p78 − 0.30)` (best blended) | 72.8% |
| GBM regressor on 57 segment-level features (5-fold OOF) | 75.4% |
| GBM regressor on segment + aggregated raw 183 features | **78.1%** |
| SVR continuous per-window scores, round(p73) | 69.9% |

Greedy set-cover: `round(p73)` covers 72%, `round(wmean)` adds 10%,
`round(p84)` adds 8.3%, leaving 8.3% uncovered by any simple rule.
This means the correct rule varies per track.

GBM feature importance reveals **volume** is the strongest signal beyond
integer levels: vol_max (14.8%), vol_min (8.7%), vol_std (5.4%). Errors
concentrate at the 6/7 boundary (328 of 574 errors). The server likely
uses continuous SVM decision-function values and volume, not just rounded
integer levels. Continuous SVR predictions from our approximate SVM are
noisier than the true integer levels, so they don't help; the server's
own SVM values would be exact.

### Updated results (2,500+ pairs, 58,000+ windows)

| Output | Result |
| --- | --- |
| Window energy SVM (C=10, gamma=0.1, raw features) | ~85% exact (learning curve still rising) |
| OverallEnergy best rule (`round(wmean + 0.7*wstd)`) | 73.5% exact, 99.6% within-1 |
| OverallEnergy best ML (GBM regressor, seg+raw feats OOF) | 78.1% exact, 99.9% within-1 |
| Cue F1 (HistGBM on 8-beat candidates, 5-fold OOF) | 92.7% (P=92.7%, R=92.7%), 55.6% exact-set |
| Cue F1 (ramp + median + half-phase, no ML) | 76.8% (P=76.9%, R=76.8%), 14.8% exact-set |
| Cue F1 (sum phase greedy baseline, 4-beat tolerance) | 72.1% |

### Cue selection: expanded investigation (2,500+ pairs)

**Phase scoring**: Median (quantile 0.5) of scores within each 32-beat
phase outperforms sum for ranking (F1 0.734 vs 0.721). This aligns with the
server's `_phaseScoreQ` parameter. Adding a quadratic ramp before computing
medians gives F1 0.738. Half-phase refinement (second 16 beats >1.8× first
→ cue at offset 16) adds +3.0%, reaching **F1 0.768** combined. Section
size 32 is by far the best; sizes 8/16/24 give F1 ≤0.44.

DP spacing penalties (first/second derivative on inter-cue gaps) do NOT
improve over greedy: best penalty = 0. The DP parameters in the binary are
under `_beatGridFixV5._phaseScoreParameters`, meaning the DP optimizes
beat-grid alignment, not cue selection. Cue selection is effectively greedy
on phase scores after grid alignment.

**Error analysis**: Missed cues by median-phase rank are spread across ALL
ranks 0–6 (~270 each), spiking at rank 7 (942), 8 (598), 9 (420). This
means ~28% of errors are phases where the server ranks a phase higher than
we do (ranks 0–6), and ~72% are phases just below our top-7 cutoff. The
server likely uses a slightly different phase-scoring method (the exact
`_phaseScoreQ` and `_phaseScorePow` values are unknown).

Cue spacing is not regular: CV of inter-cue spacing is 0.51 (median),
most common spacing is 64 beats (28.3%), then 32 (22.4%), 128 (13.7%).
84.8% of spacings are multiples of 32 beats.

**ML approach (`cue_learn.py`)**: HistGradientBoosting classifier on
candidates at every 8-beat position (32-beat windows). Features: window
sum/max/std, quarter sums, ramp, relative rank, neighbor sums, score at
key beat positions, and phase context. AUC 0.991 (5-fold GroupKFold by
track). End-to-end with greedy selection (min_dist=24): OOF F1 92.7%,
55.6% exact-set match. The 8-beat grid captures offset-0 (82%),
offset-16 (13%), offset-8 (2.4%) and offset-24 (2.0%) cues; the 16-beat
grid misses the latter two and achieves only 90.4% F1. With fixed
n_cues=8 (no oracle): F1 91.5%, showing modest degradation since 86.4%
of tracks have exactly 8 cues.

Beat mapping: score array index `i` corresponds to beat `sb + i` where
`sb = CuePointStartBeat` (typically 32). Scores peak ~16 beats before the
cue position (shift -16 gives 99.1% of cues with score ≥ 50% of phase max).

## Not established

- The exact server-side overall energy aggregation (78.1% with GBM on
  segment + aggregated raw features). Top GBM features: wmean (33.4%),
  p73 (10.2%), p78 (9.8%), vol_std (4.8%), frac_7 (4.5%). Errors
  concentrate at the 6/7 boundary.
- Server-side cue selection parameter values. The full parameter schema
  is recovered (Pass1: 22 params, Pass2: 12 params) but values are
  server-side config. The DP is for beat-grid alignment, not cue
  selection. Best rule-based cue F1 is 76.8% (ramp + median + half-phase);
  best ML is 92.7% (HistGBM on 8-beat candidates).
- Cue count stopping rule (86.3% have 8 cues; 7.9% have 7).

Patents checked: US 2008/0314231 covers key prediction only; US10854180B2 is an
unrelated music-generation patent. Neither describes energy or cue selection.
