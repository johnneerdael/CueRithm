# Equal-input comparison with Lexicon and Mixed In Key tags, 2026-10-20

Aggregate results only. Per-track detail stays under `.analysis/`.

## Method

- **Songs.** The 434 files of the Beatport development folder are 364 distinct songs (70 files are exact
  duplicates by FLAC audio checksum). One file per song is counted. The reference corpus fixes the split by a hash
  of the audio group: 282 songs in the tuning split, 82 held out. **Every number below is the tuning split
  (n = 282).** The held-out split stays sealed until the engine and configuration are frozen.
- **Disclosure.** A first pass over all 364 songs, including the held-out ones, printed only blended totals. Those
  reports are quarantined and not used. The comparison script now refuses to read the held-out split.
- **Inputs.** A clone of the audio with every tag removed except title, artist and remixer: no BPM, key, energy, cue,
  beatgrid, genre, release date or artwork. The audio checksums equal the originals'. Neither tool received a genre.
- **Lexicon 1.11.15.** Own analysis with tempo range 78–155, key and energy analysis on, and the Cue Point Generator
  with the Default template (overwrite on, start at first beat, breakdown minimum 64 beats, drop at start never,
  no custom anchors, no emergency loop). Read from a read-only copy of its database.
- **CueRithm.** `camelot analyze --lexicon-timing --lexicon-tempo 78_155 --lexicon-cues --lexicon-energy` for timing,
  cues and energy, and `camelot analyze --key-openkeyscan-model models/builtin/openkeyscan3-logits.onnx` (built with
  the `openkeyscan-key` feature) for key, which is the model the desktop uses. A first key pass used the CLI's fallback
  estimator by mistake and reported 35.8% against MIK; that figure was wrong and is withdrawn.
- **MIK column.** The base64-JSON tags that Mixed In Key wrote into the original files (`source: mixedinkey`).
  This is not a fresh MIK run, and the MIK version that wrote them is not established.
- Identical audio gives identical results: no disagreement across the 42 songs with more than one copy, in either tool.

## Results (tuning split, n = 282)

| Measure | CueRithm vs Lexicon | CueRithm vs MIK tags | Lexicon vs MIK tags |
| --- | ---: | ---: | ---: |
| BPM within 0.001 | 282/282 (100%) | 239/282 (84.8%) | 239/282 (84.8%) |
| First beat within 1 ms | 282/282 (100%) | 44/282 (15.6%) | 44/282 (15.6%) |
| First beat within 5 ms | 282/282 (100%) | 154/282 (54.6%) | 154/282 (54.6%) |
| Published cue set within 5 ms | 281/282 (99.6%) | — | — |
| Grid and published cues both agree | 281/282 (99.6%) | — | — |
| Key exact | 213/282 (75.5%) | 274/282 (97.2%) | 213/282 (75.5%) |
| Key adjacent on the wheel | 23/282 (8.2%) | 6/282 (2.1%) | 24/282 (8.5%) |
| Energy exact | 54/282 (19.1%) | 128/282 (45.4%) | 102/282 (36.2%) |
| Energy within one level | 243/282 (86.2%) | 258/282 (91.5%) | 237/282 (84.0%) |

The energy tags in the original files changed on 54 of the 282 songs after the first version of this comparison (the
key and beat-grid tags did not change). Probably Mixed In Key rewrote them when the cue points were exported to
Serato; that is inferred, not observed. The first version reported 64.5% exact (94.3% within one) for CueRithm and
30.1% (83.7%) for Lexicon against the earlier tags; the table above uses the tags as they stand now, which equal the
values the Mixed In Key server returns. CueRithm's own energy results did not change between the two runs.

Lexicon stores keys in Open Key notation (`11M` minor, `11D` major); the mapping to Camelot was checked against the
MIK tags (`9M` is `4A`, `1M` is `8A`, `6M` is `1A`). MIK cues are phrase positions named "Energy N", not structural
cues, so cue comparison with MIK is not meaningful.

