#!/usr/bin/env python3
"""Gather everything the decision-stage research needs from a machine that has Mixed In Key 11 and the music.

One command, run on the capture laptop after the scan has finished (quit Mixed In Key first):

    python3 gather_mik_data.py --out ~/mik-gather --flow ~/capture.flow

What it does (each step can be skipped with --skip backup,export,inventory,tags,pairs):

  backup     a consistent copy of the whole Mixed In Key database (sqlite backup, read-only on the original)
  export     final.jsonl: one line per analysed song with the finished key, energy, tempo, genre, cues, energy and key
             segments, the decoded beat grid and every other stored value
  inventory  inventory.jsonl: every file under the folders that were scanned (name, extension, size), so formats are known
  tags       tags.jsonl: every tag frame written to each audio file, no artwork and no audio (needs `pip install mutagen`)
  pairs      turns the mitmproxy flow file(s) into sanitized request/response pairs (offline) and checks the capture
             against the database
  setup      setup.json: app build, system and the analysis-related settings (an allow-list; licence data is never read)

Nothing is written to the database, the music or the flow file, and nothing is sent anywhere. The result is private:
it holds file names and paths. Copy `mik-metadata.zip`, `mik-pairs.zip` and the database backup to the research machine.

Trial run on ten songs while a scan is still going:   python3 gather_mik_data.py --limit 10 --skip backup --allow-running
"""
import argparse, base64, collections, datetime, glob, hashlib, json, os, platform, plistlib, re, shutil, sqlite3, subprocess, sys, time, unicodedata, urllib.parse, zipfile

APPLE_EPOCH = 978307200
AUDIO_EXTENSIONS = ('.flac', '.mp3', '.m4a', '.aac', '.wav', '.aif', '.aiff', '.ogg', '.oga', '.opus', '.wma', '.alac', '.mp4')
PATH_PATTERN = re.compile(rb'/[^\x00-\x1f]*?\.(?:' + b'|'.join(e[1:].encode() for e in AUDIO_EXTENSIONS) + rb')', re.I)
STRING_PATTERN = re.compile(rb'[^\x00-\x1f]{4,}')
# Settings worth recording; the preferences file also holds a licence code and an installation id, which are never read.
SETTING_INCLUDE = re.compile(r'writes|tag option|tag value|notation|key view|referencekey|pad camelot|energy|cue|analy|tempo|bpm', re.I)
SETTING_EXCLUDE = re.compile(r'vip|licen|install|serial|regist|token|password|secret|email|user|account|session', re.I)
SONG_SKIP = {'Z_ENT', 'Z_OPT', 'ZBOOKMARKDATA', 'ZCLOUDSYNCTRACKID', 'ZBEATS', 'ZTAGCOMMENTS'}


def log(*parts):
    line = time.strftime('%H:%M:%S ') + ' '.join(str(p) for p in parts)
    print(line, flush=True)
    if LOG:
        LOG.write(line + '\n'); LOG.flush()


LOG = None


def nfc(text):
    return unicodedata.normalize('NFC', text)


def stem(name):
    name = os.path.basename(name)
    lower = name.lower()
    for ext in AUDIO_EXTENSIONS:
        if lower.endswith(ext):
            return nfc(name[:-len(ext)]).lower()
    return nfc(name).lower()


def iso(apple_seconds):
    if apple_seconds is None:
        return None
    return (datetime.datetime(1970, 1, 1) + datetime.timedelta(seconds=apple_seconds + APPLE_EPOCH)).isoformat() + 'Z'


# ---------------------------------------------------------------- setup facts

def mik_running():
    try:
        return bool(subprocess.run(['pgrep', '-f', 'Mixed In Key 11.app/Contents/MacOS'], capture_output=True, text=True).stdout.strip())
    except OSError:
        return False


