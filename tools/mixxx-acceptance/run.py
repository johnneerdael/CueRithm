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

`--linux` runs the `formats` scenario against Debian's Mixxx 2.5.0 in a Docker container
instead (image from `tools/mixxx-acceptance/linux/Dockerfile`): the real Linux Mixxx on a
virtual display (Xvfb) with software OpenGL and no audio device. Nobody clicks anything;
the checks read Mixxx's database and debug log. MP3 is decoded there with libmad, and the
first click is measured by `mad_probe.c`, which does what Mixxx's MP3 source does. The sync
runs on the host as a Linux system would (`CUERITHM_MIXXX_HOST=linux`: libmad timing).

`--linux-main VOLUME` does the same with a Mixxx built from source (`main`, schema 40, with
FAAD and libmad) in a Docker volume made by `linux-main/build.sh`: AAC in M4A is then decoded
by FAAD, measured with `faad_probe.c`, and the library CueRithm creates (schema 39) is
upgraded to 40 by Mixxx itself, after which its tables must equal a library Mixxx created.

`--windows` runs the `formats` and `created` scenarios against Mixxx for Windows (an installed
or unpacked release; `windows/bootstrap.ps1` sets up everything without administrator rights).
AAC and WMA are decoded there by Media Foundation, measured with `mf_probe.cpp`, which opens the
file the way Mixxx's Media Foundation source does. The sync is the native Windows one. MP3 is
decoded by libmad, for which there is no Windows probe, so its alignment is reported as not
measured (the libmad rule was measured on Linux); the other formats are compared with FFmpeg's
decode as on Linux.

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
import itertools
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
DOCKER = "docker"
LINUX_IMAGE = "cuerithm-mixxx-linux"
LINUX_MAIN = None  # the Docker volume holding a Mixxx built from source, with --linux-main
WINDOWS = False  # with --windows: Mixxx for Windows, Media Foundation, no Docker
_containers = itertools.count()
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


def mixxx_path(path):
    """A folder or file as Mixxx stores it: Qt's separators, so forward slashes on Windows too."""
    resolved = Path(path).resolve()
    return resolved.as_posix() if WINDOWS else str(resolved)


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


def read_library(database, live=False):
    """The library's rows. While Mixxx is running (`live`), read without taking SQLite's locks:
    over a shared mount a reader's lock can make Mixxx's own writes fail with "database is
    locked", and a count that is a moment stale does no harm."""
    uri = f"file:{database}?mode=ro" + ("&immutable=1" if live else "")
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
    if WINDOWS:
        out = run("tasklist", "/FI", "IMAGENAME eq mixxx.exe", "/NH").stdout.decode().splitlines()
        return [line for line in out if line.strip().lower().startswith("mixxx.exe")]
    out = run("ps", "-A", "-o", "comm=").stdout.decode().splitlines()
    return [line for line in out if line.strip().lower().endswith("mixxx")]


