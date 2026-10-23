#!/usr/bin/env python3
"""Move user-guide pages and keep every reference to them valid.

Usage: python3 tools/docs/move_pages.py MAP.json [--apply]

MAP.json maps old page paths to new page paths, both relative to docs/user-guide/
({"prepare/add-tracks.md": "files/add-tracks.md"}). Without --apply it only reports.

What it updates, in one pass:
  - the pages themselves (moved to the new path);
  - every relative link, image and href inside every page, resolved against the old
    location of the source and of the target and rewritten for the new ones (anchors kept);
  - the mkdocs.yml navigation entries;
  - the `page` of every figure request in docs/user-guide/_meta/figures/*.json;
  - the targets (and anchor targets) in docs/hooks/redirects.json;
  - repository paths and published URLs that point at a moved page, in the README and
    in docs/*.md outside the guide.
"""
import glob
import json
import os
import re
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
DOCS = os.path.join(ROOT, "docs", "user-guide")
SITE = "https://johnneerdael.github.io/CueRithm/"
SKIP = ("_meta/", "_snippets/", "assets/", "stylesheets/")

LINK = re.compile(r"(\]\(|href=\"|src=\")(?!https?:|mailto:|#|/)([^)\"\s#]+)(#[^)\"\s]*)?")


def page_url(page):
    if page == "index.md":
        return SITE
    if page.endswith("/index.md"):
        return SITE + page[: -len("index.md")]
    return SITE + page[:-3] + "/"


def write_moves(docs, outputs):
    """Write moved pages. All moved sources are removed before any target is written, so a
    chain (a -> b, b -> c) or a swap never deletes a page that was just written."""
    for old, (new, _) in outputs.items():
        if new != old:
            os.remove(os.path.join(docs, old))
    for _, (new, text) in outputs.items():
        target = os.path.join(docs, new)
        os.makedirs(os.path.dirname(target), exist_ok=True)
        with open(target, "w", encoding="utf-8") as handle:
            handle.write(text)


def fix_stale(mapping):
    """Rewrite links that point at a moved page's OLD path, for pages written after the move."""
    changed = 0
    for folder, _, files in os.walk(DOCS):
        for name in files:
            rel = os.path.relpath(os.path.join(folder, name), DOCS).replace(os.sep, "/")
            if not rel.endswith(".md") or rel.startswith(SKIP):
                continue
            with open(os.path.join(DOCS, rel), encoding="utf-8") as handle:
                text = handle.read()
            base = os.path.dirname(rel)

            def fix(match):
                lead, target, frag = match.group(1), match.group(2), match.group(3) or ""
                absolute = os.path.normpath(os.path.join(base, target)).replace(os.sep, "/")
                if os.path.exists(os.path.join(DOCS, absolute)) or absolute not in mapping:
                    return match.group(0)
                return f"{lead}{os.path.relpath(mapping[absolute], base or '.').replace(os.sep, '/')}{frag}"

            new = LINK.sub(fix, text)
            if new != text:
                changed += 1
                with open(os.path.join(DOCS, rel), "w", encoding="utf-8") as handle:
                    handle.write(new)
    print(f"stale links fixed in {changed} pages")