def setup_facts(db_path):
    facts = {'system': platform.platform(), 'machine': platform.machine(), 'python': sys.version.split()[0], 'gathered': datetime.datetime.now().isoformat(timespec='seconds')}
    app = '/Applications/Mixed In Key 11.app/Contents/Info.plist'
    if os.path.exists(app):
        info = plistlib.load(open(app, 'rb'))
        facts['mik_version'] = info.get('CFBundleShortVersionString'); facts['mik_build'] = info.get('CFBundleVersion')
    prefs = os.path.expanduser('~/Library/Preferences/com.mixedinkey.application.plist')
    settings, skipped = {}, 0
    if os.path.exists(prefs):
        try:
            for key, value in plistlib.load(open(prefs, 'rb')).items():
                wanted = SETTING_INCLUDE.search(key) and not SETTING_EXCLUDE.search(key)
                if wanted and (isinstance(value, (bool, int, float)) or (isinstance(value, str) and len(value) <= 40)):
                    settings[key] = value
                else:
                    skipped += 1
        except Exception as error:
            facts['settings_error'] = str(error)
    facts['settings'] = settings; facts['settings_not_recorded'] = skipped
    facts['database'] = db_path
    return facts


# ---------------------------------------------------------------- database

def find_db(explicit):
    if explicit:
        return explicit
    for pattern in ('~/Library/Application Support/Mixedinkey/Collection*.mikdb', '~/Library/Application Support/*/Collection*.mikdb',
                    '~/Library/Containers/*/Data/Library/Application Support/*/Collection*.mikdb'):
        found = sorted(glob.glob(os.path.expanduser(pattern)))
        if found:
            return found[-1]
    raise SystemExit('No Mixed In Key database found; pass --db PATH/Collection11.mikdb')


def backup_database(src, dst, out):
    """Consistent copy of the whole database through sqlite's backup API; the original is opened read-only."""
    def run(path):
        source = sqlite3.connect('file:' + urllib.parse.quote(path) + '?mode=ro', uri=True)
        target = sqlite3.connect(dst)
        started, last = time.time(), [-1]

        def progress(status, remaining, total):
            percent = 100 * (total - remaining) // max(total, 1)
            if percent // 10 != last[0]:
                last[0] = percent // 10
                log(f'  backup {percent}%')
        source.backup(target, pages=4096, progress=progress)
        target.execute('pragma journal_mode=delete'); target.commit()
        check = target.execute('pragma quick_check').fetchone()[0]
        target.close(); source.close()
        log(f'  backup done in {time.time() - started:.0f}s, quick_check: {check}')
        return check
    if os.path.exists(dst):
        os.remove(dst)
    try:
        return run(src)
    except sqlite3.Error as error:
        log('  read-only open failed (' + str(error) + '); copying the database files instead')
        work = os.path.join(out, 'work'); os.makedirs(work, exist_ok=True)
        for suffix in ('', '-wal', '-shm'):
            if os.path.exists(src + suffix):
                shutil.copyfile(src + suffix, os.path.join(work, os.path.basename(src) + suffix))
        check = run(os.path.join(work, os.path.basename(src)))
        shutil.rmtree(work, ignore_errors=True)
        return check


def sha256_file(path):
    digest = hashlib.sha256()
    with open(path, 'rb') as handle:
        for chunk in iter(lambda: handle.read(1 << 22), b''):
            digest.update(chunk)
    return digest.hexdigest()


def resolve_archive(blob):
    """Decode an NSKeyedArchiver blob into plain Python (dicts, lists, numbers, strings, bytes)."""
    archive = plistlib.loads(blob)
    objects = archive['$objects']

    def walk(value, depth=0):
        if depth > 60:
            return None
        if isinstance(value, plistlib.UID):
            return walk(objects[value.data], depth + 1)
        if isinstance(value, dict):
            if 'NS.keys' in value and 'NS.objects' in value:
                return {str(walk(k, depth + 1)): walk(v, depth + 1) for k, v in zip(value['NS.keys'], value['NS.objects'])}
            if 'NS.objects' in value:
                return [walk(v, depth + 1) for v in value['NS.objects']]
            if 'NS.data' in value:
                return base64.b64encode(value['NS.data']).decode()
            return {k: walk(v, depth + 1) for k, v in value.items() if k != '$class'}
        if isinstance(value, list):
            return [walk(v, depth + 1) for v in value]
        if isinstance(value, bytes):
            return base64.b64encode(value).decode()
        return value
    return walk(archive['$top']['root'])