class Mixxx:
    """One Mixxx process on the scratch settings folder; always stopped on exit."""

    def __init__(self, mixxx, settings, tracks, log):
        self.log = open(log, "wb")
        self.settings = settings
        self.container = None
        self.volume = None
        if mixxx == DOCKER:
            # Mixxx on a virtual display. Its library folder is ~/.mixxx, the Linux default, and
            # the music is mounted at the same path the host wrote into the library.
            #
            # The library lives on a Docker volume while Mixxx runs. SQLite's locking is not
            # reliable on a folder shared with the host: Mixxx main failed its import there with
            # "database is locked" every time, and imported all tracks in seconds on a volume. The
            # host's copy (the one CueRithm syncs) is copied in before the start and back after
            # the stop.
            music = (settings.parent / "music").resolve()
            self.container = f"cuerithm-mixxx-{os.getpid()}-{next(_containers)}"
            self.volume = self.container + "-settings"
            subprocess.run(["docker", "volume", "create", self.volume], capture_output=True, check=True)
            subprocess.run(["docker", "run", "--rm", "-v", f"{self.volume}:/v", "-v", f"{settings}:/h:ro",
                            LINUX_IMAGE, "sh", "-c", "cp -a /h/. /v/"], capture_output=True, check=True)
            extra, program = [], ["mixxx"]
            if LINUX_MAIN:
                extra = ["-v", f"{LINUX_MAIN}:/out"]
                program = ["env", "LD_LIBRARY_PATH=/out/mp4v2/lib", "/out/install/bin/mixxx"]
            command = ["docker", "run", "--rm", "--init", "--name", self.container,
                       "-v", f"{self.volume}:/root/.mixxx", "-v", f"{music}:{music}", *extra, LINUX_IMAGE,
                       "timeout", "900", "xvfb-run", "-a", *program, "--log-level", "debug",
                       "--log-flush-level", "debug", *map(str, tracks)]
        else:
            command = [mixxx, "--settings-path", str(settings), "--log-level", "debug",
                       "--log-flush-level", "debug", *map(str, tracks)]
        self.process = subprocess.Popen(command, stdout=self.log, stderr=subprocess.STDOUT)

    def stop(self):
        if self.container and self.process.poll() is None:
            subprocess.run(["docker", "stop", "-t", "10", self.container], capture_output=True)
        if self.process.poll() is None:
            self.process.terminate()
            try:
                self.process.wait(20)
            except subprocess.TimeoutExpired:
                self.process.kill()
                self.process.wait()
        self.log.close()
        if self.volume:
            # bring the library back to the host, exactly as Mixxx left it
            subprocess.run(["docker", "run", "--rm", "-v", f"{self.volume}:/v:ro", "-v", f"{self.settings}:/h",
                            LINUX_IMAGE, "sh", "-c", "find /h -mindepth 1 -delete; cp -a /v/. /h/"],
                           capture_output=True, check=True)
            subprocess.run(["docker", "volume", "rm", "-f", self.volume], capture_output=True)
            self.volume = None

    def sql(self, query):
        """Run a query on the library of the running Linux Mixxx (None until it can answer)."""
        done = subprocess.run(["docker", "exec", self.container, "sqlite3",
                               "file:/root/.mixxx/mixxxdb.sqlite?mode=ro&immutable=1", query],
                              capture_output=True, text=True)
        return done.stdout.strip() if done.returncode == 0 else None

    def ready(self):
        """Mixxx has created and migrated its library."""
        if self.container:
            return bool(self.sql("SELECT value FROM settings WHERE name = 'mixxx.schema.version'"))
        return library_ready(self.settings / "mixxxdb.sqlite")

    def track_count(self):
        if self.container:
            count = self.sql("SELECT COUNT(*) FROM library")
            return int(count) if count and count.isdigit() else 0
        try:
            return len(read_library(self.settings / "mixxxdb.sqlite", live=True)["tracks"])
        except RuntimeError:
            return 0

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

    def note(self, text):
        """Something that was not measured: shown, and counted as neither a pass nor a failure."""
        self.notes = getattr(self, "notes", []) + [text]
        print(f"NOTE  {text}")


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
# Windows can decode WMA through Media Foundation, so its run measures it as well.
WINDOWS_FORMATS = (("click128.wma", ["-c:a", "wmav2", "-b:a", "192k"]),)


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


def mad_frame(path):
    """Mixxx's frame of the first click for an MP3 read with libmad (see mad_probe.c)."""
    out = run("docker", "run", "--rm", "-v", f"{HERE}:/probe:ro", "-v", f"{path.parent}:{path.parent}:ro",
              LINUX_IMAGE, "sh", "-c",
              f"gcc -O2 -o /tmp/mad_probe /probe/mad_probe.c -lmad -lm && /tmp/mad_probe '{path}'").stdout.decode()
    return int(dict(item.split("=") for item in out.split())["mixxx_frame"])


def faad_frame(path):
    """Mixxx's frame of the first click for an M4A read with FAAD (see faad_probe.c)."""
    build = ("gcc -O2 -I/out/mp4v2/include -o /tmp/faad_probe /probe/faad_probe.c "
             "-L/out/mp4v2/lib -lfaad -lmp4v2 -lm")
    out = run("docker", "run", "--rm", "-v", f"{HERE}:/probe:ro", "-v", f"{path.parent}:{path.parent}:ro",
              "-v", f"{LINUX_MAIN}:/out:ro", LINUX_IMAGE, "sh", "-c",
              f"{build} && LD_LIBRARY_PATH=/out/mp4v2/lib /tmp/faad_probe '{path}'").stdout.decode()
    return int(dict(item.split("=") for item in out.split())["mixxx_frame"])