## What the numbers say

- **Timing and cues.** CueRithm reproduces Lexicon's BPM and first beat on every song (282/282, the largest
  difference is 0 ms) and its published cue set on 281/282 (99.6%). Cues here are the published set, after the
  template rule that Lexicon's renderer applies and `lexicon_publication.rs` ports (a Second Drop or Second Breakdown
  exists only if its first one exists and differs in time). Read as raw detector output, 7 songs differ; six of those
  are that rule and are not differences in the app.
- **What used to differ in seven first beats.** Lexicon hands its analysis worker the first channel of what
  Chromium's `decodeAudioData` returns. For 16-bit integer sources Chromium converts to float with an uneven scale:
  positive samples times 1/32767, negative samples times 1/32768. FFmpeg and Symphonia divide every sample by 32768.
  The gap is under 3e-5 per sample, but it was enough to move the first beat the tempo and phase search picked on
  7 of 282 songs. How this was found: Lexicon's original worker, run in Node on our samples, returned CueRithm's
  value on those songs, so the port was faithful and the input had to differ. Stock Electron 32.3.3 (the version
  Lexicon ships) decoding the same files gave samples that differ from FFmpeg's. The worker fed those samples
  returned Lexicon's stored tempo and first beat on both songs tried. The uneven scale then reproduced Chromium's
  samples bit for bit on both (1,692,168 and 11,571,438 samples). `audio.rs` now applies it to the timing and cue
  input for 16-bit integer sources (FLAC, ALAC, WAV, AIFF). All seven songs now match Lexicon exactly. Two further
  songs whose first beat was already within 1 ms moved onto Lexicon's exact value, and no other result changed.
  Not covered: lossy codecs decode to float in Chromium, so nothing changes for them; sources that are not 44.1 kHz
  are resampled by Chromium's own resampler, which is not ported (the whole development folder is 44.1 kHz and
  16-bit, so this is unmeasured); 24-bit sources scale the same to within float precision.
- **Other tempo presets.** The whole-split comparison above is at the 78–155 preset only. For five presets (58–115,
  68–135, 78–155, 88–175, 98–195) CueRithm was compared with Lexicon's shipped analysis worker, run in Node on the
  same Chromium-scaled samples, on 20 songs (the seven that used to differ plus 13 drawn at random with a fixed
  seed): BPM and first beat agree in 100 of 100 runs, and the presets are not trivial on these songs (in 21 of the 80
  runs outside 78–155 the tempo differs from the 78–155 result and in 16 the first beat does). This checks the port
  against the worker, not against the app's stored results at those presets; the app's preset selector was not driven,
  so the app-level comparison at the other presets, and the 50–300, 128–255 and 180–300 presets, are not measured.
- **The one remaining song.** One song's Last beat is one 16-beat grid step earlier in Lexicon (350.5 s against
  358.1 s). Its length is exactly 753 beats, so the beat count `floor((duration - first beat) / beat length)` is
  sensitive to the last sample: with that sample removed CueRithm returns Lexicon's value. Removing it from every
  song does not work, because it breaks two others that currently agree, so the app's buffer length depends on
  something per song that is not identified. Chromium reports the file's full length for this song, both for
  `decodeAudioData` and for the media element. Nothing was changed for it.
- **Energy.** CueRithm's rating in this run is Lexicon's published local formula. Against the current Mixed In Key
  tags it matches 45.4% exactly (91.5% within one level), a little above Lexicon's own rating (36.2%, 84.0%), and it
  does not reproduce Lexicon's rating on real music (19.1%). Probed on 12 songs, Lexicon feeds the formula the left
  channel decimated to 22.05 kHz without a low-pass filter, which reproduced its values on all 12; CueRithm averages
  the channels and low-passes first. That makes the formula score one level lower on most tracks. The window
  classifier with the 80th-percentile rule (`cuerithm-svc`, not in the app) matches the current tags on 79.8% (99.3%
  within one level) of the same 282 songs; it was fit on the earlier capture's labels, so part of that is in-sample,
  and the out-of-fold figure is the 74.8% given below.
