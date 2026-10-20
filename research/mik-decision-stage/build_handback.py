#!/usr/bin/env python3
"""Package the output of gather_mik_data.py for hand-back, or verify a package that was received.

    python3 build_handback.py --gather ~/mik-gather --notes ~/NOTES.md --out ~/mik-handback    # on the capture laptop
    python3 build_handback.py --verify ~/mik-handback                                          # again on the research machine

The package is a folder: the gatherer's archives and database backup, `MANIFEST.json` (size and SHA-256 of every file, counts,
tool versions), `HANDBACK.md` (what is inside and how to continue) and the notes written by whoever made it. Building ends with
a verification. Verifying re-hashes every file, tests the archives, checks the counts against each other and checks that no
credential-looking value survives in the sanitized request/response pairs. Nothing is analysed, and nothing is sent anywhere.
"""
import argparse, datetime, hashlib, json, os, re, shutil, subprocess, sys, zipfile

REQUIRED = ('mik-metadata.zip', 'mik-pairs.zip', 'report.json', 'setup.json')
OPTIONAL = ('Collection11.backup.mikdb', 'missing_from_capture.txt', 'gather.log')
TOOLS = ('gather_mik_data.py', 'import_capture.py', 'build_handback.py')
SECRET = re.compile(r'vipcode|authorization|cookie|token|password|secret|licen[cs]e|apikey|session|email|username', re.I)


def sha256(path):
    digest = hashlib.sha256()
    with open(path, 'rb') as handle:
        for chunk in iter(lambda: handle.read(1 << 22), b''):
            digest.update(chunk)
    return digest.hexdigest()


def copy_file(src, dst):
    """Clone on APFS when possible (instant, no extra space), else a normal copy."""
    if subprocess.run(['cp', '-c', src, dst], capture_output=True).returncode:
        shutil.copyfile(src, dst)


def summary_of(report):
    export, capture, tags, inventory, setup = (report.get(k) or {} for k in ('export', 'capture', 'tags', 'inventory', 'setup'))
    return {'songs': export.get('songs'), 'pairs': capture.get('pairs'), 'pairs_joined': capture.get('pairs_joined'),
            'database_songs_without_pair': capture.get('database_songs_without_pair'),
            'pairs_without_database_song': capture.get('pairs_without_database_song'),
            'pairs_with_error_or_no_segments': capture.get('pairs_with_error_or_no_segments', 0),
            'stored_equals_response': {k: capture.get(k) for k in ('key_equal', 'energy_equal', 'tempo_equal_1e-3', 'cue_count_equal', 'response_cues_all_in_database', 'downbeat_on_stored_grid_10ms')},
            'genres': (capture.get('import_summary') or {}).get('genres'),
            'tags': {k: v for k, v in tags.items() if k in ('read', 'error', 'missing', 'mutagen_missing')},
            'inventory': {k: v for k, v in inventory.items()}, 'mik_version': setup.get('mik_version'), 'mik_build': setup.get('mik_build'),
            'system': setup.get('system'), 'machine': setup.get('machine')}


