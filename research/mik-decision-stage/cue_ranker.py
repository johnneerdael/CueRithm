"""Cues: a list-wise ranker over candidate beats with hierarchical-phase and global-rank features, then the greedy pick.

    python build_cache.py --data DIR --deny-hashes sealed_corpus_holdout_feature_sha1.txt --out cache.npz
    python cue_ranker.py --cache cache.npz [--limit 200] [--objective rank|binary]

Facts used (all exact on the captures): the first cue is the origin beat ``CuePointStartBeat - 32`` and is never predicted; the other
cues are beats counted from the downbeat (score index = beat + 32), on the 32/16-beat grid of the downbeat, at least 32 beats apart, at
most seven more. Candidates are every 8th beat (99.6% of true cues sit on that grid). A candidate is described by the per-beat scores
around it (normalised by the song's 95th percentile), where it sits in the 32/64/128/256-beat hierarchy of the downbeat and of the origin,
its rank among all candidates, how it compares with the largest score within 16/32/64 beats and sums of the scores in the blocks before
and after it. The energy windows and the energy levels were tried as extra inputs and added nothing. Only tuning songs are used unless --split says otherwise.
"""
import argparse
import time

import lightgbm as lgb
import numpy as np
from sklearn.model_selection import GroupKFold

OFFSETS = np.arange(-32, 33, 4)


def song_table(scores, start, truth, n_truth):
    """Candidate matrix, labels and beats of one song. ``truth`` holds its cue beats from the downbeat, lowest first (the origin)."""
    n = len(scores)
    span = n - start
    scale = np.percentile(scores[start:], 95) + 1e-9
    z = scores.astype(np.float64) / scale
    padded = np.pad(z, (64, 64))
    beat = np.arange(-(-start // 8) * 8, n - 32, 8)  # beats from the downbeat, first one at least 32 after the origin (start - 32)
    index = beat + 32  # score index of a beat; true for every CuePointStartBeat, not only 32
    origin = start - 32
    columns = [padded[index + 64 + o] for o in OFFSETS]
    cum = np.concatenate([[0], np.cumsum(padded)])

    def block(lo, hi):
        return cum[index + 64 + hi] - cum[index + 64 + lo]

    def local_max(width):
        return np.array([z[max(0, k - width):k + width + 1].max() for k in index])

    rank_pct = np.argsort(np.argsort(-z[index])) / len(index)
    since = beat - origin
    extra = [beat % 32, beat % 64, beat % 128, beat % 256, since % 32, (beat // 32) % 2, since / span, (span - since) / 32.0, rank_pct,
             z[index] / (local_max(16) + 1e-9), z[index] / (local_max(32) + 1e-9), z[index] / (local_max(64) + 1e-9),
             block(-32, 0), block(0, 32), block(-16, 0), block(0, 16), block(-8, 0), block(0, 8),
             np.full(len(index), scale / 100.0), np.full(len(index), span / 800.0), z[index] / (z[start:].max() + 1e-9)]
    wanted = set(int(b) for b in truth[1:n_truth])  # drops the origin cue itself
    label = np.array([int(b in wanted) for b in beat])
    return np.column_stack(columns + extra), label, beat, wanted


def greedy(beats, score, count, separation=32):
    chosen = []
    for j in np.argsort(-score):
        if all(abs(beats[j] - beats[c]) >= separation for c in chosen):
            chosen.append(j)
        if len(chosen) >= count:
            break
    return {int(beats[j]) for j in chosen}


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument('--cache', required=True)
    ap.add_argument('--split', default='tuning', choices=['tuning', 'holdout', 'all'])
    ap.add_argument('--objective', default='rank', choices=['rank', 'binary'])
    ap.add_argument('--folds', type=int, default=5)
    ap.add_argument('--limit', type=int)
    ap.add_argument('--out', help='write per-song exact/recall (no names) as json')
    a = ap.parse_args()
    d = np.load(a.cache, allow_pickle=True)
    songs = np.arange(len(d['ids'])) if a.split == 'all' else np.where(d['split'] == a.split)[0]
    if a.limit:
        songs = songs[:a.limit]
    t0 = time.time()
    tables = {}
    for s in songs:
        o0, o1 = d['cue_off'][s], d['cue_off'][s + 1]
        tables[s] = song_table(d['cue_scores'][o0:o1], int(d['cue_start'][s]), d['cue_truth'][s], int(d['cue_ntruth'][s]))
    params = dict(n_estimators=400, learning_rate=0.05, num_leaves=31, min_child_samples=40, subsample=0.8, subsample_freq=1,
                  colsample_bytree=0.8, verbose=-1, n_jobs=4)
    exact, recall = {}, {}
    for train, test in GroupKFold(min(a.folds, len(songs))).split(songs, groups=d['h'][songs]):
        tr = songs[train]
        X = np.vstack([tables[s][0] for s in tr])
        y = np.concatenate([tables[s][1] for s in tr])
        if a.objective == 'rank':
            model = lgb.LGBMRanker(objective='lambdarank', lambdarank_truncation_level=10, **params).fit(X, y, group=[len(tables[s][1]) for s in tr])
            predict = model.predict
        else:
            model = lgb.LGBMClassifier(**params).fit(X, y)
            predict = lambda m: model.predict_proba(m)[:, 1]
        for s in songs[test]:
            matrix, _, beat, wanted = tables[s]
            picked = greedy(beat, predict(matrix), len(wanted))
            exact[s], recall[s] = picked == wanted, len(picked & wanted) / max(len(wanted), 1)
    ok = np.array([exact[s] for s in songs])
    print(f'{len(songs)} {a.split} songs, {a.objective}: tracks with every cue right {ok.mean():.4f}, cue recall {np.mean([recall[s] for s in songs]):.4f} '
          f'({time.time() - t0:.0f}s)')
    if a.out:
        import json
        json.dump({'exact': [bool(exact[s]) for s in songs], 'recall': [recall[s] for s in songs]}, open(a.out, 'w'))


if __name__ == '__main__':
    main()
