"""Fetch and verify pinned source. Build tooling only; never used by the app."""
from pathlib import Path
import hashlib
import json
import subprocess
import tarfile

root = Path(__file__).resolve().parents[2]
config = json.loads((root / "tools/media-runtime/manifest.json").read_text())
build = root / ".superpowers/media-build"
build.mkdir(parents=True, exist_ok=True)
archive = build / ("ffmpeg-" + config["version"] + ".tar.xz")
if not archive.exists():
    subprocess.run(["curl", "--fail", "--location", "--output", str(archive), config["source"]], check=True)
assert hashlib.sha256(archive.read_bytes()).hexdigest() == config["source_sha256"], "source hash mismatch"
target = build / ("ffmpeg-" + config["version"])
if not target.exists():
    with tarfile.open(archive) as stream:
        stream.extractall(build, filter="data")