def main():
    if "--fix-stale" in sys.argv:
        with open(sys.argv[1], encoding="utf-8") as handle:
            fix_stale(json.load(handle))
        return 0
    apply = "--apply" in sys.argv
    with open(sys.argv[1], encoding="utf-8") as handle:
        mapping = json.load(handle)
    pages = []
    for folder, _, files in os.walk(DOCS):
        for name in files:
            rel = os.path.relpath(os.path.join(folder, name), DOCS).replace(os.sep, "/")
            if rel.endswith(".md") and not rel.startswith(SKIP):
                pages.append(rel)
    missing = [old for old in mapping if old not in pages]
    if missing:
        sys.exit("not found: " + ", ".join(missing))
    if len(set(mapping.values())) != len(mapping):
        sys.exit("two pages are mapped to the same target")
    clashes = [new for new in mapping.values() if new in pages and new not in mapping]
    if clashes:
        sys.exit("target already exists: " + ", ".join(clashes))

    def new_of(path):
        return mapping.get(path, path)

    rewritten_links = 0
    outputs = {}
    for old in pages:
        new = new_of(old)
        with open(os.path.join(DOCS, old), encoding="utf-8") as handle:
            text = handle.read()
        base_old = os.path.dirname(old)
        base_new = os.path.dirname(new)

        def fix(match):
            nonlocal rewritten_links
            lead, target, frag = match.group(1), match.group(2), match.group(3) or ""
            absolute_old = os.path.normpath(os.path.join(base_old, target)).replace(os.sep, "/")
            if absolute_old.startswith(".."):
                return match.group(0)
            exists = os.path.exists(os.path.join(DOCS, absolute_old)) or absolute_old in mapping
            if not exists:
                return match.group(0)
            relative = os.path.relpath(new_of(absolute_old), base_new or ".").replace(os.sep, "/")
            if relative == target:
                return match.group(0)
            rewritten_links += 1
            return f"{lead}{relative}{frag}"

        outputs[old] = (new, LINK.sub(fix, text))

    print(f"{len(mapping)} pages to move, {rewritten_links} links to rewrite")

    # mkdocs.yml, figures, redirects, external files
    with open(os.path.join(ROOT, "mkdocs.yml"), encoding="utf-8") as handle:
        mkdocs = handle.read()
    mkdocs_new = re.sub(r"[\w./-]+\.md", lambda m: new_of(m.group(0)), mkdocs)

    figures = {}
    for path in glob.glob(os.path.join(DOCS, "_meta", "figures", "*.json")):
        with open(path, encoding="utf-8") as handle:
            entries = json.load(handle)
        for entry in entries:
            entry["page"] = new_of(entry["page"])
        figures[path] = entries

    redirects_path = os.path.join(ROOT, "docs", "hooks", "redirects.json")
    with open(redirects_path, encoding="utf-8") as handle:
        redirects = json.load(handle)

    def fix_target(target):
        page, hash_, fragment = target.partition("#")
        return new_of(page) + (hash_ + fragment if hash_ else "")

    for old, value in list(redirects.items()):
        if isinstance(value, str):
            redirects[old] = fix_target(value)
        else:
            value["to"] = fix_target(value["to"])
            value["anchors"] = {k: fix_target(v) for k, v in value.get("anchors", {}).items()}

    external = {}
    path_rx = re.compile(r"((?:\.\./)*(?:docs/)?user-guide/)([\w./-]+?\.md)(#[\w-]+)?")
    url_pairs = [(page_url(o), page_url(n)) for o, n in mapping.items()]
    for path in [os.path.join(ROOT, "README.md")] + glob.glob(os.path.join(ROOT, "docs", "**", "*.md"), recursive=True):
        rel = os.path.relpath(path, ROOT).replace(os.sep, "/")
        if rel.startswith(("docs/user-guide/", "docs/research/")):
            continue
        with open(path, encoding="utf-8") as handle:
            text = handle.read()
        new_text = path_rx.sub(lambda m: m.group(1) + new_of(m.group(2)) + (m.group(3) or ""), text)
        for old_url, new_url in url_pairs:
            new_text = re.sub(re.escape(old_url) + r"(?=[)\s\"'<>#\]]|$)", new_url, new_text)
        if new_text != text:
            external[path] = new_text

    print(f"{len(figures)} figure files, {len(external)} external files")
    if not apply:
        for path in sorted(external):
            print("  would update", os.path.relpath(path, ROOT))
        return 0

    write_moves(DOCS, outputs)
    for folder, dirs, files in os.walk(DOCS, topdown=False):
        if not files and not dirs and folder != DOCS:
            os.rmdir(folder)
    with open(os.path.join(ROOT, "mkdocs.yml"), "w", encoding="utf-8") as handle:
        handle.write(mkdocs_new)
    for path, entries in figures.items():
        with open(path, "w", encoding="utf-8") as handle:
            json.dump(entries, handle, indent=1, ensure_ascii=False)
    with open(redirects_path, "w", encoding="utf-8") as handle:
        json.dump(dict(sorted(redirects.items())), handle, indent=1)
    for path, text in external.items():
        with open(path, "w", encoding="utf-8") as handle:
            handle.write(text)
    print("done")
    return 0


if __name__ == "__main__":
    sys.exit(main())
