"""How many detected structural events does a cue set cover? Counts only, over every track that has audio.

    python event_coverage.py --pairs DIR --audio DIR [--tolerance 8]

A cue covers an event when it lies within ``tolerance`` beats of it. Reported per event kind: how many events there are per
track and the share covered by the service's cues, and how many of the service's cues sit on some event.
"""
import argparse
import glob
import json
import os
from collections import Counter, defaultdict
from multiprocessing import Pool

import numpy as np

import event_view as v
import events as ev


def one(args):
    path, audio_dir = args
    request = json.load(open(path))
    response = json.load(open(path.replace('.request.json', '.response.json')))
    if response.get('Error') or not response.get('CuePoints'):
        return None
    audio = v.find_audio(audio_dir, request['EliteData']['FileName'])
    if not audio:
        return None
    curves = v.track_curves(request, audio)
    period = 60.0 / request['Tempo']
    cues = [int(round((c['Time'] - request['DownbeatTime']) / period)) for c in response['CuePoints']]
    return {'cues': cues, 'events': ev.detect(curves), 'beats': len(curves)}


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument('--pairs', required=True)
    ap.add_argument('--audio', required=True)
    ap.add_argument('--tolerance', type=int, default=8)
    ap.add_argument('--out', help='write per-track cues and events (no names) as json')
    a = ap.parse_args()
    paths = sorted(glob.glob(os.path.join(a.pairs, '*.request.json')))
    with Pool(8) as pool:
        results = [r for r in pool.map(one, [(p, a.audio) for p in paths]) if r]
    print(f'{len(results)} tracks with audio and cues')
    total, covered, per_track = Counter(), Counter(), defaultdict(list)
    cue_on_event, cues_total = 0, 0
    for r in results:
        counts = Counter(k for k, _ in r['events'])
        for kind in ('intro', 'breakdown', 'buildup', 'drop', 'outro'):
            per_track[kind].append(counts[kind])
        for kind, b in r['events']:
            total[kind] += 1
            covered[kind] += any(abs(b - c) <= a.tolerance for c in r['cues'])
        for c in r['cues'][1:]:
            cues_total += 1
            cue_on_event += any(abs(b - c) <= a.tolerance for _, b in r['events'])
    print(f'tolerance +-{a.tolerance} beats')
    for kind in ('intro', 'breakdown', 'buildup', 'drop', 'outro'):
        print(f'  {kind:10s}: {total[kind]:5d} events ({np.mean(per_track[kind]):.2f} per track), covered by the service cues: {covered[kind] / max(total[kind], 1):.3f}')
    allt = sum(total.values()); allc = sum(covered.values())
    print(f'  all events: {allt}, covered {allc / allt:.3f} | service cues (after the first) that sit on an event: {cue_on_event / cues_total:.3f}')
    if a.out:
        json.dump(results, open(a.out, 'w'))


if __name__ == '__main__':
    main()
