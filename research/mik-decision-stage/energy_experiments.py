"""Energy: window levels, segment structure and the overall rating from the request windows.

Exact (checked on every pair so far): each response segment's Volume is the VolumeRmsDb of its first window, segments
are runs of equal window level and bounds are raw window bounds. Learned: the level of each window, then the overall
rating. Windows hold 183 raw features and one volume; the best model so far is an RBF SVC on the RAW features.
"""
import argparse, collections, json, time, warnings
import numpy as np
from sklearn.model_selection import GroupKFold
from sklearn.svm import SVC
import mikdata as md
warnings.filterwarnings('ignore')


def overall_from_levels(levels, q=0.8):
    return int(np.sort(levels)[min(len(levels) - 1, int(len(levels) * q))])


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument('--data', required=True); ap.add_argument('--allow-names'); ap.add_argument('--deny-hashes')
    ap.add_argument('--only-split', choices=['tuning', 'holdout']); ap.add_argument('--folds', type=int, default=5)
    ap.add_argument('--C', type=float, default=10); ap.add_argument('--gamma', type=float, default=0.1)
    ap.add_argument('--grid', action='store_true', help='small C/gamma grid first'); ap.add_argument('--curve', action='store_true')
    ap.add_argument('--out')
    a = ap.parse_args()
    songs = []
    for s in md.iter_songs(a.data, md.read_list(a.allow_names), md.read_list(a.deny_hashes), a.only_split):
        X, y, v = md.window_arrays(s)
        if len(X): songs.append(dict(s, X=X, y=y, v=v, overall=s['r']['OverallEnergy']))
    groups = np.array([s['h'] for s in songs]); idx = np.arange(len(songs))
    print('songs', len(songs), 'windows', sum(len(s['y']) for s in songs), dict(collections.Counter(s['grp'] for s in songs)))

    def oof(C, gamma, train_idx=None):
        P = [None] * len(songs)
        for tr, te in GroupKFold(a.folds).split(idx, idx, groups):
            if train_idx is not None: tr = np.array([i for i in tr if i in train_idx])
            m = SVC(C=C, gamma=gamma).fit(np.vstack([songs[i]['X'] for i in tr]), np.concatenate([songs[i]['y'] for i in tr]))
            for i in te: P[i] = m.predict(songs[i]['X'])
        return P

    if a.grid:
        for C in (3, 10, 30, 100):
            for g in (0.03, 0.1, 0.3):
                P = oof(C, g); acc = np.concatenate([P[i] == songs[i]['y'] for i in idx]).mean()
                print(f'  C={C} gamma={g}: window exact {acc:.4f}', flush=True)
    t0 = time.time(); P = oof(a.C, a.gamma)
    allt = np.concatenate([s['y'] for s in songs]); allp = np.concatenate(P)
    print(f'\nwindow level: exact {(allt == allp).mean():.4f}  within one {(abs(allt - allp) <= 1).mean():.4f}  ({time.time() - t0:.0f}s)')
    print('error sizes', dict(sorted(collections.Counter((allp - allt).tolist()).items())))
    for L in sorted(set(allt)):
        m = allt == L; print(f'  true level {L}: n={m.sum():5d} exact={(allp[m] == L).mean():.3f}')
    md.breakdown('WINDOW agreement by genre', songs, [np.mean(P[i] == songs[i]['y']) for i in idx])
    bnd = np.concatenate([np.r_[False, s['y'][1:] != s['y'][:-1]] | np.r_[s['y'][:-1] != s['y'][1:], False] for s in songs])
    print(f'\nwindows next to a level change: exact {(allt[bnd] == allp[bnd]).mean():.3f} | inside runs {(allt[~bnd] == allp[~bnd]).mean():.3f}')
    tv = np.array([s['overall'] for s in songs])
    for q in (0.5, 0.6, 0.7, 0.75, 0.8, 0.85, 0.9):
        print(f'  overall = {int(q * 100)}th percentile of predicted window levels: exact {np.mean([overall_from_levels(P[i], q) == tv[i] for i in idx]):.3f}')
    ok = [overall_from_levels(P[i]) == tv[i] for i in idx]
    md.breakdown('OVERALL (80th percentile of predicted windows) agreement by genre', songs, ok)
    print('overall error sizes', dict(sorted(collections.Counter([overall_from_levels(P[i]) - tv[i] for i in idx]).items())))
    if a.curve:
        rng = np.random.RandomState(0); order = rng.permutation(len(songs))
        for n in (25, 50, 100, 200, 400, 800, 1600, 3200):
            if n >= len(songs): break
            keep = set(order[:n].tolist()); P2 = oof(a.C, a.gamma, keep)
            print(f'  learning curve, {n} training songs per run at most: window exact {np.concatenate([P2[i] == songs[i]["y"] for i in idx]).mean():.4f}', flush=True)
    if a.out:
        json.dump(dict(id=[s['id'] for s in songs], overall=tv.tolist(), pred=[overall_from_levels(P[i]) for i in idx], genre=[s['genre'] for s in songs]), open(a.out, 'w'))


if __name__ == '__main__':
    main()
