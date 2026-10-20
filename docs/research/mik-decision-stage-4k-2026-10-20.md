# Mixed In Key decision stage on a 4,000-track capture, 2026-10-20

Aggregate findings only. The data (sanitized request/response pairs, stored analysis, tags, a copy of the database) is private and
stays under `.analysis/`. Code: `research/mik-decision-stage/`. Background: [request-response-mapping.md](request-response-mapping.md),
[mik-profile-times-2026-10-20.md](mik-profile-times-2026-10-20.md), [mixed-in-key-patents.md](mixed-in-key-patents.md).

## Data

A clean scan of 4,168 files in Mixed In Key 11.2.6 (build 7446) on a second Mac, saved as 4,168 analysis calls and joined to the
stored analysis (all 4,168 joined, stored key, energy, tempo and cues equal to the response in every case). 4,084 distinct songs
(equal 352-value key features are one song). 39 songs belong to the sealed held-out part of the first 434-file corpus and were dropped,
leaving **4,045**. The split is the fixed one in `mikdata.split_of`: **3,244 tuning** songs (76,304 energy windows) and **801 held-out**
songs (19,047 windows). 98% of the songs are melodic house and techno; 65 songs carry 16 other genre strings (15 of them in the
held-out part). 4,159 of the 4,168 files are FLAC; nine are MP3, M4A or WAV (not evaluated separately).

## Result on the 801 held-out songs, scored once after the configuration was frozen

The freeze record (configuration, SHA-256 of the scripts and of the data cache, git commit `b3bc7bc`) was written before anything was
fitted; the script refuses a second scoring. Failures are in every denominator. Standard errors are binomial for the song-level rows.

