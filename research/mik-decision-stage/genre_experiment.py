#!/usr/bin/env python3
"""Does the genre tag the application sends change the service's decisions? A controlled test with identical audio.

    python3 genre_experiment.py make    --gather ~/mik-gather --out ~/genre-test            # on the machine with the music
    (scan ~/genre-test/files in Mixed In Key while mitmweb saves the flows, see GENRE-EXPERIMENT.md)
    python3 genre_experiment.py pack    --plan ~/genre-test/plan.json --flow genre.flow --out ~/genre-handback
    python3 genre_experiment.py compare --plan plan.json --pairs DIR                        # the same comparison, anywhere
    python3 genre_experiment.py verify  ~/genre-handback

`make` copies a few tracks (FLAC only, the audio stays byte-identical), strips every tag from the copies and writes one copy
per genre variant. Everything else about the copies is the same, so any difference between the responses of one track comes
from the tag that changed. `plan.json` holds synthetic names and the variant list, no titles, artists or paths.
Nothing here contacts any server; `pack` reads the saved flow file offline with mitmdump.
"""
import argparse, collections, datetime, hashlib, json, os, random, re, shutil, struct, subprocess, sys, zipfile

GENRES = ['Melodic House & Techno', 'Techno (Peak Time / Driving)', 'Techno (Raw / Deep / Hypnotic)', 'Trance (Main Floor)',
          'Hardstyle', 'Tech House', 'Drum & Bass', 'Progressive House', 'Deep House', 'Pop']
VARIANTS = [('g%02d' % i, g) for i, g in enumerate(GENRES)] + [('none', ''), ('meta', None)]  # ('meta': original genre, other tags neutral)
CAMELOT_ORDER = 'strata: low (<0.7), middle, high (>=0.97) stored key confidence'
SECRET = re.compile(r'vipcode|authorization|cookie|token|password|secret|licen[cs]e|apikey|session|email|username', re.I)


def feature_hash(request):
    """SHA-1 of the 352 key features as float64, the same identity the research code uses (needs no numpy)."""
    f = request['Segments'][0]['Features']
    return hashlib.sha1(struct.pack('<%dd' % len(f), *map(float, f))).hexdigest()


def sha256(path):
    digest = hashlib.sha256()
    with open(path, 'rb') as handle:
        for chunk in iter(lambda: handle.read(1 << 22), b''):
            digest.update(chunk)
    return digest.hexdigest()


def first(value):
    while isinstance(value, list) and value:
        value = value[0]
    return None if isinstance(value, list) else value


# ---------------------------------------------------------------- make

