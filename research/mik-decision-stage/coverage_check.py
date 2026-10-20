"""Do our predicted cue sets contain the important structural moments? Out of sample, counts only.

    python coverage_check.py --cache cache.npz --pairs DIR --audio DIR [--tolerance 8]

* Trains the cue ranker on every cached song that is NOT among the tracks being checked (matched by feature hash).
* Predicts eight cues per checked track from its request (no response information).
* Detects structural events from the audio bands (events.py) and reports, per kind, how many are within ``tolerance`` beats of a
  cue: for the service's own cues, for our predicted cues, and for our cues after the validator replaces the weakest cues
  by uncovered important events (never more than eight in total).
"""
import argparse
import glob
import hashlib
import json
import os
from collections import Counter, defaultdict
from multiprocessing import Pool

import numpy as np

import cue_ranker as cr
import event_view as v
import events as ev
import mikdata as md

IMPORTANT = ('intro', 'breakdown', 'buildup', 'drop', 'outro')


def curves_for(args):
    path, audio_dir = args
    request = json.load(open(path))
    audio = v.find_audio(audio_dir, request['EliteData']['FileName'])
    return (path, v.track_curves(request, audio)) if audio else (path, None)


def predict_cues(model, request):
    from cue_signal import ramp  # noqa: F401  (kept for symmetry with training)
    scores = md.decode_cue_scores(request['CuePointData']).astype(np.float32)
    start = int(request['CuePointStartBeat'])
    X, _, beat, _ = cr.song_table(scores, start, np.array([start - 32] + [-999] * 7), 1)
    picked = sorted(cr.greedy(beat, model(X), 7))
    return [start - 32] + picked


def validate(cues, events, tolerance, slots=8):
    """Add the uncovered important events, dropping the cues that cover no event first (never the origin), up to ``slots`` cues."""
    cues = list(cues)
    missing = [(k, b) for k, b in events if not any(abs(b - c) <= tolerance for c in cues)]
    order = {'drop': 0, 'breakdown': 1, 'outro': 2, 'intro': 3, 'buildup': 4}
    missing.sort(key=lambda kb: order[kb[0]])
    useless = [c for c in cues[1:] if not any(abs(b - c) <= tolerance for _, b in events)]
    for kind, b in missing:
        if len(cues) < slots:
            cues.append(b)
        elif useless:
            cues.remove(useless.pop())
            cues.append(b)
    return sorted(cues)


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument('--cache', required=True)
    ap.add_argument('--pairs', required=True)
    ap.add_argument('--audio', required=True)
    ap.add_argument('--tolerance', type=int, default=8)
    a = ap.parse_args()
    paths = sorted(glob.glob(os.path.join(a.pairs, '*.request.json')))
    with Pool(8) as pool:
        curves = dict(pool.map(curves_for, [(p, a.audio) for p in paths]))
    checked = {}
    for p in paths:
        if curves.get(p) is None:
            continue
        request = json.load(open(p))
        response = json.load(open(p.replace('.request.json', '.response.json')))
        if response.get('Error') or not response.get('CuePoints'):
            continue
        checked[p] = (request, response, md.feature_hash(request))
    hashes = {h for _, _, h in checked.values()}
    d = np.load(a.cache, allow_pickle=True)
    train = np.array([i for i, h in enumerate(d['h']) if h not in hashes])
    print(f'{len(checked)} checked tracks; ranker trained on {len(train)} cached songs that are not among them (of {len(d["h"])})')
    tables = cr.song_tables(d, train)
    model = cr.fit_ranker(tables, train, 'rank')
    stats = defaultdict(Counter)
    totals = Counter()
    slots_needed = []
    full = Counter()
    for p, (request, response, _) in checked.items():
        period = 60.0 / request['Tempo']
        mik = [int(round((c['Time'] - request['DownbeatTime']) / period)) for c in response['CuePoints']]
        ours = predict_cues(model, request)
        events = ev.detect(curves[p])
        valid = validate(ours, events, a.tolerance)
        for kind, b in events:
            totals[kind] += 1
            for name, cues in (('service', mik), ('ours', ours), ('ours + validator', valid)):
                stats[name][kind] += any(abs(b - c) <= a.tolerance for c in cues)
        for name, cues in (('service', mik), ('ours', ours), ('ours + validator', valid)):
            full[name] += all(any(abs(b - c) <= a.tolerance for c in cues) for _, b in events)
        slots_needed.append(len(events))
    print(f'events per track: mean {np.mean(slots_needed):.2f}, tracks with more than 8 events: {np.mean(np.array(slots_needed) > 8):.3f}')
    print(f'share of events within +-{a.tolerance} beats of a cue')
    print('  ' + ' ' * 18 + ''.join(f'{k:>11s}' for k in IMPORTANT) + f'{"all":>9s}{"tracks with every event":>26s}')
    for name in ('service', 'ours', 'ours + validator'):
        row = ''.join(f'{stats[name][k] / max(totals[k], 1):11.3f}' for k in IMPORTANT)
        print(f'  {name:18s}{row}{sum(stats[name].values()) / sum(totals.values()):9.3f}{full[name] / len(checked):26.3f}')


if __name__ == '__main__':
    main()
