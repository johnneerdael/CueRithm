# Task: reproduce Mixed In Key's key, overall energy and cue decisions as closely as possible

You are continuing research that was started in another session. Read this whole file first. It is written so that a fresh
session on any machine can pick the work up from the repository (branch `feat/release-gaps` or its merge) plus a dataset
of request/response pairs ([DATA_FORMAT.md](DATA_FORMAT.md)).

## Goal

The owner wants **near-perfect alignment with Mixed In Key 11's results for the cue points, the key and the overall energy
rating** of a track (Mixed In Key is considered the better product for key and energy, and the user prefers its cues). BPM and
beat grid are already good enough (see below). Do not chase Lexicon's energy value. Melodic house and techno is the main
genre (87% of the first corpus, 98% of the second); the genre tag is not an input of the service (see the genre section).

Suggested success bar, measured once on sealed data and reported as exact agreement and as within one level separately:

| Output | Today (801 held-out songs, scored once; tuning out of fold in brackets) | Target |
| --- | --- | --- |
| Key (24 Camelot codes) | 96.8% (97.1%); 100% when the server's confidence is at least 0.97, 65% below 0.7 | at least 99% |
| Overall energy (1-10) | 75.7% exact (77.2%), 99.9% within one; window level 92.7% (92.0%) | at least 95% exact |
| Cues (every one of the eight right) | 67.9% of tracks with the server's cue count given, 56.3% with eight fixed (65.1% tuning); cue recall 93% | at least 90% of tracks |

## How the service works (established)

The Mixed In Key client computes everything locally, sends the result to a vendor service, and the service returns the
decisions. The response holds no beat grid: tempo, downbeat and cue scores come from the request, so they are computed
fully locally by the client.

| Request field | Meaning | Reproduced from audio by this repo? |
| --- | --- | --- |
| `Tempo`, `DownbeatTime`, `CuePointStartBeat` | the client's beat grid and where cue scoring starts | yes, exactly |
| `CuePointData` | base64, per-beat scores (little-endian float32 with a zero-prefix and suffix count) | yes, exactly |
| `Segments[0].Features` | 352 key features: 4 banks of 88 piano-bin values (bin j is pitch class (j+9)%12) | yes, exactly |
| `EnergySegmentData[]` | one window per 32 beats, each with 183 features and `VolumeRmsDb` | yes, exactly |
| `EliteData` | tags the application sent (genre, file name, tempo...) | n/a (metadata, never used by the client DSP) |

Response fields and how well they are understood:

| Response field | Understood | Open |
| --- | --- | --- |
| `EnergyLevelSegments[].Volume` | exactly the `VolumeRmsDb` of the segment's first window | nothing |
| segment bounds | runs of equal window level; neighbours never share a level; bounds are raw window bounds | nothing |
| window level (1-9) | a function of the window's 183 features and nothing else we could find (neighbours, position and track statistics made it worse); best model an RBF SVC on the raw features, 85.3% exact, 96.6% within one; errors are +-1 and concentrate at the extremes and next to level changes | the exact function |
| `OverallEnergy` | **not a function of the response's own window levels**: with the TRUE levels as input the best rule or learned model reaches only about 80.5% (80th percentile, mean of the top 40%, logistic on the level histogram plus volumes; gradient boosting 75%; `overall_experiments.py`), so a better window classifier cannot get past it; the 80th percentile of predicted levels gives 76.5% | the service uses something finer than rounded levels (continuous scores) or another input |
| cue time from a beat | exact (`time_at_beat` in `mikdata.py`) | nothing |
| first cue | always the origin beat (`CuePointStartBeat - 32`) | nothing |
| cue selection | greedy-like peak picking with at least 32 beats between cues, at most eight; 87.5% of gaps are multiples of 32; 38% of tracks have one swapped cue (MIK's cue can be ranked about 19th by raw score) | the rule |
| cue `EnergyLevel` | median of the window levels between this cue and the next (about 75% by rule) | exact rule |
| `Key`, `MainKeyConfidence`, `SecondKey` | key is nearly linear in the tonic-aligned features; `SecondKey` was never present in 434 pairs | exact classifier, confidence |

Server latency is 1-38 ms, which fits a small classifier rather than a large model. Identical audio gives identical
requests and identical responses (45 of 45 duplicate pairs), so the response is a deterministic function of the request.

### Stage timings (established, small)

Responses carry the service's own per-stage times. On 218 tuning songs: key is a fixed 16.5 ms whatever the song (no early
exit), energy 0.90 ms per window with no per-song stage, cue points 5.65 us per beat and linear (no pairwise search); costs
are equal across genres. Read: dense fixed-size models, about 10^6 operations per window and 10^7 per key decision, far above a
linear model or a tree ensemble. Details and caveats: `profile_time_experiments.py` and `docs/research/mik-profile-times-2026-10-20.md`.
Rerun it on the new set: a per-genre cost difference would be the first direct sign of genre-specific models.

