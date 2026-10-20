"""What the server-side profile times in the responses say about the decision stage.

Every response carries KeyDetectionProfileTime, EnergyAnalysisProfileTime and CuePointAnalysisProfileTime (seconds). They are
the time the service spent in each stage after the request was parsed, so they bound what the stages can be: how much work
per window, per beat and per song, whether the work depends on the genre, the key or the result, and how much of the
variation is server load. Nothing here uses the labels; it reads the request sizes and the response times only.

    python profile_time_experiments.py --data DIR --deny-hashes sealed_corpus_holdout_feature_sha1.txt --only-split tuning
"""
import argparse, base64, collections

import numpy as np

import mikdata


def rows_of(songs):
    rows = []
    for s in songs:
        q, r = s['q'], s['r']
        times = [r.get(k) for k in ('KeyDetectionProfileTime', 'EnergyAnalysisProfileTime', 'CuePointAnalysisProfileTime')]
        if any(t is None for t in times):
            continue
        beats = q['DurationInSeconds'] * q['Tempo'] / 60.0
        windows = q['EnergySegmentData']
        rows.append(dict(key_ms=times[0] * 1e3, en_ms=times[1] * 1e3, cue_ms=times[2] * 1e3, nwin=len(windows), beats=beats,
                         dur=q['DurationInSeconds'], cue_bytes=len(base64.b64decode(q['CuePointData'])) if q.get('CuePointData') else 0,
                         grp=s['grp'], genre=s['genre'] or '(none)', key=r['KeySummary']['MainKey'], conf=r['KeySummary']['MainKeyConfidence'],
                         overall=r.get('OverallEnergy')))
    return rows


def col(rows, name):
    return np.array([r[name] for r in rows], float)


def spread(x):
    return f'min {x.min():6.2f}  p10 {np.percentile(x, 10):6.2f}  median {np.median(x):6.2f}  p90 {np.percentile(x, 90):6.2f}  max {x.max():7.2f}'


def fit(x, y, trim=0.9):
    """Straight line through the lower `trim` share of residuals (server load only ever adds time)."""
    p = np.polyfit(x, y, 1)
    keep = np.abs(y - np.polyval(p, x)) <= np.quantile(np.abs(y - np.polyval(p, x)), trim)
    p = np.polyfit(x[keep], y[keep], 1)
    resid = y[keep] - np.polyval(p, x[keep])
    r2 = 1 - resid.var() / y[keep].var()
    quad = np.polyfit(x[keep], y[keep], 2)
    return p, r2, quad[0]


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument('--data', required=True); ap.add_argument('--allow-names'); ap.add_argument('--deny-hashes'); ap.add_argument('--only-split')
    args = ap.parse_args()
    songs = list(mikdata.iter_songs(args.data, mikdata.read_list(args.allow_names), mikdata.read_list(args.deny_hashes), args.only_split))
    rows = rows_of(songs)
    print('songs with the three times:', len(rows), dict(collections.Counter(r['grp'] for r in rows)))

    print('\n== how long each stage takes (ms)')
    for name in ('key_ms', 'en_ms', 'cue_ms'):
        print(f'  {name:7s}', spread(col(rows, name)))

    print('\n== what the time depends on (line through the lower 90% of residuals)')
    beats, cue_ms = col(rows, 'beats'), col(rows, 'cue_ms')
    p, r2, quad = fit(beats, cue_ms)
    print(f'  cue    : {p[0] * 1e3:.2f} us per beat + {p[1]:.2f} ms, R2 {r2:.3f}, quadratic term {quad:.1e} per beat^2 (a pairwise search would show here)')
    nwin, en_ms = col(rows, 'nwin'), col(rows, 'en_ms')
    p, r2, quad = fit(nwin, en_ms)
    print(f'  energy : {p[0]:.3f} ms per window + {p[1]:.2f} ms, R2 {r2:.3f} (windows per song {int(nwin.min())}-{int(nwin.max())})')
    key_ms = col(rows, 'key_ms')
    for name in ('dur', 'beats', 'nwin', 'cue_bytes', 'conf'):
        print(f'  key time vs {name:9s}: correlation {np.corrcoef(col(rows, name), key_ms)[0, 1]:+.3f}')

    print('\n== server load: does a slow request slow every stage?')
    per_window, per_beat = en_ms / nwin, cue_ms / beats
    print(f'  corr(key, energy per window) {np.corrcoef(key_ms, per_window)[0, 1]:+.3f}   corr(key, cue per beat) {np.corrcoef(key_ms, per_beat)[0, 1]:+.3f}'
          f'   corr(energy per window, cue per beat) {np.corrcoef(per_window, per_beat)[0, 1]:+.3f}')
    floor = np.percentile(key_ms, 5)
    print(f'  key floor (5th percentile) {floor:.2f} ms; share of requests more than 1.5x the floor: {np.mean(key_ms > 1.5 * floor):.2f}')

    print('\n== relative cost (medians): one key decision = {:.1f} energy-window evaluations = {:.0f} beats of cue scoring'.format(
        np.median(key_ms) / np.median(per_window), np.median(key_ms) / np.median(per_beat)))

    print('\n== same work for every genre? (median per genre: key ms, energy ms per window, cue us per beat)')
    by = collections.defaultdict(list)
    for r in rows:
        by[r['grp']].append(r); by['genre: ' + r['genre']].append(r)
    for name, group in sorted(by.items(), key=lambda kv: -len(kv[1])):
        if len(group) >= 5:
            print(f'  {name:46s} n={len(group):4d}  key {np.median(col(group, "key_ms")):6.2f}  window {np.median(col(group, "en_ms") / col(group, "nwin")):.3f}'
                  f'  beat {np.median(col(group, "cue_ms") / col(group, "beats")) * 1e3:.2f}')

    print('\n== does the key search depend on the result? (median key ms by detected tonic mode, and by confidence band)')
    for mode in ('A', 'B'):
        group = [r for r in rows if r['key'].endswith(mode)]
        if group:
            print(f'  mode {mode}: n={len(group):4d} median {np.median(col(group, "key_ms")):.2f} ms')
    for lo, hi in ((0, 0.7), (0.7, 0.9), (0.9, 0.97), (0.97, 1.01)):
        group = [r for r in rows if lo <= r['conf'] < hi]
        if group:
            print(f'  confidence {lo}-{hi}: n={len(group):4d} median {np.median(col(group, "key_ms")):.2f} ms')


if __name__ == '__main__':
    main()
