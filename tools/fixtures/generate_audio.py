"""Generate original test media with a developer FFmpeg encoder build."""
from pathlib import Path
import hashlib
import array
import json
import subprocess
import tarfile
import tempfile

root = Path(__file__).resolve().parents[2]
source = root / "crates/camelot-source-analysis/tests/fixtures/pulses64-stereo16.flac"
formats = [
    ("mp3", "libmp3lame", [], False),
    ("aac", "aac", [], False),
    ("m4a", "aac", [], False),
    ("alac.m4a", "alac", [], True),
    ("flac", "flac", [], True),
    ("wav", "pcm_s16le", [], True),
    ("aiff", "pcm_s16be", [], True),
    ("ogg", "vorbis", ["-strict", "experimental"], False),
    ("opus", "libopus", [], False),
    ("wma", "wmav2", [], False),
    ("24.wav", "pcm_s24le", [], True),
    ("float.wav", "pcm_f32le", [], True),
    ("mono48.wav", "pcm_s16le", ["-ac", "1", "-ar", "48000"], False),
    ("stereo96.wav", "pcm_s16le", ["-ar", "96000"], False),
]
with tempfile.TemporaryDirectory() as folder:
    tmp = Path(folder)
    rows = []
    for ext, codec, extra, lossless in formats:
        name = "pulses." + ext
        path = tmp / name
        subprocess.run(["ffmpeg", "-nostdin", "-v", "error", "-i", str(source),
            "-map", "0:a:0", "-c:a", codec, *extra, str(path)], check=True)
        pcm = subprocess.check_output(["ffmpeg", "-nostdin", "-v", "error", "-i", str(path),
            "-map", "0:a:0", "-ac", "2", "-ar", "44100", "-af",
            "aresample=44100:resampler=swr:dither_method=none", "-c:a", "pcm_f32le", "-f", "f32le", "-"])
        rows.append(dict(file=name, frames=len(pcm)//8, sha256=hashlib.sha256(pcm).hexdigest(),
            lossless=lossless, encoder=codec, options=extra,
            checkpoints=[[i,v] for i,v in enumerate(array.array("f",pcm)) if i % 44100 == 0]))
    (tmp / "cases.json").write_text(json.dumps(rows, indent=2) + "\n")
    with tarfile.open(root / "tools/fixtures/media.tar.gz", "w:gz") as out:
        for path in sorted(tmp.iterdir()):
            out.add(path, arcname=path.name)
    (root / "tools/fixtures/media-cases.json").write_text(json.dumps([{k:v for k,v in r.items() if k != "checkpoints"} for r in rows], indent=2) + "\n")
print("Generated", len(rows), "original fixtures")
