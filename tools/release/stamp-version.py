#!/usr/bin/env python3
"""Stamp one release identity into the disposable packaging checkout."""
import json
from pathlib import Path
import re
import sys
import tomllib


def stamp(version: str, destination: Path, workspace: Path | None = None) -> None:
    # Published releases use numeric SemVer; reject header/control/path injection.
    if not re.fullmatch(r"(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)", version):
        raise ValueError("Release version must be numeric SemVer")
    root = workspace if workspace is not None else Path(__file__).resolve().parents[2]
    manifest = root / "Cargo.toml"
    original = manifest.read_text(encoding="utf-8")
    package = tomllib.loads(original)["workspace"]
    previous = package["package"]["version"]
    members = set()
    for member in package["members"]:
        info = tomllib.loads((root / member / "Cargo.toml").read_text(encoding="utf-8"))["package"]
        if info["version"] != {"workspace": True}:
            raise ValueError(f"Release member does not inherit the workspace version: {member}")
        members.add(info["name"])
    section = re.compile(r"(?ms)(^\[workspace\.package\]\n)(.*?)(?=^\[|\Z)")
    matches = list(section.finditer(original))
    if len(matches) != 1:
        raise ValueError("Expected one workspace package section")
    body, count = re.subn(r'^version\s*=\s*"[^"\n]+"\s*$',
                          f'version = "{version}"', matches[0][2], flags=re.MULTILINE)
    if count != 1:
        raise ValueError("Expected one workspace package version")
    updated = original[:matches[0].start(2)] + body + original[matches[0].end(2):]
    lock = root / "Cargo.lock"
    blocks = re.split(r"(?m)(?=^\[\[package\]\]$)", lock.read_text(encoding="utf-8"))
    found = set()
    for index, block in enumerate(blocks):
        if not block.startswith("[[package]]"):
            continue
        info = tomllib.loads(block)["package"][0]
        if info["name"] in members and "source" not in info:
            if info["version"] != previous or info["name"] in found:
                raise ValueError(f"Unexpected locked release identity: {info['name']}")
            found.add(info["name"])
            block, count = re.subn(r'^version = "[^"\n]+"$', f'version = "{version}"',
                                  block, count=1, flags=re.MULTILINE)
            if count != 1:
                raise ValueError(f"Missing locked release version: {info['name']}")
            blocks[index] = block
    if found != members:
        raise ValueError("Cargo.lock does not contain every release member")
    locked = "".join(blocks)
    # Cargo disambiguates dependency names with versions when needed. External
    # source-qualified references remain untouched, even at the same version.
    for member in members:
        locked = locked.replace(f'"{member} {previous}"', f'"{member} {version}"')
    # Validate all input before mutating the build checkout. The source archive
    # still comes from HEAD; checked-in manifests never receive a release bump.
    destination.parent.mkdir(parents=True, exist_ok=True)
    manifest.write_text(updated, encoding="utf-8")
    lock.write_text(locked, encoding="utf-8")
    destination.write_text(json.dumps({"version": version}) + "\n", encoding="utf-8")


if __name__ == "__main__":
    if len(sys.argv) != 3:
        raise SystemExit("usage: stamp-version.py VERSION OUTPUT")
    stamp(sys.argv[1], Path(sys.argv[2]))