def bookmark_path(blob):
    """The file path stored in a macOS bookmark, or None."""
    if not blob:
        return None
    matches = PATH_PATTERN.findall(blob)
    if matches:
        return nfc(max(matches, key=len).decode('utf-8', 'replace'))
    names = [m for m in STRING_PATTERN.findall(blob) if m.lower().endswith(tuple(e.encode() for e in AUDIO_EXTENSIONS))]
    return nfc(names[-1].decode('utf-8', 'replace')) if names else None


def export_songs(db_path, out_path, limit):
    db = sqlite3.connect('file:' + urllib.parse.quote(db_path) + '?mode=ro', uri=True)
    db.row_factory = sqlite3.Row
    columns = [(r['name'], r['type']) for r in db.execute('pragma table_info(ZSONG)')]
    cues, energy_segments, key_segments = (collections.defaultdict(list) for _ in range(3))
    for r in db.execute('select ZSONG, ZTIME, ZNAME, ZENERGYLEVEL from ZCUEPOINT order by ZSONG, ZTIME'):
        cues[r['ZSONG']].append({'time': r['ZTIME'], 'name': r['ZNAME'], 'energy': r['ZENERGYLEVEL']})
    for r in db.execute('select ZSONG, ZSTARTTIME, ZLENGTH, ZENERGY, ZCOMMENT from ZENERGYSEGMENT order by ZSONG, ZSTARTTIME'):
        energy_segments[r['ZSONG']].append({'start': r['ZSTARTTIME'], 'length': r['ZLENGTH'], 'energy': r['ZENERGY'], 'comment': r['ZCOMMENT']})
    for r in db.execute('select ZSONG, ZSTARTTIME, ZLENGTH, ZKEY, ZCONFIDENCE, ZENERGY, ZVOLUME, ZSINGLENOTE from ZKEYSEGMENT order by ZSONG, ZSTARTTIME'):
        key_segments[r['ZSONG']].append({'start': r['ZSTARTTIME'], 'length': r['ZLENGTH'], 'key': r['ZKEY'], 'confidence': r['ZCONFIDENCE'],
                                         'energy': r['ZENERGY'], 'volume': r['ZVOLUME'], 'single_note': r['ZSINGLENOTE']})
    songs, counts = [], collections.Counter()
    with open(out_path, 'w', encoding='utf-8') as output:
        for n, row in enumerate(db.execute('select * from ZSONG order by Z_PK')):
            if limit and n >= limit:
                break
            path = bookmark_path(row['ZBOOKMARKDATA'])
            other = {name: (iso(row[name]) if typ == 'TIMESTAMP' else row[name]) for name, typ in columns if name not in SONG_SKIP and row[name] is not None}
            beats, tags = None, None
            try:
                archive = resolve_archive(row['ZBEATS']) if row['ZBEATS'] else None
                beats = {'tempo': archive.get('MIKBeats_tempo'), 'times': archive.get('MIKBeats_beatTimes')} if isinstance(archive, dict) else None
            except Exception as error:
                counts['beats_decode_error'] += 1
            try:
                tags = resolve_archive(row['ZTAGCOMMENTS']) if row['ZTAGCOMMENTS'] else None
            except Exception:
                tags = base64.b64encode(row['ZTAGCOMMENTS']).decode() if row['ZTAGCOMMENTS'] else None
            pk = row['Z_PK']
            record = {'id': pk, 'file': os.path.basename(path) if path else None, 'path': path,
                      'key': row['ZKEY'], 'second_key': row['ZSECONDKEY'], 'energy': row['ZENERGY'], 'tempo': row['ZTEMPO'], 'genre': row['ZGENRE'],
                      'cues': cues[pk], 'energy_segments': energy_segments[pk], 'key_segments': key_segments[pk], 'beats': beats,
                      'tag_comments': tags, 'db': other}
            output.write(json.dumps(record, ensure_ascii=False) + '\n')
            songs.append(record)
            counts['songs'] += 1
            counts['with_path'] += bool(path); counts['with_beats'] += bool(beats and beats['times']); counts['with_cues'] += bool(cues[pk])
            counts['with_energy_segments'] += bool(energy_segments[pk]); counts['with_key_segments'] += bool(key_segments[pk])
    db.close()
    return songs, dict(counts)


