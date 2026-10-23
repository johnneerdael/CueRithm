"""Keep old user-guide URLs working after pages move or split.

GitHub Pages cannot send HTTP redirects, so every old page gets a small static
forwarding page that keeps the query string and the #anchor.

`redirects.json` maps an old page to the page that replaced it. A value is
either a page path, or an object for a page that was split into several:

    "guides/settings.md": {
      "to": "reference/settings/index.md",
      "anchors": {"incoming": "reference/settings/incoming.md",
                  "cue-generation": "reference/settings/cue-generation.md#rules"}
    }

`anchors` sends a known old #anchor to the page (and optional #anchor) that now
holds that section. An anchor that is not listed stays on `to`, so a link to a
heading that kept its id on the new page keeps working.

The build fails when a target is not a published page, when an old path is
still a real page (a redirect must never hide content) or when a target
#anchor does not exist in the built page. Every page the guide has ever
published is listed in `_meta/legacy-pages.txt`; `tools/docs/check_guide.py`
fails if one has no redirect and no page of its own.
"""

import html
import json
import os
import posixpath
import re

from mkdocs.exceptions import PluginError
from mkdocs.structure.files import File

DEFAULT_MAP = "docs/hooks/redirects.json"
_moves = {}


def _map_path(config):
    base = os.path.dirname(os.path.abspath(config.config_file_path))
    return os.path.join(base, config.get("extra", {}).get("redirects_file", DEFAULT_MAP))


def _normalise(value):
    if isinstance(value, str):
        return {"to": value, "anchors": {}}
    return {"to": value["to"], "anchors": dict(value.get("anchors", {}))}


def _split(target):
    page, _, fragment = target.partition("#")
    return page, fragment


def on_files(files, config):
    with open(_map_path(config), encoding="utf-8") as handle:
        raw = json.load(handle)
    pages = {file.src_uri for file in files.documentation_pages()}
    moves = {old: _normalise(value) for old, value in raw.items()}
    for old, move in moves.items():
        if old in pages:
            raise PluginError(f"redirect {old}: a page still exists at the old path")
        for target in [move["to"], *move["anchors"].values()]:
            page, _ = _split(target)
            if page not in pages:
                raise PluginError(f"redirect {old} -> {target}: the target is not a published page")
    _moves.clear()
    _moves.update(moves)
    return files


def _url_from(old_file, new_file, use_dirs):
    start = posixpath.dirname(old_file.dest_uri) or "."
    if use_dirs and new_file.dest_uri.endswith("/index.html"):
        return posixpath.relpath(posixpath.dirname(new_file.dest_uri) or ".", start) + "/"
    return posixpath.relpath(new_file.dest_uri, start)


def _page(default, anchors, canonical):
    escaped = html.escape(default, quote=True)
    link = f'<link rel="canonical" href="{html.escape(canonical, quote=True)}">\n' if canonical else ""
    script = (
        "<script>\n"
        f"var base = {json.dumps(default)}, map = {json.dumps(anchors)};\n"
        "var id = decodeURIComponent(location.hash.slice(1));\n"
        "var target = Object.prototype.hasOwnProperty.call(map, id) ? map[id] : base;\n"
        "var cut = target.indexOf('#');\n"
        "var path = cut < 0 ? target : target.slice(0, cut);\n"
        "var hash = cut < 0 ? '' : target.slice(cut);\n"
        "if (!Object.prototype.hasOwnProperty.call(map, id) && location.hash) { hash = location.hash; }\n"
        "location.replace(path + location.search + hash);\n"
        "</script>\n"
    )
    return (
        "<!doctype html>\n<html lang=\"en\">\n<head>\n<meta charset=\"utf-8\">\n"
        "<title>This page has moved</title>\n"
        f"{link}<meta name=\"robots\" content=\"noindex\">\n"
        f"<meta http-equiv=\"refresh\" content=\"0; url={escaped}\">\n"
        f"{script}"
        "</head>\n<body>\n"
        f"<p>This page has moved to <a href=\"{escaped}\">{escaped}</a>.</p>\n"
        "</body>\n</html>\n"
    )


def _has_anchor(site_dir, new_file, fragment):
    path = os.path.join(site_dir, new_file.dest_uri)
    with open(path, encoding="utf-8") as handle:
        text = handle.read()
    return re.search(r'\sid="' + re.escape(fragment) + r'"', text) is not None


def on_post_build(config):
    site_url = (config.get("site_url") or "").rstrip("/")
    use_dirs = config["use_directory_urls"]
    site_dir = config["site_dir"]

    def file_for(src):
        return File(src, config["docs_dir"], site_dir, use_dirs)

    for old, move in _moves.items():
        old_file = file_for(old)
        destination = os.path.join(site_dir, old_file.dest_uri)
        if os.path.exists(destination):
            raise PluginError(f"redirect {old}: the built site already has {old_file.dest_uri}")

        default_page, default_fragment = _split(move["to"])
        default_file = file_for(default_page)
        if default_fragment and not _has_anchor(site_dir, default_file, default_fragment):
            raise PluginError(f"redirect {old} -> {move['to']}: no element with that id")
        default = _url_from(old_file, default_file, use_dirs) + (f"#{default_fragment}" if default_fragment else "")

        anchors = {}
        for old_id, target in move["anchors"].items():
            page, fragment = _split(target)
            new_file = file_for(page)
            if fragment and not _has_anchor(site_dir, new_file, fragment):
                raise PluginError(f"redirect {old}#{old_id} -> {target}: no element with that id")
            anchors[old_id] = _url_from(old_file, new_file, use_dirs) + (f"#{fragment}" if fragment else "")

        canonical = f"{site_url}/{default_file.url}" if site_url else ""
        os.makedirs(os.path.dirname(destination), exist_ok=True)
        with open(destination, "w", encoding="utf-8") as handle:
            handle.write(_page(default, anchors, canonical))
