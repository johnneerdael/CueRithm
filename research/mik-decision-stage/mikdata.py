"""Shared loading, labelling and reporting helpers for the Mixed In Key decision-stage research.

A dataset is a folder of sanitized request/response pairs: ``<id>.request.json`` with ``<id>.response.json`` (any id;
the capture tooling names them ``transaction-<id>``). Nothing here talks to any server.
"""
import base64, collections, glob, hashlib, json, math, os, struct
import numpy as np

# Camelot code -> (tonic pitch class with C = 0, 1 for minor / 0 for major)
MINOR = {'1A': 8, '2A': 3, '3A': 10, '4A': 5, '5A': 0, '6A': 7, '7A': 2, '8A': 9, '9A': 4, '10A': 11, '11A': 6, '12A': 1}
MAJOR = {'1B': 11, '2B': 6, '3B': 1, '4B': 8, '5B': 3, '6B': 10, '7B': 5, '8B': 0, '9B': 7, '10B': 2, '11B': 9, '12B': 4}
CAMELOT = {k: (v, 1) for k, v in MINOR.items()} | {k: (v, 0) for k, v in MAJOR.items()}
FROM_CAMELOT = {v: k for k, v in CAMELOT.items()}


def feature_hash(request):
    """Identity of the audio as the server saw it: SHA-1 of the 352 key features. Equal hashes, equal songs."""
    return hashlib.sha1(np.array(request['Segments'][0]['Features'], float).tobytes()).hexdigest()


def split_of(feature_hash_hex, every=5):
    """Fixed split for datasets without one: one in ``every`` songs is held out, decided by a hash of the song only."""
    digest = hashlib.sha256(('camelot-v2:' + feature_hash_hex).encode()).hexdigest()
    return 'holdout' if int(digest[:8], 16) % every == 0 else 'tuning'


def genre_group(genre):
    return 'MHT' if (genre or '').startswith('Melodic House & Techno') else 'other'


def decode_cue_scores(encoded):
    raw = base64.b64decode(encoded, validate=True)
    if len(raw) < 2 or (len(raw) - 2) % 4:
        raise ValueError('invalid CuePointData packing')
    values = [x[0] for x in struct.iter_unpack('<f', raw[2:])]
    return np.array([0.0] * raw[0] + values + [0.0] * raw[1])


def time_at_beat(downbeat, tempo, beat):
    second = downbeat + 60.0 / tempo
    period = second - downbeat
    return second + (beat - 1) * period if beat >= 1 else downbeat + beat * period


AUDIO_EXTENSIONS = ('.flac', '.mp3', '.wav', '.m4a', '.aiff', '.aif', '.ogg', '.opus', '.wma')


def strip_audio_extension(name):
    """Drop a trailing audio extension only; names such as '58. Artist - Title' contain dots of their own."""
    return name[:-len(ext)] if any(name.lower().endswith(ext) for ext in AUDIO_EXTENSIONS) and (ext := next(e for e in AUDIO_EXTENSIONS if name.lower().endswith(e))) else name


def read_list(path):
    return set(l.strip() for l in open(path) if l.strip()) if path else None


def iter_songs(data_dir, allow_names=None, deny_hashes=None, only_split=None, dedupe=True):
    """Yield one dict per distinct song. ``allow_names`` filters on EliteData.FileName (without extension),
    ``deny_hashes`` drops songs by feature hash (use it for any sealed set), ``only_split`` applies split_of()."""
    seen = set()
    for f in sorted(glob.glob(os.path.join(data_dir, '*.request.json'))):
        g = f.replace('.request.json', '.response.json')
        if not os.path.exists(g):
            continue
        q, r = json.load(open(f)), json.load(open(g))
        if r.get('Error') or not r.get('Segments'):
            continue
        name = strip_audio_extension(q.get('EliteData', {}).get('FileName', '')) or os.path.basename(f)
        h = feature_hash(q)
        if allow_names is not None and name not in allow_names:
            continue
        if deny_hashes and h in deny_hashes:
            continue
        if only_split and split_of(h) != only_split:
            continue
        if dedupe and h in seen:
            continue
        seen.add(h)
        genre = q.get('EliteData', {}).get('Genre') or ''
        yield dict(id=os.path.basename(f).split('.request')[0], name=name, h=h, q=q, r=r, genre=genre, grp=genre_group(genre))


def window_arrays(song):
    """Raw energy windows of one song: features (n,183), the level the server gave the window midpoint, volume."""
    q, r = song['q'], song['r']
    X, lev, vol = [], [], []
    for w in q['EnergySegmentData']:
        if not w['Features']:
            continue
        mid = (w['StartTime'] + w['EndTime']) / 2
        level = 0
        for s in r['EnergyLevelSegments']:
            if s['StartTime'] < mid < s['EndTime']:
                level = s['EnergyLevel']
        X.append(w['Features']); lev.append(level); vol.append(w['VolumeRmsDb'])
    return np.array(X, float), np.array(lev), np.array(vol)


def cue_truth_beats(song):
    """The server's cue points as integer beats from the downbeat (the first one is always the origin)."""
    q = song['q']
    period = 60.0 / q['Tempo']
    return sorted(int(round((c['Time'] - q['DownbeatTime']) / period)) for c in song['r']['CuePoints'])


def breakdown(title, songs, ok):
    """Agreement of one boolean/float per song, split by MHT / other and by the exact genre tag."""
    print(f'\n== {title}')
    by = collections.defaultdict(list)
    for s, o in zip(songs, ok):
        by[s['grp']].append(float(o)); by['genre: ' + s['genre']].append(float(o))
    for k in sorted(by, key=lambda k: (k not in ('MHT', 'other'), k)):
        print(f'  {k:55s} n={len(by[k]):4d}  agree={np.mean(by[k]):.3f}')