- **MIK energy.** MIK's tag rating is a local aggregation of segment levels that its server returns, so a local
  rating can only imitate it. The earlier replacement of the SVC by the Lexicon formula (2026-10-18) was made for
  independence from a model file and a server and was not compared with MIK.
- **Key.** OpenKeyScan agrees with MIK's tags on 97.2% (274/282), matching the earlier development measurement of
  274/282, and agrees with Lexicon's own key on 75.5%. Lexicon's key is itself 75.5% against MIK, so CueRithm is
  much closer to MIK than Lexicon is.

## Energy recalibration toward MIK, tuning split only

Question: does refitting the energy formula's four features (crest factor, spectral flatness, high-mid ratio,
sub-bass ratio, taken from the production decode path) to MIK's overall level improve agreement? Five-fold
cross-validation over the 282 songs, folds assigned by a hash of the song, MIK tag level as the target:

| Method | Exact | Within one level |
| --- | ---: | ---: |
| Current formula (no fitting) | 64.5% | 94.3% |
| Always predict 6 (the most common MIK level) | 53.9% | — |
| Linear refit of the four features | 59.6% | 97.2% |
| Multinomial logistic regression | 58.2% | 96.8% |
| Random forest | 56.0% | 97.9% |
| Gradient boosting | 52.5% | 95.4% |

MIK rates 94% of these songs 5, 6 or 7, so exact agreement has a low ceiling. Refitting trades exact agreement for
within-one agreement and does not beat the unfitted formula on exact agreement, so these four features, not the
amount of data, are the limit. A closer imitation would need MIK-style segment features with a segment classifier
trained on captured server labels and then MIK's aggregation; the later window classifier with the 80th-percentile
rule did that (see the next section). The percentages in this table were measured against the energy tags as they
stood before they changed on 54 songs (see the note under the results table); against the current tags the unfitted
formula scores 45.4% exact and 91.5% within one level. The comparison between the methods was not re-run on the
current tags. Nothing here uses the held-out split.

## Energy against Mixed In Key's server contract, tuning split only

Measured on the 282 development request and response pairs captured earlier from the licensed app, and on the
tuning-split files. Aggregates only.

- **The tag is the server's overall value.** The `OverallEnergy` field of the analysis response equals the energy tag
  Mixed In Key writes on every one of the 255 songs whose request windows were securely matched to a file (255/255).
  The client's duration-weighted mode over the response's segments equals the tag on only 58.7% of the 126 songs
  matched by duration, so that rule is not what produces the tag.
- **The overall value is not a function of the response's segment levels.** On the 282 responses it never exceeds the
  highest segment level or falls below the lowest, but the best rule found (a duration-weighted 80th percentile of the
  segment levels) reproduces it on only 80.1%; no rule tried reached 100%. It is therefore computed from something
  the response does not expose, such as per-window scores before they are rounded and merged. The server answers in
  roughly 4 to 20 ms for cues, key and energy, which is the speed of a calculation, not of a large model.
- **Our request windows match Mixed In Key's.** A fresh capture of the licensed app (2026-10-20, 434 analysis
  transactions, all HTTP 200) shows the 183-value energy windows computed from the files are bit-identical to the
  captured request vectors on all 272 tuning-split songs (100%). Against the earlier capture they matched on 247 of
  272 (90.8%), and the 25 other songs are ones where the two Mixed In Key captures differ from each other (new equals
  old on the same 247), so the earlier "27 unexplained songs" were a difference between captures, not in our code.
  Exact duplicate audio gives identical requests and identical responses in all 45 tuning-split pairs, so Mixed In
  Key's pipeline is deterministic in the audio. The earlier training labels were therefore wrong on those 25 songs.
