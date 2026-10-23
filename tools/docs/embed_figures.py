#!/usr/bin/env python3
"""Insert user-guide screenshots into pages from docs/user-guide/_meta/figures/*.json.

A figure is written only when both assets/screens/<scene>-light.webp and
-dark.webp exist, so requests can be listed before the captures exist and the
strict build stays green. Figures are fenced by `<!-- figure:<scene> -->` markers,
so running this again updates them in place instead of duplicating them.

Usage: python3 tools/docs/embed_figures.py [--check]
  --check   report missing images and unknown pages or headings; change nothing
"""
import glob
import html
import json
import os
import re
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
DOCS = os.path.join(ROOT, "docs", "user-guide")
SCREENS = os.path.join(DOCS, "assets", "screens")
MANIFESTS = os.path.join(DOCS, "_meta", "figures", "*.json")
BLOCK_START = re.compile(r"^(\s*)(```|~~~|\||[-*+] |\d+\. |!!! |\?\?\? |<|\{|=== )")
MARKER = re.compile(r"<!-- figure:(?P<scene>[\w-]+) -->.*?<!-- /figure -->\n?", re.S)


def figure(entry, page):
    rel = os.path.relpath(SCREENS, os.path.dirname(os.path.join(DOCS, page))).replace(os.sep, "/")
    alt = entry["alt"].replace("[", "(").replace("]", ")")
    scene = entry["scene"]
    if entry.get("images"):  # existing light and dark images with their own names
        pictures = [
            f"![{alt}]({rel}/{entry['images']['light']}#only-light){{ .screen loading=lazy }}",
            f"![{alt}]({rel}/{entry['images']['dark']}#only-dark){{ .screen loading=lazy }}",
        ]
    elif entry.get("image"):  # an existing single image, used until a light/dark pair is captured
        pictures = [f"![{alt}]({rel}/{entry['image']}){{ .screen loading=lazy }}"]
    else:
        pictures = [
            f"![{alt}]({rel}/{scene}-light.webp#only-light){{ .screen loading=lazy }}",
            f"![{alt}]({rel}/{scene}-dark.webp#only-dark){{ .screen loading=lazy }}",
        ]
    lines = [f"<!-- figure:{scene} -->", "<figure markdown>", *pictures]
    if entry.get("caption"):
        lines.append(f"<figcaption>{html.escape(entry['caption'], quote=False)}</figcaption>")
    lines += ["</figure>", "<!-- /figure -->"]
    return "\n".join(lines) + "\n"


def insertion_point(lines, after):
    """Line index at which to insert: after the heading's first paragraph."""
    if after == "intro":
        start = next((i for i, line in enumerate(lines) if line.startswith("# ")), -1)
        if start >= 0:  # skip the lead paragraph element too
            j = start + 1
            while j < len(lines) and not lines[j].strip():
                j += 1
            if j < len(lines) and lines[j].startswith('<p class="lead"'):
                while j < len(lines) and "</p>" not in lines[j]:
                    j += 1
                return j + 1
    else:
        start = next((i for i, line in enumerate(lines) if line.strip() == after), -1)
    if start < 0:
        raise KeyError(after)
    i = start + 1
    while i < len(lines) and not lines[i].strip():
        i += 1
    if i >= len(lines) or BLOCK_START.match(lines[i]) or lines[i].startswith("#"):
        return start + 1
    while i < len(lines) and lines[i].strip():
        i += 1
    return i


def skip_placed(lines, at):
    """Move past figures already placed at this point, so co-located figures keep manifest order."""
    while True:
        j = at
        while j < len(lines) and not lines[j].strip():
            j += 1
        if j < len(lines) and lines[j].startswith("<!-- figure:"):
            while j < len(lines) and not lines[j].startswith("<!-- /figure -->"):
                j += 1
            at = j + 1
        else:
            return at


def have_pair(scene):
    return all(os.path.exists(os.path.join(SCREENS, f"{scene}-{theme}.webp")) for theme in ("light", "dark"))


def load():
    entries = []
    for path in sorted(glob.glob(MANIFESTS)):
        with open(path, encoding="utf-8") as handle:
            for entry in json.load(handle):
                entry["_manifest"] = os.path.basename(path)
                entries.append(entry)
    return entries


def main():
    check = "--check" in sys.argv
    entries = load()
    by_page = {}
    problems, pending, placed = [], [], 0
    for entry in entries:
        page = entry["page"]
        if not os.path.exists(os.path.join(DOCS, page)):
            problems.append(f"{entry['_manifest']}: {entry['scene']}: unknown page {page}")
            continue
        by_page.setdefault(page, []).append(entry)
    for page, items in sorted(by_page.items()):
        path = os.path.join(DOCS, page)
        with open(path, encoding="utf-8") as handle:
            text = handle.read()
        text = MARKER.sub("", text)  # re-place every figure so order follows the manifests
        lines = text.split("\n")
        for entry in items:
            scene = entry["scene"]
            if entry.get("image") or entry.get("images"):
                names = [entry["image"]] if entry.get("image") else list(entry["images"].values())
                missing_image = [n for n in names if not os.path.exists(os.path.join(SCREENS, n))]
                if missing_image:
                    problems.append(f"{entry['_manifest']}: {scene}: image not found: {missing_image[0]}")
                    continue
                if entry.get("fallback_for") and have_pair(entry["fallback_for"]):
                    continue  # the captured light/dark pair replaces this stand-in
            elif not have_pair(scene):
                pending.append(scene)
                continue
            try:
                at = skip_placed(lines, insertion_point(lines, entry["after"]))
            except KeyError:
                problems.append(f"{entry['_manifest']}: {scene}: heading not found in {page}: {entry['after']}")
                continue
            block = figure(entry, page).rstrip("\n").split("\n")
            lines[at:at] = [""] + block + [""]
            placed += 1
        new = re.sub(r"\n{3,}", "\n\n", "\n".join(lines))
        if not check and new != text:
            with open(path, "w", encoding="utf-8") as handle:
                handle.write(new)
    print(f"{len(entries)} requests, {placed} placed, {len(set(pending))} waiting for images, {len(problems)} problems")
    for problem in problems:
        print("PROBLEM", problem)
    if pending and "--list-pending" in sys.argv:
        print("pending:", ", ".join(sorted(set(pending))))
    return 1 if problems else 0


if __name__ == "__main__":
    sys.exit(main())
