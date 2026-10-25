#!/usr/bin/env python3
"""Capture user-guide screenshots from the running CueRithm window.

Usage:
  capture.py [--themes dark,light] [--out DIR] [--only a,b,c] [--file scenes/x.json ...]
  capture.py --shot NAME [--themes dark]        # capture the current screen as-is

A scene file is a JSON array of {"scene": "name", "steps": [[verb, arg...], ...]}.
Verbs: click PATTERN [NTH] | dblclick PATTERN [NTH] | key KEY | type TEXT | set PATTERN VALUE |
       scroll PATTERN DIR PAGES | clickxy X Y | dblclickxy X Y | drag X1 Y1 X2 Y2 (window points) |
       wait SECONDS | wait_for PATTERN | gone PATTERN | reset | view NAME
A scene may carry "crop": "window" (default) | "panel" | "player"; without it the crop requested in
docs/user-guide/_meta/figures/*.json for that scene id is used.
Patterns are case-insensitive regular expressions over `<role> <label>` of an
accessibility element. Raw captures are PNG with the real window shape; `to_webp.py`
downsizes them. Needs the sandboxed CueRithm that launch-copy.sh started (the only one running) and its window at 1440x940.
"""
import argparse
import glob
import json
import os
import subprocess
import sys
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from cu import Cu, park_cursor, require_capture_app  # noqa: E402

HERE = os.path.dirname(os.path.abspath(__file__))
FIGURES = os.path.join(HERE, "..", "..", "..", "docs", "user-guide", "_meta", "figures")
# Window points at the 1440x940 size: the main panel is right of the sidebar and above the player.
CROPS = {"window": None, "panel": (336, 0, 1440, 737), "player": (336, 737, 1440, 940)}


def requested_crops():
    crops = {}
    for path in glob.glob(os.path.join(FIGURES, "*.json")):
        with open(path, encoding="utf-8") as handle:
            for figure in json.load(handle):
                crops.setdefault(figure["scene"], figure.get("crop", "window"))
    return crops


def windowid_tool():
    """Path of the compiled window-id helper; built from windowid.swift on first use."""
    path = os.environ.get("CUERITHM_WINDOWID") or os.path.expanduser("~/.cache/cuerithm-docs/windowid")
    if not os.path.exists(path):
        os.makedirs(os.path.dirname(path), exist_ok=True)
        subprocess.run(["swiftc", "-O", os.path.join(HERE, "windowid.swift"), "-o", path], check=True)
    return path


def window():
    out = subprocess.run([windowid_tool(), "CueRithm"], capture_output=True, text=True).stdout.split()
    if len(out) != 5:
        raise RuntimeError("CueRithm window not found on screen")
    return [int(v) for v in out]


def ensure_geometry():
    wid, x, y, w, h = window()
    if (w, h) != (1440, 940):
        subprocess.run(["osascript", "-e", 'tell application "System Events" to tell process "CueRithm" to set size of window 1 to {1440, 940}'], check=False)
        time.sleep(0.8)


def activate():
    """Bring the running sandboxed CueRithm to the front. It never starts an app: `open -a` would
    start the installed one on the real library, outside the sandbox, if the sandboxed one had quit."""
    require_capture_app()
    subprocess.run(["osascript", "-e", 'tell application "System Events" to set frontmost of process "CueRithm" to true'], check=True)
    time.sleep(0.6)


def shoot(name, theme, out, crop="window"):
    activate()
    park_cursor()
    time.sleep(0.5)
    wid = window()[0]
    path = os.path.join(out, f"{name}-{theme}.png")
    subprocess.run(["screencapture", "-x", "-o", f"-l{wid}", path], check=True)
    if CROPS[crop]:
        from PIL import Image
        image = Image.open(path)
        scale = image.width / 1440
        image.crop(tuple(round(v * scale) for v in CROPS[crop])).save(path)
    return path


def current_theme(cu):
    cu.state()
    for e in cu.elements:
        if e["text"].startswith("button Colour theme"):
            return e["text"].split("Colour theme: ")[1].split(" ")[0]
    return None


def set_theme(cu, target):
    for _ in range(4):
        have = current_theme(cu)
        if have == target:
            return
        cu.click(r"^button Colour theme", refresh=False)
        time.sleep(0.8)
    raise RuntimeError(f"could not reach theme {target}")