def mf_frame(probe, path):
    """Mixxx's frame of the first click for a file read with Media Foundation (see mf_probe.cpp)."""
    out = run(str(probe), str(path)).stdout.decode()
    return int(dict(item.split("=") for item in out.split())["mixxx_frame"])


def format_truth(probe, files, providers):
    """Where the decoder Mixxx uses reads the first click, and at what rate, for each file."""
    truth, rates = {}, {}
    for name, path in files.items():
        extension = Path(name).suffix[1:]
        first = (providers.get(extension) or ["?"])[0]
        rates[name] = native_rate(path)
        if WINDOWS and "Media Foundation" in first:
            truth[name] = mf_frame(probe, path)
        elif WINDOWS and first.startswith("MAD"):
            truth[name] = None  # no libmad probe for Windows
        elif first.startswith("MAD"):
            truth[name] = mad_frame(path)
        elif LINUX_MAIN and ("FAAD" in first.upper() or "M4A" in first.upper()):
            try:
                truth[name] = faad_frame(path)
            except subprocess.CalledProcessError:
                # FAAD cannot open Apple Lossless in an M4A; Mixxx then tries its next decoder.
                truth[name] = ffmpeg_first_click(path, rates[name])
        elif "CoreAudio" in first:
            truth[name] = probe_frames(probe, {name: path})[name]
        else:
            truth[name] = ffmpeg_first_click(path, rates[name])
    return truth, rates


