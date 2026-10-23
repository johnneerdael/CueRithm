# Requesting and placing screenshots

Writers request screenshots; the lead captures them; a script places them. No
page contains hand-written figure markup or a link to an image that does not
exist, so the strict build stays green while captures are pending.

## 1. Request (writers)

One JSON array per tab in `_meta/figures/<tab>.json` (`get-started.json`,
`prepare.json`, `tracks.json`, `tag.json`, `playlists.json`, `sync.json`,
`library.json`, `customize.json`, `how-it-works.json`, `reference.json`,
`help.json`, `home.json`). Entry fields:

| Field | Meaning |
|---|---|
| `scene` | kebab-case id, unique per picture, reused across pages when the picture is the same |
| `page` | path of the page relative to `docs/user-guide/` |
| `after` | the exact heading line the figure follows, or `intro` (after the first paragraph) |
| `alt` | what the reader needs to see; real values from `library-facts.md` |
| `caption` | one sentence that makes the point |
| `state` | how to reproduce it in the app: sidebar item, tab, selection, what must be visible, demo playlists/tracks |
| `crop` | optional: `window` (default, the whole app window), `panel` (only the main panel), `player` (only the player) |
| `needs` | optional: extra demo-library state the scene needs |

## 2. Capture (lead)

Each scene becomes `assets/screens/<scene>-light.webp` and `<scene>-dark.webp`,
captured from the installed CueRithm on the real library at 1440×940, downscaled
to 2000 px wide. Every image is opened and checked before it is kept: no error
banner, no private path, no credential, no hover highlight.

## 3. Place (script)

`python3 tools/docs/embed_figures.py` inserts

```html
<!-- figure:scene -->
<figure markdown>
![alt](…/scene-light.webp#only-light){ .screen loading=lazy }
![alt](…/scene-dark.webp#only-dark){ .screen loading=lazy }
<figcaption>caption</figcaption>
</figure>
<!-- /figure -->
```

after the named heading's first paragraph, only when both images exist, and
updates a placed figure in place on re-run. `--check` reports missing images and
unknown pages or headings and changes nothing.
