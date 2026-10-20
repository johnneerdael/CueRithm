# Mixed In Key decision stage

Research code for reproducing the server decisions (key, overall energy, cue points) of the analysis service that
Mixed In Key 11 calls, from the request fields alone. Start with [AGENT_TASK.md](AGENT_TASK.md); the dataset layout is in
[DATA_FORMAT.md](DATA_FORMAT.md).

| File | Purpose |
| --- | --- |
| `mikdata.py` | loader, genre grouping, split policy, cue/energy helpers |
| `key_experiments.py` | key from the 352 features: tonic-rotation-shared model, confidence and genre breakdowns, baselines |
| `energy_experiments.py` | window levels (RBF SVC), overall rating rules, boundary and genre breakdowns, learning curve |
| `overall_experiments.py` | how much of the overall rating the true window levels can explain (about 80.5%) |
| `cue_experiments.py` | cue ground truth, candidate ranker baseline, evaluation harness |
| `sealed_corpus_holdout_feature_sha1.txt` | hashes of the sealed held-out songs of the first corpus: never train or tune on these |

```sh
python -m venv .venv && . .venv/bin/activate && pip install -r requirements.txt
python key_experiments.py    --data DIR --deny-hashes sealed_corpus_holdout_feature_sha1.txt --only-split tuning --baseline
python energy_experiments.py --data DIR --deny-hashes sealed_corpus_holdout_feature_sha1.txt --only-split tuning --grid --curve
python cue_experiments.py    --data DIR --deny-hashes sealed_corpus_holdout_feature_sha1.txt --only-split tuning
```

Nothing in this folder is data: no music, no per-song labels, no credentials.
