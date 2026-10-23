#!/usr/bin/env python3
"""Run an installed Mixxx against a library CueRithm wrote, and check what Mixxx does.

What it does, on a scratch folder (your own Mixxx library is never touched; Mixxx is
started with `--settings-path` pointing into the scratch folder). The official macOS
Mixxx is App Sandboxed, so the folder is a sibling of Mixxx's own `Mixxx` folder inside
its container, `~/Library/Containers/org.mixxx.mixxx/Data/Library/Application Support`;
outside the container the sandbox makes the folder read-only to Mixxx and it cannot
open its database.

Makes three click tracks (WAV, FLAC, MP3) at 128 BPM with the first click at 0.5 s, then
runs four scenarios, each on its own scratch library:

  fresh      Mixxx creates an empty library and waits at its "Choose music library
             directory" prompt (it shows that whenever its list of music folders is empty).
             CueRithm adds the three tracks, and must also give Mixxx a music folder, so
             that the next start does not prompt.
  existing   Mixxx has a music folder and itself adds the three tracks. CueRithm then
             replaces their grid, key and cues in place (Manage-library mode). The folder
             row is inserted by this script, standing in for choosing it in the dialog.
  overwrite  As existing, but CueRithm replaces the whole library with a new one.
  created    Mixxx has never run: there is no settings folder. CueRithm creates the
             library. Its tables must equal those Mixxx itself made in the scenarios above,
             and Mixxx must then start on it without prompting or upgrading anything.
  formats    As existing, with AAC and ALAC (M4A), AIFF, Ogg Vorbis and Opus. For each the
             first click is found the way the decoder Mixxx chose for that file type reads
             it (CoreAudio, or the format's own library), and the written grid must land
             on it. Run alone with `--only formats`.

In all of them the sync goes through the real preview/apply path. The written rows are
checked (grid, key, BPM lock, hot cues and loop, the playlist, the music folder, and that
the first beat lands where CoreAudio, the decoder Mixxx uses on macOS, puts the first
click). Then Mixxx is started again on the same library with the same files and its debug
log is checked: it must not stop at the folder prompt, must find the schema current, must
read the grids without errors, and the rows must be unchanged afterwards, including after
its own library scan.

Mixxx must not already be running. Quitting Mixxx here is a plain SIGTERM of the
process this script started; Mixxx has no signal handler, so changes it keeps only in
memory are not saved. Needs macOS, an installed Mixxx, cargo, clang, ffmpeg and lame.

    python3 tools/mixxx-acceptance/run.py [--mixxx PATH] [--work DIR]

Exit status 0 means every check passed. A report is printed and written to
`<work>/report.json`.
"""
import argparse
import array
import json
import math
import os
import re
import shutil
import sqlite3
import struct
import subprocess
import sys
import tempfile
import time
import wave
from pathlib import Path

HERE = Path(__file__).resolve().parent
REPO = HERE.parent.parent
DEFAULT_MIXXX = "/Applications/Mixxx.app/Contents/MacOS/Mixxx"
CONTAINER = Path.home() / "Library/Containers/org.mixxx.mixxx/Data/Library/Application Support"
SAMPLE_RATE = 44100
BPM = 128.0
FIRST_BEAT = 0.5
DURATION = 24.0
CUES = [
    {"index": 0, "name": "Drop", "start": 4.0, "end": None, "color": 0xE60E0E},
    {"index": 1, "name": "Phrase", "start": 8.0, "end": None, "color": None},
    {"index": 2, "name": "Loop", "start": 12.0, "end": 14.0, "color": 0x1058DE},
]


def run(*args, **kwargs):
    return subprocess.run(args, check=True, capture_output=True, **kwargs)


def write_click_track(path):
    """Two cycles of a 2 kHz tone at every beat, stereo 16-bit, DURATION seconds."""
    beat = 60.0 / BPM
    burst = round(2 * SAMPLE_RATE / 2000)
    starts = []
    t = FIRST_BEAT
    while t < DURATION - 0.1:
        starts.append(round(t * SAMPLE_RATE))
        t += beat
    samples = array.array("h", bytes(SAMPLE_RATE * int(DURATION) * 4))
    for start in starts:
        for i in range(burst):
            value = round(30000 * math.sin(2 * math.pi * 2000 * i / SAMPLE_RATE))
            samples[(start + i) * 2] = value
            samples[(start + i) * 2 + 1] = value
    with wave.open(str(path), "wb") as out:
        out.setnchannels(2)
        out.setsampwidth(2)
        out.setframerate(SAMPLE_RATE)
        out.writeframes(samples.tobytes())