- **Window classifier plus an 80th-percentile rule.** The shipped SVC with the 80th percentile of window levels
  (`EnergyModel::estimate_overall`, method `svc-percentile80-v1`) matches the server value on 78.4% (99.2% within one
  level) of the 255 matched songs, in-sample, and on 74.8% (99.6%) out of fold on the 282 development songs. The
  Lexicon formula gets 46.3% (91.0%) on the same 255 songs. On the whole tuning split, scored against the file tags
  as they stood when this was first written, both methods matched 64.5% exactly; against the current tags (which
  equal the server values) the 80th-percentile route matches 79.8% (99.3% within one level, partly in-sample) and the
  formula 45.4% (91.5%). The desktop still uses the acoustic formula; switching is a product decision, because the
  window route needs the 10 MB model file.
- **A direct whole-track model does worse.** Ridge, logistic, SVM and random-forest models on per-track summaries of
  the window features reached 63 to 66% exact, below the window route.

## Capturing Mixed In Key's analysis traffic

Run `mitmweb --mode local:"Mixed In Key 11"` with the sanitising add-on, naming the process and never its PID (a
PID breaks when the app relaunches, and an attach to a stale PID records nothing). The analysis starts paused and
only began after Mixed In Key was restarted. Scope the capture to the analysis endpoint, redact credentials and
license values, keep the output private, and stop mitmweb when finished.

## Refit on the fresh capture, tuning split only

The window SVC (C=10, gamma=0.1), refit out of fold by song on the 272 clean songs (5,956 windows): window level
85.4% exact and 96.5% within one; overall level as the 80th percentile of predicted window levels 75.0% exact and
99.6% within one (70th: 72.4%, 75th: 73.5%, 85th: 72.1%; always predicting 6: 51.5%). Cleaner labels did not move
the ceiling, so the missing piece is the server's rule for the overall value, not the window classifier.

## What Mixed In Key's window energy level depends on (fresh capture, tuning split, 272 songs)

Aggregates only; five-fold cross-validation by song.

- **Inputs.** Adding genre, tempo, duration, downbeat, per-window cue-score statistics or key features to the 184
  window numbers (183 features and the window's volume) did not change window accuracy (76.9% to at most 77.2%
  with gradient boosting), so the level depends on the window vector, not those fields.
- **It is a trained kernel model, not a compact formula.** An RBF SVM on the raw feature scale reaches 84 to 85% of
  windows exactly; gradient boosting reaches 77%, the same SVM after standardising 72%, a small neural network 70%
  and kNN 62%. No single statistic (best 63%) or frequency band (best 58%) carries the level, dropping any one
  statistic costs at most 6 points, and the best 80 features reach about 80% against 84.4% for all 183. One feature
  (index 171, the top band) is dominant: shuffling it alone drops the SVM from 84.6% to 52.4%.
- **More labelled songs help, with diminishing returns.** Window accuracy by training songs: 21 songs 71.1%, 54
  songs 77.3%, 108 songs 81.4%, 162 songs 83.3%, 217 songs 84.4%. Overall energy (80th percentile of window levels)
  moves from 70% to 74% and is noisy. SVM settings `C` 10 to 100 and `gamma` 0.1 to 0.3 are equivalent; `gamma`
  above 0.5 is worse.
- **Genre.** 227 of the 272 songs (83%) are melodic house and techno, and the rest are 20 house, 13 indie dance, 8
  progressive house and a few others. There are no trance, hardstyle or plain techno songs, so genre effects on
  those cannot be fitted or checked from this capture.
- **Cues.** A literal implementation of the cue patent's scoring (compare per-beat values 16, 8, 4 and 2 beats apart,
  asymmetric penalty, peaks at least 32 beats apart) recovers 22% of Mixed In Key's cues, against 95.7% for the
  earlier reverse-engineered rule, which scores every cue within 50 ms for 62% of whole tracks.