def reset(cu):
    activate()
    cu.key("Escape")
    time.sleep(0.2)
    cu.key("Escape")
    time.sleep(0.2)
    cu.click(r"^button Tracks \d")
    time.sleep(0.6)


def run_steps(cu, steps):
    for step in steps:
        verb, args = step[0], step[1:]
        if verb == "click":
            cu.click(args[0], int(args[1]) if len(args) > 1 else 0)
            time.sleep(0.7)
        elif verb == "dblclick":
            cu.click(args[0], int(args[1]) if len(args) > 1 else 0, click_count=2)
            time.sleep(1.2)
        elif verb in ("clickxy", "dblclickxy"):
            x, y = cu.shot_point(float(args[0]), float(args[1]))
            cu.call("click", app="CueRithm", x=x, y=y, click_count=2 if verb == "dblclickxy" else 1)
            time.sleep(1.2 if verb == "dblclickxy" else 0.7)
        elif verb == "drag":
            (x1, y1), (x2, y2) = cu.shot_point(float(args[0]), float(args[1])), cu.shot_point(float(args[2]), float(args[3]))
            cu.call("drag", app="CueRithm", from_x=x1, from_y=y1, to_x=x2, to_y=y2)
            time.sleep(0.8)
        elif verb == "key":
            cu.key(args[0])
            time.sleep(0.5)
        elif verb == "type":
            cu.type(args[0])
            time.sleep(0.5)
        elif verb == "set":
            e = cu.find(args[0])
            cu.call("set_value", app="CueRithm", element_index=e["idx"], value=args[1])
            time.sleep(0.8)
        elif verb == "scroll":
            e = cu.find(args[0])
            cu.call("scroll", app="CueRithm", element_index=e["idx"], direction=args[1], pages=float(args[2]))
            time.sleep(0.6)
        elif verb == "wait":
            time.sleep(float(args[0]))
        elif verb == "wait_for":
            cu.wait_for(args[0], timeout=float(args[1]) if len(args) > 1 else 30)
        elif verb == "gone":
            cu.wait_for(args[0], timeout=60, absent=True)
        elif verb == "reset":
            reset(cu)
        elif verb == "view":
            cu.click(rf"^button {args[0]}\b")
            time.sleep(0.8)
        else:
            raise ValueError(f"unknown verb {verb}")


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--themes", default="dark,light")
    parser.add_argument("--out", default="/tmp/cuerithm-raw")
    parser.add_argument("--only", default="")
    parser.add_argument("--file", nargs="*")
    parser.add_argument("--shot")
    args = parser.parse_args()
    os.makedirs(args.out, exist_ok=True)
    cu = Cu()
    activate()
    ensure_geometry()
    themes = args.themes.split(",")
    if args.shot:
        for theme in themes:
            set_theme(cu, theme)
            print(shoot(args.shot, theme, args.out))
        return 0
    files = args.file or sorted(glob.glob(os.path.join(HERE, "scenes", "*.json")))
    scenes = []
    for path in files:
        with open(path, encoding="utf-8") as handle:
            scenes += json.load(handle)
    wanted = set(filter(None, args.only.split(",")))
    crops = requested_crops()
    failures = []
    volume = cu.find(r"^slider .*Volume")
    cu.call("set_value", app="CueRithm", element_index=volume["idx"], value="0")  # never play audio out loud
    for theme in themes:
        set_theme(cu, theme)
        for scene in scenes:
            if wanted and scene["scene"] not in wanted:
                continue
            try:
                reset(cu)
                run_steps(cu, scene["steps"])
                time.sleep(float(scene.get("settle", 0.8)))
                print("ok ", theme, shoot(scene["scene"], theme, args.out, scene.get("crop") or crops.get(scene["scene"], "window")))
            except Exception as error:  # keep going: report at the end
                failures.append((theme, scene["scene"], str(error)[:200]))
                print("FAIL", theme, scene["scene"], str(error)[:200])
    try:
        cu.state()
        cu.call("set_value", app="CueRithm", element_index=cu.find(r"^slider .*Volume", refresh=False)["idx"], value="0.65")
    except Exception:
        pass
    cu.close()
    for failure in failures:
        print("FAILED", *failure)
    return 1 if failures else 0


if __name__ == "__main__":
    sys.exit(main())