### Client reproduction at scale

`crates/camelot-source-analysis` (`cargo run --release -p camelot-source-analysis --example analyze_features -- file.flac`)
rebuilds the whole request from a 44.1 kHz stereo 16-bit FLAC. Against the captured requests it matched **all seven
fields (tempo, downbeat, cue start, cue scores, 352 key features, duration, 183-value energy windows) on 272 of 272 tuning
songs, with no exceptions**, about 10 s per song. For other formats the client's own decoder would have to be matched.
So the decision stage is the only unknown, and it can be fitted from request/response pairs without audio.

### Patents (Mixed in Key LLC)

Read `docs/research/mixed-in-key-patents.md`. Tested so far:

- US 7,842,878 (key): folded 12-class profile, clusters of average profiles per root key or naive Bayes. Fitted to our folded
  banks these reach 77-89% on the tuning songs, below a plain linear model on the 352 features (92.7%) and far below a
  tonic-rotation-shared linear model on the unfolded bins (96.0%): the current service does not fold octaves first.
- US 8,865,993 (energy): 14 bands, 32-beat windows (they match: 15.6 s at 123 BPM), a profile correlated with reference
  works of known energy (an SVM in some embodiments), window predictions combined by median, mean or 75th percentile.
  The RBF SVC fits that shape; the aggregator is still open (80th percentile is the best simple rule).
- US 10,366,121 (cues): per-beat features, score = values before against after in blocks of 16/8/4/2 beats with an asymmetric
  distance, weights tuned and possibly chosen by genre, cues at least 32 beats apart, at most eight. The client's
  per-beat scores are genre independent (reproduced exactly without any genre input); a genre effect, if any, is in the
  server's selection or weights.

## Genre: what the data says so far

**Result of the controlled test (2026-10-20):** 24 tracks, each scanned as 12 copies with byte-identical audio: ten different genre tags, no genre, and the original genre with neutral title and artist. All 288 requests were identical outside the tags and all 288 responses were identical in every field except the three timing values, the key confidence bit for bit (tracks spanned key confidence 0.28 to 0.998). The genre tag, title and artist do not influence the key, energy or cues; the decisions are a function of the numeric request fields (`GENRE-EXPERIMENT.md`, `genre_experiment.py`). Pool all genres when fitting; a genre input would add nothing. What the audio itself says about genre is a separate question the request cannot answer.

On 272 tuning songs (227 melodic house and techno, 45 other; one song, one vote):

| Output | MHT | other genres |
| --- | --- | --- |
| Key agreement | 96.0% | 88.9% (progressive house 75% of 8) |
| Window level agreement | 84.9% | 87.3% |
| Overall energy agreement | 75.8% | 80.0% |
| Every cue right (baseline ranker, 48.9% overall) | 50.7% | 40.0% |

Most mismatches are inside melodic house and techno (9 of 14 key mismatches, almost all energy mismatches), so genre
alone does not explain them. Key mismatches concentrate where the server is itself unsure: agreement is 100% when its
confidence is at least 0.97, 98% from 0.9, 91% from 0.7 and 58% below 0.7; the confusions are neighbours on the wheel and
parallel major/minor (the corpus has only 11 major keys). The genre-chosen-weights idea from the cue patent is ruled out for the
tag the application sends by the controlled test above.

## Data

- First corpus (private, not in the repo): 434 FLAC files, 364 distinct songs, 434 captured pairs; 282 tuning groups,
  82 groups sealed. `sealed_corpus_holdout_feature_sha1.txt` lists the sealed songs by feature hash; never train, tune or
  look at results on them until the final scoring.
