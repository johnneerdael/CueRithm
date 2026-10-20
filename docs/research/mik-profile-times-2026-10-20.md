# What the service's own stage timings say, 2026-10-20

Aggregate findings only. Every response carries `KeyDetectionProfileTime`, `EnergyAnalysisProfileTime` and
`CuePointAnalysisProfileTime` (seconds): the time the service spent in each stage after parsing the request. Read on the 218
songs of the first corpus's tuning split (sealed songs untouched), with `research/mik-decision-stage/profile_time_experiments.py`.

## Seen

| Stage | Cost | Depends on |
| --- | --- | --- |
| Key | median 16.5 ms (fastest 13.2, floor about 14.8) | nothing in the request: not duration, beats, windows, cue data, confidence or mode (all correlations within 0.02) |
| Energy | 0.90 ms per window plus 0.04 ms (R2 0.97, 2 to 36 windows) | the number of windows only; no per-song stage |
| Cue points | 5.65 us per beat plus about 0 ms (R2 0.98) | beats only; no quadratic term (1.8e-07 per beat squared) |

- One key decision costs about 18 energy-window evaluations.
- The per-unit cost is the same in every genre group with at least five songs: energy 0.885 to 0.907 ms per window, cue 5.5
  to 5.7 us per beat, key 15.8 to 17.2 ms (one group of six songs at 22 ms is inside the key stage's heavy tail).
- Slow requests are not slow in every stage (correlation between stages 0.15 or less), so the tails are per-stage noise, not
  machine load; a quarter of the key times are more than 1.5 times the floor.

## Inferred (the service's hardware is unknown, so absolute operation counts are good to about ten times)

- Each stage is a fixed amount of dense computation per decision, with no early exit and no search whose length depends on the
  song: the key time does not move with the result.
- About 10^6 operations per energy window and about 10^7 per key decision. A linear model on the 183 or 352 features would be
  roughly a thousand times cheaper, and a tree ensemble or a template correlation cheaper still. The stage looks like a
  kernel machine, a nearest-neighbour lookup or a network of that size. This fits the learning curve (more data helps) and
  our linear key model stalling near 96%.
- The overall rating after the window levels costs under 0.1 ms: a cheap rule, not a second model.
- The cue stage scores every beat with a fixed amount of work (about 10^4 operations per beat), then picks cheaply; there is
  no pairwise optimisation over beats.
- No genre-dependent computation shows. This does not rule out genre-specific weights of the same shape, only a different
  model size or a search over genre candidates.

## To revisit

With the 4,000-song set the genre comparison has real groups (trance, techno, hardstyle, tech house, drum and bass at 50 or
more songs each): a cost that differs by genre would be the first direct sign of genre-dependent models. Rerun the script on
the new pairs and compare.
