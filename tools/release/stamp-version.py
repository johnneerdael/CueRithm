#!/usr/bin/env python3
"""Write a Tauri version override without modifying checked-in manifests."""
import json
from pathlib import Path
import re
import sys


def stamp(version: str, destination: Path) -> None:
    # Published releases use numeric SemVer; reject header/control/path injection.
    if not re.fullmatch(r"(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)", version):
        raise ValueError("Release version must be numeric SemVer")
    destination.parent.mkdir(parents=True, exist_ok=True)
    destination.write_text(json.dumps({"version": version}) + "\n", encoding="utf-8")


if __name__ == "__main__":
    if len(sys.argv) != 3:
        raise SystemExit("usage: stamp-version.py VERSION OUTPUT")
    stamp(sys.argv[1], Path(sys.argv[2]))
