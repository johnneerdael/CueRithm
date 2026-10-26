#!/usr/bin/env python3
"""Blur every file path in raw captures, so a guide image never shows one.

AGENTS.md lets the guide's screenshots show the library as the app displays it, but not audio files, file
paths, user names, credentials or licence details. A settings page cannot help showing a watch folder, so
this finds each line of text that looks like a path (Vision OCR, via ocrboxes) and blurs its box.

Usage: redact_paths.py IMAGE...      (edits the images in place; prints one line per redaction)
"""
import os
import re
import subprocess
import sys

PATH_LINE = re.compile(r"(?:^|\s)(?:/(?:private|tmp|users|home|volumes|mnt|var)/|~/|[A-Za-z]:\\)", re.I)


def is_path(text):
    """True for a line that contains a file path: a rooted system folder, a home shortcut or a drive letter."""
    return bool(PATH_LINE.search(text))


def ocr_tool():
    path = os.environ.get("CUERITHM_OCRBOXES") or os.path.expanduser("~/.cache/cuerithm-docs/ocrboxes")
    if not os.path.exists(path):
        os.makedirs(os.path.dirname(path), exist_ok=True)
        here = os.path.dirname(os.path.abspath(__file__))
        subprocess.run(["swiftc", "-O", os.path.join(here, "ocrboxes.swift"), "-o", path], check=True)
    return path


def path_boxes(image_path):
    lines = subprocess.run([ocr_tool(), image_path], capture_output=True, text=True, check=True).stdout.splitlines()
    boxes = []
    for line in lines:
        x, y, w, h, text = line.split("\t", 4)
        if is_path(text):
            boxes.append((int(x), int(y), int(w), int(h), text))
    return boxes


def blur(image_path, boxes, pad=8, radius=14):
    from PIL import Image, ImageFilter
    image = Image.open(image_path).convert("RGBA")
    for x, y, w, h, _ in boxes:
        region = (max(0, x - pad), max(0, y - pad), min(image.width, x + w + pad), min(image.height, y + h + pad))
        crop = image.crop(region).filter(ImageFilter.GaussianBlur(radius))
        image.paste(crop, region[:2])
    image.save(image_path)


def main(paths):
    for path in paths:
        boxes = path_boxes(path)
        if boxes:
            blur(path, boxes)
            print(f"{os.path.basename(path)}: blurred {len(boxes)} path line(s)")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
