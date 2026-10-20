"""Freeze the decision-stage configuration, then score the held-out songs exactly once.

    python freeze_and_score.py --cache cache.npz --out RESULTS_DIR [--limit-tuning N --limit-holdout M]   # smoke test: no record is kept
    python freeze_and_score.py --cache cache.npz --out RESULTS_DIR --score                               # the one scoring

Without ``--score`` it only writes the freeze record (configuration, scripts' SHA-256, cache SHA-256, git commit, counts) and exits.
With ``--score`` it refuses to run if a score file already exists in RESULTS_DIR, writes the freeze record first, fits everything on the
tuning songs only, predicts the held-out songs and writes aggregate numbers (no names, no per-song rows).

Held-out here means the songs ``mikdata.split_of`` assigns to 'holdout' in the cache (one in five, by hash of the song) after the sealed
songs of the first corpus were dropped. The sealed first-corpus songs are not touched.

Numbers reported, all with failures in the denominator:
* key: exact agreement on the 24 codes, and separately agreement up to a neighbour on the wheel
* window energy level: exact and within one level
* overall energy: exact and within one level (the multinomial logistic aggregator, and the 80th-percentile rule the desktop uses today)
* cues: tracks with every cue right and cue recall, once with the server's cue count given (diagnostic) and once with eight cues fixed
  (no response information used)
"""
import argparse
import datetime
import hashlib
import json
import os
import subprocess
import sys
import time

import numpy as np

import cue_ranker as cr
import energy_overall as eo
import key_models as km

HERE = os.path.dirname(os.path.abspath(__file__))
SCRIPTS = ('key_models.py', 'energy_overall.py', 'cue_ranker.py', 'mikdata.py', 'build_cache.py', 'freeze_and_score.py')
CONFIG = {
    'key': 'pitch-shared two-convolution network, 5-seed average, 60 epochs, 16 channels, kernel 5, hidden 256, dropout 0.3, AdamW lr 2e-3 wd 1e-2',
    'window_level': 'RBF SVC C=10 gamma=0.1 on the raw 183 window features',
    'overall': 'multinomial logistic regression (C=10, standardised) on level shares, mean, std, quantiles 25..95 of the predicted levels and window volume mean/std/max; '
               'trained on out-of-fold predicted levels of the tuning songs',
    'cues': 'LightGBM lambdarank (400 trees, 31 leaves) over every 8th beat with hierarchical-phase and global-rank features, greedy pick with at least 32 beats between cues',
    'inputs': 'request fields only; genre, title, artist and every response field are never inputs',
}


def sha256(path):
    h = hashlib.sha256()
    with open(path, 'rb') as f:
        for chunk in iter(lambda: f.read(1 << 22), b''):
            h.update(chunk)
    return h.hexdigest()


def wheel_neighbour(a, b):
    """Same tonic number on the other letter, or one number apart on the same letter (Camelot wheel)."""
    na, la = int(a[:-1]), a[-1]
    nb, lb = int(b[:-1]), b[-1]
    if na == nb:
        return True
    return la == lb and min((na - nb) % 12, (nb - na) % 12) == 1


