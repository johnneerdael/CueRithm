"""Validate decoders and portable linkage; write a runtime hash manifest."""
from pathlib import Path
import hashlib
import json
import platform
import subprocess
import sys

dest = Path(sys.argv[1]).resolve()
config = json.loads((Path(__file__).parent / "manifest.json").read_text())
ext = ".exe" if platform.system() == "Windows" or (dest / "ffmpeg.exe").exists() else ""
ffmpeg = dest / ("ffmpeg" + ext)
decoders = subprocess.check_output([str(ffmpeg), "-hide_banner", "-decoders"], text=True)
available = {line.split()[1] for line in decoders.splitlines() if len(line.split()) > 2}
assert set(config["required_decoders"]) <= available, "missing required decoders"
for name in ["ffmpeg", "ffprobe"]:
    path = dest / (name + ext)
    if platform.system() == "Darwin":
        deps = subprocess.check_output(["otool", "-L", str(path)], text=True).splitlines()[1:]
        assert all(line.strip().startswith(("/usr/lib/", "/System/Library/")) for line in deps), deps
    elif platform.system() == "Linux":
        deps = subprocess.run(["ldd", str(path)], text=True, capture_output=True)
        assert "not found" not in deps.stdout, deps.stdout
    config[name + "_sha256"] = hashlib.sha256(path.read_bytes()).hexdigest()
(dest / "runtime.json").write_text(json.dumps(config, indent=2) + "\n")
print("Sealed runtime:", dest)
