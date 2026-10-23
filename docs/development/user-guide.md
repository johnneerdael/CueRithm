# Build and maintain the user guide

The published guide is <https://johnneerdael.github.io/CueRithm/>. Its source is
`docs/user-guide/`; `mkdocs.yml` defines the theme and the public navigation, and
`.github/workflows/pages.yml` builds it with `--strict` and publishes it when
`docs/user-guide/**`, `docs/hooks/**`, `mkdocs.yml` or the workflow change on `main`.

## How the guide is organised

The guide is organised by what a DJ wants to do, not by document type. Each tab holds
one job's tutorial, task pages, recipes and reference side by side, and every tab starts
with a landing page of *Your goal → Page* tables. Page titles are goals in the reader's
words ("Find and merge duplicate tracks"), never bare feature names.

The plan, the demo data and the writing rules live beside the pages and are not
published (`_meta/` is excluded):

| File | What it is |
|---|---|
| `docs/user-guide/_meta/ia.md` | Tabs, the page plan, owners, the demo library, redirects, status |
| `docs/user-guide/_meta/style.md` | Voice, headings, page skeletons, links, screenshots, checklist |
| `docs/user-guide/_meta/library-facts.md` | The real tracks, keys, BPMs and cues every example uses |
| `docs/user-guide/_meta/figures/` | Screenshot requests, one JSON file per tab |
| `docs/user-guide/_meta/legacy-pages.txt`, `legacy-anchors.json` | Every page and heading id the previous guide published, used to prove each old URL has a redirect |
| `docs/user-guide/_meta/nav.yml` | The navigation skeleton; `tools/docs/build_nav.py` writes `mkdocs.yml`'s `nav:` from it with each page's H1 as its label |

## Build and check

```sh
python3 -m venv /tmp/cuerithm-docs && /tmp/cuerithm-docs/bin/pip install -r docs/site-requirements.txt
/tmp/cuerithm-docs/bin/python -m mkdocs build --strict      # what CI runs; must pass
/tmp/cuerithm-docs/bin/python tools/docs/check_guide.py     # plan versus files
/tmp/cuerithm-docs/bin/python -m mkdocs serve               # preview at http://127.0.0.1:8000
```

`check_guide.py` reports navigation entries without a file, pages that are not in the
navigation, legacy URLs and anchors without a redirect, redirect targets that do not
exist, and figure requests that point at a missing page or heading. The strict build
fails on a broken link or anchor and on a redirect whose target page or `#anchor` does
not exist.

Check the result in both colour schemes, at a narrow width, with search, and with the
copy button on code blocks.

## Move, split or rename a page

Every URL the guide has published must keep working, because the README, release notes
and other sites link to them. GitHub Pages cannot send HTTP redirects, so
`docs/hooks/redirects.py` writes a small forwarding page at each old URL that keeps the
query string and the `#anchor`.

1. Move the page. Never leave a page at an old path that is mapped elsewhere.
2. Add or update the entry in `docs/hooks/redirects.json`: a string for a page that moved
   whole, or `{"to": "…", "anchors": {"old-id": "new/page.md#new-id"}}` for a page that was
   split, so each old section lands on the page that now holds it.
3. If the page used to exist, add it to `docs/user-guide/_meta/legacy-pages.txt`
   (already done for every page published before the restructure).
4. Run `tools/docs/check_guide.py` and the strict build.
5. Update links from outside the guide with
   `python3 tools/docs/update_external_links.py` (a dry run; add `--write` to apply it).
   It rewrites repository paths and published URLs in the README and `docs/*.md` using
   the same map.

## Screenshots

Screenshots are requested, captured and placed in three separate steps so that no page
links to an image that does not exist yet:

1. **Request.** A writer adds an entry to `docs/user-guide/_meta/figures/<tab>.json`
   (scene id, page, the heading it follows, alt text, caption and how to reproduce the
   state). See `docs/user-guide/_meta/figures/README.md`.
2. **Capture.** `tools/docs/capture/` drives the installed CueRithm window on the real
   demo library at 1440×940 with `open-computer-use` and `screencapture`, in dark and
   light, as raw PNGs with the window's own shape. `to_webp.py` converts them to
   2000 px wide WebP files named `assets/screens/<scene>-light.webp` and `-dark.webp`.
   Open every image before keeping it: reject an error banner, a hover highlight, a
   credential, or anything outside the app window. Real library data is the intent; check
   each image for file paths that include a user name.
3. **Place.** `python3 tools/docs/embed_figures.py` inserts the `<figure>` markup after
   the named heading, only when both theme images exist, and updates a placed figure in
   place on re-run. Never write figure markup by hand.

Driving the app while it is being captured clicks real controls on the real library:
scans and previews are read-only, so capture those freely, and never script an Apply,
Delete or Archive step without a backup of the app data folder.

## Related

- [Desktop development and acceptance](desktop.md)
- [Pull request and release workflow](pr-workflow.md)
