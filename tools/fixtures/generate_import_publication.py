#!/usr/bin/env python3
"""Regenerate invented one-second tag-publication fixtures (developer tool only).

Requires an encoder-enabled FFmpeg on PATH. Product code and tests use the
committed archive and bundled decoder; no installed encoder/Python is required.
The original signal and these generated fixtures are dedicated to CC0.
"""
import gzip
import hashlib
import io
import json
import math
from pathlib import Path
import struct
import subprocess
import tarfile
import tempfile
import wave

OUTPUT = Path(__file__).with_name("import-publication-media.tar.gz")
CASES = [
    ("mp3", "mp3", "libmp3lame"), ("adts", "aac", "aac"),
    ("aac", "m4a", "aac"), ("alac", "m4a", "alac"),
    ("flac", "flac", "flac"), ("wav", "wav", "pcm_s16le"),
    ("aiff", "aiff", "pcm_s16be"), ("vorbis", "ogg", "libvorbis"),
    ("opus", "opus", "libopus"), ("wma", "wma", "wmav2"),
]

def main():
    with tempfile.TemporaryDirectory() as directory:
        root = Path(directory)
        source = root / "input.wav"
        with wave.open(str(source), "wb") as output:
            output.setparams((1, 2, 44100, 44100, "NONE", "not compressed"))
            output.writeframes(b"".join(struct.pack("<h", int(math.sin(n * 440 * math.tau / 44100) * 10000)) for n in range(44100)))
        entries = {}
        cases = []
        for name, extension, codec in CASES:
            filename = f"{name}.{extension}"
            path = root / filename
            subprocess.run(["ffmpeg", "-v", "error", "-i", str(source), "-c:a", codec,
                            "-metadata", "comment=Retain original comment", str(path)], check=True)
            data = path.read_bytes()
            entries[filename] = data
            cases.append({"file": filename, "extension": extension, "codec": codec,
                          "sha256": hashlib.sha256(data).hexdigest()})
        version = subprocess.check_output(["ffmpeg", "-version"], text=True).splitlines()[0]
        entries["manifest.json"] = (json.dumps({"license": "CC0-1.0", "signal": "440Hz mono 16-bit PCM, 44100Hz, one second", "generator": version, "cases": cases}, indent=2) + "\n").encode()
        with OUTPUT.open("wb") as raw, gzip.GzipFile(fileobj=raw, mode="wb", mtime=0) as zipped, tarfile.open(fileobj=zipped, mode="w") as archive:
            for name, data in sorted(entries.items()):
                entry = tarfile.TarInfo(name)
                entry.size = len(data)
                entry.mode = 0o644
                entry.mtime = 0
                archive.addfile(entry, io.BytesIO(data))
        print(f"{OUTPUT.name}: {OUTPUT.stat().st_size} bytes; sha256={hashlib.sha256(OUTPUT.read_bytes()).hexdigest()}")

if __name__ == "__main__":
    main()
