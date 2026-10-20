"""Exercise real child processes: overlap, isolation, and failure propagation."""
import json
import os
from pathlib import Path
import sys
import subprocess
import tempfile
import unittest

from windows_combined import prepare, run_groups


class CombinedChecksTest(unittest.TestCase):
    def test_unicode_output_survives_a_windows_code_page(self):
        # Windows Python pipes can default to cp1252; logging must not kill a worker.
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            for group in ('engine', 'desktop'):
                (root / group).mkdir()
            script = '''from pathlib import Path
import sys
from windows_combined import run_groups
root = Path(sys.argv[1])
workspaces = {name: root / name for name in ('engine', 'desktop')}
commands = {name: [[sys.executable, '-c', "import sys; sys.stdout.buffer.write(chr(0x240d).encode('utf-8'))"]] for name in workspaces}
results = run_groups(workspaces, commands, root / 'logs', 8)
raise SystemExit(any(result['exit_code'] for result in results.values()))
'''
            env = os.environ.copy()
            env['PYTHONIOENCODING'] = 'cp1252'
            result = subprocess.run([sys.executable, '-c', script, str(root)],
                                    cwd=Path(__file__).parent, env=env, capture_output=True,
                                    text=True, encoding='utf-8', errors='replace', timeout=10)
            self.assertEqual(result.returncode, 0, result.stderr)
            for group in ('engine', 'desktop'):
                self.assertIn('\u240d', (root / 'logs' / (group + '.log')).read_text(encoding='utf-8'))
            self.assertTrue((root / 'logs' / 'results.json').exists())

    def test_prepare_copies_generated_media_and_refuses_existing_workspaces(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            source = root / 'source'
            source.mkdir()
            subprocess.run(['git', 'init', '--quiet', str(source)], check=True)
            (source / 'tracked').write_text('same source')
            subprocess.run(['git', 'add', '.'], cwd=source, check=True)
            subprocess.run(['git', '-c', 'user.name=CI test', '-c', 'user.email=ci@example.invalid',
                            'commit', '--quiet', '-m', 'fixture'], cwd=source, check=True)
            binary = source / 'tools/media-runtime/bin/ffmpeg.exe'
            # Checks require generated binaries, not a particular source archive.
            for path in (binary,):
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_bytes(b'generated media')
            workspaces = {'engine': root / 'e', 'desktop': root / 'd'}
            prepare(source, workspaces)
            for path in workspaces.values():
                self.assertEqual((path / 'tracked').read_text(), 'same source')
                self.assertEqual((path / binary.relative_to(source)).read_bytes(), b'generated media')
            (workspaces['engine'] / 'keep').write_text('keep')
            with self.assertRaises(subprocess.CalledProcessError):
                prepare(source, workspaces)
            self.assertEqual((workspaces['engine'] / 'keep').read_text(), 'keep')

    def test_groups_overlap_and_have_isolated_workspaces(self):
        # Serial execution deadlocks this rendezvous and returns a failure.
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            script = root / 'worker.py'
            script.write_text('''import os, pathlib, sys, time
root = pathlib.Path(sys.argv[1])
name, other = sys.argv[2:]
(root / (name + '.ready')).touch()
deadline = time.monotonic() + 5
while not (root / (other + '.ready')).exists():
    if time.monotonic() > deadline: raise SystemExit(9)
    time.sleep(.02)
pathlib.Path('identity.txt').write_text(os.environ['GITHUB_WORKSPACE'] + '\\n' + os.environ['CARGO_BUILD_JOBS'])
print(name + ' complete')
''')
            workspaces = {name: root / name for name in ('engine', 'desktop')}
            for path in workspaces.values():
                path.mkdir()
            commands = {
                'engine': [[sys.executable, str(script), str(root), 'engine', 'desktop']],
                'desktop': [[sys.executable, str(script), str(root), 'desktop', 'engine']],
            }
            result = run_groups(workspaces, commands, root / 'logs', 8)
            self.assertEqual({name: value['exit_code'] for name, value in result.items()},
                             {'engine': 0, 'desktop': 0})
            for name, path in workspaces.items():
                self.assertEqual((path / 'identity.txt').read_text(), str(path) + '\n8')
                self.assertIn(name + ' complete', (root / 'logs' / (name + '.log')).read_text())

    def test_failure_retains_both_results_and_stops_failed_group(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            workspaces = {name: root / name for name in ('engine', 'desktop')}
            for path in workspaces.values():
                path.mkdir()
            commands = {
                'engine': [[sys.executable, '-c', 'raise SystemExit(7)'],
                           [sys.executable, '-c', "open('unexpected', 'w').close()"]],
                'desktop': [[sys.executable, '-c', "open('completed', 'w').close()"]],
            }
            result = run_groups(workspaces, commands, root / 'logs', 8)
            self.assertEqual(result['engine']['exit_code'], 7)
            self.assertEqual(result['desktop']['exit_code'], 0)
            self.assertTrue((workspaces['desktop'] / 'completed').exists())
            self.assertFalse((workspaces['engine'] / 'unexpected').exists())
            self.assertEqual(json.loads((root / 'logs' / 'results.json').read_text()), result)


if __name__ == '__main__':
    unittest.main()
