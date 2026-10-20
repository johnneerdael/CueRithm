"""Turn a folder of sanitized request/response pairs into one numpy cache that the experiment scripts load in a second.

    python build_cache.py --data DIR --deny-hashes sealed_corpus_holdout_feature_sha1.txt --out cache.npz [--limit N]

One entry per distinct song (equal 352-value key features are one song), in the order of ``mikdata.iter_songs``. Everything the
decision-stage experiments use is stored: the 352 key features, the energy windows with their levels and volumes, the packed
per-beat cue scores with the true cue beats, the response key, confidence and overall energy, the split and the genre tag
(never an input to the key, energy or cue models unless an experiment says so). The cache holds no file names.
"""
import argparse

import numpy as np

import mikdata as md

PAD = -999  # unused slot in cue_truth; real cue beats can be negative (the origin cue sits at CuePointStartBeat - 32)


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument('--data', required=True)
    ap.add_argument('--deny-hashes')
    ap.add_argument('--out', required=True)
    ap.add_argument('--limit', type=int, help='stop after N songs (smoke test)')
    args = ap.parse_args()
    deny = md.read_list(args.deny_hashes)
    cols = {k: [] for k in ('ids', 'h', 'split', 'genre', 'key', 'conf', 'overall', 'dur', 'tempo', 'X')}
    win = {k: [] for k in ('wx', 'wy', 'wv', 'ws')}
    cue_scores, cue_off, cue_start, cue_truth, cue_ntruth = [], [0], [], [], []
    for i, song in enumerate(md.iter_songs(args.data, deny_hashes=deny)):
        if args.limit and i >= args.limit:
            break
        q, r = song['q'], song['r']
        summary = r['KeySummary']
        cols['ids'].append(song['id']); cols['h'].append(song['h']); cols['split'].append(md.split_of(song['h'])); cols['genre'].append(song['genre'])
        cols['key'].append(summary['MainKey']); cols['conf'].append(summary['MainKeyConfidence']); cols['overall'].append(r['OverallEnergy'])
        cols['dur'].append(q['DurationInSeconds']); cols['tempo'].append(q['Tempo']); cols['X'].append(q['Segments'][0]['Features'])
        x, level, volume = md.window_arrays(song)
        win['wx'].append(x.astype(np.float32)); win['wy'].append(level.astype(np.int8)); win['wv'].append(volume.astype(np.float32))
        win['ws'].append(np.full(len(level), i, np.int32))
        scores = md.decode_cue_scores(q['CuePointData']).astype(np.float32)
        cue_scores.append(scores); cue_off.append(cue_off[-1] + len(scores)); cue_start.append(q['CuePointStartBeat'])
        beats = md.cue_truth_beats(song)[:8]
        cue_truth.append(beats + [PAD] * (8 - len(beats)))
        cue_ntruth.append(len(beats))
    n = len(cols['ids'])
    np.savez_compressed(
        args.out, **{k: np.array(v, np.float64 if k == 'X' else None) for k, v in cols.items()},
        **{k: np.concatenate(v) for k, v in win.items()},
        cue_scores=np.concatenate(cue_scores), cue_off=np.array(cue_off), cue_start=np.array(cue_start),
        cue_truth=np.array(cue_truth, np.int32), cue_ntruth=np.array(cue_ntruth))
    print(f'{n} songs, {sum(len(v) for v in win["wy"])} windows, {cue_off[-1]} cue scores -> {args.out}')


if __name__ == '__main__':
    main()
