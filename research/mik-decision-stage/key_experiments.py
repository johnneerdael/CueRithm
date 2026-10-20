"""Key: learn the server's key (and see how its confidence behaves) from the 352 request features.

Layout found in the captures: the 352 features are 4 banks of 88 piano-bin values; bin j belongs to pitch class
(j + 9) % 12. The strongest folded class is the tonic. Models share their weights across the 12 tonics by rotating
the profile so that the candidate tonic sits at a fixed position, so major keys are learned from minor examples too.
"""
import argparse, collections, json, warnings
import numpy as np
from sklearn.linear_model import LogisticRegression
from sklearn.model_selection import GroupKFold
from sklearn.preprocessing import StandardScaler
import mikdata as md
warnings.filterwarnings('ignore')
L = 76  # bins kept per bank after aligning the candidate tonic to the first octave


def candidate(x, tonic, scale='sqrt'):
    start = (tonic - 9) % 12
    v = np.concatenate([x[88 * b + start: 88 * b + start + L] for b in range(4)])
    return np.sqrt(v) if scale == 'sqrt' else np.log1p(v) if scale == 'log' else v


def fit_rotation_shared(train, labels, C, scale):
    models = {}
    for mode in (0, 1):
        A = np.array([candidate(s['x'], t, scale) for s in train for t in range(12)])
        y = np.array([int(lab == (t, mode)) for lab in labels for t in range(12)])
        mu, sd = A.mean(0), A.std(0) + 1e-9
        models[mode] = (mu, sd, LogisticRegression(C=C, max_iter=5000, class_weight='balanced').fit((A - mu) / sd, y))
    return models


def predict_rotation_shared(models, x, scale):
    best = (-1e9, None, 0.0)
    scores = []
    for mode in (0, 1):
        mu, sd, model = models[mode]
        for t in range(12):
            s = model.decision_function(((candidate(x, t, scale) - mu) / sd)[None])[0]
            scores.append(s)
            if s > best[0]:
                best = (s, (t, mode), 0.0)
    p = np.exp(np.array(scores) - max(scores)); best = (best[0], best[1], float(1 / p.sum()))  # softmax share of the winner
    return best[1], best[2]


def confusion_kind(a, b):
    na, la, nb, lb = int(a[:-1]), a[-1], int(b[:-1]), b[-1]
    if na == nb and la != lb:
        return 'relative major/minor'
    d = min((na - nb) % 12, (nb - na) % 12)
    if la == lb and d == 1: return 'neighbour on the wheel (fifth)'
    if la == lb and d == 2: return 'two steps (whole tone)'
    if la != lb and ((nb - na) % 12 == 3 or (na - nb) % 12 == 3): return 'parallel major/minor'
    return f'other ({d} steps, {"same" if la == lb else "different"} letter)'


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument('--data', required=True); ap.add_argument('--allow-names'); ap.add_argument('--deny-hashes')
    ap.add_argument('--only-split', choices=['tuning', 'holdout']); ap.add_argument('--folds', type=int, default=5)
    ap.add_argument('--scale', default='sqrt'); ap.add_argument('--C', type=float, default=0.01)
    ap.add_argument('--baseline', action='store_true', help='also run plain logistic regression on log1p features')
    ap.add_argument('--out')
    a = ap.parse_args()
    songs = [dict(s, x=np.array(s['q']['Segments'][0]['Features'], float), key=s['r']['KeySummary']['MainKey'],
                  conf=s['r']['KeySummary']['MainKeyConfidence']) for s in
             md.iter_songs(a.data, md.read_list(a.allow_names), md.read_list(a.deny_hashes), a.only_split)]
    labels = [md.CAMELOT[s['key']] for s in songs]
    groups = np.array([s['h'] for s in songs])
    print('songs', len(songs), dict(collections.Counter(s['grp'] for s in songs)),
          'minor share', round(np.mean([m for _, m in labels]), 3))
    pred = [None] * len(songs); share = np.zeros(len(songs))
    for tr, te in GroupKFold(a.folds).split(songs, labels, groups):
        models = fit_rotation_shared([songs[i] for i in tr], [labels[i] for i in tr], a.C, a.scale)
        for i in te:
            pred[i], share[i] = predict_rotation_shared(models, songs[i]['x'], a.scale)
    ok = np.array([pred[i] == labels[i] for i in range(len(songs))])
    print(f'rotation-shared logistic ({a.scale}, C={a.C}): agreement with the server key {ok.mean():.4f}')
    md.breakdown('KEY agreement by genre', songs, ok)
    conf = np.array([s['conf'] for s in songs])
    print('\n== agreement by the server KeyConfidence')
    for lo, hi in ((0, .7), (.7, .9), (.9, .97), (.97, 1.01)):
        m = (conf >= lo) & (conf < hi)
        if m.any(): print(f'  confidence {lo}-{hi}: n={m.sum():4d} agree={ok[m].mean():.3f}')
    bad = [(songs[i]['grp'], songs[i]['key'], md.FROM_CAMELOT[pred[i]], round(conf[i], 3)) for i in range(len(songs)) if not ok[i]]
    print('\nconfusion kinds:', dict(collections.Counter(confusion_kind(b[1], b[2]) for b in bad)))
    print('mismatches (group, server key, ours, server confidence):', *sorted(bad, key=lambda b: b[3])[:40], sep='\n  ')
    print('\ncorrelation of our winner share with the server confidence:', round(float(np.corrcoef(share, conf)[0, 1]), 3))
    if a.baseline:
        X = np.log1p(np.array([s['x'] for s in songs])); names = np.array([s['key'] for s in songs]); out = np.empty(len(songs), object)
        for tr, te in GroupKFold(a.folds).split(X, names, groups):
            sc = StandardScaler().fit(X[tr]); m = LogisticRegression(C=0.05, max_iter=3000).fit(sc.transform(X[tr]), names[tr])
            out[te] = m.predict(sc.transform(X[te]))
        print('\nplain logistic regression on all 352 log1p features:', round(float((out == names).mean()), 4))
    if a.out:
        json.dump(dict(id=[s['id'] for s in songs], key=[s['key'] for s in songs], ours=[md.FROM_CAMELOT[p] for p in pred],
                       conf=conf.tolist(), genre=[s['genre'] for s in songs]), open(a.out, 'w'))


if __name__ == '__main__':
    main()
