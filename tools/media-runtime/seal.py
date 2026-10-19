"""Validate decoders and portable linkage; write a runtime hash manifest."""
from pathlib import Path
import hashlib
import json
import platform
import subprocess
import sys
import re
from linkage import system_dependencies

dest = Path(sys.argv[1]).resolve()
config = json.loads((Path(__file__).parent / "manifest.json").read_text())
ext = ".exe" if platform.system() == "Windows" or (dest / "ffmpeg.exe").exists() else ""
ffmpeg = dest / ("ffmpeg" + ext)
decoders = subprocess.check_output([str(ffmpeg), "-hide_banner", "-decoders"], text=True)
available = {line.split()[1] for line in decoders.splitlines() if len(line.split()) > 2}
assert set(config["required_decoders"]) <= available, "missing required decoders"
muxers = subprocess.check_output([str(ffmpeg), "-hide_banner", "-muxers"], text=True)
available_muxers = {line.split()[1] for line in muxers.splitlines() if len(line.split()) > 2}
assert {"f32le", "image2pipe", "md5"} <= available_muxers, "missing PCM/artwork/fingerprint muxers"
encoders = subprocess.check_output([str(ffmpeg), "-hide_banner", "-encoders"], text=True)
available_encoders = {line.split()[1] for line in encoders.splitlines() if len(line.split()) > 2}
assert {"pcm_f32le", "pcm_s16le"} <= available_encoders, "missing playback/fingerprint encoders"
for name in ["ffmpeg", "ffprobe"]:
    path = dest / (name + ext)
    if platform.system() == "Darwin":
        deps = subprocess.check_output(["otool", "-L", str(path)], text=True).splitlines()[1:]
        deps = [line.strip().split(" (", 1)[0] for line in deps]
        assert deps and system_dependencies("Darwin", deps), deps
    elif platform.system() == "Linux":
        deps = subprocess.run(["ldd", str(path)], text=True, capture_output=True)
        assert deps.returncode == 0, deps.stderr
        assert "not found" not in deps.stdout, deps.stdout
        names = [line.strip().split()[0].split("/")[-1]
                 for line in deps.stdout.splitlines() if line.strip()]
        assert names and system_dependencies("Linux", names), deps.stdout
    elif ext == ".exe":
        deps = subprocess.check_output(["objdump", "-p", str(path)], text=True)
        names = re.findall(r"DLL Name:\s*(\S+)", deps)
        assert names and system_dependencies("Windows", names), names
    else:
        raise RuntimeError("Unsupported runtime platform")
    config[name + "_sha256"] = hashlib.sha256(path.read_bytes()).hexdigest()
(dest / "runtime.json").write_text(json.dumps(config, indent=2) + "\n")
print("Sealed runtime:", dest)