def freeze_record(d, tuning, holdout):
    try:
        commit = subprocess.run(['git', '-C', HERE, 'rev-parse', 'HEAD'], capture_output=True, text=True).stdout.strip()
        dirty = bool(subprocess.run(['git', '-C', HERE, 'status', '--short', '--', '.'], capture_output=True, text=True).stdout.strip())
    except OSError:
        commit, dirty = '', None
    return {'frozen_at': datetime.datetime.now(datetime.timezone.utc).isoformat(timespec='seconds'), 'git_commit': commit, 'research_folder_has_uncommitted_changes': dirty,
            'cache_sha256': None, 'tuning_songs': int(len(tuning)), 'holdout_songs': int(len(holdout)),
            'tuning_windows': int(np.isin(d['ws'], tuning).sum()), 'holdout_windows': int(np.isin(d['ws'], holdout).sum()),
            'config': CONFIG, 'scripts_sha256': {s: sha256(os.path.join(HERE, s)) for s in SCRIPTS}}


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument('--cache', required=True)
    ap.add_argument('--out', required=True)
    ap.add_argument('--score', action='store_true', help='fit on the tuning songs and score the held-out songs (once)')
    ap.add_argument('--limit-tuning', type=int)
    ap.add_argument('--limit-holdout', type=int)
    a = ap.parse_args()
    d = np.load(a.cache, allow_pickle=True)
    tuning, holdout = np.where(d['split'] == 'tuning')[0], np.where(d['split'] == 'holdout')[0]
    smoke = bool(a.limit_tuning or a.limit_holdout)
    if a.limit_tuning:
        tuning = tuning[:a.limit_tuning]
    if a.limit_holdout:
        holdout = holdout[:a.limit_holdout]
    os.makedirs(a.out, exist_ok=True)
    score_path = os.path.join(a.out, 'holdout-score.json')
    record = freeze_record(d, tuning, holdout)
    record['cache_sha256'] = sha256(a.cache)
    if not smoke:
        record_path = os.path.join(a.out, 'freeze-record.json')
        if a.score and os.path.exists(score_path):
            raise SystemExit(f'{score_path} exists: the held-out songs were already scored; they are not scored a second time.')
        if not os.path.exists(record_path):
            json.dump(record, open(record_path, 'w'), indent=1)
        print('freeze record:', record_path)
        if not a.score:
            return
    t0 = time.time()
    # --- fit on the tuning songs only
    wt = np.isin(d['ws'], tuning)
    xw, yw, sw, vw = d['wx'][wt].astype(float), d['wy'][wt].astype(int), d['ws'][wt], d['wv'][wt]
    print(f'fitting on {len(tuning)} tuning songs ({len(yw)} windows)', flush=True)
    key_proba = km.fit_cnn_ensemble(d['X'][tuning], km.encode(d['key'][tuning]), seeds=1 if smoke else 5, epochs=5 if smoke else 60)
    oof = eo.window_oof(xw, yw, sw, folds=min(5, len(tuning)))
    window_model = eo.fit_windows(xw, yw)
    aggregator = eo.fit_aggregator(eo.level_features(oof, vw, sw, tuning), d['overall'][tuning].astype(int))
    tables = cr.song_tables(d, np.concatenate([tuning, holdout]))
    rank = cr.fit_ranker(tables, tuning, 'rank')
    print(f'fitted ({time.time() - t0:.0f}s); scoring {len(holdout)} held-out songs', flush=True)
    # --- predict the held-out songs
    out = {'frozen_commit': record['git_commit'], 'holdout_songs': int(len(holdout))}
    y_key = km.encode(d['key'][holdout])
    pk = key_proba(d['X'][holdout]).argmax(1)
    conf = d['conf'][holdout]
    exact = pk == y_key
    nb = np.array([wheel_neighbour(km.decode(p), km.decode(t)) for p, t in zip(pk, y_key)])
    out['key'] = {'exact': float(exact.mean()), 'exact_or_wheel_neighbour': float(nb.mean()), 'server_confidence_below_0.7': float(exact[conf < .7].mean()) if (conf < .7).any() else None,
                  'server_confidence_at_least_0.97': float(exact[conf >= .97].mean()) if (conf >= .97).any() else None,
                  'n_below_0.7': int((conf < .7).sum()), 'server_major_keys': int((y_key < 12).sum()), 'exact_on_server_major_keys': float(exact[y_key < 12].mean()) if (y_key < 12).any() else None}
    wh = np.isin(d['ws'], holdout)
    xh, yh, sh, vh = d['wx'][wh].astype(float), d['wy'][wh].astype(int), d['ws'][wh], d['wv'][wh]
    ph = window_model.predict(xh)
    out['window_level'] = {'windows': int(len(yh)), 'exact': float(np.mean(ph == yh)), 'within_one': float(np.mean(abs(ph - yh) <= 1))}
    truth_overall = d['overall'][holdout].astype(int)
    feats = eo.level_features(ph, vh, sh, holdout)
    logistic = aggregator.predict(feats)
    rule = eo.percentile_rule(ph, sh, holdout)
    out['overall'] = {'logistic_exact': float(np.mean(logistic == truth_overall)), 'logistic_within_one': float(np.mean(abs(logistic - truth_overall) <= 1)),
                      'percentile80_exact': float(np.mean(rule == truth_overall)), 'percentile80_within_one': float(np.mean(abs(rule - truth_overall) <= 1))}
    given, fixed = [], []
    for s in holdout:
        matrix, _, beat, wanted = tables[s]
        score = rank(matrix)
        for bucket, count in ((given, len(wanted)), (fixed, 7)):
            picked = cr.greedy(beat, score, count)
            bucket.append((picked == wanted, len(picked & wanted) / max(len(wanted), 1)))
    out['cues'] = {'with_server_cue_count_every_cue_right': float(np.mean([e for e, _ in given])), 'with_server_cue_count_recall': float(np.mean([r for _, r in given])),
                   'eight_cues_fixed_every_cue_right': float(np.mean([e for e, _ in fixed])), 'eight_cues_fixed_recall': float(np.mean([r for _, r in fixed])),
                   'tracks_where_server_gave_eight_cues': float(np.mean([len(tables[s][3]) == 7 for s in holdout]))}
    grp = np.array([md_group(g) for g in d['genre'][holdout]])
    out['by_group'] = {g: {'songs': int((grp == g).sum()), 'key_exact': float(exact[grp == g].mean()) if (grp == g).any() else None,
                           'overall_logistic_exact': float(np.mean((logistic == truth_overall)[grp == g])) if (grp == g).any() else None} for g in ('MHT', 'other')}
    out['seconds'] = round(time.time() - t0)
    print(json.dumps(out, indent=1))
    if not smoke:
        json.dump(out, open(score_path, 'w'), indent=1)
        print('written', score_path)


def md_group(genre):
    return 'MHT' if (genre or '').startswith('Melodic House & Techno') else 'other'


if __name__ == '__main__':
    main()