# ---------------------------------------------------------------- folders and tags

def scan_roots(songs, extra):
    """The folders that were scanned: the parent folders of the analysed files plus any given folders, without nested duplicates."""
    folders = {os.path.dirname(s['path']) for s in songs if s['path']} | {os.path.abspath(os.path.expanduser(e)) for e in extra}
    roots = []
    for folder in sorted(folders):
        if not any(folder == r or folder.startswith(r.rstrip('/') + '/') for r in roots):
            roots.append(folder)
    return roots


def inventory(roots, db_paths, out_path, max_files):
    rows, counts = 0, collections.Counter()
    with open(out_path, 'w', encoding='utf-8') as output:
        for root in roots:
            if not os.path.isdir(root):
                counts['missing_root'] += 1
                continue
            for folder, _, files in os.walk(root):
                for name in sorted(files):
                    if rows >= max_files:
                        counts['stopped_at_max_files'] = 1
                        return counts
                    path = os.path.join(folder, name)
                    try:
                        info = os.stat(path)
                    except OSError:
                        continue
                    ext = os.path.splitext(name)[1].lower()
                    row = {'root': root, 'rel': os.path.relpath(path, root), 'ext': ext, 'size': info.st_size,
                           'mtime': datetime.datetime.fromtimestamp(info.st_mtime).isoformat(timespec='seconds'),
                           'audio': ext in AUDIO_EXTENSIONS, 'in_database': nfc(path) in db_paths}
                    output.write(json.dumps(row, ensure_ascii=False) + '\n')
                    rows += 1
                    counts['files'] += 1; counts['audio'] += row['audio']; counts['audio_not_in_database'] += row['audio'] and not row['in_database']
                    counts['ext' + (ext or '(none)')] += 1
    return counts


def jsonable(value, limit=262144):
    if isinstance(value, (bytes, bytearray)):
        return {'b64': base64.b64encode(bytes(value[:limit])).decode(), 'bytes': len(value)}
    if value is None or isinstance(value, (str, int, float, bool)):
        return value
    if isinstance(value, (list, tuple)):
        return [jsonable(v, limit) for v in value]
    if isinstance(value, dict):
        return {str(k): jsonable(v, limit) for k, v in value.items()}
    if hasattr(value, 'value'):  # ASF attributes
        return jsonable(value.value, limit)
    return str(value)


def frame_json(frame):
    try:
        return {spec.name: jsonable(getattr(frame, spec.name, None)) for spec in frame._framespec}
    except Exception:
        return {'text': frame.pprint()}


def read_tags(path, mutagen):
    audio = mutagen.File(path)
    if audio is None:
        return {'error': 'not recognised'}
    info = audio.info
    record = {'container': type(audio).__name__, 'length': getattr(info, 'length', None), 'sample_rate': getattr(info, 'sample_rate', None),
              'channels': getattr(info, 'channels', None), 'bits_per_sample': getattr(info, 'bits_per_sample', None),
              'bitrate': getattr(info, 'bitrate', None), 'codec': getattr(info, 'codec', None)}
    frames, pictures = {}, 0
    tags = audio.tags
    if tags is not None:
        for key, value in tags.items():
            key = str(key)
            if key.startswith('APIC') or key == 'covr' or key.upper() == 'METADATA_BLOCK_PICTURE' or key == 'WM/Picture':
                pictures += 1
                continue
            if hasattr(value, '_framespec'):
                frames.setdefault(key, []).append(frame_json(value))
            else:
                frames.setdefault(key, []).append(jsonable(value))
    pics = getattr(audio, 'pictures', None)
    record['pictures'] = pictures + (len(pics) if pics else 0)
    record['frames'] = frames
    return record


