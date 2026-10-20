# Mixed In Key's published patents, 2026-10-20

Public documents only, read through a fetch tool; check the sources before relying on any detail. Patents give the
structure of a method, not its constants, weights or trained models.

## Cue points: US 10,366,121 B2

[Google Patents](https://patents.google.com/patent/US10366121) ("Apparatus, method, and computer-readable medium for
cue point generation", Mixed in Key LLC, priority 2016-06-24, granted 2019-07-30; a continuation, US 11,354,355, was
published in 2022).

- A beat grid is built, and loudness is normalised first. One value per audio feature is derived **per beat** (the
  mean, geometric mean, RMS or standard deviation of an overlapping window per beat; RMS in the example).
- Features named: transient energy (the main one), melodic energy (frequencies above about 250 Hz), and others such
  as spectral centroid and flatness. An equal-loudness or A-weighting pre-filter is allowed.
- The **score at a position compares the values before it with the values after it**, in blocks of 16, then 8, then 4,
  then 2 beats, summed (these are the only numbers the claims give). The distance is a template, for example
  `D(x, y) = m * |x - y|` or `m * (x - y)^2` with `m = 1` when `x >= y` and `m = c` (`c < 1`, value not given) when
  `x < y`, which penalises drops against rises. Several features are combined as `S = f1*S1 + f2*S2 + ...` with weights
  that are tuned against ground-truth cue points and may be chosen by **genre**.
- Scores can be computed every 4th beat and refined around peaks. Candidates are local peaks that are not far below
  the largest; cue points must be **at least 32 beats apart**, positions may be nudged to multiples of 32, 16, 8 or 4
  beats, and at most **8** are kept (the DJ-software limit).
- No dedicated rule for the first cue, no score normalisation formula, no peak thresholds, and no energy level
  attached to cue points are given.

## Energy: US 8,865,993 B2

See [energy-literature-2026-10-20.md](energy-literature-2026-10-20.md): 14 frequency bands, 32-beat segments, transient
and ambient loudness, an attribute profile correlated with reference works of known energy (a support vector machine in
some embodiments), segment predictions combined by median, mean or 75th percentile. No constants or reference data.

## Key: US 7,842,878 B2 (listed as expired)

[Google Patents](https://patents.google.com/patent/US7842878) ("System and method for predicting musical keys from an
audio source", filed 2008, granted 2010).

- A 12-value note-strength (pitch-class) profile from a short-time Fourier transform, with tuning estimated by three
  parallel resonance-filter banks one semitone apart (after Lerch, ISMIR 2006), peaks shifted to the pitch domain,
  and octaves folded. No normalisation formula.
- Reference works carry a root key and a profile. Matching either correlates the input with the **average profile of
  N > 1 clusters per root key** (the highest-correlating cluster gives the key) or uses a naive Bayes classifier on
  categorised profiles. The correlation function and any confidence threshold are not specified.
- Mixed In Key has said it combined an external tuning-aware algorithm with its own; the patent is not the current
  server algorithm's full description.

## How this maps to the captured requests

The cue request carries one per-beat score array and the energy request one 183-value vector per 32-beat window, which
fits the patents' per-beat and per-segment features. The server's decisions (cue selection, window levels, overall
energy, key) are made from those inputs, so the unknown constants and trained parameters can be fitted from the
request and response pairs.
