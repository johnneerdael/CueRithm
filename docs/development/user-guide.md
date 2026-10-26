# Build and maintain the user guide

The published guide is <https://cuerithm.neerdael.nl/> (the old `github.io` address redirects to it). Its source is
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
| `docs/user-guide/_meta/the private demo-library facts sheet` | The real tracks, keys, BPMs and cues every example uses |
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
2. **Capture.** `tools/docs/capture/` drives the sandboxed CueRithm window (see below) on
   the real demo library at 1440×940 with `open-computer-use` and `screencapture`, in dark and
   light, as raw PNGs with the window's own shape. `to_webp.py` converts them to
   2000 px wide WebP files named `assets/screens/<scene>-light.webp` and `-dark.webp`.
   Open every image before keeping it: reject an error banner, a hover highlight, a
   credential, or anything outside the app window. Real library data is the intent; check
   each image for file paths that include a user name.
3. **Place.** `python3 tools/docs/embed_figures.py` inserts the `<figure>` markup after
   the named heading, only when both theme images exist, and updates a placed figure in
   place on re-run. Never write figure markup by hand.

### Which library to capture from

The guide's screenshots come from the owner's own library, as the app displays it: titles,
artists, keys, BPMs, energy values and cues are what the app shows, and the AGENTS.md rule on
publishing makes the guide the one exception to its ban on per-track information. What it
keeps out is everything the screen can leak by accident: audio files, library paths, user
names, credentials and licence details. Open every image before keeping it and check the
title bar, path fields, tooltips and the Finder or Explorer window behind the app. The
per-track tables used for research stay in the private `.analysis/` area.

Capture from a copy of the app data, with the latest released app, never from the live
library. Opening a library with a newer release migrates it, and that migration does not go
back, so the original must not be the one that is opened.

A copy of the data folder is not enough on its own. It still names the owner's real music
folders, and CueRithm starts its background work as soon as it opens a library: it watches the
saved watch folders, renames or tags new files there according to the saved settings, and
continues queued jobs. Start the copy with the launcher, which runs the app in a macOS sandbox
that allows writing only to the copy and to the app's own cache, web-view, log and temporary
folders:

```sh
cp -R "$HOME/Library/Application Support/com.cuerithm.app" /path/to/capture-data   # with CueRithm quit
tools/docs/capture/launch-copy.sh /path/to/capture-data
```

The launcher sets `CUERITHM_DATA_DIR` to the copy, refuses the live data folder (also through a
link) and refuses to start while CueRithm is already running, because that instance would be
outside the sandbox. The launcher also records the process it starts, and the capture driver acts
only while that is the only CueRithm running. The driver never starts an app, so a sandboxed app
that quits cannot be replaced by the installed one, running on the real library. Nothing can rename, tag, move or delete the owner's music from the
sandboxed app, whatever the saved settings say and wherever the music is (an external drive,
`~/Music` or a cloud folder under `~/Library/CloudStorage`); reading is unaffected, so waveforms, playback
and scans work. A write the sandbox blocks appears as a failed job or an error banner: reject
that screenshot and fix the cause instead of widening the sandbox. `launch-copy.sh --print-profile
DATA_COPY` prints the sandbox rules without starting anything.

The sandbox is a backstop. Scans and previews only read, so capture those freely, and do not
script an Apply, Delete or Archive step even though the sandbox would stop it.

### What the driver can and cannot do

The driver talks to the app through the accessibility tree, and falls back to real pointer
clicks (only while the sandboxed app is the frontmost app) for what the tree does not list, such
as the table's column headings. Things that surprised us, so scene files do not repeat the work:

- **Shared web storage.** The sandboxed app has the installed app's bundle id, so both read and
  write the same saved interface settings (theme, column choice, sidebar width) under
  `~/Library/WebKit/com.cuerithm.app`. A capture run changes the owner's theme and columns:
  note the values before a run (see the `localstorage.sqlite3` file there; values are UTF-16)
  and put them back afterwards with the sandboxed app stopped. Column scenes run last
  (`scenes/zz-columns-last.json`), because a column that was ever ticked stays in the saved order.
