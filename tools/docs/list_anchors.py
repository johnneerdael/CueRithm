#!/usr/bin/env python3
"""List the heading ids of the old (legacy) guide pages, as published.

Used once to build the redirect map; the output is `_meta/legacy-anchors.json`.
"""
import json
import os
import sys

import markdown

ROOT = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
LEGACY = os.path.join(ROOT, "docs", "user-guide", "_meta", "legacy")


def ids(path):
    md = markdown.Markdown(extensions=["toc", "attr_list", "admonition", "tables", "pymdownx.superfences", "pymdownx.tabbed", "def_list", "md_in_html"],
                           extension_configs={"toc": {"permalink": False}})
    with open(path, encoding="utf-8") as handle:
        md.convert(handle.read())
    out = []

    def walk(tokens):
        for token in tokens:
            out.append(token["id"])
            walk(token["children"])

    walk(md.toc_tokens)
    return out


def main():
    result = {}
    for folder, _, files in os.walk(LEGACY):
        for name in sorted(files):
            if name.endswith(".md"):
                rel = os.path.relpath(os.path.join(folder, name), LEGACY).replace(os.sep, "/")
                result[rel] = ids(os.path.join(folder, name))
    json.dump(dict(sorted(result.items())), sys.stdout, indent=1, ensure_ascii=False)


if __name__ == "__main__":
    main()