| Output | Exact | Within one level / wheel neighbour | Target |
| --- | ---: | ---: | ---: |
| Key (24 Camelot codes) | **96.8%** (±0.6) | 99.1% (same number, or one number apart on the same letter) | 99% exact |
| Window energy level (1-10) | **92.7%** | 97.8% | n/a |
| Overall energy (1-10) | **75.7%** (±1.5) | 99.9% | 95% exact |
| Overall energy, 80th percentile of predicted levels (what the desktop's opt-in rating uses today) | 71.8% | 99.9% | |
| Cues, tracks with every cue right, server's cue count given | **67.9%** (±1.7) | recall 93.0% | 90% |
| Cues, tracks with every cue right, eight cues fixed (no response information) | 56.3% | recall 93.3% | |

Inside the key row: 100% when the server's own confidence is at least 0.97; 65% for the 72 songs below 0.7; 67% for the 24 major keys
(the models see few of them). Melodic house and techno 96.8% (786 songs), other genres 93.3% (15 songs). 84.8% of the held-out tracks
have eight cues, which caps the fixed-count row.

None of the three targets is met. The distance is in the near-ties, not in the clear cases.

## What the tuning split (out of fold, grouped by song) says about each step

| Step | Result |
| --- | --- |
| Key: tonic-rotation-shared logistic model (previous best family) | 94.6%; plain logistic on all 352 features 94.2% |
| Key: RBF SVC on the square root of the features | 95.2% (normalisations and per-bank scaling make no difference) |
| Key: the same SVC trained with every song shifted by every semitone, labels shifted along | 96.7% (12% of the songs 94.7%, 25% 95.5%, 50% 96.5%, 100% 96.7%) |
| Key: pitch-shared network with two convolutions along the pitch axis, five seeds | **97.1%** (a single seed 96.5-96.9%, a network on the whole profile 96.6%) |
| Window level: RBF SVC C=10, gamma=0.1 on 61k training windows | **92.0%** exact, 97.7% within one (285 songs gave 85.3%; C 10-30 and gamma 0.1 stay best) |
| Overall: 80th percentile of the predicted levels | 71.0% |
| Overall: multinomial logistic regression on level shares, quantiles and window volume | **77.2%** |
| Overall: boosted trees on the true levels and volume (an upper bound for any function of the response's own levels) | 77.8% |
| Cues: ranker with 19 features | 47.4% |
| Cues: list-wise ranker with the 32/64/128/256-beat hierarchy of the origin, global rank and block sums | **65.1%** (the 121 songs whose cue start is not beat 32 reach only 21%) |

The learning curves are flat for the key and slowly rising for the window level (about two points per doubling of the songs), so
more songs alone do not close the gap.

## Ruled out

* **Genre, title and artist do not change the response.** Controlled test: 24 tracks, each scanned as 12 copies of byte-identical audio
  (ten different genre tags, no genre, and the original genre with neutral title and artist). All 288 requests were identical outside the
  tags and all 288 responses identical in every field except the three timing values, the key confidence bit for bit; the tracks
  spanned key confidence 0.28 to 0.998. 84 further duplicate recordings with different file names also got identical outputs.
* Other inputs for the key: tempo and duration add nothing (95.4% with and without) and the mean energy-window features make it worse (94.6%). The request's
  `PitchProbabilities` is zero in all 4,045 songs and the stored tuning pitch is 440.0 for every song.
* A patent-style pipeline (fold octaves, then correlate) is worse than the unfolded models: 83-93% by bank.
* A nearest-reference method with transposition (correlate with every other song at every semitone shift, score a key by its best
  match) reaches 92.7% and its margin tracks the server confidence worse than the classifiers do.
* Every one of the 183 window columns' families and bands is used: dropping any one quantity family or band changes the window level by at most 0.1 point or loses up to 15 points, never gains more.
* Overall energy: tempo, duration, key, confidence and genre group add nothing to the true levels; soft (probability) window scores add
  nothing over hard levels; learning from the song's rating at window level, or from song-level means of the raw window features, is worse
  (62-70%). 2.2% of the 3.2 million song pairs in which one song stochastically dominates another on level have the lower rating, always
  by one: the rating is almost, not exactly, monotone in the level distribution.
* Cues: neither section structure (level changes at the boundary, run lengths) nor the energy-window feature changes help, although
  62% of true cues sit where the window level changes against 43% of all 32-beat boundaries; a learned template over the raw scores
  (45%) does no better than the gradient-boosted ranker.

## What the timings add

Replicated on 3,244 songs: the cue stage costs 5.8 us per beat (linear, no quadratic term), the energy stage 0.89 ms per window, one key
decision costs as much as 18 energy windows, and none of it depends on the genre. New: the key stage costs about **1.4 ms (5-9%) more for
low-confidence songs** (median 18.0 ms below confidence 0.5 against 16.5 ms at 0.97 and above, p = 4e-5; the same shift shows in the per-track
minima of the 24 repeated tracks, Spearman -0.40). A small extra step on near-ties is consistent with the errors, which sit almost entirely
there: on the tuning split the server's key is among my first two choices for 99.5% of the songs (out of fold), and 5% of the songs (top probability under 0.9) carry
nearly all the loss.

## Reading

* The key is a smooth, shift-invariant function of the 352 features for 95% of the songs and a knife-edge for the rest. The convolutional
  family is the best so far, and its cost profile fits the fixed 16 ms. More songs in the near-tie region, and in particular more major
  keys, are the lever; width and depth of the network are not.
* The window level is a multi-class RBF SVM (the client's `OutputToSvm`); with 6-12 times more windows the fit rose from 85% to 92%. The
  exact function (and the hidden aggregator for the overall rating) cannot be recovered at this rate.
* The cue selection is not a function of the local scores alone, nor of the energy stage. What is left to find sits in how the eight
  cues are chosen as a set.

## Limits

One library, one machine, one Mixed In Key build; almost only FLAC (the client features were verified from audio only for 44.1 kHz stereo 16-bit FLAC). The held-out songs come from the same library as the tuning songs, so near-versions of a track can sit on both sides. Several
descriptive statistics (duplicates, timings, monotonicity, bank structure) were computed over all 4,045 songs; no model was fitted or
chosen on the held-out part. 15 held-out songs are not melodic house and techno, so the other genres are not evaluated.