def dump_tags(paths, out_path):
    try:
        import mutagen
    except ImportError:
        log('  mutagen is not installed: run  python3 -m pip install --user mutagen  and rerun with --skip backup,export,pairs')
        return {'mutagen_missing': 1}
    counts = collections.Counter()
    with open(out_path, 'w', encoding='utf-8') as output:
        for n, path in enumerate(paths):
            row = {'path': path, 'size': os.path.getsize(path) if os.path.exists(path) else None}
            if not os.path.exists(path):
                row['error'] = 'missing'; counts['missing'] += 1
            else:
                try:
                    row.update(read_tags(path, mutagen)); counts['read'] += 1
                    counts['container ' + row.get('container', '?')] += 1
                    for key in row.get('frames', {}):
                        counts['frame ' + key.split(':')[0]] += 1
                except Exception as error:
                    row['error'] = type(error).__name__ + ': ' + str(error); counts['error'] += 1
            output.write(json.dumps(row, ensure_ascii=False) + '\n')
            if n and n % 500 == 0:
                log(f'  tags {n}/{len(paths)}')
    return counts


# ---------------------------------------------------------------- capture

def import_flows(flows, pairs_dir):
    script = os.path.join(os.path.dirname(os.path.abspath(__file__)), 'import_capture.py')
    mitmdump = shutil.which('mitmdump') or next((p for p in ('/opt/homebrew/bin/mitmdump', '/usr/local/bin/mitmdump') if os.path.exists(p)), None)
    if not mitmdump:
        log('  mitmdump not found: install mitmproxy (it ships mitmdump next to mitmweb) or run import_capture.py yourself')
        return False
    for flow in flows:
        log('  importing', os.path.basename(flow))
        done = subprocess.run([mitmdump, '-q', '-nr', flow, '-s', script, '--set', 'out_dir=' + pairs_dir], capture_output=True, text=True)
        if done.returncode:
            log('  importer failed:', done.stderr[-400:])
            return False
    return True


def load_pairs(pairs_dir):
    pairs = {}
    for f in sorted(glob.glob(os.path.join(pairs_dir, '*.request.json'))):
        g = f.replace('.request.json', '.response.json')
        if not os.path.exists(g):
            continue
        q, r = json.load(open(f)), json.load(open(g))
        pairs[os.path.basename(f).split('.request')[0]] = (q, r)
    return pairs


