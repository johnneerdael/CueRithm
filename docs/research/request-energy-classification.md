# Offline request-to-energy classification, 2026-10-17

The Rust request producer already reproduces all ten captured analysis fields.
This study evaluates the separate energy decision stage using the captured
183-dimensional energy vectors and VolumeRmsDb. It supplies no filenames,
metadata tags, track IDs or reference labels as prediction features.

All282 development groups and6182 raw windows remain in the denominators.
Each window's target is the unique response interval covering its midpoint;
nominal windows beyond track duration are preserved. Five fixed folds keep
whole groups apart. Every model fits only the other four folds. The82 reserved
groups remain unopened. These are exploratory development comparisons against
MiK labels, not a musical-quality study or reserved-set accuracy estimate.

## Same-fold results

| Method | Exact window agreement | Within one level | Mean absolute error | Macro recall, observed levels1–9 | Tracks with every window exact |
| --- | ---: | ---: | ---: | ---: | ---: |
| Train-fold majority |45.10%|83.00%|0.762|11.11%|0|
| Earlier local classifier, rerun |75.32%|95.21%|0.307|44.93%|6|
| Larger local classifier |76.77%|95.34%|0.291|47.45%|6|
| Larger local classifier, balanced training weights |76.88%|95.21%|0.293|50.22%|7|
| Larger classifier with neighboring/track context |75.80%|95.07%|0.305|45.07%|6|

The larger local model uses300 histogram-boosting iterations,31 leaves,depth5,
learning rate0.08,L2=1 and minimum leaf size20. Balancing changes training-fold
class weights only. Context adds a centered three-window mean and componentwise
track median to each local vector. Ordinal regression and fixed temporal
smoothing also ran; both reduced exact agreement. Their negative results are
retained with the other variants.

The improvement over the same-fold earlier classifier is modest:1.46 percentage
points for the larger model and1.57 for its balanced variant. A post-selection,
paired whole-track bootstrap gives exploratory95% intervals of0.64..2.26 and
0.76..2.39 percentage points respectively. The balanced-versus-unbalanced exact
agreement difference is only0.11 points, with an interval spanning zero
(-0.60..0.84). This diagnostic does not correct for adaptive development/model
selection and does not identify a production winner.

Rare levels remain weak. The balanced model recalls27.35% of117 level8 windows,
and none of the five level1 or seven level9 windows. No level10 training example
exists. Pooled accuracy therefore hides substantial coverage gaps.

## Overall energy remains a separate problem

Fixed window summaries were tested without fitting an overall-energy head.
For the balanced model, the75th percentile matches200/282 overall labels
(70.92%). Even the same summary of teacher window labels matches only220/282
(78.01%). The median,90th percentile and maximum do worse. Thus the tested
summaries do not explain the overall-energy mapping, even with correct windows.
Do not describe window classification as a completed overall-energy estimator.

## Native Rust execution

A private Rust inference prototype evaluates exported numeric tree splits,
leaf values, initial logits and class order. It also constructs the declared
context and performs softmax and the fixed categorical sequence decoder. It
requires no Python, original application, analysis service or neural runtime
during inference. Python is used for fitting, export and reference evaluation.

All four initial classification arms pass their excluded-group fold models
through Rust:1128 track evaluations and24728 window predictions. All222552 raw
logits and247280 probabilities match the saved model outputs numerically
exactly; argmax and sequence-decoder labels have zero mismatches. The balanced
follow-up also passes all282 tracks/6182 windows, with55638 logits and61820
probabilities matching exactly. The declared numeric tolerance was1e-12absolute
plus relative and was not changed after execution.

Four Rust domain/behavior tests, formatting, Clippy and a release build pass.
Representative actual sandbox controls deny evaluator-content reads and a TCP
connection, while allowing configured request inputs. All workers exit before
expected outputs are opened for grading. A separate evaluator audits all13
reported variants, all rows, input/source hashes and disjoint group folds.

This native gate is on macOS ARM64. It verifies model execution from request
vectors, not fresh PCM-to-energy production integration or Linux/Windows
execution of this new prototype. The normal engine's energy field/default is
unchanged. No final model is selected, trained for release or integrated into
the public Engine API. Energy quality and whole-engine acceptance remain open.

Private evidence: `request-energy-offline-v1`, `request-energy-offline-v2`,
`request-energy-offline-v3`, `request-energy-native-v1`,
`request-energy-native-v2` and `request-energy-evaluation-audit-v1`.
