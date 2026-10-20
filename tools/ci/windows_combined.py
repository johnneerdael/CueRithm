"""Run the existing Windows engine and desktop checks on one ephemeral host."""
import argparse
from concurrent.futures import ThreadPoolExecutor
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import time


def cargo(*args):
    return ['cargo', '+stable', *args]


def pnpm(*args):
    # pnpm is a .cmd shim on Windows; use PowerShell's native command resolution.
    quoted = ' '.join("'" + arg.replace("'", "''") + "'" for arg in args)
    return ['pwsh', '-NoProfile', '-Command', '& pnpm ' + quoted + '; exit $LASTEXITCODE']


ENGINE = ['--workspace', '--exclude', 'cuerithm-desktop', '--exclude', 'cuerithm-library']
DESKTOP = ['-p', 'cuerithm-desktop', '-p', 'cuerithm-library', '-p', 'cuerithm-rekordbox']
COMMANDS = {
    'engine': [
        cargo('fmt', '--all', '--', '--check'),
        cargo('clippy', *ENGINE, '--all-targets', '--all-features', '--locked', '--', '-D', 'warnings'),
        cargo('test', *ENGINE, '--all-features', '--release', '--locked', '--', '--test-threads=2'),
        cargo('test', '-p', 'cuerithm-cleanup', '--locked'),
        cargo('build', *ENGINE, '--all-features', '--release', '--locked'),
    ],
    'desktop': [
        pnpm('--dir', 'apps/desktop', 'test'),
        pnpm('--dir', 'apps/desktop', 'build'),
        cargo('fmt', '--all', '--', '--check'),
        cargo('clippy', *DESKTOP, '--all-targets', '--locked', '--', '-D', 'warnings'),
        cargo('test', '-p', 'cuerithm-autotag', '-p', 'cuerithm-library', '-p', 'cuerithm-media',
              '-p', 'cuerithm-desktop', '-p', 'cuerithm-rekordbox', '--release', '--locked', '--', '--test-threads=2'),
        [sys.executable, '-m', 'unittest', 'discover', '-s', 'tools/fixtures/rekordbox', '-v'],
        [sys.executable, '-m', 'unittest', 'discover', '-s', 'tools/media-runtime', '-v'],
    ],
}


def run_groups(workspaces, commands, logs, build_jobs):
    logs.mkdir(parents=True, exist_ok=True)

    def run(name):
        start = time.monotonic()
        env = os.environ.copy()
        temp = logs / 'temp' / name
        temp.mkdir(parents=True, exist_ok=True)
        env.update(GITHUB_WORKSPACE=str(workspaces[name]), CARGO_BUILD_JOBS=str(build_jobs),
                   CARGO_TARGET_DIR=str(workspaces[name] / 'target'),
                   RUNNER_TEMP=str(temp), TEMP=str(temp), TMP=str(temp))
        if name == 'desktop':
            env['CARGO_PROFILE_RELEASE_DEBUG'] = '0'
        code = 0
        stages = []
        with (logs / (name + '.log')).open('w', encoding='utf-8') as output:
            try:
                for command in commands[name]:
                    stage_start = time.monotonic()
                    output.write('COMMAND ' + json.dumps(command) + '\n')
                    output.flush()
                    with subprocess.Popen(command, cwd=workspaces[name], env=env,
                                          stdout=subprocess.PIPE, stderr=subprocess.STDOUT,
                                          text=True, encoding='utf-8', errors='replace') as child:
                        for line in child.stdout:
                            output.write(line)
                            output.flush()
                            print('[' + name + '] ' + line.rstrip(), flush=True)
                        code = child.wait()
                    stages.append({'command': command, 'seconds': round(time.monotonic() - stage_start, 2),
                                   'exit_code': code})
                    if code:
                        break
            except OSError as error:
                output.write(str(error) + '\n')
                code = 1
        return {'exit_code': code, 'seconds': round(time.monotonic() - start, 2), 'stages': stages}

    with ThreadPoolExecutor(max_workers=2) as pool:
        futures = {name: pool.submit(run, name) for name in workspaces}
        results = {name: future.result() for name, future in futures.items()}
    (logs / 'results.json').write_text(json.dumps(results, indent=2), encoding='utf-8')
    return results


def prepare(source, workspaces):
    for path in workspaces.values():
        # git refuses an existing worktree path; never remove an unknown directory.
        subprocess.run(['git', 'worktree', 'add', '--detach', str(path), 'HEAD'], cwd=source, check=True)
        for relative in ('tools/media-runtime/bin', '.superpowers/media-build/ffmpeg-9.0.2.tar.xz'):
            origin = source / relative
            destination = path / relative
            destination.parent.mkdir(parents=True, exist_ok=True)
            if origin.is_dir():
                shutil.copytree(origin, destination)
            else:
                shutil.copy2(origin, destination)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('mode', choices=['prepare', 'run'])
    parser.add_argument('--root', type=Path, default=Path('C:/cr'))
    args = parser.parse_args()
    root = args.root.resolve()
    workspaces = {'engine': root / 'e', 'desktop': root / 'd'}
    if args.mode == 'prepare':
        prepare(Path.cwd(), workspaces)
        return 0
    results = run_groups(workspaces, COMMANDS, root / 'logs', 8)
    summary = os.environ.get('GITHUB_STEP_SUMMARY')
    if summary:
        with open(summary, 'a', encoding='utf-8') as output:
            output.write('| Group | Exit code | Seconds |\n| --- | ---: | ---: |\n')
            for name, result in results.items():
                output.write(f"| {name} | {result['exit_code']} | {result['seconds']} |\n")
    return int(any(result['exit_code'] for result in results.values()))


if __name__ == '__main__':
    sys.exit(main())
