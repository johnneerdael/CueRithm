#!/usr/bin/env python3
"""Convert raw window captures to the guide's screenshots.

Usage: to_webp.py RAW_DIR DEST_DIR [scene ...]
A raw capture is 2880x1880 (a 1440x940 window at 2x). Each becomes a 2000 px wide
WebP at quality 82 named `<scene>-<theme>.webp`, keeping the window's rounded corners.
"""
import glob
import os
import subprocess
import sys

from PIL import Image

raw, dest, *only = sys.argv[1:]
os.makedirs(dest, exist_ok=True)
for path in sorted(glob.glob(os.path.join(raw, "*.png"))):
    name = os.path.basename(path)[:-4]
    scene = name.rsplit("-", 1)[0]
    if only and scene not in only:
        continue
    image = Image.open(path).convert("RGBA")
    height = round(image.height * 2000 / image.width)
    small = image.resize((2000, height), Image.LANCZOS)
    tmp = path[:-4] + ".small.png"
    small.save(tmp)
    out = os.path.join(dest, name + ".webp")
    subprocess.run(["cwebp", "-quiet", "-q", "82", "-alpha_q", "90", tmp, "-o", out], check=True)
    os.remove(tmp)
    print(out, os.path.getsize(out) // 1024, "KB")
