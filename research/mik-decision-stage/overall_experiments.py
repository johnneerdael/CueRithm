"""Overall energy: how much of OverallEnergy can be derived from the response's own window levels?

If a rule on the TRUE window levels (plus volumes) cannot reach the target, no better window classifier can either: the
service must use something finer than the rounded levels (continuous scores) or inputs not yet considered.
"""
import argparse, collections, warnings
import numpy as np
from sklearn.ensemble import GradientBoostingClassifier
from sklearn.linear_model import LogisticRegression
from sklearn.model_selection import GroupKFold, cross_val_predict
import mikdata as md
warnings.filterwarnings('ignore')


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument('--data', required=True); ap.add_argument('--allow-names'); ap.add_argument('--deny-hashes'); ap.add_argument('--only-split', choices=['tuning', 'holdout'])
    a = ap.parse_args()
    songs = []
    for s in md.iter_songs(a.data, md.read_list(a.allow_names), md.read_list(a.deny_hashes), a.only_split):
        X, y, v = md.window_arrays(s)
        if len(X): songs.append(dict(s=s, wl=y, wv=v, overall=s['r']['OverallEnergy'], h=s['h']))
    T = np.array([x['overall'] for x in songs]); G = np.array([x['h'] for x in songs])
    print('songs', len(songs), 'overall distribution', dict(sorted(collections.Counter(T.tolist()).items())))
    wls = [x['wl'][x['wl'] > 0] for x in songs]
    rules = {'median': np.median, 'p75': lambda w: np.percentile(w, 75), 'p80': lambda w: np.percentile(w, 80), 'p85': lambda w: np.percentile(w, 85),
             'mean of top 33%': lambda w: np.mean(np.sort(w)[int(len(w) * 0.67):]), 'mean of top 40%': lambda w: np.mean(np.sort(w)[int(len(w) * 0.6):]),
             'mean of top 50%': lambda w: np.mean(np.sort(w)[int(len(w) * 0.5):]), 'mean': np.mean}
    print('\nrules on the TRUE window levels (exact agreement):')
    for name, fn in rules.items():
        print(f'  {name:18s}', *[f'{r} {np.mean([int(rf(fn(w))) == t for w, t in zip(wls, T)]):.3f}' for r, rf in (('round', np.round), ('floor', np.floor), ('ceil', np.ceil))])

    def stats(x):
        w = x['wl'][x['wl'] > 0]
        return np.r_[[(w == L).mean() for L in range(1, 10)], w.mean(), np.median(w), np.percentile(w, 75), np.percentile(w, 80), np.percentile(w, 90), w.max(), w.min(), len(w), x['wv'].mean(), x['wv'].std()]
    F = np.array([stats(x) for x in songs]); F = (F - F.mean(0)) / (F.std(0) + 1e-9)
    print('\nlearned upper bounds from the true levels, their histogram, statistics and the volumes (5-fold by song):')
    for name, model in (('logistic', LogisticRegression(C=1, max_iter=3000)), ('gradient boosting', GradientBoostingClassifier(n_estimators=150, max_depth=2, learning_rate=0.05, random_state=0))):
        P = cross_val_predict(model, F, T, groups=G, cv=GroupKFold(5))
        print(f'  {name:18s} exact {(P == T).mean():.3f}  within one {(abs(P - T) <= 1).mean():.3f}')


if __name__ == '__main__':
    main()