- Second set (arrived 2026-10-20 as a hand-back package, verified with `build_handback.py --verify`): 4,168 analysis calls from a clean scan of a second library, 4,084 distinct songs, 4,045 after dropping the 39 sealed first-corpus songs. 3,244 are tuning songs and 801 are the **held-out part, which was scored once on 2026-10-20** (freeze record in `.analysis`, results in `docs/research/mik-decision-stage-4k-2026-10-20.md`) and must not be used to choose anything any more. A new claim needs a new held-out set: the sealed songs of the first corpus (only with the owner's word) or a further capture. The package holds the stored analysis with decoded beat grids, every tag, a file inventory and a copy of the database; the library is 98% melodic house and techno.
- Genre is not an input (controlled test above): fit all songs together. Per-genre agreement is still worth reporting as a check on the data, but there is nothing to tune per genre.
- Learning curve for the window classifier on the first corpus: 21 songs 71.1%, 54 songs 77.3%, 108 songs 81.4%,
  162 songs 83.3%, 217 songs 84.4%: still rising by about 3 points per doubling. Sixteen times more songs is the largest
  lever there is.

## Protocol (do not break these)

1. Split by song, never by window or cue. `mikdata.split_of` holds out one song in five by hash; additionally drop every song
   in the sealed list. Tune and select on the tuning part only; score the held-out part once after the configuration is
   frozen and written down, and report exact agreement and within-one agreement separately, with failures and unknowns in
   the denominator.
2. Compute predictions from the request fields only. Never use tags, file names or the response itself as an input to a
   prediction (the genre tag is an allowed input only when you are testing the genre hypothesis, and say so).
3. Do not contact the vendor's service and do not replay requests. Do not read, copy or print licence credentials. Keep raw
   captures, music and per-song results private; publish aggregates only.
4. Do not publish decompiled code or proprietary artifacts. A fitted model (weights) of the service's behaviour is the
   deliverable; keep its provenance clear in its own file and in the docs.
5. Test a new model on a small sample first (ten songs) before running it on everything.

## Ranked experiments

Energy (window level, then overall):
1. Fit the window classifier on the whole new set: RBF SVC grid on raw features (the current best; do not standardize first,
   standardizing lost 12 points), then gradient boosting and an MLP for comparison, learning curve, per-genre results. If it
   climbs toward 95%+, keep the SVC (it exports as plain arrays and `crates/camelot-core/src/energy.rs` already loads it).
2. Overall energy (the ceiling on true levels is about 80.5%, so this is the real bottleneck, not the level classifier): with the **true** window levels as input, find the aggregator. Try median, mean, 75th and 80th percentile
   (the patent), duration weighting, the top fraction, and rules that use `VolumeRmsDb`; compute an upper bound with gradient
   boosting on the levels, volumes and durations. The tuning songs already show the upper bound is not far above the rules, which points at continuous
   window scores or another input: train an ordinal regression or SVR on the windows, with a bag-level objective (the song's
   overall rating as the target, windows as instances), then test the same aggregators on its scores; direct song-level models
   on averaged features reached only 63-66% on 282 songs, so this needs the larger set.
3. Cue `EnergyLevel`: confirm the beat-median rule on the new data.

Key:
1. Fit the tonic-rotation-shared model (`key_experiments.py`) on the whole new set; try LR, an MLP and gradient boosting on the
   tonic-aligned features; the rotation trick matters because major keys are rare in melodic house and techno.
2. Model `MainKeyConfidence` (the softmax share of the winner correlates 0.58 with it now) and check whether low-confidence
   songs are genuinely ambiguous (parallel major/minor, fifths) or a different feature would separate them.
3. Per-genre accuracy; check whether genre or tempo enters the decision.

Cues:
1. Candidate ranker at scale with the 44-feature recipe in `docs/research/request-response-mapping.md` plus the patent-shaped
   before/after scores and genre; then a set-level step (the server picks a set, not eight independent peaks).
2. Explain the one-swap failures: look at which cue is swapped, whether it sits on the same 32-beat phrase grid, whether
   energy segment boundaries predict the swap, and whether the choice depends on the track's length or genre.
3. The beat grid needs no work: it is reproduced exactly.

## Handing back

- Update or add a results note under `docs/research/` with aggregate numbers only (exact and within one, per genre).
- Put fitted models under `models/` with a SHA-256 manifest. The energy model format is in `crates/camelot-core/src/energy.rs`;
  the desktop's opt-in "MIK-style (beta)" energy rating embeds `models/builtin/energy-svc-v1.json` and verifies its hash
  (`apps/desktop/src-tauri/src/builtin.rs`, `models/builtin/manifest.json`). A key or cue model needs engine integration;
  the exact client features exist only for 44.1 kHz stereo 16-bit FLAC, so say what other formats get.
- Keep `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings` and the tests green, and do not cut releases.
