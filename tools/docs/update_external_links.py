#!/usr/bin/env python3
"""Rewrite links that point into the user guide from outside it, using docs/hooks/redirects.json.

Handles repository paths (`user-guide/learn/auto-tagging.md#provider-setup`, with any
number of leading `../` or `docs/`) and published URLs
(`https://johnneerdael.github.io/CueRithm/learn/`). A split page's #anchor goes to the
page that now holds it; an anchor that cannot be checked here is dropped, and the
forwarding page for the old URL still resolves it.

Usage: python3 tools/docs/update_external_links.py [--write]   (default: dry run)
"""
import json
import os
import re
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
SITE = "https://johnneerdael.github.io/CueRithm/"
with open(os.path.join(ROOT, "docs", "hooks", "redirects.json"), encoding="utf-8") as handle:
    REDIRECTS = json.load(handle)
with open(os.path.join(ROOT, "docs", "user-guide", "_meta", "legacy-pages.txt"), encoding="utf-8") as handle:
    LEGACY = {line.strip() for line in handle if line.strip()}


def new_target(legacy, anchor):
    """(new page, new anchor or '') for a legacy page and optional #anchor, or None if it still exists."""
    move = REDIRECTS.get(legacy)
    if move is None:
        return None
    if isinstance(move, str):
        return move, ""
    if anchor and anchor in move.get("anchors", {}):
        page, _, fragment = move["anchors"][anchor].partition("#")
        return page, fragment
    return move["to"].partition("#")[0], ""


def url_for(page):
    if page == "index.md":
        return SITE
    if page.endswith("/index.md"):
        return SITE + page[: -len("index.md")]
    return SITE + page[:-3] + "/"


PATH = re.compile(r"((?:\.\./)*(?:docs/)?user-guide/)([\w./-]+?\.md)(#[\w-]+)?")
URL = re.compile(re.escape(SITE) + r"([\w./-]*?)(#[\w-]+)?(?=[)\s\"'<>\]]|$)")


def rewrite_path(match):
    prefix, legacy, frag = match.group(1), match.group(2), (match.group(3) or "")[1:]
    target = new_target(legacy, frag)
    if target is None:
        return match.group(0)
    page, fragment = target
    return f"{prefix}{page}" + (f"#{fragment}" if fragment else "")


def rewrite_url(match):
    path, frag = match.group(1), (match.group(2) or "")[1:]
    if path == "":
        return match.group(0)
    legacy = path[:-1] + "/index.md" if path.endswith("/") else path + ".md"
    if path.endswith("/") and path[:-1] + ".md" in LEGACY:
        legacy = path[:-1] + ".md"
    target = new_target(legacy, frag)
    if target is None:
        return match.group(0)
    page, fragment = target
    return url_for(page) + (f"#{fragment}" if fragment else "")


def main():
    write = "--write" in sys.argv
    changed = 0
    for folder, dirs, files in os.walk(ROOT):
        dirs[:] = [d for d in dirs if d not in {".git", "node_modules", "target", ".worktrees", "build", "vendor", ".analysis"}]
        rel_folder = os.path.relpath(folder, ROOT).replace(os.sep, "/")
        if rel_folder.startswith(("docs/user-guide", "docs/research", "tools/docs")):
            continue
        for name in files:
            if not name.endswith((".md", ".yml", ".html", ".py", ".json")):
                continue
            path = os.path.join(folder, name)
            try:
                with open(path, encoding="utf-8") as handle:
                    text = handle.read()
            except UnicodeDecodeError:
                continue
            new = URL.sub(rewrite_url, PATH.sub(rewrite_path, text))
            if new != text:
                changed += 1
                print(("rewrote " if write else "would rewrite ") + os.path.relpath(path, ROOT))
                if write:
                    with open(path, "w", encoding="utf-8") as handle:
                        handle.write(new)
    print(f"{changed} files")


if __name__ == "__main__":
    main()
