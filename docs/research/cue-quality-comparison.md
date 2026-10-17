> Historical research. Cuerithm now uses the Lexicon-compatible cue engine.
> Mixed In Key cue inference and cue/segment energy labels are retired from the
> active product. Retained energy research may inform the single track rating.

# Quality-first cue comparison, 2026-10-17

The user prioritizes useful musical/DJ cues over exact MiK reproduction. This
first experiment tests whether a small native structural method adds value
before adopting a separation-plus-neural structure stack. It is a **research
prototype**, not a new production backend or a completed quality assessment.

## Fixed comparison

All282 tuning groups use the same retained, integrity-verified PCM-derived
structure features and source timing. The four-beat candidate lattice comes
from independently computed cue scores and a fixed phase algorithm, without
reference cue positions. Its phase explains all1913 non-first MiK cue positions,
but musical downbeats and four-beat meter have not been independently validated.
The82 reserved groups remain unused.

The three arms are the unchanged Rust novelty function on this common timing;
the same function restricted to bar starts with16-beat context; and CBM over
mean RMS/20, six band fractions and flux per complete bar. The CBM variant uses
normalized cosine similarity, seven off-diagonal bands,4..32-bar DP segments,
modulo8 length penalty weight0.15, a four-bar boundary contrast score>0.05, and
8-bar cue separation. It retains an entry candidate using a fixed-80dBFS gate, returns at most eight cues,
and does not pad sparse candidates. All policies share inputs; this isolates
selector behavior rather than comparing different decoders or timing models.

CBM mathematical provenance is the [author's BSD-3-Clause source](https://github.com/ax-le/autosimilarity_segmentation/tree/ebbbd46e7ae98283ad16de9256faad2f7741e12b).
The acoustic8/cosine representation and cue selector are separate experimental
choices. They are not the paper's within-bar spectrogram representation, a
mel80 replication, or copied code from the unlicensed automatic-cuepoints
wrapper. Their results do not establish the original tool's quality.

## Observations

| Policy | Cues across282 tracks | MiK one-to-one F1 at50ms | MiK F1 at one assumed4-beat bar |
| --- | ---: | ---: | ---: |
| Existing novelty on common timing |2251|53.94%|59.56%|
| Bar-constrained novelty |2251|54.21%|58.52%|
| Acoustic8/cosine CBM |1887|61.00%|64.28%|

All282 native jobs succeed. Ten synthetic behavior/domain tests pass, and four
matcher checks pass. An independent greedy matcher reproduces the dynamic
programming matcher's counts at50ms and one bar. Policy/source/input hashes and
reference-content denial were verified. The median/p95 selector process time
is27/35ms; decoding and feature extraction were not timed here. The macOS worker
requires system C/iconv, without Python, model weights or network at runtime.
Linux/Windows execution of this new worker remains unverified.

Higher MiK F1 is a secondary observation, not proof that CBM cues are more useful.
It also produces fewer cues (6.69 per track), which may reduce redundancy or miss
useful transitions. Only listening/task-based assessment can distinguish those
outcomes. No default was changed, no model was trained, and no quality winner
has been selected.

## Listening preparation and next decisions

Twenty-four tuning groups were selected by a fixed SHA256 ranking before cue
comparison scoring. Private anonymous inputs contain the three policies plus
MiK as an unlabeled fourth comparator, with the assignment key stored separately.
No audio was uploaded and no ratings have been collected. The prepared inputs
are not a completed blind study or a listening UI.

Assess phrase/downbeat correctness, useful mix-in/out runway, meaningful
transitions, redundant/missing cues, and accept/move/remove decisions. Compare
all available slots and allow multiple musically acceptable choices. Keep the
same source grid when isolating selection errors, then assess grid errors
separately. Genre diversity and uncommon meter need independent coverage.

If the small representation misses recurrent arrangement changes, test a
within-bar spectral CBM representation before concluding that a learned model
is necessary. All-In-One remains a separately researched candidate for semantic
structure/downbeats; its separation, custom attention and postprocessing add
runtime/porting work. It supplies structure rather than a complete DJ cue policy.

Private evidence: `cue-quality-acoustic-cbm-v1`. Worker SHA256
`5e6a7be544d7af80590f7682107d64ae1b65c7817524f2bb8d999462e0da6cc8`.
Production cue integration, energy classification, broader formats/platforms
and the final reserved evaluation remain
unfinished parts of the engine goal.

## All-In-One full-ensemble runtime pilot

Following the user's preference to test the richer model, the original
All-In-One source and all eight official Harmonix checkpoints now run offline
on one full development track. Five portable-attention golden/domain tests and
an eight-fold synthetic network-health check pass. Original Demucs separation
normalization is preserved; the model runs through an explicit compatibility
adapter rather than a compiled NATTEN extension.

On a7.07-minute FLAC, the original model predicts22 functional segments,
875 beats and219 downbeats. A separate native Rust research policy selects
eight cue candidates from those outputs. These predictions have not been
validated as musically correct. Observed CPU/two-thread time is272seconds,
including207.8s separation,4.35s feature extraction and56.6s inference and
postprocessing. Peak memory in the analysis parent is4.58GiB; this is not an
aggregate-system memory measurement or a controlled cross-method speed test.

The fixed24-track reference comparison has completed:24 successful model runs,
24 successful fixed cue-policy outputs and502 raw sections. No original model
or cue-policy parameters changed during the run. On these same24 groups:

| Policy | Predicted cues | MiK F1 at50ms | MiK F1 at one assumed4-beat bar |
| --- | ---: | ---: | ---: |
| Existing novelty |192|52.36%|59.69%|
| Bar-constrained novelty |192|52.88%|58.12%|
| Acoustic8/cosine CBM |162|62.50%|64.77%|
| Initial All-In-One cue policy |192|41.36%|57.07%|

All-In-One uses its own predicted downbeats; the three earlier policies share
source timing. These figures therefore compare complete candidate paths, not
selector behavior in isolation. They measure agreement with MiK, not musical
usefulness. No listening ratings or quality winner exists. A local five-arm
anonymous label/rating packet is ready; its editor import format has been
checked, but no installed-editor roundtrip has been performed.

All-In-One median CPU/two-thread elapsed time is189.14s per track, including
separation and inference. Maximum analysis-parent RSS is6.95GiB. These are not
controlled cross-method timings or aggregate-system memory measurements.

The reference pipeline remains Python. A separate native Rust gate executes
all eight structure-network folds on Mac and Linux ARM64, matching original
eager raw outputs on247195 generated-input values across seven input lengths
within the declared tolerance. Real-audio preprocessing, separation, final
postprocessing and production integration remain unfinished. All82 reserved
groups remain excluded.

Private experiment: `allinone-pilot-v1`. Original project:
[All-In-One Music Structure Analyzer](https://github.com/mir-aidj/all-in-one).