# --- reading Mixxx's database -------------------------------------------------------

def varint(data, at):
    value = shift = 0
    while True:
        byte = data[at]
        at += 1
        value |= (byte & 0x7F) << shift
        if not byte & 0x80:
            return value, at
        shift += 7


def fields(data):
    at, out = 0, []
    while at < len(data):
        tag, at = varint(data, at)
        number, wire = tag >> 3, tag & 7
        if wire == 0:
            value, at = varint(data, at)
        elif wire == 1:
            value, at = data[at:at + 8], at + 8
        elif wire == 2:
            size, at = varint(data, at)
            value, at = data[at:at + size], at + size
        else:
            raise ValueError(f"wire type {wire}")
        out.append((number, value))
    return out


def decode_beat_grid(blob):
    parts = dict(fields(blob))
    bpm = struct.unpack("<d", dict(fields(parts[1]))[1])[0]
    frame = dict(fields(parts[2]))[1]
    if frame >= 1 << 63:
        frame -= 1 << 64
    return bpm, frame


def decode_key_map(blob):
    parts = dict(fields(blob))
    return parts[1], parts[2].decode(), parts.get(4, 0)


def read_library(database):
    uri = f"file:{database}?mode=ro"
    for _ in range(40):
        try:
            connection = sqlite3.connect(uri, uri=True, timeout=2)
            connection.row_factory = sqlite3.Row
            rows = connection.execute(
                "SELECT l.*, t.location AS path FROM library l JOIN track_locations t ON t.id = l.location"
            ).fetchall()
            tracks = {Path(row["path"]).name: dict(row) for row in rows}
            for track in tracks.values():
                track["cues"] = [
                    dict(cue)
                    for cue in connection.execute(
                        "SELECT type, position, length, hotcue, label, color FROM cues WHERE track_id = ? ORDER BY hotcue, type",
                        (track["id"],),
                    )
                ]
            playlists = [
                {
                    "name": row["name"],
                    "tracks": [r[0] for r in connection.execute(
                        "SELECT track_id FROM PlaylistTracks WHERE playlist_id = ? ORDER BY position", (row["id"],))],
                }
                for row in connection.execute("SELECT id, name FROM Playlists WHERE hidden = 0")
            ]
            version = connection.execute(
                "SELECT value FROM settings WHERE name = 'mixxx.schema.version'").fetchone()
            connection.close()
            return {"tracks": tracks, "playlists": playlists, "schema": version[0] if version else None}
        except sqlite3.Error:
            time.sleep(0.5)
    raise RuntimeError("could not read the Mixxx library")


COLUMNS = ["bpm", "bpm_lock", "beats_version", "beats", "key", "key_id", "keys_version", "keys",
           "header_parsed", "source_synchronized_ms", "title", "artist", "rating", "cuepoint"]


def digest(library):
    return {
        name: ({c: track[c] for c in COLUMNS}, track["cues"])
        for name, track in library["tracks"].items()
    }


# --- running Mixxx --------------------------------------------------------------------

def mixxx_running():
    out = run("ps", "-A", "-o", "comm=").stdout.decode().splitlines()
    return [line for line in out if line.strip().lower().endswith("mixxx")]


class Mixxx:
    """One Mixxx process on the scratch settings folder; always stopped on exit."""

    def __init__(self, mixxx, settings, tracks, log):
        self.log = open(log, "wb")
        self.process = subprocess.Popen(
            [mixxx, "--settings-path", str(settings), "--log-level", "debug",
             "--log-flush-level", "debug", *map(str, tracks)],
            stdout=self.log, stderr=subprocess.STDOUT)

    def stop(self):
        if self.process.poll() is None:
            self.process.terminate()
            try:
                self.process.wait(20)
            except subprocess.TimeoutExpired:
                self.process.kill()
                self.process.wait()
        self.log.close()

    def __enter__(self):
        return self

    def __exit__(self, *_):
        self.stop()