def compare_capture(pairs, songs, report_dir):
    """Join captured calls to database songs by file name (fallback: unique tempo) and compare response with the stored result."""
    by_stem = collections.defaultdict(list)
    for song in songs:
        if song['file']:
            by_stem[stem(song['file'])].append(song)
    result = collections.Counter()
    joined, used = [], set()
    for pid, (q, r) in pairs.items():
        if r.get('Error') or not r.get('Segments'):
            result['pairs_with_error_or_no_segments'] += 1
            continue
        name = (q.get('EliteData') or {}).get('FileName') or ''
        candidates = [s for s in by_stem.get(stem(name), []) if s['id'] not in used]
        if len(candidates) > 1:
            candidates = [s for s in candidates if s['tempo'] is not None and q.get('Tempo') is not None and abs(s['tempo'] - q['Tempo']) < 1e-6] or candidates
        if not candidates:
            result['pairs_without_database_song'] += 1
            continue
        song = candidates[0]
        used.add(song['id']); joined.append((q, r, song))
    result['pairs_joined'] = len(joined)
    for q, r, song in joined:
        summary = r.get('KeySummary') or {}
        result['key_equal'] += (summary.get('MainKey') == song['key'])
        overall = r.get('OverallEnergy')
        result['energy_equal'] += (overall is not None and song['energy'] is not None and round(song['energy']) == round(overall))
        result['tempo_equal_1e-3'] += (q.get('Tempo') is not None and song['tempo'] is not None and abs(q['Tempo'] - song['tempo']) < 1e-3)
        result['cue_count_equal'] += (len(r.get('CuePoints') or []) == len(song['cues']))
        times = [c['time'] for c in song['cues']]
        result['response_cues_all_in_database'] += all(any(abs(c.get('Time', -1) - t) < 1e-2 for t in times) for c in (r.get('CuePoints') or []))
        grid = (song['beats'] or {}).get('times') or []
        if grid and q.get('DownbeatTime') is not None:
            result['downbeat_on_stored_grid_10ms'] += any(abs(q['DownbeatTime'] - t) < 0.01 for t in grid[:64])
    result['database_songs_without_pair'] = sum(1 for s in songs if s['id'] not in used)
    with open(os.path.join(report_dir, 'missing_from_capture.txt'), 'w', encoding='utf-8') as output:
        for song in songs:
            if song['id'] not in used:
                output.write((song['path'] or str(song['id'])) + '\n')
    return dict(result)


# ---------------------------------------------------------------- main

def zip_files(target, files):
    with zipfile.ZipFile(target, 'w', zipfile.ZIP_DEFLATED, compresslevel=6) as archive:
        for path, name in files:
            if os.path.exists(path):
                archive.write(path, name)


