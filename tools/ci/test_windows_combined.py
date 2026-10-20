"""Exercise real child processes: overlap, isolation, and failure propagation."""
import json
from pathlib import Path
import sys
import subprocess
import tempfile
import unittest

from windows_combined import prepare, run_groups


class CombinedChecksTest(unittest.TestCase):
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
            archive = source / '.superpowers/media-build/ffmpeg-9.0.2.tar.xz'
            for path in (binary, archive):
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_bytes(b'generated media')
            workspaces = {'engine': root / 'e', 'desktop': root / 'd'}
            prepare(source, workspaces)
            for path in workspaces.values():
                self.assertEqual((path / 'tracked').read_text(), 'same source')
                self.assertEqual((path / binary.relative_to(source)).read_bytes(), b'generated media')
                self.assertEqual((path / archive.relative_to(source)).read_bytes(), b'generated media')
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