def handback_text(summary, files):
    rows = '\n'.join(f'| `{f["name"]}` | {f["bytes"] / 1e6:,.1f} MB | `{f["sha256"][:16]}...` |' for f in files)
    return f"""# Mixed In Key capture, hand-back package

Made {datetime.datetime.now().isoformat(timespec='seconds')} by `tools/build_handback.py` from the output of `tools/gather_mik_data.py`.
Mixed In Key {summary['mik_version']} build {summary['mik_build']}; {summary['system']} ({summary['machine']}).
Private: it holds file names and paths. Do not commit or publish it.

| Count | Value |
| --- | --- |
| songs analysed in the Mixed In Key database | {summary['songs']} |
| captured request/response pairs | {summary['pairs']} |
| pairs joined to a database song | {summary['pairs_joined']} |
| database songs without a captured pair | {summary['database_songs_without_pair']} |
| captured pairs without a database song | {summary['pairs_without_database_song']} |

## Files

| File | Size | SHA-256 |
| --- | --- | --- |
{rows}

- `mik-pairs.zip`: the sanitized `<id>.request.json` / `<id>.response.json` pairs (credential-looking values replaced by
  `"[REDACTED]"`, every other number exactly as received). Unzip it: the `pairs/` folder is the dataset folder the research
  code takes.
- `mik-metadata.zip`: `final.jsonl` (per song: stored key, energy, tempo, genre, cues, energy and key segments, decoded beat
  grid and every other stored value), `tags.jsonl` (every tag frame written to each file, no artwork), `inventory.jsonl`
  (every file under the scanned folders), `setup.json`, `report.json`, `missing_from_capture.txt`, `gather.log`.
- `Collection11.backup.mikdb`: a consistent copy of the whole Mixed In Key database.
- `NOTES.md`: facts recorded by whoever made the package (commands, durations, anything that failed).
- `tools/`: the exact scripts that produced it.

## To continue

1. `python3 tools/build_handback.py --verify .` re-hashes every file and re-checks the counts.
2. Unzip `mik-pairs.zip` and `mik-metadata.zip` into a private folder outside the repository.
3. Drop the sealed songs of the first corpus (`--deny-hashes research/mik-decision-stage/sealed_corpus_holdout_feature_sha1.txt`)
   and follow `research/mik-decision-stage/README.md` and `AGENT_TASK.md`. Format of every file: `DATA_FORMAT.md`.
"""