def choose_tracks(final, count, seed):
    """Spread the tracks over the stored key confidence: the low-confidence ones are where a hidden input would show first."""
    rng = random.Random(seed)
    usable = []
    for song in final:
        path = song.get('path') or ''
        if not path.lower().endswith('.flac') or not os.path.exists(path):
            continue
        segments = song.get('key_segments') or []
        confidence = segments[0].get('confidence') if segments else None
        usable.append((confidence if confidence is not None else 1.0, song))
    strata = [[s for c, s in usable if c < 0.7], [s for c, s in usable if 0.7 <= c < 0.97], [s for c, s in usable if c >= 0.97]]
    chosen, per = [], [count // 3, count // 3, count - 2 * (count // 3)]
    for group, n in zip(strata, per):
        rng.shuffle(group)
        chosen += group[:n]
    rest = [s for group in strata for s in group if s not in chosen]
    rng.shuffle(rest)
    return (chosen + rest)[:count]


def make(args):
    from mutagen.flac import FLAC
    gather, out = os.path.abspath(os.path.expanduser(args.gather)), os.path.abspath(os.path.expanduser(args.out))
    final = [json.loads(line) for line in open(os.path.join(gather, 'final.jsonl'))]
    tracks = choose_tracks(final, args.tracks, args.seed)
    if len(tracks) < args.tracks:
        print(f'only {len(tracks)} usable FLAC tracks found')
    need = sum(os.path.getsize(t['path']) for t in tracks) * len(VARIANTS)
    os.makedirs(out, exist_ok=True)
    free = shutil.disk_usage(out).free
    if need + 3e9 > free:
        raise SystemExit(f'needs about {need / 1e9:.1f} GB plus 3 GB spare, {free / 1e9:.1f} GB free: use --tracks with a smaller number')
    files_dir = os.path.join(out, 'files')
    os.makedirs(files_dir, exist_ok=True)
    plan = {'format': 1, 'made': datetime.datetime.now().isoformat(timespec='seconds'), 'strata': CAMELOT_ORDER, 'tracks': [], 'files': []}
    for number, song in enumerate(tracks, 1):
        tags = FLAC(song['path'])
        keep = {k: tags[k][0] for k in ('title', 'artist') if k in tags and tags[k]}
        original_genre = first(tags.get('genre')) or ''
        stored = (song.get('key_segments') or [{}])[0].get('confidence')
        plan['tracks'].append({'track': number, 'original_genre': original_genre, 'stored_key': song.get('key'), 'stored_confidence': stored,
                               'stored_energy': song.get('energy'), 'tempo': song.get('tempo')})
        for code, genre in VARIANTS:
            name = f't{number:02d}_{code}.flac'
            destination = os.path.join(files_dir, name)
            shutil.copyfile(song['path'], destination)
            copy = FLAC(destination)
            copy.delete()
            copy.clear_pictures()
            if code == 'meta':
                copy['title'], copy['artist'] = f'Test track {number}', 'Test artist'
                genre = original_genre
            else:
                for key, value in keep.items():
                    copy[key] = value
            if genre:
                copy['genre'] = genre
            copy.save()
            plan['files'].append({'name': name, 'track': number, 'variant': code, 'genre': genre or '', 'meta_neutral': code == 'meta'})
    json.dump(plan, open(os.path.join(out, 'plan.json'), 'w'), indent=1)
    print(f'{len(plan["files"])} files ({len(tracks)} tracks x {len(VARIANTS)} variants, {need / 1e9:.1f} GB) in {files_dir}; plan in {out}/plan.json')
    print('Scan that folder in Mixed In Key while mitmweb saves the flows (see GENRE-EXPERIMENT.md).')


# ---------------------------------------------------------------- compare

def comparable(response):
    key = response.get('KeySummary') or {}
    return {'key': key.get('MainKey'), 'confidence': key.get('MainKeyConfidence'), 'second': key.get('SecondKey'),
            'overall': response.get('OverallEnergy'),
            'cues': [(c['Time'], c['EnergyLevel']) for c in response.get('CuePoints') or []],
            'segments': [(s['EnergyLevel'], s['StartTime'], s['EndTime'], s['Volume']) for s in response.get('EnergyLevelSegments') or []]}


def load_pairs(pairs_dir):
    pairs = {}
    for name in sorted(os.listdir(pairs_dir)):
        if not name.endswith('.request.json'):
            continue
        request = json.load(open(os.path.join(pairs_dir, name)))
        response = json.load(open(os.path.join(pairs_dir, name.replace('.request.json', '.response.json'))))
        stem = os.path.splitext((request.get('EliteData') or {}).get('FileName') or '')[0]
        pairs.setdefault(stem, []).append((request, response))
    return pairs


def compare(plan, pairs):
    """Counts only: were the inputs identical, did the tag reach the request, and did any output differ between variants."""
    report = collections.OrderedDict()
    by_track = collections.defaultdict(list)
    missing, repeated, errors = [], [], 0
    for entry in plan['files']:
        stem = os.path.splitext(entry['name'])[0]
        got = pairs.get(stem) or []
        if not got:
            missing.append(entry['name']); continue
        if len(got) > 1:
            repeated.append(entry['name'])
        request, response = got[0]
        if response.get('Error') or not response.get('Segments'):
            errors += 1; continue
        by_track[entry['track']].append((entry, request, response))
    report['files_planned'], report['files_captured'] = len(plan['files']), len(plan['files']) - len(missing)
    report['missing'], report['captured_more_than_once'], report['error_responses'] = len(missing), len(repeated), errors
    sent_ok = sum(1 for rows in by_track.values() for e, q, r in rows if ((q.get('EliteData') or {}).get('Genre') or '') == e['genre'] or e['meta_neutral'])
    report['genre_sent_equals_tag'] = f"{sent_ok}/{sum(len(v) for v in by_track.values())}"
    sent_values = collections.Counter(((q.get('EliteData') or {}).get('Genre') or '(none)') for rows in by_track.values() for e, q, r in rows)
    report['genres_sent'] = dict(sent_values.most_common())
    same_audio = 0; differing = collections.Counter(); per_track = []
    changed_by_variant = collections.defaultdict(collections.Counter)
    for track, rows in sorted(by_track.items()):
        hashes = {feature_hash(q) for e, q, r in rows}
        same_audio += len(hashes) == 1
        base = next((x for x in rows if x[0]['variant'] == 'g00'), rows[0])
        base_view = comparable(base[2])
        track_diff = collections.Counter()
        for entry, q, r in rows:
            view = comparable(r)
            for field in base_view:
                if view[field] != base_view[field]:
                    track_diff[field] += 1; changed_by_variant[entry['variant']][field] += 1
        differing.update(track_diff)
        per_track.append({'track': track, 'variants': len(rows), 'audio_identical': len(hashes) == 1, 'fields_that_differ_from_g00': dict(track_diff)})
    report['tracks_compared'] = len(by_track)
    report['tracks_with_identical_features_in_all_variants'] = same_audio
    report['field_differences_vs_first_genre_total'] = dict(differing)
    report['field_differences_by_variant'] = {v: dict(c) for v, c in sorted(changed_by_variant.items())}
    report['per_track'] = per_track
    report['verdict'] = ('no output changed with the tag' if not differing and not missing and same_audio == len(by_track) and sent_ok
                         else 'outputs changed or the test is incomplete: read the fields above')
    return report


# ---------------------------------------------------------------- pack / verify

def pack(args):
    plan_path = os.path.abspath(os.path.expanduser(args.plan))
    plan = json.load(open(plan_path))
    out = os.path.abspath(os.path.expanduser(args.out))
    os.makedirs(out, exist_ok=True)
    pairs_dir = os.path.join(out, 'pairs')
    here = os.path.dirname(os.path.abspath(__file__))
    importer = os.path.join(here, 'import_capture.py')
    if not shutil.which('mitmdump'):
        raise SystemExit('mitmdump not found: it comes with mitmproxy, next to mitmweb')
    if not os.path.isdir(pairs_dir) or not os.listdir(pairs_dir):
        subprocess.run(['mitmdump', '-q', '-nr', os.path.expanduser(args.flow), '-s', importer, '--set', f'out_dir={pairs_dir}'], check=True)
    report = compare(plan, load_pairs(pairs_dir))
    json.dump(report, open(os.path.join(out, 'genre_report.json'), 'w'), indent=1)
    shutil.copyfile(plan_path, os.path.join(out, 'plan.json'))
    zip_path = os.path.join(out, 'genre-pairs.zip')
    with zipfile.ZipFile(zip_path, 'w', zipfile.ZIP_DEFLATED) as archive:
        for name in sorted(os.listdir(pairs_dir)):
            archive.write(os.path.join(pairs_dir, name), 'pairs/' + name)
    os.makedirs(os.path.join(out, 'tools'), exist_ok=True)
    for tool in ('genre_experiment.py', 'import_capture.py'):
        shutil.copyfile(os.path.join(here, tool), os.path.join(out, 'tools', tool))
    files = ['genre-pairs.zip', 'plan.json', 'genre_report.json', 'tools/genre_experiment.py', 'tools/import_capture.py']
    manifest = {'format': 1, 'made': datetime.datetime.now().isoformat(timespec='seconds'),
                'files': [{'name': n, 'bytes': os.path.getsize(os.path.join(out, n)), 'sha256': sha256(os.path.join(out, n))} for n in files]}
    json.dump(manifest, open(os.path.join(out, 'MANIFEST.json'), 'w'), indent=1)
    print(json.dumps({k: v for k, v in report.items() if k != 'per_track'}, indent=1))
    return verify(out)


def walk_secrets(node, found):
    if isinstance(node, dict):
        for key, value in node.items():
            if SECRET.search(re.sub(r'[^a-z]', '', key.lower())) and value != '[REDACTED]':
                found.append(key)
            walk_secrets(value, found)
    elif isinstance(node, list):
        for value in node:
            walk_secrets(value, found)


def verify(folder):
    folder = os.path.abspath(os.path.expanduser(folder))
    problems = []
    manifest = json.load(open(os.path.join(folder, 'MANIFEST.json')))
    for entry in manifest['files']:
        path = os.path.join(folder, entry['name'])
        if not os.path.exists(path):
            problems.append('missing ' + entry['name'])
        elif os.path.getsize(path) != entry['bytes'] or sha256(path) != entry['sha256']:
            problems.append('changed or damaged: ' + entry['name'])
    if not problems:
        archive = zipfile.ZipFile(os.path.join(folder, 'genre-pairs.zip'))
        if archive.testzip():
            problems.append('genre-pairs.zip is damaged')
        leaks = 0
        for name in archive.namelist():
            if name.endswith('.json') and 'import-summary' not in name:
                found = []
                walk_secrets(json.loads(archive.read(name)), found)
                leaks += bool(found)
        if leaks:
            problems.append(f'{leaks} pair files hold a credential-looking value that is not redacted')
        report = json.load(open(os.path.join(folder, 'genre_report.json')))
        if report['files_captured'] < report['files_planned']:
            problems.append(f"only {report['files_captured']} of {report['files_planned']} planned files were captured (say so in the notes; do not rescan)")
    if problems:
        print('VERIFY FAILED:'); [print('  - ' + p) for p in problems]
        return 1
    print(f'VERIFY OK: {len(manifest["files"])} files match the manifest')
    return 0


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    sub = ap.add_subparsers(dest='command', required=True)
    m = sub.add_parser('make'); m.add_argument('--gather', required=True, help='gather_mik_data.py output folder (needs final.jsonl)')
    m.add_argument('--out', required=True); m.add_argument('--tracks', type=int, default=24); m.add_argument('--seed', type=int, default=1)
    p = sub.add_parser('pack'); p.add_argument('--plan', required=True); p.add_argument('--flow', required=True); p.add_argument('--out', required=True)
    c = sub.add_parser('compare'); c.add_argument('--plan', required=True); c.add_argument('--pairs', required=True)
    v = sub.add_parser('verify'); v.add_argument('folder')
    args = ap.parse_args()
    if args.command == 'make':
        make(args)
    elif args.command == 'pack':
        sys.exit(pack(args))
    elif args.command == 'compare':
        report = compare(json.load(open(os.path.expanduser(args.plan))), load_pairs(os.path.expanduser(args.pairs)))
        print(json.dumps(report, indent=1))
    else:
        sys.exit(verify(args.folder))


if __name__ == '__main__':
    main()
