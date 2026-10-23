#!/usr/bin/env python3
"""Write the `nav:` of mkdocs.yml from docs/user-guide/_meta/nav.yml.

The skeleton lists tabs in order. An item is a page path (the nav label is that page's H1),
a glob such as `files/recipes/*.md`, or a one-key mapping {Group label: [items]}. The first
page of a group is its landing page and gets no label. Pages named in `labels:` use that
label instead of their H1 (for long titles).

Usage: python3 tools/docs/build_nav.py [--check]
"""
import glob
import os
import re
import sys

import yaml

ROOT = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
DOCS = os.path.join(ROOT, "docs", "user-guide")


MISSING = []


def h1(page):
    with open(os.path.join(DOCS, page), encoding="utf-8") as handle:
        for line in handle:
            if line.startswith("# "):
                return re.sub(r"\s*\{.*\}\s*$", "", line[2:].strip())
    raise SystemExit(f"{page}: no H1")


def expand(items):
    out = []
    for item in items:
        if isinstance(item, str) and not any(c in item for c in "*?[") and not os.path.exists(os.path.join(DOCS, item)):
            MISSING.append(item)
            continue
        if isinstance(item, str) and any(c in item for c in "*?["):
            for path in sorted(glob.glob(os.path.join(DOCS, item))):
                rel = os.path.relpath(path, DOCS).replace(os.sep, "/")
                if not rel.endswith("/index.md"):
                    out.append(rel)
        else:
            out.append(item)
    return out


def emit(items, indent, labels, lines, first_is_index):
    for position, item in enumerate(expand(items)):
        pad = " " * indent
        if isinstance(item, str):
            if position == 0 and item.endswith("index.md"):
                lines.append(f"{pad}- {item}")
            else:
                label = labels.get(item) or h1(item)
                label = label.replace('"', '\\"')
                lines.append(f'{pad}- "{label}": {item}')
        else:
            (label, children), = item.items()
            lines.append(f'{pad}- "{label}":')
            emit(children, indent + 4, labels, lines, True)


def main():
    with open(os.path.join(DOCS, "_meta", "nav.yml"), encoding="utf-8") as handle:
        skeleton = yaml.safe_load(handle)
    labels = skeleton.get("labels", {})
    lines = ["nav:"]
    for tab in skeleton["tabs"]:
        if isinstance(tab, str):  # a single page, for example Home
            lines.append(f'  - "{labels.get(tab) or h1(tab)}": {tab}' if tab != "index.md" else f"  - Home: {tab}")
        else:
            (label, children), = tab.items()
            lines.append(f'  - "{label}":')
            emit(children, 6, labels, lines, True)
    nav = "\n".join(lines) + "\n"
    if MISSING:
        print("skipped, page does not exist:", ", ".join(MISSING))
        if "--strict" in sys.argv:
            return 1
    path = os.path.join(ROOT, "mkdocs.yml")
    with open(path, encoding="utf-8") as handle:
        text = handle.read()
    head = text[: text.index("\nnav:\n")] + "\n"
    if "--check" in sys.argv:
        print("up to date" if text == head + nav else "mkdocs.yml nav differs from _meta/nav.yml")
        return 0 if text == head + nav else 1
    with open(path, "w", encoding="utf-8") as handle:
        handle.write(head + nav)
    print(f"wrote nav: {len(lines)} lines")
    return 0


if __name__ == "__main__":
    sys.exit(main())
