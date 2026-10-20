"""Learn the low/mid/high band level of every 32-beat energy window from its 183 request features, using local audio.

    python window_bands.py --pairs DIR --audio DIR --out model.npz

The request carries 183 numbers per window but no direct band levels. For tracks with audio, the real band level of each
window is measured (the same bands as event_view) and a ridge model maps the 183 features plus the window volume to it. The model
is then applied to songs without audio, giving a coarse low/mid/high structure for every captured track.
"""
import argparse
import glob
import json
import os
from multiprocessing import Pool

import numpy as np
from sklearn.linear_model import RidgeCV
from sklearn.model_selection import GroupKFold, cross_val_predict
from sklearn.pipeline import make_pipeline
from sklearn.preprocessing import StandardScaler

import event_view as v


def one(args):
    path, audio_dir = args
    request = json.load(open(path))
    audio = v.find_audio(audio_dir, request['EliteData']['FileName'])
    if not audio:
        return None
    power = v.band_power(v.decode(audio))
    frame_time = (np.arange(len(power)) * v.HOP + v.N_FFT / 2) / v.RATE
    X, y = [], []
    for w in request['EnergySegmentData']:
        if not w['Features']:
            continue
        sel = (frame_time >= w['StartTime']) & (frame_time < w['EndTime'])
        if sel.sum() < 4:
            continue
        X.append(list(w['Features']) + [w['VolumeRmsDb']])
        y.append(np.sqrt(power[sel].mean(0)))
    return (np.array(X), np.array(y)) if X else None


def features(windows):
    """Model input for a list of window dicts (as in the request)."""
    return np.array([list(w['Features']) + [w['VolumeRmsDb']] for w in windows if w['Features']])


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument('--pairs', required=True)
    ap.add_argument('--audio', required=True)
    ap.add_argument('--out', required=True)
    a = ap.parse_args()
    paths = sorted(glob.glob(os.path.join(a.pairs, '*.request.json')))
    with Pool(8) as pool:
        got = [r for r in pool.map(one, [(p, a.audio) for p in paths]) if r is not None]
    X = np.vstack([g[0] for g in got]); Y = np.vstack([g[1] for g in got]); groups = np.concatenate([np.full(len(g[0]), i) for i, g in enumerate(got)])
    print(f'{len(got)} tracks, {len(X)} windows')
    target = np.log(Y + 1e-9)
    for j, name in enumerate(('low', 'mid', 'high')):
        model = make_pipeline(StandardScaler(), RidgeCV(alphas=np.logspace(-1, 4, 11)))
        pred = cross_val_predict(model, X, target[:, j], groups=groups, cv=GroupKFold(5))
        r2 = 1 - ((target[:, j] - pred) ** 2).sum() / ((target[:, j] - target[:, j].mean()) ** 2).sum()
        # the same, but within each track (does it follow the track's own ups and downs?)
        within = np.mean([np.corrcoef(target[groups == g, j], pred[groups == g])[0, 1] for g in np.unique(groups) if (groups == g).sum() > 8])
        print(f'  {name}: out-of-track R2 {r2:.3f}, mean within-track correlation {within:.3f}')
    models = {}
    for j, name in enumerate(('low', 'mid', 'high')):
        m = make_pipeline(StandardScaler(), RidgeCV(alphas=np.logspace(-1, 4, 11))).fit(X, target[:, j])
        models[name] = (m['standardscaler'].mean_, m['standardscaler'].scale_, m['ridgecv'].coef_, m['ridgecv'].intercept_)
    np.savez(a.out, **{f'{k}_{i}': val for k, parts in models.items() for i, val in enumerate(parts)})


if __name__ == '__main__':
    main()
