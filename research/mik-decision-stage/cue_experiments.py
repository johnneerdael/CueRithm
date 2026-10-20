"""Cues: which eight beats does the server pick from the per-beat scores in CuePointData?

Known and exact: the first cue is the origin beat (CuePointStartBeat - 32), cue time follows from the integer beat with
time_at_beat(), and 86.8% of tracks get exactly eight cues. The selection is the open part (the earlier rule and a
gradient-boosted ranker both reach about 60% of tracks with every cue right). This script gives the evaluation harness
and two baselines: a ranker over candidate beats, and the patent-shaped before/after scoring with fitted constants.
"""
import argparse, collections, json, warnings
import numpy as np
from sklearn.ensemble import HistGradientBoostingClassifier
from sklearn.model_selection import GroupKFold
import mikdata as md
warnings.filterwarnings('ignore')


def prepare(song):
    q = song['q']
    s = md.decode_cue_scores(q['CuePointData']); start = q['CuePointStartBeat']
    truth = set(b for b in md.cue_truth_beats(song))
    return dict(s=s, start=start, truth=truth, n=len(s))


def before_after(s, widths=(16, 8, 4, 2), weights=(1, 1, 1, 1), c=0.5, power=1.0):
    """Patent-shaped score at index j: blocks of `width` values after j against the same number before it; a drop counts
    c < 1 times a rise. Summed over the widths with the given weights."""
    n = len(s); S = np.zeros(n)
    for width, wt in zip(widths, weights):
        d = np.zeros(n); diff = s[width:] - s[:-width]
        d[width:] = np.where(diff >= 0, 1.0, c) * np.abs(diff) ** power
        cs = np.concatenate([[0], np.cumsum(d)]); idx = np.arange(n)
        S += wt * (cs[np.minimum(idx + width, n)] - cs[idx])
    return S


def candidates(t):
    return [i for i in range(t['start'] + 32, t['n']) if (i - t['start']) % 4 == 0]


def features(t, cand):
    s, start, n = t['s'], t['start'], t['n']
    S1 = before_after(s); S2 = before_after(s, c=1.0)
    grid = {i: k for k, i in enumerate(sorted(cand, key=lambda i: -S1[i]))}
    def at(a, i): return a[i] if 0 <= i < n else 0.0
    rows = []
    for i in cand:
        w = lambda a, lo, hi: a[max(0, i + lo): max(0, i + hi)]
        f = [s[i], S1[i], S2[i], grid[i] / max(len(cand), 1), (i - start) / max(n - start, 1), (n - i), (i - start) % 32, (i - start) % 16, (i - start) % 8,
             w(s, -16, 0).sum(), w(s, 0, 16).sum(), w(s, -16, 0).max(initial=0), w(s, 0, 16).max(initial=0),
             at(S1, i - 32), at(S1, i + 32), at(S1, i - 4), at(S1, i + 4), at(S1, i - 16), at(S1, i + 16)]
        rows.append(f)
    return np.array(rows, float)


def greedy(cand, score, k=7, sep=32):
    chosen = []
    for j in np.argsort(-score):
        if all(abs(cand[j] - cand[c]) >= sep for c in chosen): chosen.append(j)
        if len(chosen) >= k: break
    return sorted(cand[j] for j in chosen)


def evaluate(song_data, predicted):
    exact = [set(p) == t['truth'] for t, p in zip(song_data, predicted)]
    rec = [len(set(p) & t['truth']) / max(len(t['truth']), 1) for t, p in zip(song_data, predicted)]
    prec = [len(set(p) & t['truth']) / max(len(p), 1) for t, p in zip(song_data, predicted)]
    return exact, rec, prec


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument('--data', required=True); ap.add_argument('--allow-names'); ap.add_argument('--deny-hashes')
    ap.add_argument('--only-split', choices=['tuning', 'holdout']); ap.add_argument('--folds', type=int, default=5)
    ap.add_argument('--out')
    a = ap.parse_args()
    songs = list(md.iter_songs(a.data, md.read_list(a.allow_names), md.read_list(a.deny_hashes), a.only_split))
    T = [prepare(s) for s in songs]
    print('songs', len(songs), 'cue counts', dict(sorted(collections.Counter(len(t['truth']) for t in T).items())))
    gaps = [b - a_ for t in T for a_, b in zip(sorted(t['truth']), sorted(t['truth'])[1:])]
    print('gaps that are multiples of 32 beats:', round(float(np.mean([g % 32 == 0 for g in gaps])), 3))
    groups = np.array([s['h'] for s in songs]); idx = np.arange(len(songs))
    # truth indices in score space: index = beat + start (the first cue, beat 0, is the origin)
    pred_rank = [None] * len(songs)
    for tr, te in GroupKFold(a.folds).split(idx, idx, groups):
        X, y = [], []
        for i in tr:
            t = T[i]; cand = candidates(t); F = features(t, cand)
            X.append(F); y.extend(int((c - t['start']) in t['truth']) for c in cand)
        model = HistGradientBoostingClassifier(max_iter=300, learning_rate=0.05, max_depth=5, random_state=0).fit(np.vstack(X), y)
        for i in te:
            t = T[i]; cand = candidates(t)
            if not cand: pred_rank[i] = [0]; continue
            p = model.predict_proba(features(t, cand))[:, 1]
            # the server always keeps the origin beat and up to seven more
            pred_rank[i] = sorted({0, *[c - t['start'] for c in greedy(cand, p, k=max(len(t['truth']) - 1, 0))]})
    exact, rec, prec = evaluate(T, pred_rank)
    print(f'\nranker over candidate beats (oracle cue count): tracks with every cue right {np.mean(exact):.3f}  recall {np.mean(rec):.3f}  precision {np.mean(prec):.3f}')
    md.breakdown('CUES: every cue right, by genre', songs, exact)
    if a.out:
        json.dump(dict(id=[s['id'] for s in songs], exact=[bool(e) for e in exact], genre=[s['genre'] for s in songs]), open(a.out, 'w'))


if __name__ == '__main__':
    main()