def wait_for(condition, timeout, what):
    deadline = time.time() + timeout
    while time.time() < deadline:
        value = condition()
        if value:
            return value
        time.sleep(1)
    raise RuntimeError(f"timed out waiting for {what}")


# --- the checks -------------------------------------------------------------------------

class Report:
    def __init__(self):
        self.checks = []

    def check(self, name, ok, detail=""):
        self.checks.append({"check": name, "ok": bool(ok), "detail": detail})
        print(f"{'PASS' if ok else 'FAIL'}  {name}" + (f"  [{detail}]" if detail else ""))
        return ok


def probe_frames(probe, files):
    """CoreAudio's Mixxx frame of the first click, with the probe's 0.3 threshold."""
    out = {}
    for name, path in files.items():
        line = dict(item.split("=") for item in run(str(probe), str(path)).stdout.decode().split())
        out[name] = int(line["mixxx_frame"])
    return out


NAMES = ("click128.wav", "click128.flac", "click128.mp3")

# Formats of the `formats` scenario: name, ffmpeg arguments. WMA is left out on purpose: the
# installed Mixxx registers no decoder for it, which the scenario checks.
FORMATS = (
    ("click128.aac.m4a", ["-c:a", "aac", "-b:a", "192k"]),
    ("click128.alac.m4a", ["-c:a", "alac"]),
    ("click128.aiff", ["-c:a", "pcm_s16be"]),
    ("click128.ogg", ["-c:a", "vorbis", "-strict", "-2", "-b:a", "192k"]),
    ("click128.opus", ["-c:a", "libopus", "-b:a", "128k"]),
)


def native_rate(path):
    """The sample rate the file declares (Opus decodes at 48000 whatever it was made from)."""
    out = run("ffprobe", "-v", "error", "-select_streams", "a:0", "-show_entries",
              "stream=sample_rate", "-of", "default=nw=1:nk=1", str(path)).stdout.decode()
    return int(out.strip())


