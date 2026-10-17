# Key cache reconstruction

Subsequent [independent tuning reconstruction](key-tuning-reconstruction.md)
removes supplied finalized tuning from the composed macOS/Linux chain. The original
cache-component proofs below retain their original peak/tuning input scope.

This records the original private cache-component experiment, not a production
engine benchmark. Original tracked spectral peaks, finalized tuning and archived
beat context were supplied inputs to that proof. Subsequent
[real-audio reconstruction](key-real-audio-reconstruction.md) removes original
peaks/tuning/cache inputs on five development files while still supplying the
refined beat grid and downbeat. Service key classification remains unfinished.

## Recovered calculation

An independent Rust implementation of the default44100Hz/N4096 path reproduces
all2,379,872 cached float32 values across27,044 frames on the five original
development cases on macOS ARM64. The source trace follows constructor0x1a578
and Finished0x1bab8 in the pinned MIKAnalysis artifact. Retained REA Evidence:
`ev_5d2d465d2501cef02bda0ef66f6b06f319027582fa8eb8348d7f2f03b6db5d2e`.

Peak frequency is a fractional FFT-bin coordinate. The implementation preserves
the float32 pitch chain, double wrapped tuning subtraction, half-grid rounding,
double cosine/FMA rasterization, weighting/compression and suppression order.
Two89-cell grids are required. Only cells0..87 are compressed; raw guard
cells88 affect the upper suppression neighborhood. Initial empty frames are
retained without shifting the supplied frame sequence.

Composing reconstructed caches with the independently implemented four-bank
aggregation produces all1,760 outgoing key-feature values exactly on macOS.
The bank worker receives newly computed cache bytes, never original cached88
values. Expected request/cache values stay in the evaluator after worker exit.

## Numerical portability investigation

The initial Linux ARM64 cache run failed strict parity:1,206 values differed,
with a maximum837ULP difference. Unit tests and invocation still passed. All
failed outputs and every mismatch were preserved; no tolerance was relaxed.

A bounded2014-case math probe identified platform logarithm/power rounding.
100/200-digit Decimal calculations with exact binary32 midpoint checks showed
that the macOS logarithm was nearest in the one examined disagreement. All
seven examined macOS power discrepancies were oneULP away from nearest, in
both directions. Replacing power with higher precision therefore improves
mathematical rounding while changing the required reference behavior.

Four explicit diagnostic modes confirmed the distinction. Linux differences
from original caches were1,206 for native math,1,044 for a promoted logarithm,
1,200 for promoted power, and1,038 for both. The last mode agreed across the two
platforms but failed the original reference. None of those modes was accepted.

A separate handwritten Rust power approximation now follows the pinned runtime's
positive finite path, using mathematical tables, FMA, bit manipulation and
ties-away rounding. The narrow routine/constant observations were captured from
an agent-owned Python diagnostic process; the purchased application was not
attached or modified. The implementation makes no host power/log/exp calls and
contains no fixture/output lookup. Its API excludes negative bases, -0 and
nonpositive/nonfinite exponents; it is not a complete IEEE powf replacement.

Seven non-nearest regressions were observed failing with a higher-precision stub
and passing with the reconstructed routine. A larger synthetic comparison
matches22,729/22,729 native macOS results on both macOS ARM64 and Linux ARM64:
591 prior source-domain cases,90 boundaries,20,000 finite samples and2,048
subnormal samples. All8 tests pass on both platforms. This is sampled coverage.

Version3 combines that power routine with the measured double-logarithm-to-f32
path. It preserves all2,379,872 original cache values on both macOS ARM64 and
Linux ARM64, with zero bit differences. All17 tests and release builds pass on
both platforms; macOS formatting and Clippy also pass. Its composed request
matches all1,760 values on both platforms. The Linux bank worker receives only
freshly computed version3 caches and projected numeric beat context; its binary
is unchanged. Expected outputs stay outside both Linux workers. Host double
ln/cos and float exp2 remain in this implementation;
every possible floating input and Windows execution are not established.

Cache V4 subsequently replaces the promoted logarithm with the source-derived
positive-finite float32 logarithm used by the independent tuning stage. It keeps
all2,379,872 cache values exact on both platforms and passes23 tests. The final
three-stage chain now computes tuning, cache and request banks from retained
peaks plus beat context, matching all1,760 request feature values on both
platforms without original tuning/cache inputs. These later proofs preserve
the original V3 records below rather than rewriting their input scope.

## Retained identities and limits

The experiments and original-derived artifacts remain private under `.analysis`.
No production integration, fresh production benchmark, held-out evaluation,
classifier recovery or UI work was performed.

| Experiment | Frozen identity or report |
| --- | --- |
| Initial cache component | `356d21ad4714253cdfbdc008b7da27d9795d19eddbaad2b3be030d4cb13ff238` |
| Failed Linux v1 report | `0c56a5c6d45e8aff5799ae6774d14dccacb3f993357a64aa8ed965215fc53338` |
| Four-mode diagnostic | `6b68c09f1b0a93105b134609039af8bb7ad75681ee560705d35c788752d6d8db` |
| Failed Linux v2 report | `ed2fce5a3d76e06816dcc6088d39f42ee58bbff6ec2a79c2d1db896a0ba53db5` |
| Portable power component | `3ce8eacbf2d6ba16c207ebc092a606d6a6a7df1c5a1cd5c991416b9a77b6ceb7` |
| Private runtime witness | `f8660c237d57b5239fab5401a6c8f5e4c9086afeed8d8a4c40d54acf29874484` |
| Version3 cache component | `cce660b337bbb149866401cf357bc090dd478053bfd3299093e35ce32410a2b9` |
| Exact Linux version3 cache report | `73984cf456198c5fcad06f53eb7647b229c73c6621cdff5f4d99796d4b381235` |
| Exact Linux composed request report | `a4689d626cc9ef95eb33314ed801c121479823e65066bc9032306582e82e4293` |

These are component proofs on five development music cases and synthetic math
inputs. Perfect full-engine agreement remains unproven. The next source
boundary is producing the tracked peaks and finalized tuning from audio.
