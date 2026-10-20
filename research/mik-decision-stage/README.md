# Mixed In Key decision stage

Research code for reproducing the server decisions (key, overall energy, cue points) of the analysis service that
Mixed In Key 11 calls, from the request fields alone. Start with [AGENT_TASK.md](AGENT_TASK.md); the dataset layout is in
[DATA_FORMAT.md](DATA_FORMAT.md).

| File | Purpose |
| --- | --- |
| `import_capture.py` | mitmproxy addon: saved flow file -> sanitized request/response folder (offline, redacts credentials) |
| `gather_mik_data.py` | runs on the capture laptop: database backup, per-song export with beat grids, file inventory, tag dump, capture check (see CAPTURE-LAPTOP.md) |
| `CAPTURE-LAPTOP.md` | the task for the agent on the capture laptop: gather, check and package only, no analysis there |
| `build_handback.py` | builds the hand-back package (manifest with SHA-256, counts, tools) and verifies one that was received |
| `genre_experiment.py` | controlled genre test: tag-only variants of the same audio, then pack and compare the responses (see GENRE-EXPERIMENT.md) |
| `GENRE-EXPERIMENT.md` | the task for the agent on the capture laptop for that test (the owner does the scanning) |
| `build_cache.py` | request/response folder -> one numpy cache (key features, energy windows, cue scores, labels) that the scripts below load in a second |
| `key_models.py` | key: shift-augmented RBF SVC (96.7%) and the pitch-shared convolutional network (97.1%), out of fold on the tuning split |
| `energy_overall.py` | window level (RBF SVC, 92.0%) and the overall rating from the predicted levels (logistic aggregator 77.2% vs 71.0% for the 80th-percentile rule) |
| `cue_ranker.py` | cues: list-wise ranker over candidate beats with hierarchical-phase features (65% of tracks with every cue right) |
| `freeze_and_score.py` | writes the freeze record, then scores the held-out songs once (refuses a second scoring) |
| `events.py`, `event_view.py` | structural events (intro, breakdown, build-up, drop, outro) from the low/mid/high bands, and the beat-grid band view with cues overlaid |
| `coverage_check.py`, `event_coverage.py` | how many detected events a cue set covers; the validator that adds the uncovered ones within eight slots |
| `window_bands.py`, `cue_bands.py` | learn each 32-beat window's low/mid/high level from its 183 request features (R2 0.85-0.89) |
| `cue_signal.py` | the client's ramp and band-pass conditioning of the per-beat cue scores |
| `mikdata.py` | loader, genre grouping, split policy, cue/energy helpers |
| `key_experiments.py` | key from the 352 features: tonic-rotation-shared model, confidence and genre breakdowns, baselines |
| `energy_experiments.py` | window levels (RBF SVC), overall rating rules, boundary and genre breakdowns, learning curve |
| `overall_experiments.py` | how much of the overall rating the true window levels can explain (about 80.5%) |
| `cue_experiments.py` | cue ground truth, candidate ranker baseline, evaluation harness |
| `profile_time_experiments.py` | what the responses' per-stage timings say about the stages (cost per window, beat and song, genre dependence) |
| `sealed_corpus_holdout_feature_sha1.txt` | hashes of the sealed held-out songs of the first corpus: never train or tune on these |

```sh
python -m venv .venv && . .venv/bin/activate && pip install -r requirements.txt
python build_cache.py       --data DIR --deny-hashes sealed_corpus_holdout_feature_sha1.txt --out cache.npz
python key_models.py        --cache cache.npz --model cnn   # or svc; torch is needed for cnn
python energy_overall.py    --cache cache.npz
python cue_ranker.py        --cache cache.npz
python key_experiments.py    --data DIR --deny-hashes sealed_corpus_holdout_feature_sha1.txt --only-split tuning --baseline
python energy_experiments.py --data DIR --deny-hashes sealed_corpus_holdout_feature_sha1.txt --only-split tuning --grid --curve
python cue_experiments.py    --data DIR --deny-hashes sealed_corpus_holdout_feature_sha1.txt --only-split tuning
```

Nothing in this folder is data: no music, no per-song labels, no credentials.