def ffmpeg_first_click(path, rate, threshold=0.3):
    """First frame whose left channel exceeds the threshold, in FFmpeg's decode of `path`."""
    raw = run("ffmpeg", "-v", "error", "-i", str(path), "-f", "f32le", "-ac", "2",
              "-ar", str(rate), "-").stdout
    samples = array.array("f")
    samples.frombytes(raw[: len(raw) - len(raw) % 4])
    for frame in range(len(samples) // 2):
        if abs(samples[frame * 2]) > threshold:
            return frame
    raise RuntimeError(f"no click found in {path}")


def providers_in(log):
    """File type -> decoders Mixxx lists for it, highest priority first (from its debug log)."""
    lines = log.splitlines()
    found = {}
    for i, line in enumerate(lines):
        match = re.search(r'SoundSource providers for file type "([^"]+)"', line)
        if match:
            names = []
            for follow in lines[i + 1:i + 5]:
                if "SoundSourceProxy - " in follow and " : " in follow:
                    names.append(re.sub(r"^.*SoundSourceProxy - ", "", follow).split(" : ", 1)[1].strip('"'))
                else:
                    break
            found[match.group(1)] = names
    return found


def format_truth(probe, files, providers):
    """Where the decoder Mixxx uses reads the first click, and at what rate, for each file."""
    truth, rates = {}, {}
    for name, path in files.items():
        extension = Path(name).suffix[1:]
        first = (providers.get(extension) or ["?"])[0]
        rates[name] = native_rate(path)
        if "CoreAudio" in first:
            truth[name] = probe_frames(probe, {name: path})[name]
        else:
            truth[name] = ffmpeg_first_click(path, rates[name])
    return truth, rates


def library_ready(database):
    """Mixxx has created and migrated its library."""
    if not database.exists():
        return False
    try:
        connection = sqlite3.connect(f"file:{database}?mode=ro", uri=True, timeout=2)
        try:
            return connection.execute(
                "SELECT value FROM settings WHERE name = 'mixxx.schema.version'").fetchone() is not None
        finally:
            connection.close()
    except sqlite3.Error:
        return False


def scan_list(database):
    connection = sqlite3.connect(f"file:{database}?mode=ro", uri=True, timeout=5)
    try:
        return sorted(row[0] for row in connection.execute("SELECT directory FROM directories"))
    finally:
        connection.close()


def check_rows(report, label, written, first, truth, rates=None, count=3):
    """The synced rows. `first` is the library as Mixxx left it before the sync, or None."""
    rates = rates or {}
    report.check(f"[{label}] {count} tracks, none duplicated", len(written["tracks"]) == count,
                 f"{len(written['tracks'])}")
    for name, track in written["tracks"].items():
        tag = f"[{label}] {name}"
        if first is not None:
            report.check(f"{tag}: the row Mixxx created was updated in place",
                         track["id"] == first["tracks"][name]["id"])
        report.check(f"{tag}: BeatGrid-2.0", track["beats_version"] == "BeatGrid-2.0")
        bpm, first_beat = decode_beat_grid(track["beats"])
        report.check(f"{tag}: grid tempo {BPM}", abs(bpm - BPM) < 1e-9 and abs(track["bpm"] - BPM) < 1e-9, f"{bpm}")
        report.check(f"{tag}: BPM locked", track["bpm_lock"] == 1)
        gap = first_beat - truth[name]
        report.check(f"{tag}: first beat within 16 frames of CoreAudio's first click",
                     abs(gap) <= 16, f"beat {first_beat}, click {truth[name]}, difference {gap}")
        key = decode_key_map(track["keys"])
        report.check(f"{tag}: key A minor, source user",
                     (track["key"], track["key_id"], key) == ("Am", 22, (22, "Am", 2)), f"{track['key']} {key}")
        was = None if first is None else first["tracks"][name]["source_synchronized_ms"]
        report.check(f"{tag}: tags will not be re-read (parsed, sync time untouched)",
                     track["header_parsed"] == 1 and track["source_synchronized_ms"] == was,
                     f"sync time {track['source_synchronized_ms']}")
        kinds = [(c["type"], c["hotcue"], c["label"]) for c in track["cues"] if c["type"] in (1, 4)]
        report.check(f"{tag}: two hot cues and one loop",
                     kinds == [(1, 0, "Drop"), (1, 1, "Phrase"), (4, 2, "Loop")], str(kinds))
        rate = rates.get(name, SAMPLE_RATE)
        shift = first_beat - round(FIRST_BEAT * rate)
        drop = next(c for c in track["cues"] if c["label"] == "Drop")
        loop = next(c for c in track["cues"] if c["label"] == "Loop")
        report.check(f"{tag}: cue positions are frames x 2 with the same shift as the grid",
                     drop["position"] == 2 * (round(4.0 * rate) + shift)
                     and loop["length"] == 2 * round(2.0 * rate), f"shift {shift} at {rate} Hz")
    report.check(f"[{label}] the playlist holds the {count} tracks",
                 [len(p["tracks"]) for p in written["playlists"] if p["name"] == "CueRithm acceptance"] == [count])


def master(database):
    """Every table, index and view Mixxx made, by name; SQLite's own objects left out."""
    connection = sqlite3.connect(f"file:{database}?mode=ro", uri=True, timeout=5)
    try:
        return sorted(connection.execute(
            "SELECT type, name, tbl_name, sql FROM sqlite_master WHERE name NOT LIKE 'sqlite_%'").fetchall())
    finally:
        connection.close()


def mixxx_rows(database):
    """The rows a new Mixxx library starts with: schema settings and the two playlists."""
    connection = sqlite3.connect(f"file:{database}?mode=ro", uri=True, timeout=5)
    try:
        return (
            sorted(connection.execute("SELECT name, value FROM settings WHERE name LIKE 'mixxx.schema.%'")),
            sorted(connection.execute(
                "SELECT id, name, position, hidden, locked FROM Playlists WHERE hidden IN (1, -1)")),
        )
    finally:
        connection.close()


def scenario(report, mixxx, work, sources, truth, mode, reference, names=NAMES, probe=None):
    """mode is fresh, existing, overwrite or created; reference collects what Mixxx made."""
    label = mode
    settings, music = work / "settings", work / "music"
    work.mkdir(parents=True)
    music.mkdir()
    for name in names:
        shutil.copy2(sources / name, music / name)
    shutil.copy2(sources / "manifest.json", music / "manifest.json")
    files = {name: (music / name).resolve() for name in names}
    rates = {}
    database = settings / "mixxxdb.sqlite"
    created = ""
    first = None

    if mode == "created":
        # Mixxx has never run: the settings folder does not exist either.
        report.check(f"[{label}] there is no settings folder and no library before the sync",
                     not settings.exists())
    else:
        # 1. Mixxx creates its own, empty library.
        settings.mkdir()
        print(f"[{label}] Starting Mixxx (a window may open) so it creates its library...")
        with Mixxx(mixxx, settings, [], work / "mixxx-create.log"):
            wait_for(lambda: library_ready(database), 120, "Mixxx to create its library")
            time.sleep(10)
        created = (work / "mixxx-create.log").read_text(errors="replace")
        report.check(f"[{label}] a new library has no music folder and Mixxx waits at its folder prompt",
                     scan_list(database) == [] and "Creating ControllerManager" not in created,
                     "directories empty; start-up never got past the prompt")
        reference.setdefault("master", master(database))
        reference.setdefault("rows", mixxx_rows(database))

    if mode in ("existing", "overwrite", "formats"):
        # Stands in for choosing the folder in Mixxx's dialog, which also adds the row.
        connection = sqlite3.connect(database)
        connection.execute("INSERT INTO directories(directory) VALUES (?)", (str(music.resolve()),))
        connection.commit()
        connection.close()
        print(f"[{label}] Starting Mixxx so that it adds the tracks itself...")
        with Mixxx(mixxx, settings, files.values(), work / "mixxx-first.log"):
            def added():
                library = read_library(database)
                return library if len(library["tracks"]) == len(names) else None
            first = wait_for(added, 180, f"Mixxx to add the {len(names)} tracks")
            time.sleep(5)
        report.check(f"[{label}] Mixxx created a schema 39 or 40 library", first["schema"] in ("39", "40"),
                     f"schema {first['schema']}")
        report.check(f"[{label}] Mixxx itself added all {len(names)} tracks", len(first["tracks"]) == len(names))
        log = (work / "mixxx-first.log").read_text(errors="replace")
        lines = log.splitlines()
        for extension in ("mp3", "m4a") if mode != "formats" else ():
            at = next((i for i, line in enumerate(lines)
                       if f'SoundSource providers for file type "{extension}"' in line), None)
            providers = [] if at is None else [
                re.sub(r"^.*SoundSourceProxy - ", "", line) for line in lines[at + 1:at + 4]
                if "SoundSourceProxy - " in line and " : " in line]
            report.check(f"[{label}] Mixxx decodes {extension} with Apple CoreAudio first (MAD is not in this build)",
                         bool(providers) and "CoreAudio" in providers[0] and not any("MAD" in p for p in providers),
                         "; ".join(providers))
        if mode == "formats":
            providers = providers_in(log)
            for name in names:
                extension = Path(name).suffix[1:]
                chosen = (providers.get(extension) or [None])[0]
                report.check(f"[{label}] Mixxx has a decoder for .{extension}", chosen is not None,
                             f"{chosen}")
            report.check(f"[{label}] the installed Mixxx registers no decoder for .wma (so WMA cannot be opened)",
                         "wma" not in providers, "providers: " + ", ".join(sorted(providers)))
            truth, rates = format_truth(probe, files, providers)
            print(f"    first click by decoder: {truth}; rates: {rates}")
            for name in names:
                row = first["tracks"][name]
                report.check(f"[{label}] {name}: Mixxx's own sample rate is the one the grid is counted in",
                             row["samplerate"] == rates[name], f"Mixxx {row['samplerate']}, file {rates[name]}")
        if mode == "overwrite":
            # Something CueRithm does not have, to show the replacement drops it.
            connection = sqlite3.connect(database)
            connection.execute(
                "INSERT INTO Playlists(name, position, hidden, date_created, date_modified, locked)"
                " VALUES ('Only in Mixxx', 99, 0, datetime('now'), datetime('now'), 0)")
            connection.commit()
            connection.close()

    # 2. CueRithm syncs, through the real preview and apply path.
    print(f"[{label}] Syncing through prepare_sync and apply_sync...")
    env = dict(os.environ, CUERITHM_MIXXX_ACCEPTANCE=str(work),
               CUERITHM_MIXXX_MODE="replace" if mode in ("existing", "formats") else "keep",
               CUERITHM_MIXXX_LIBRARY="overwrite" if mode == "overwrite" else "update")
    sync = subprocess.run(
        ["cargo", "test", "-p", "cuerithm-dj-sync", "--lib", "--", "--ignored",
         "write_acceptance_library", "--nocapture"],
        cwd=REPO, env=env, capture_output=True, text=True)
    applied = "APPLIED Applied" in sync.stdout
    report.check(f"[{label}] the sync applied", applied,
                 "" if applied else (sync.stdout + sync.stderr)[-500:])
    for line in sync.stdout.splitlines():
        if line.startswith(("WARNING", "FILES")):
            print("   ", line[:260])
    if not applied:
        return {}

    # 3. What was written.
    written = read_library(database)
    check_rows(report, label, written, first if mode in ("existing", "formats") else None,
               truth, rates, len(names))
    folders = scan_list(database)
    report.check(f"[{label}] Mixxx's list of music folders is the folder holding the tracks",
                 folders == [str(music.resolve())], str(folders))
    if mode == "overwrite":
        report.check(f"[{label}] a playlist only Mixxx had is gone",
                     [p["name"] for p in written["playlists"]] == ["CueRithm acceptance"],
                     str([p["name"] for p in written["playlists"]]))
    if mode in ("created", "overwrite"):
        ours = master(database)
        same = ours == reference["master"]
        report.check(f"[{label}] the tables, indexes and views equal the ones Mixxx made itself",
                     same, f"{len(ours)} objects" if same else "differs: " + str(
                         [o[1] for o in set(ours) ^ set(reference["master"])][:6]))
        report.check(f"[{label}] the schema settings and Mixxx's two default playlists equal Mixxx's own"
                     " (ids and timestamps aside)",
                     [(n, v) for n, v in mixxx_rows(database)[0]] == [(n, v) for n, v in reference["rows"][0]]
                     and [r[1:] for r in mixxx_rows(database)[1]] == [r[1:] for r in reference["rows"][1]])
    after_sync = digest(written)

    # 4. Mixxx starts again on the synced library.
    print(f"[{label}] Starting Mixxx again on the synced library...")
    second_log = work / "mixxx-second.log"
    with Mixxx(mixxx, settings, files.values(), second_log):
        def text():
            return second_log.read_text(errors="replace")
        try:
            wait_for(lambda: "Creating ControllerManager" in text(), 90, "Mixxx to get past the folder prompt")
        except RuntimeError:
            pass
        try:
            wait_for(lambda: text().count("Successfully deserialized Beats") >= 2 and
                     text().count("Track is BpmLocked") >= 2, 90, "Mixxx to load the grids")
        except RuntimeError:
            pass
        time.sleep(8)
        second = text()
    report.check(f"[{label}] Mixxx started without asking for a music folder",
                 "Creating ControllerManager" in second)
    report.check(f"[{label}] Mixxx found the schema current and upgraded nothing",
                 "Database schema is up-to-date" in second and "Upgrading database schema" not in second,
                 next((line.split("SchemaManager - ")[-1] for line in second.splitlines()
                       if "Database schema is up-to-date" in line), ""))
    grids = second.count('Successfully deserialized Beats ("BeatGrid-2.0")')
    keys = second.count("Successfully deserialized KeyMap")
    locked = second.count("Track is BpmLocked: Beat calculation will not start")
    report.check(f"[{label}] Mixxx deserialized the BeatGrid-2.0 grid and the key of the tracks it loaded",
                 grids >= 2 and keys >= 2, f"{grids} grids, {keys} keys")
    report.check(f"[{label}] Mixxx's analyzer left the locked grids alone",
                 locked >= 2, f"{locked} tracks reported BpmLocked")
    report.check(f"[{label}] Mixxx reported no beat or key deserialization failure",
                 "Failed to deserialize" not in second)
    report.check(f"[{label}] Mixxx did not re-read tags over the synced rows",
                 "Initializing track metadata" not in second and "Re-importing track metadata" not in second)
    after = read_library(database)
    report.check(f"[{label}] the library scan found the same {len(names)} tracks, not duplicates",
                 len(after["tracks"]) == len(names), f"{len(after['tracks'])}")
    report.check(f"[{label}] the synced rows are unchanged after Mixxx loaded and scanned them",
                 digest(after) == after_sync)
    report.check(f"[{label}] the list of music folders is unchanged", scan_list(database) == folders)
    return {"first_log_lines": created.count("\n"), "second_log_lines": second.count("\n")}


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--mixxx", default=DEFAULT_MIXXX)
    parser.add_argument("--only", help="run just these scenarios (comma separated)")
    parser.add_argument("--work", help="scratch folder inside Mixxx's sandbox container "
                        "(default: MixxxAcceptance/run-<time> beside Mixxx's own folder)")
    args = parser.parse_args()
    if sys.platform != "darwin":
        sys.exit("this harness drives the macOS Mixxx build and its CoreAudio decoder")
    if not Path(args.mixxx).is_file():
        sys.exit(f"Mixxx not found at {args.mixxx}")
    if running := mixxx_running():
        sys.exit(f"Mixxx is already running ({running[0].strip()}); quit it and run again")
    work = Path(args.work or CONTAINER / "MixxxAcceptance" / time.strftime("run-%Y%m%d-%H%M%S"))
    sources = work / "source"
    sources.mkdir(parents=True, exist_ok=True)
    report = Report()

    # audio, made once and copied into each scenario
    wav = sources / "click128.wav"
    write_click_track(wav)
    run("ffmpeg", "-v", "error", "-y", "-i", str(wav), "-c:a", "flac", str(sources / "click128.flac"))
    run("lame", "--quiet", "-b", "192", str(wav), str(sources / "click128.mp3"))
    manifest = [
        {"file": name, "title": f"Acceptance {Path(name).suffix[1:].upper()}", "key": "8A", "bpm": BPM,
         "first_beat": FIRST_BEAT, "duration": DURATION, "sample_rate": SAMPLE_RATE, "cues": CUES}
        for name in NAMES
    ]
    (sources / "manifest.json").write_text(json.dumps(manifest))
    probe = work / "coreaudio_probe"
    run("clang", "-framework", "AudioToolbox", "-framework", "CoreFoundation",
        str(HERE / "coreaudio_probe.c"), "-o", str(probe))
    truth = probe_frames(probe, {name: sources / name for name in NAMES})
    print(f"CoreAudio's Mixxx frame of the first click: {truth}")

    # the files of the formats scenario: encoded from the same click track
    format_names = []
    manifest_formats = []
    for name, codec in FORMATS:
        target = sources / name
        run("ffmpeg", "-v", "error", "-y", "-i", str(wav), *codec, str(target))
        format_names.append(name)
        manifest_formats.append(
            {"file": name, "title": f"Acceptance {name}", "key": "8A", "bpm": BPM, "first_beat": FIRST_BEAT,
             "duration": DURATION, "sample_rate": native_rate(target), "cues": CUES})
    (sources / "manifest-formats.json").write_text(json.dumps(manifest_formats))

    wanted = args.only.split(",") if args.only else ["fresh", "existing", "overwrite", "created", "formats"]
    logs, reference = {}, {}
    for name in wanted:
        if name == "formats":
            shutil.copy2(sources / "manifest-formats.json", sources / "manifest.json")
            logs[name] = scenario(report, args.mixxx, work / name, sources, {}, name, reference,
                                  names=tuple(format_names), probe=probe)
        else:
            (sources / "manifest.json").write_text(json.dumps(manifest))
            logs[name] = scenario(report, args.mixxx, work / name, sources, truth, name, reference)

    failed = [c for c in report.checks if not c["ok"]]
    (work / "report.json").write_text(json.dumps(
        {"mixxx": args.mixxx, "checks": report.checks, "log_lines": logs}, indent=1))
    print(f"\n{len(report.checks) - len(failed)} of {len(report.checks)} checks passed. Files: {work}")
    return 1 if failed else 0


if __name__ == "__main__":
    sys.exit(main())