def library_ready(database):
    """Mixxx has created and migrated its library."""
    if not database.exists():
        return False
    try:
        connection = sqlite3.connect(f"file:{database}?mode=ro&immutable=1", uri=True, timeout=2)
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
        if truth[name] is None:
            report.note(f"{tag}: alignment with Mixxx's decoder was not measured (written first beat {first_beat})")
        else:
            gap = first_beat - truth[name]
            report.check(f"{tag}: first beat within 16 frames of the first click as Mixxx's decoder reads it",
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


def scenario(report, mixxx, work, sources, truth, mode, reference, names=NAMES, probe=None, host="",
             upgrade=False, aac=""):
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
        with Mixxx(mixxx, settings, [], work / "mixxx-create.log") as running:
            wait_for(running.ready, 120, "Mixxx to create its library")
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
        connection.execute("INSERT INTO directories(directory) VALUES (?)", (mixxx_path(music),))
        connection.commit()
        connection.close()
        print(f"[{label}] Starting Mixxx so that it adds the tracks itself...")
        # On Linux and Windows the scanner alone adds the tracks. Files given on the command line are
        # loaded onto decks at once, and racing that against the scan made the deck's insert into
        # track_locations fail: "database is locked" on Mixxx main (schema 40) in Docker, and a
        # failed insert of the same files in Mixxx 2.5.6 for Windows.
        first_files = [] if mixxx == DOCKER or WINDOWS else list(files.values())
        # Mixxx for Windows 2.5.6 registers no decoder for WMA (checked against its own list below),
        # and its scanner only adds files it can decode.
        undecodable = [n for n in names if WINDOWS and n.endswith(".wma")]
        wanted_tracks = len(names) - len(undecodable)
        with Mixxx(mixxx, settings, first_files, work / "mixxx-first.log") as running:
            wait_for(lambda: running.track_count() >= wanted_tracks, 180, f"Mixxx to add the {wanted_tracks} tracks")
            time.sleep(5)
        first = read_library(database)
        report.check(f"[{label}] Mixxx created a schema 39 or 40 library", first["schema"] in ("39", "40"),
                     f"schema {first['schema']}")
        log = (work / "mixxx-first.log").read_text(errors="replace")
        lines = log.splitlines()
        if undecodable:
            left_out = [n for n in undecodable if n not in first["tracks"]]
            report.check(f"[{label}] Mixxx lists no decoder for .wma, so its scanner leaves the file out",
                         left_out == undecodable and "wma" not in providers_in(log), f"left out: {left_out}")
            # carry on with the files Mixxx has: the sync is given the same list
            names = tuple(n for n in names if n not in left_out)
            files = {n: p for n, p in files.items() if n not in left_out}
            listed = music / "manifest.json"
            listed.write_text(json.dumps([m for m in json.loads(listed.read_text()) if m["file"] not in left_out]))
        report.check(f"[{label}] Mixxx itself added all {len(names)} tracks that it has a decoder for",
                     len(first["tracks"]) == len(names))
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
            if not WINDOWS:
                report.check(f"[{label}] the installed Mixxx registers no decoder for .wma (so WMA cannot be opened)",
                             "wma" not in providers, "providers: " + ", ".join(sorted(providers)))
            else:
                report.note("decoders Mixxx lists: " + "; ".join(
                    f"{kind}: {names[0] if names else '?'}" for kind, names in sorted(providers.items())))
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
               CUERITHM_MIXXX_LIBRARY="overwrite" if mode == "overwrite" else "update",
               CUERITHM_MIXXX_HOST=host, CUERITHM_MIXXX_AAC=aac)
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
                 folders == [mixxx_path(music)], str(folders))
    if mode == "overwrite":
        report.check(f"[{label}] a playlist only Mixxx had is gone",
                     [p["name"] for p in written["playlists"]] == ["CueRithm acceptance"],
                     str([p["name"] for p in written["playlists"]]))
    if mode in ("created", "overwrite") and not upgrade:
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
    if upgrade and mode in ("created", "overwrite"):
        # Mixxx here is newer than the schema CueRithm creates: it must upgrade it itself, and
        # what it leaves must be the library Mixxx would have created.
        done = next((line.split("SchemaManager - ")[-1] for line in second.splitlines()
                     if "Upgraded database schema to version" in line), "")
        report.check(f"[{label}] Mixxx upgraded the schema 39 library CueRithm created",
                     "Upgrading database schema from version 39" in second and bool(done), done)
        ours = master(database)
        same = ours == reference["master"]
        report.check(f"[{label}] after the upgrade the tables, indexes and views equal a library Mixxx created",
                     same, f"{len(ours)} objects" if same else "differs: " + str(
                         [o[1] for o in set(ours) ^ set(reference["master"])][:6]))
        report.check(f"[{label}] after the upgrade the schema settings equal Mixxx's own",
                     [(n, v) for n, v in mixxx_rows(database)[0] if n != "mixxx.schema.last_used_version"]
                     == [(n, v) for n, v in reference["rows"][0] if n != "mixxx.schema.last_used_version"],
                     str(mixxx_rows(database)[0]))
    else:
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