- **Playing and pausing.** Press the Pause button, not Space: Space presses whichever control
  holds keyboard focus. The play button of a row only exists once the row is selected.
- **Typing.** `type` presses one key at a time, because `type_text` sets the field's accessibility
  value (an empty field reports its placeholder) and never reaches the page's handlers. Only letters,
  digits, space and the keypad keys exist; the keypad point types the system's decimal comma, so
  `fill` sets a decimal value and steps it up and down so the page reads it.
- **Selecting many rows.** The click tool cannot hold Command or Shift; use Command+A, which
  selects every row of the view or search.
- **Focus rings.** A control pressed through the accessibility tree keeps a keyboard focus ring;
  `blur` clicks the brand text to clear it before the shot.
- **Pop-up menus.** A menu is a `pop up button`; its entries appear in the accessibility tree as
  `menu item NAME ID: …` only while it is open, so pick one with `click "^menu item NAME ID"`. A window
  capture never includes an open menu, so a figure of an open menu cannot be taken this way.
- **Long pages.** Controls scrolled out of view have no position, and a page long enough (Organize files
  with a long history) fills the accessibility tree before the player. `scrollto TEXT TOP` finds the text
  on screen with OCR and scrolls until it sits where the shot needs it; `MIN_X` picks the pane that scrolls
  (the tag editor's inspector scrolls on its own). A pane that ignores scrolling is not captured.
- **Text the key map cannot type.** `fill` and `paste` use the accessibility value for brackets, commas,
  colons and line breaks, then a space and a backspace so the page reads it.
- **History in the copy.** A scene that shows a list of past work (Organize history, drafts) starts from
  whatever the copy holds. Clear it with the app stopped (the rows of the `organization_*` tables) or
  discard it in the app (Drafts and history), never in the owner's library.
- **Session drops.** The shared computer-use agent closes the connection now and then; the driver
  restarts it and repeats the step, so a step must read the state before it clicks.

### Make the copy match the guide

The guide's text quotes one small library, the 26 tracks of the private facts sheet
(`.analysis/guide/library-facts.md`), and the owner's library has grown past it: a capture from
the whole library would contradict the guide's own numbers ("Energy 7 and up: 9 tracks"). With
the sandboxed app stopped, trim the copy first:

```sh
python3 tools/docs/capture/make_demo_copy.py /path/to/capture-data .analysis/guide/library-facts.md --stale keep
```

It keeps the 26 tracks, and with `--stale keep` the 14 stale rows left by files that moved (the
broken-library scenes need them; `--stale drop` removes them for the healthy scenes), removes every
other track from the copy, and replaces the watch folder with the generic, empty
`/private/tmp/demo/Music/Beatport`, so the settings screens show no user name. It edits only the
copy, and refuses the live data folder and any copy that has an `active-library.json` (a switched
library generation), because the app would open that library instead of the file the builder edits.
Track locations still hold the real paths: never show the
Location column, and treat every scan result that lists paths as something to redact or skip.

### Check every image

Open each raw capture, blur the file paths in it, then run the text check on all of them:

```sh
python3 tools/docs/capture/redact_paths.py /path/to/raw/*.png      # a guide image never shows a file path
swiftc -O tools/docs/capture/leakcheck.swift -o ~/.cache/cuerithm-docs/leakcheck
~/.cache/cuerithm-docs/leakcheck /path/to/raw/*.png
```

`AGENTS.md` bars audio files, file paths, user names, credentials and licence details from guide
images, and a generic demo folder is still a path, so `redact_paths.py` blurs every line of text
that looks like one (a settings page cannot help listing its watch folder). The text check then
reads the text in each image and lists any user name, home folder, file path, e-mail address, web
address (a configured service URL is private), or credential or licence word, so an empty result
means nothing matched. It is a second check on top of looking at the image, not a replacement for
it. Name the blurred path in the figure's alt text ("a watch folder (its path blurred)").

## Related

- [Desktop development and acceptance](desktop.md)
- [Pull request and release workflow](pr-workflow.md)