## Mixed In Key cue points, fresh capture, tuning split (272 songs)

Rule under test: the earlier reverse-engineered cue rule (per-beat scores from the cue request, a 32-beat phase search,
bar-position weights, greedy peaks at least 32 beats apart, the downbeat as the first cue). Whole-track correct means
every cue within 50 ms and none extra or missing.

- **Baseline.** 169 of 272 tracks (62.1%) are exactly right; 95.7% of individual cues are matched. Of the 103 wrong
  tracks, 63 differ by one cue (the rule picks a different cue for one slot), 33 get too many cues (Mixed In Key
  returned fewer than 8 on about 12% of songs and the rule always picks 8), 1 gets too few and the rest differ in
  position.
- **The swapped cue is not a near-tie.** In all 63 one-swap tracks, Mixed In Key's cue sits on a position the rule
  considers, so the phase is right, but the rule ranks that position 19th of the candidates (median).
- **No clean stopping threshold.** Among tracks where the rule's first picks already match, a peak is kept by Mixed In
  Key at as little as 14% of the strongest peak and dropped at as much as 42%, so the early stop is not a threshold on
  this score.
- **Tuning the selection constants does not help.** Fitting the weights, ramp width and exponent, margin and
  separation on 60 training tracks gained 3 tracks there and lost 7 on the other 202 (125 against 132 right), and moved
  the 10 test tracks from 5 to 6: overfitting.
- **A learned candidate ranker is worse.** LightGBM ranking over the rule's own candidates, with the rule's score,
  before-and-after block differences at 16, 8, 4 and 2 beats, local prominence and bar position, reaches 52.6% (5/10 on
  the test tracks); adding Mixed In Key's true energy levels as an upper bound gives 51.1%, so energy changes do not
  explain the swapped cues.
- **Genre.** By the exact genre string Mixed In Key receives, plain "Melodic House & Techno" (202 songs) is 60% right
  and the suffixed variant (24 songs) 75%; the other groups hold 8 songs or fewer. The failures are within the
  majority genre, so genre weights are not the main missing piece for it.
- **What separates the swapped cue.** In the 63 one-swap tracks the rule's wrong pick beats Mixed In Key's cue on the raw
  per-beat score in 47 (by 12% on average); the ramp and the bar-position weight rarely decide it (equal in 60 and 43
  tracks). Both cues usually sit on the same 32-beat phrase grid (46 and 56 of 63 at phrase position 0), but Mixed In
  Key's cue is earlier in the track (median 58% of the way through) than the rule's pick (72%). A single exponential
  decay with position nevertheless does not help: zero decay is best on the training sample, the unseen tracks and the
  test tracks, and every other value loses tracks (for example 158 and 152 of 272 at decay 0.1 and 0.2, against 169).
  The rule is at a local optimum for simple changes; the missing signal has not been found.

## The overall-energy rule, fresh capture, tuning split (272 songs)

- **No exact rule from the rounded levels.** Scored against the server's `OverallEnergy` (equal to the file tag), the
  best rules over the response's own rounded window levels reproduce it on 81.2% (mean of the top 40% of window
  levels), 80.5% (80th percentile) and 79.4% (85th percentile); rules over the 8 cue energies reach at most 75.7%,
  and always predicting 6 gets 51.5%. The overall value is therefore not a function of the rounded levels the
  response returns, which fits a calculation on continuous per-window scores before rounding.
- **Predicted levels or predicted continuous scores.** Five-fold by song, the 80th percentile of an RBF SVC's
  predicted window levels gives 73.9% exact; the same percentile of an RBF SVR's continuous predictions (C 10, gamma
  0.2, epsilon 0.25) gives 76.8%, 75th gives 76.1%, and the mean of the top 40% gives 74.6% (levels) and 72.8%
  (scores). The best achievable from predicted windows is still below the 81% that true levels allow, and the gain from
  using continuous scores is small (about 3 points).
