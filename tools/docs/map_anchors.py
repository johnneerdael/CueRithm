#!/usr/bin/env python3
"""Map the legacy guide's #anchors onto the pages that now hold those sections.

For every legacy page with a redirect, each legacy anchor that is not already handled
(present on the default target page, or listed in `anchors`) is matched against the explicit
heading ids of the new pages. A unique match, or a match on a page the legacy page already
redirects to, is added to `docs/hooks/redirects.json`. The rest is printed for manual mapping.

Usage: python3 tools/docs/map_anchors.py [--write]
"""
import json
import os
import sys

import markdown

ROOT = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
DOCS = os.path.join(ROOT, "docs", "user-guide")
SKIP = ("_meta/", "_snippets/", "assets/", "stylesheets/")


def ids_of(path):
    md = markdown.Markdown(extensions=["toc", "attr_list", "admonition", "tables", "pymdownx.superfences", "pymdownx.tabbed", "def_list", "md_in_html"])
    with open(path, encoding="utf-8") as handle:
        md.convert(handle.read())
    out = []

    def walk(tokens):
        for token in tokens:
            out.append(token["id"])
            walk(token["children"])

    walk(md.toc_tokens)
    return set(out)


def main():
    write = "--write" in sys.argv
    with open(os.path.join(ROOT, "docs", "hooks", "redirects.json"), encoding="utf-8") as handle:
        redirects = json.load(handle)
    with open(os.path.join(DOCS, "_meta", "legacy-anchors.json"), encoding="utf-8") as handle:
        legacy = json.load(handle)
    by_page = {}
    for folder, _, files in os.walk(DOCS):
        for name in files:
            rel = os.path.relpath(os.path.join(folder, name), DOCS).replace(os.sep, "/")
            if rel.endswith(".md") and not rel.startswith(SKIP):
                by_page[rel] = ids_of(os.path.join(DOCS, rel))
    holders = {}
    for page, ids in by_page.items():
        for anchor in ids:
            holders.setdefault(anchor, []).append(page)

    added, unresolved = 0, []
    for old, value in sorted(redirects.items()):
        move = {"to": value, "anchors": {}} if isinstance(value, str) else value
        default = move["to"].partition("#")[0]
        related = {default} | {t.partition("#")[0] for t in move.get("anchors", {}).values()}
        for anchor in legacy.get(old, [])[1:]:
            if anchor in move.get("anchors", {}):
                continue
            if default in by_page and anchor in by_page[default]:
                continue
            candidates = holders.get(anchor, [])
            near = [p for p in candidates if p in related]
            choice = near[0] if len(near) == 1 else (candidates[0] if len(candidates) == 1 else None)
            if choice:
                move.setdefault("anchors", {})[anchor] = f"{choice}#{anchor}"
                added += 1
            else:
                unresolved.append((old, anchor, default))
        redirects[old] = move if move.get("anchors") else value
    print(f"{added} anchors matched, {len(unresolved)} left on the top of their new page")
    for old, anchor, default in unresolved:
        print(f"  {old}#{anchor} -> {default} (top)")
    if write:
        with open(os.path.join(ROOT, "docs", "hooks", "redirects.json"), "w", encoding="utf-8") as handle:
            json.dump(dict(sorted(redirects.items())), handle, indent=1)
    return 0


if __name__ == "__main__":
    sys.exit(main())
