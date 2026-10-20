# Energy estimation: published methods, 2026-10-20

A web search for published ways to rate how energetic a track is. Only public documents were read. Summaries of
patent and web pages were produced by a fetch tool and should be checked against the sources before being relied on.

## What was found

- **Mixed In Key's own patent, US 8,865,993** ([Google Patents](https://patents.google.com/patent/US8865993);
  assignee Mixed in Key LLC, filed 2013, granted 2014). It describes the method that Mixed In Key's energy level is
  built on:
  - Tempo is estimated, and for 14 frequency bands it measures the loudness of transients on and off quarter-beat
    positions, the number of prominent transients per 32-beat segment, the loudness of slowly changing ambient
    sound, and how often a prominent transient recurs 1/4, 1/2 and 1 beat later. Input is an STFT with a 2048-sample
    Hann window and a hop of 682 samples at 44.1 kHz, a hearing-curve weighting, volume normalisation and median
    filtering to split transient from ambient sound. Attribute values are summarised with percentiles.
  - The track may be split into sections, each predicted on its own and combined by median, mean or the 75th
    percentile (the last to ignore quiet intros, outros and breakdowns).
  - The value is not a formula. The input's attribute profile is correlated with reference works of known energy,
    and in some embodiments the model is a support vector machine. Reference values come from a trained ear, the
    composer or a neural engine. No accuracy figures are given.
  This agrees with the earlier reverse engineering of the client: 14 bands, 32-beat segments and 183 reduced
  features per segment. It supports an SVM trained on segment features as the family of method, but it does not
  give the classifier, its training data or its calibration.
- **Mixed In Key's help page** ([energy level detection](https://mixedinkey.com/workflows/use-energy-level-detection/))
  describes a 1–10 rating of intensity that is not just tempo, and suggests mixing songs within one level of each
  other. It gives no method.
- **Spotify's energy** ([third-party summary of the documentation](https://help.spotontrack.com/article/what-do-the-audio-features-mean))
  is a 0–1 perceived intensity built from dynamic range, perceived loudness, timbre, onset rate and general entropy.
  No formula is published.
- **Drop detection** (Yadati et al., ISMIR 2014, as cited by the
  [drop-detector project](https://github.com/felixmeyer6/drop-detector)) targets the high-energy payoff of dance
  music, not an overall rating.
- **Music emotion arousal** is the academic neighbour of energy. DEAM (1,802 songs with arousal and valence at 2 Hz,
  used for the MediaEval Emotion in Music task 2013–2015) is the common benchmark, with spectrogram CNN and GRU
  models ([example](https://arxiv.org/pdf/2008.02194)). These measure emotional arousal, not a DJ's energy level,
  and no result there transfers directly.
- **Raveform** ([dataset](https://mir-aidj.github.io/raveform/)) has functional structure labels (intro, buildup,
  breakdown, drop) but no energy labels. Two student theses on classifying energy levels
  ([Lund](https://lup.lub.lu.se/student-papers/record/8971762), [LUISS](https://tesi.luiss.it/37423)) were found
  and not read.

## What it means here

No public dataset labels DJ-style 1–10 energy, and no peer-reviewed method reproduces Mixed In Key's rating. The
only grounded route to MIK-like energy is the one the patent and the reverse engineering both point to: segment
features that we already reproduce exactly, a segment classifier trained on captured Mixed In Key labels, and a
combination of segment predictions. The earlier SVC followed that route and reached 58.5% on the tuning split, below
the unfitted Lexicon formula's 64.5%, so the open questions are the classifier and the combination rule (median,
mean, 75th percentile, or Mixed In Key's client-side duration-weighted mode), not the amount of data.