def build(args):
    gather, out = os.path.abspath(os.path.expanduser(args.gather)), os.path.abspath(os.path.expanduser(args.out))
    missing = [n for n in REQUIRED if not os.path.exists(os.path.join(gather, n))]
    if missing:
        raise SystemExit('Missing from the gather folder: ' + ', '.join(missing) + '. Run gather_mik_data.py with --flow first.')
    report = json.load(open(os.path.join(gather, 'report.json')))
    if not report.get('capture'):
        raise SystemExit('report.json has no capture section: run gather_mik_data.py with --flow (or --pairs).')
    os.makedirs(out, exist_ok=True)
    names = list(REQUIRED) + [n for n in OPTIONAL if os.path.exists(os.path.join(gather, n))]
    for name in names:
        dst = os.path.join(out, name)
        if name == 'Collection11.backup.mikdb' and args.compress_db:
            dst += '.zip'
            with zipfile.ZipFile(dst, 'w', zipfile.ZIP_DEFLATED, compresslevel=1) as archive:
                archive.write(os.path.join(gather, name), name)
        else:
            copy_file(os.path.join(gather, name), dst)
    if args.notes and os.path.exists(os.path.expanduser(args.notes)):
        shutil.copyfile(os.path.expanduser(args.notes), os.path.join(out, 'NOTES.md'))
    else:
        open(os.path.join(out, 'NOTES.md'), 'w').write('No notes were provided.\n')
    os.makedirs(os.path.join(out, 'tools'), exist_ok=True)
    here = os.path.dirname(os.path.abspath(__file__))
    for tool in TOOLS:
        if os.path.exists(os.path.join(here, tool)):
            shutil.copyfile(os.path.join(here, tool), os.path.join(out, 'tools', tool))
    summary = summary_of(report)
    listing = sorted(f for f in os.listdir(out) if os.path.isfile(os.path.join(out, f)) and f not in ('MANIFEST.json', 'HANDBACK.md')) \
        + ['tools/' + t for t in sorted(os.listdir(os.path.join(out, 'tools')))]
    files = [{'name': n, 'bytes': os.path.getsize(os.path.join(out, n)), 'sha256': sha256(os.path.join(out, n))} for n in listing]
    open(os.path.join(out, 'HANDBACK.md'), 'w').write(handback_text(summary, files))
    manifest = {'format': 1, 'made': datetime.datetime.now().isoformat(timespec='seconds'), 'summary': summary, 'files': files}
    json.dump(manifest, open(os.path.join(out, 'MANIFEST.json'), 'w'), indent=2)
    print(f'package written to {out} ({sum(f["bytes"] for f in files) / 1e9:.2f} GB, {len(files)} files)')
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
    problems, notes = [], []
    manifest = json.load(open(os.path.join(folder, 'MANIFEST.json')))
    for entry in manifest['files']:
        path = os.path.join(folder, entry['name'])
        if not os.path.exists(path):
            problems.append('missing file ' + entry['name']); continue
        if os.path.getsize(path) != entry['bytes'] or sha256(path) != entry['sha256']:
            problems.append('changed or damaged: ' + entry['name'])
    summary = manifest['summary']
    intact = {}
    for name in ('mik-metadata.zip', 'mik-pairs.zip'):
        path = os.path.join(folder, name)
        intact[name] = False
        if os.path.exists(path):
            try:
                bad = zipfile.ZipFile(path).testzip()
            except (zipfile.BadZipFile, OSError) as error:
                bad = str(error)
            if bad:
                problems.append(f'{name}: damaged ({bad})')
            else:
                intact[name] = True
    if intact['mik-pairs.zip']:
        archive = zipfile.ZipFile(os.path.join(folder, 'mik-pairs.zip'))
        requests = [n for n in archive.namelist() if n.endswith('.request.json')]
        responses = {n for n in archive.namelist() if n.endswith('.response.json')}
        unmatched = [n for n in requests if n.replace('.request.json', '.response.json') not in responses]
        if len(requests) != summary['pairs'] or unmatched:
            problems.append(f'pairs: {len(requests)} request files ({len(unmatched)} without a response) but the report says {summary["pairs"]}')
        leaks = 0
        for name in requests + sorted(responses):
            found = []
            walk_secrets(json.loads(archive.read(name)), found)
            leaks += bool(found)
        if leaks:
            problems.append(f'pairs: {leaks} files hold a credential-looking value that is not redacted')
        notes.append(f'{len(requests)} pairs, {leaks} files with an unredacted credential-looking value')
    if intact['mik-metadata.zip']:
        archive = zipfile.ZipFile(os.path.join(folder, 'mik-metadata.zip'))
        if 'final.jsonl' not in archive.namelist():
            problems.append('mik-metadata.zip has no final.jsonl')
        else:
            songs = sum(1 for _ in archive.open('final.jsonl'))
            notes.append(f'{songs} songs in final.jsonl')
            if songs != summary['songs']:
                problems.append(f'final.jsonl has {songs} songs but the report says {summary["songs"]}')
        for name in ('tags.jsonl', 'inventory.jsonl'):
            if name not in archive.namelist():
                problems.append(f'mik-metadata.zip has no {name}')
    if (summary.get('tags') or {}).get('mutagen_missing') or not (summary.get('tags') or {}).get('read'):
        problems.append('no tags were read (install mutagen: python3 -m pip install --user mutagen) and rerun gather_mik_data.py')
    if not os.path.exists(os.path.join(folder, 'Collection11.backup.mikdb')) and not os.path.exists(os.path.join(folder, 'Collection11.backup.mikdb.zip')):
        problems.append('no database backup')
    for line in notes:
        print('  ' + line)
    if problems:
        print('VERIFY FAILED:'); [print('  - ' + p) for p in problems]
        return 1
    print(f'VERIFY OK: {len(manifest["files"])} files match the manifest')
    return 0


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument('--gather', help='output folder of gather_mik_data.py')
    ap.add_argument('--out', help='folder to create')
    ap.add_argument('--notes', help='NOTES.md written by whoever made the package')
    ap.add_argument('--compress-db', action='store_true', help='store the database backup zipped (smaller to move, slower to make)')
    ap.add_argument('--verify', metavar='DIR', help='verify an existing package instead of building one')
    args = ap.parse_args()
    if args.verify:
        sys.exit(verify(args.verify))
    if not (args.gather and args.out):
        ap.error('--gather and --out are required (or use --verify DIR)')
    sys.exit(build(args))


if __name__ == '__main__':
    main()
