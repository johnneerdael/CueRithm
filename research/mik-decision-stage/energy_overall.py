"""Energy: window levels with an RBF SVC on the raw 183 features, then the overall rating from the predicted levels.

    python build_cache.py --data DIR --deny-hashes sealed_corpus_holdout_feature_sha1.txt --out cache.npz
    python energy_overall.py --cache cache.npz [--limit 200]

Out-of-fold results on the tuning split (grouped by song). The window level is a function of the 183 window features and nothing else
we could find; the raw features work best (standardising loses about 10 points). The overall rating is not a function of the response's
own window levels (even the true levels reach only about 78% with a boosted model); the best exportable rule found is a multinomial
logistic regression on statistics of the predicted levels and the window volumes, which beats the 80th-percentile rule by about seven points.
"""
import argparse
import time

import numpy as np
from joblib import Parallel, delayed
from sklearn.linear_model import LogisticRegression
from sklearn.model_selection import GroupKFold
from sklearn.pipeline import make_pipeline
from sklearn.preprocessing import StandardScaler
from sklearn.svm import SVC

LEVELS = np.arange(1, 11)
QUANTILES = (25, 50, 65, 75, 80, 85, 90, 95)


def fit_windows(x, y, c=10.0, gamma=0.1):
    return SVC(C=c, gamma=gamma, cache_size=3000).fit(x, y)


def window_oof(x, y, song, c=10.0, gamma=0.1, folds=5, workers=5):
    """Out-of-fold predicted levels, folds grouped by song. Threads share the arrays (libsvm releases the GIL)."""
    splits = list(GroupKFold(folds).split(x, y, song))
    parts = Parallel(n_jobs=workers, prefer='threads')(delayed(lambda tr, te: (te, fit_windows(x[tr], y[tr], c, gamma).predict(x[te])))(tr, te) for tr, te in splits)
    predicted = np.zeros(len(y), int)
    for te, p in parts:
        predicted[te] = p
    return predicted


def level_features(levels, volume, song, songs):
    """Per song: share of windows at each level, mean, std, quantiles of the levels, and mean/std/max of the window volume."""
    rows = []
    for s in songs:
        m = song == s
        lv, vol = levels[m].astype(float), volume[m]
        rows.append(np.r_[[np.mean(lv == k) for k in LEVELS], lv.mean(), lv.std(), np.percentile(lv, QUANTILES), vol.mean(), vol.std(), vol.max()])
    return np.array(rows)


def percentile_rule(levels, song, songs, q=0.8):
    """The rule the desktop's opt-in rating uses today: the q-quantile of the predicted levels, rounded (ties to even)."""
    return np.array([int(np.round(np.quantile(levels[song == s], q))) for s in songs])


def fit_aggregator(features, overall):
    return make_pipeline(StandardScaler(), LogisticRegression(C=10, max_iter=5000)).fit(features, overall)


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument('--cache', required=True)
    ap.add_argument('--limit', type=int, help='use only the first N tuning songs (smoke test)')
    ap.add_argument('--C', type=float, default=10.0)
    ap.add_argument('--gamma', type=float, default=0.1)
    a = ap.parse_args()
    d = np.load(a.cache, allow_pickle=True)
    songs = np.where(d['split'] == 'tuning')[0]
    if a.limit:
        songs = songs[:a.limit]
    w = np.isin(d['ws'], songs)
    x, y, song, vol = d['wx'][w].astype(float), d['wy'][w].astype(int), d['ws'][w], d['wv'][w]
    t0 = time.time()
    pred = window_oof(x, y, song, a.C, a.gamma, folds=min(5, len(songs)))
    print(f'{len(songs)} tuning songs, {len(y)} windows: window level exact {np.mean(pred == y):.4f}, within one {np.mean(abs(pred - y) <= 1):.4f} ({time.time() - t0:.0f}s)')
    overall = d['overall'][songs].astype(int)
    rule = percentile_rule(pred, song, songs)
    print(f'overall from the 80th percentile of the predicted levels: exact {np.mean(rule == overall):.4f}, within one {np.mean(abs(rule - overall) <= 1):.4f}')
    feats = level_features(pred, vol, song, songs)
    from sklearn.model_selection import cross_val_predict
    p = cross_val_predict(make_pipeline(StandardScaler(), LogisticRegression(C=10, max_iter=5000)), feats, overall, groups=d['h'][songs], cv=GroupKFold(min(5, len(songs))))
    print(f'overall from logistic regression on level statistics and volume (out of fold): exact {np.mean(p == overall):.4f}, within one {np.mean(abs(p - overall) <= 1):.4f}')


if __name__ == '__main__':
    main()