def main():
    global LOG
    ap = argparse.ArgumentParser(description='Gather the Mixed In Key database, file inventory, tags and capture check.')
    ap.add_argument('--out', default=os.path.expanduser('~/mik-gather'))
    ap.add_argument('--db', help='path to Collection11.mikdb (default: found automatically)')
    ap.add_argument('--music-dir', action='append', default=[], help='extra folder to inventory and read tags from (repeatable)')
    ap.add_argument('--flow', action='append', default=[], help='mitmproxy flow file to import (repeatable)')
    ap.add_argument('--pairs', help='an existing folder of sanitized pairs (skips the import)')
    ap.add_argument('--skip', default='', help='comma list of: backup,export,inventory,tags,pairs')
    ap.add_argument('--limit', type=int, default=0, help='only the first N songs (trial run)')
    ap.add_argument('--max-files', type=int, default=500000)
    ap.add_argument('--allow-running', action='store_true', help='read even if Mixed In Key is open (the result may be mid-scan)')
    args = ap.parse_args()
    skip = {s.strip() for s in args.skip.split(',') if s.strip()}
    out = os.path.abspath(os.path.expanduser(args.out)); os.makedirs(out, exist_ok=True)
    LOG = open(os.path.join(out, 'gather.log'), 'a')
    db_path = find_db(args.db)
    log('database:', db_path)
    if mik_running() and not args.allow_running:
        raise SystemExit('Mixed In Key is open. Quit it so the database is complete (or pass --allow-running for a trial).')
    report = {'setup': setup_facts(db_path)}
    json.dump(report['setup'], open(os.path.join(out, 'setup.json'), 'w'), indent=2)
    log('Mixed In Key', report['setup'].get('mik_version'), report['setup'].get('mik_build'), '| settings recorded:', len(report['setup']['settings']))

    backup_path = os.path.join(out, 'Collection11.backup.mikdb')
    if 'backup' not in skip:
        need = sum(os.path.getsize(db_path + s) for s in ('', '-wal') if os.path.exists(db_path + s))
        free = shutil.disk_usage(out).free
        log(f'backing up the whole database ({need / 1e9:.1f} GB; {free / 1e9:.1f} GB free; this can take several minutes) ...')
        if free < need * 1.2:
            raise SystemExit('Not enough free space for the database backup: free some space or pass --out to a bigger drive.')
        report['setup']['backup_quick_check'] = backup_database(db_path, backup_path, out)
        report['setup']['backup_sha256'] = sha256_file(backup_path); report['setup']['backup_bytes'] = os.path.getsize(backup_path)
        json.dump(report['setup'], open(os.path.join(out, 'setup.json'), 'w'), indent=2)
    source = backup_path if os.path.exists(backup_path) and 'backup' not in skip else db_path

    songs = []
    if 'export' not in skip:
        log('exporting songs from', 'the backup' if source == backup_path else 'the live database (read-only)')
        songs, report['export'] = export_songs(source, os.path.join(out, 'final.jsonl'), args.limit)
        log('  ', report['export'])
    if 'inventory' not in skip or 'tags' not in skip:
        if not songs and 'export' in skip:
            songs, _ = export_songs(source, os.devnull, args.limit)
        db_paths = {s['path'] for s in songs if s['path']}
        roots = scan_roots(songs, args.music_dir)
        report['inventory_roots'] = len(roots)
        log('folders scanned by Mixed In Key (parents of the analysed files, plus --music-dir):', len(roots))
        if 'inventory' not in skip:
            report['inventory'] = dict(inventory(roots, db_paths, os.path.join(out, 'inventory.jsonl'), args.max_files))
            log('  ', {k: v for k, v in report['inventory'].items() if not k.startswith('ext')})
        if 'tags' not in skip:
            audio_paths, seen = [], set()
            inventory_file = os.path.join(out, 'inventory.jsonl')
            listed = [os.path.join(r['root'], r['rel']) for r in map(json.loads, open(inventory_file)) if r['audio']] \
                if 'inventory' not in skip and os.path.exists(inventory_file) else []
            for p in sorted(db_paths) + ([] if args.limit else listed):
                if nfc(p) not in seen:
                    seen.add(nfc(p)); audio_paths.append(p)
            log('reading tags from', len(audio_paths), 'audio files')
            report['tags'] = dict(dump_tags(audio_paths, os.path.join(out, 'tags.jsonl')))

    if 'pairs' not in skip and (args.flow or args.pairs):
        pairs_dir = os.path.abspath(os.path.expanduser(args.pairs)) if args.pairs else os.path.join(out, 'pairs')
        if args.flow:
            log('importing the capture ...')
            if not import_flows([os.path.expanduser(f) for f in args.flow], pairs_dir):
                pairs_dir = None
        if pairs_dir and os.path.isdir(pairs_dir):
            if not songs:
                songs, _ = export_songs(source, os.devnull, 0)
            pairs = load_pairs(pairs_dir)
            report['capture'] = {'pairs': len(pairs), **compare_capture(pairs, songs, out)}
            if os.path.exists(os.path.join(pairs_dir, 'import-summary.json')):
                report['capture']['import_summary'] = json.load(open(os.path.join(pairs_dir, 'import-summary.json')))
            log('  ', {k: v for k, v in report['capture'].items() if k != 'import_summary'})
            zip_files(os.path.join(out, 'mik-pairs.zip'), [(f, os.path.join('pairs', os.path.basename(f))) for f in glob.glob(os.path.join(pairs_dir, '*.json'))])

    json.dump(report, open(os.path.join(out, 'report.json'), 'w'), indent=2)
    zip_files(os.path.join(out, 'mik-metadata.zip'), [(os.path.join(out, n), n) for n in
              ('final.jsonl', 'inventory.jsonl', 'tags.jsonl', 'setup.json', 'report.json', 'missing_from_capture.txt', 'gather.log')])
    log('done. Copy these from', out, ':')
    for name in ('mik-metadata.zip', 'mik-pairs.zip', 'Collection11.backup.mikdb'):
        if os.path.exists(os.path.join(out, name)):
            log(f'   {name}  ({os.path.getsize(os.path.join(out, name)) / 1e6:.0f} MB)')


if __name__ == '__main__':
    main()