def check_detection(report, work):
    """CueRithm's FAAD detection, run on the real Mixxx executables of the builds at hand.

    The scenarios tell the adapter which AAC decoder to assume (CUERITHM_MIXXX_AAC) because the
    sync runs on the host, not in the container, so this is what shows that reading the
    installed `mixxx` gives the same answer.
    """
    builds = {}
    if LINUX_MAIN:
        builds["CUERITHM_MIXXX_WITH_FAAD"] = [
            "-v", f"{LINUX_MAIN}:/out:ro", LINUX_IMAGE, "cat", "/out/install/bin/mixxx"]
    if subprocess.run(["docker", "image", "inspect", "cuerithm-mixxx-linux"],
                      capture_output=True).returncode == 0:
        builds["CUERITHM_MIXXX_WITHOUT_FAAD"] = ["cuerithm-mixxx-linux", "cat", "/usr/bin/mixxx"]
    env = dict(os.environ)
    for variable, docker_args in builds.items():
        target = work / variable.lower()
        with open(target, "wb") as out:
            subprocess.run(["docker", "run", "--rm", *docker_args], stdout=out, check=True)
        env[variable] = str(target)
    result = subprocess.run(
        ["cargo", "test", "-p", "cuerithm-dj-sync", "--lib", "--", "--ignored",
         "real_mixxx_executables_are_told_apart"],
        cwd=REPO, env=env, capture_output=True, text=True)
    named = " and ".join("built with FAAD" if "WITH_" in v else "built without it" for v in builds)
    report.check(f"CueRithm reads which AAC decoder the real Mixxx executable has ({named})",
                 "1 passed" in result.stdout, result.stdout[-300:] if "1 passed" not in result.stdout else "")


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--mixxx", default=DEFAULT_MIXXX)
    parser.add_argument("--only", help="run just these scenarios (comma separated)")
    parser.add_argument("--linux-main", metavar="VOLUME",
                        help="like --linux, with a Mixxx built from source in this Docker volume")
    parser.add_argument("--linux", action="store_true",
                        help="run the formats scenario against Debian's Mixxx in Docker (Linux)")
    parser.add_argument("--windows", action="store_true",
                        help="run the formats and created scenarios against Mixxx for Windows "
                        "(--mixxx defaults to the one windows/bootstrap.ps1 unpacks)")
    parser.add_argument("--work", help="scratch folder inside Mixxx's sandbox container "
                        "(default: MixxxAcceptance/run-<time> beside Mixxx's own folder)")
    args = parser.parse_args()
    if args.linux_main:
        global LINUX_IMAGE, LINUX_MAIN
        LINUX_MAIN, LINUX_IMAGE = args.linux_main, "cuerithm-mixxx-main-deps"
        args.linux = True
    if args.windows:
        global WINDOWS
        WINDOWS = True
        if args.mixxx == DEFAULT_MIXXX:
            found = sorted(Path(os.environ.get("CUERITHM_WIN_ROOT", r"D:\cuerithm-win")).glob("mixxx-*/**/mixxx.exe"))
            if not found:
                sys.exit("Mixxx for Windows not found: run windows/bootstrap.ps1 or pass --mixxx PATH")
            args.mixxx = str(found[0])
        if not Path(args.mixxx).is_file():
            sys.exit(f"Mixxx not found at {args.mixxx}")
        if running := mixxx_running():
            sys.exit(f"Mixxx is already running ({running[0].strip()}); quit it and run again")
    elif args.linux:
        args.mixxx = DOCKER
        if subprocess.run(["docker", "image", "inspect", LINUX_IMAGE], capture_output=True).returncode:
            sys.exit(f"build the image first: docker build -t {LINUX_IMAGE} "
                     f"{HERE / ('linux-main' if LINUX_MAIN else 'linux')}")
    else:
        if sys.platform != "darwin":
            sys.exit("this harness drives the macOS Mixxx build and its CoreAudio decoder")
        if not Path(args.mixxx).is_file():
            sys.exit(f"Mixxx not found at {args.mixxx}")
        if running := mixxx_running():
            sys.exit(f"Mixxx is already running ({running[0].strip()}); quit it and run again")
    default_work = (Path(tempfile.mkdtemp(prefix="mixxx-win-" if args.windows else "mixxx-linux-"))
                    if args.linux or args.windows else
                    CONTAINER / "MixxxAcceptance" / time.strftime("run-%Y%m%d-%H%M%S"))
    work = Path(args.work or default_work)
    sources = work / "source"
    sources.mkdir(parents=True, exist_ok=True)
    report = Report()

    # audio, made once and copied into each scenario
    wav = sources / "click128.wav"
    write_click_track(wav)
    run("ffmpeg", "-v", "error", "-y", "-i", str(wav), "-c:a", "flac", str(sources / "click128.flac"))
    if WINDOWS:
        run("ffmpeg", "-v", "error", "-y", "-i", str(wav), "-c:a", "libmp3lame", "-b:a", "192k",
            str(sources / "click128.mp3"))
    else:
        run("lame", "--quiet", "-b", "192", str(wav), str(sources / "click128.mp3"))
    manifest = [
        {"file": name, "title": f"Acceptance {Path(name).suffix[1:].upper()}", "key": "8A", "bpm": BPM,
         "first_beat": FIRST_BEAT, "duration": DURATION, "sample_rate": SAMPLE_RATE, "cues": CUES}
        for name in NAMES
    ]
    (sources / "manifest.json").write_text(json.dumps(manifest))
    probe, truth = None, {}
    if args.windows:
        probe = work / "mf_probe.exe"
        run("g++", "-O2", "-municode", str(HERE / "mf_probe.cpp"), "-o", str(probe),
            "-lmfplat", "-lmfreadwrite", "-lmfuuid", "-lole32")
    elif not args.linux:
        probe = work / "coreaudio_probe"
        run("clang", "-framework", "AudioToolbox", "-framework", "CoreFoundation",
            str(HERE / "coreaudio_probe.c"), "-o", str(probe))
        truth = probe_frames(probe, {name: sources / name for name in NAMES})
        print(f"CoreAudio's Mixxx frame of the first click: {truth}")

    # the files of the formats scenario: encoded from the same click track
    format_names = []
    manifest_formats = []
    for name, codec in FORMATS + (WINDOWS_FORMATS if WINDOWS else ()):
        target = sources / name
        run("ffmpeg", "-v", "error", "-y", "-i", str(wav), *codec, str(target))
        format_names.append(name)
        manifest_formats.append(
            {"file": name, "title": f"Acceptance {name}", "key": "8A", "bpm": BPM, "first_beat": FIRST_BEAT,
             "duration": DURATION, "sample_rate": native_rate(target), "cues": CUES})
    # AAC from Apple's encoder as well: it writes more priming than FFmpeg's, and a decoder that
    # does not read the priming (FAAD) would place it differently.
    fixture = HERE / "fixtures" / "click128.apple.m4a"
    if shutil.which("afconvert") or (WINDOWS and fixture.is_file()):
        apple = sources / "click128.apple.m4a"
        if shutil.which("afconvert"):
            run("afconvert", "-f", "m4af", "-d", "aac", "-b", "192000", str(wav), str(apple))
        else:
            # Made with afconvert from this script's click track (so: 128 BPM, first click at 0.5 s)
            shutil.copy2(fixture, apple)
        format_names.append(apple.name)
        manifest_formats.append(
            {"file": apple.name, "title": f"Acceptance {apple.name}", "key": "8A", "bpm": BPM,
             "first_beat": FIRST_BEAT, "duration": DURATION, "sample_rate": native_rate(apple), "cues": CUES})
    (sources / "manifest-formats.json").write_text(json.dumps(manifest_formats))

    wanted = args.only.split(",") if args.only else ["fresh", "existing", "overwrite", "created", "formats"]
    if args.linux:
        wanted = ["formats", "created"]
    elif args.windows and not args.only:
        wanted = ["formats", "created"]
    logs, reference = {}, {}
    linux_truth = {}
    if (args.linux or args.windows) and "created" in wanted:
        # the files of the created scenario, read by the decoders Mixxx lists for them there
        linux_truth, _ = format_truth(None, {n: sources / n for n in NAMES},
                                      {"mp3": ["MAD"], "wav": ["libsndfile"], "flac": ["libFLAC"]})
    for name in wanted:
        if name == "formats":
            names = tuple(format_names)
            host = ""
            if args.linux or args.windows:
                # MP3 (libmad), WAV and FLAC as well as the formats of the macOS run
                names = NAMES + names
            if args.linux:
                host = "linux"
            (sources / "manifest.json").write_text(json.dumps(
                [m for m in manifest if m["file"] in names] + manifest_formats))
            logs[name] = scenario(report, args.mixxx, work / name, sources, {}, name, reference,
                                  names=names, probe=probe, host=host,
                                  aac=("faad" if LINUX_MAIN else "ffmpeg") if args.linux else "")
        else:
            (sources / "manifest.json").write_text(json.dumps(manifest))
            logs[name] = scenario(report, args.mixxx, work / name, sources,
                                  linux_truth if args.linux or args.windows else truth, name, reference,
                                  host="linux" if args.linux else "", upgrade=bool(LINUX_MAIN),
                                  aac=("faad" if LINUX_MAIN else "ffmpeg") if args.linux else "")

    if args.linux:
        check_detection(report, work)

    failed = [c for c in report.checks if not c["ok"]]
    (work / "report.json").write_text(json.dumps(
        {"mixxx": args.mixxx, "checks": report.checks, "log_lines": logs}, indent=1))
    print(f"\n{len(report.checks) - len(failed)} of {len(report.checks)} checks passed. Files: {work}")
    return 1 if failed else 0


if __name__ == "__main__":
    sys.exit(main())
