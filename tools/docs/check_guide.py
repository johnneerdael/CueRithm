#!/usr/bin/env python3
"""Check the user guide's plan against its files.

Reports, and exits non-zero on the first group that is an error:
  - nav entries with no file, and published pages that are not in the nav
  - legacy pages and legacy anchors with neither a page nor a redirect
  - redirect targets that do not exist
  - figure requests for unknown pages or headings; scenes still waiting for images

Usage: python3 tools/docs/check_guide.py [--quiet]
Needs PyYAML and Markdown (installed with the docs requirements).
"""
import glob
import json
import os
import re
import sys

import markdown
import yaml

ROOT = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
DOCS = os.path.join(ROOT, "docs", "user-guide")
EXCLUDED = ("_meta/", "_snippets/", "assets/", "stylesheets/")


class Loader(yaml.SafeLoader):
    pass


Loader.add_multi_constructor("tag:yaml.org,2002:python/", lambda loader, suffix, node: None)
Loader.add_multi_constructor("!", lambda loader, suffix, node: None)


def nav_pages():
    with open(os.path.join(ROOT, "mkdocs.yml"), encoding="utf-8") as handle:
        config = yaml.load(handle, Loader=Loader)
    pages = []

    def walk(node):
        if isinstance(node, str):
            pages.append(node)
        elif isinstance(node, list):
            for item in node:
                walk(item)
        elif isinstance(node, dict):
            for value in node.values():
                walk(value)

    walk(config["nav"])
    return [page for page in pages if page.endswith(".md")]


def heading_ids(path):
    md = markdown.Markdown(extensions=["toc", "attr_list", "admonition", "tables", "pymdownx.superfences", "pymdownx.tabbed", "def_list", "md_in_html"])
    with open(path, encoding="utf-8") as handle:
        source = handle.read()
    md.convert(source)
    ids = []

    def walk(tokens):
        for token in tokens:
            ids.append(token["id"])
            walk(token["children"])

    walk(md.toc_tokens)
    ids += re.findall(r'<(?:span|a)[^>]*\sid="([\w-]+)"', source)  # explicit compatibility anchors
    return set(ids)


def main():
    errors = []
    nav = nav_pages()
    for page in nav:
        if not os.path.exists(os.path.join(DOCS, page)):
            errors.append(f"nav: missing file {page}")
    published = set()
    for folder, _, files in os.walk(DOCS):
        for name in files:
            rel = os.path.relpath(os.path.join(folder, name), DOCS).replace(os.sep, "/")
            if rel.endswith(".md") and not rel.startswith(EXCLUDED):
                published.add(rel)
    for rel in sorted(published - set(nav)):
        errors.append(f"nav: {rel} exists but is not in the nav")

    with open(os.path.join(ROOT, "docs", "hooks", "redirects.json"), encoding="utf-8") as handle:
        redirects = json.load(handle)
    with open(os.path.join(DOCS, "_meta", "legacy-pages.txt"), encoding="utf-8") as handle:
        legacy = [line.strip() for line in handle if line.strip()]
    with open(os.path.join(DOCS, "_meta", "legacy-anchors.json"), encoding="utf-8") as handle:
        legacy_anchors = json.load(handle)
    cache = {}

    def ids_of(page):
        if page not in cache:
            cache[page] = heading_ids(os.path.join(DOCS, page))
        return cache[page]

    for old in legacy:
        anchors = legacy_anchors.get(old, [])[1:]  # [0] is the page title
        if old in redirects:
            value = redirects[old]
            to = value if isinstance(value, str) else value["to"]
            mapped = {} if isinstance(value, str) else value.get("anchors", {})
            for target in [to, *mapped.values()]:
                page, _, fragment = target.partition("#")
                if not os.path.exists(os.path.join(DOCS, page)):
                    errors.append(f"redirect {old} -> {target}: page does not exist")
                elif fragment and fragment not in ids_of(page):
                    errors.append(f"redirect {old} -> {target}: anchor does not exist")
            default_page = to.partition("#")[0]
            for anchor in anchors:
                on_default = os.path.exists(os.path.join(DOCS, default_page)) and anchor in ids_of(default_page)
                if anchor not in mapped and not on_default:
                    errors.append(f"legacy anchor {old}#{anchor} has no mapping and is not on {default_page}")
        elif old in published:
            for anchor in anchors:
                if anchor not in ids_of(old):
                    errors.append(f"legacy anchor {old}#{anchor} no longer exists on the page that kept its path")
        else:
            errors.append(f"legacy page {old} has neither a page nor a redirect")

    for path in sorted(glob.glob(os.path.join(DOCS, "_meta", "figures", "*.json"))):
        with open(path, encoding="utf-8") as handle:
            for entry in json.load(handle):
                page = os.path.join(DOCS, entry["page"])
                if not os.path.exists(page):
                    errors.append(f"figures {os.path.basename(path)}: {entry['scene']}: unknown page {entry['page']}")
                elif entry.get("image") or entry.get("images"):
                    names = [entry["image"]] if entry.get("image") else list(entry["images"].values())
                    missing = [n for n in names if not os.path.exists(os.path.join(DOCS, "assets", "screens", n))]
                    if missing:
                        errors.append(f"figures {os.path.basename(path)}: {entry['scene']}: image not found: {missing[0]}")
                    elif entry["after"] != "intro":
                        with open(page, encoding="utf-8") as text:
                            if entry["after"] not in (line.strip() for line in text):
                                errors.append(f"figures {os.path.basename(path)}: {entry['scene']}: heading not found: {entry['after']}")
                elif entry["after"] != "intro":
                    with open(page, encoding="utf-8") as text:
                        if entry["after"] not in (line.strip() for line in text):
                            errors.append(f"figures {os.path.basename(path)}: {entry['scene']}: heading not found: {entry['after']}")

    for error in errors:
        print("ERROR", error)
    print(f"{len(nav)} pages in nav, {len(published)} published pages, {len(legacy)} legacy pages, {len(errors)} errors")
    return 1 if errors else 0


if __name__ == "__main__":
    sys.exit(main())
