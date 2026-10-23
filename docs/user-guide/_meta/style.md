# CueRithm user guide: style guide

For everyone writing pages. Not published. Read it once, then keep the page
skeletons (section 3) and the checklist (section 12) open. Read `ia.md` for
what to write and the private demo-library facts sheet for the data your examples must use.

When this guide and a finished exemplar disagree, tell the lead; the exemplar
wins until this file is fixed.

## 1. Ground rules

1. **The code is the truth.** Describe what the latest release (CueRithm 0.1.16) does, verified in
   `apps/desktop/src/` (labels, flows), `apps/desktop/src-tauri/src/` (commands)
   and `crates/` (behaviour, limits, defaults). The previous guide (in git history,
   before the feature-order restructure) is a source of facts and wording to check,
   not an authority: re-verify every claim you keep. If you cannot verify a behaviour, leave it out
   or mark it `<!-- UNVERIFIED: what, why -->` and list it in your final report.
2. **Never launch, drive or read data from the CueRithm app, and never touch
   `~/Library/Application Support/com.cuerithm.app` or `~/Music`.** The lead is
   capturing screenshots from the app on the owner's real library; a second
   writer clicking in it breaks the capture. You get everything from code, tests
   and the facts sheet.
3. **Use the real demo data.** Examples cite tracks, keys, BPMs, energy and cue
   times from the private demo-library facts sheet and playlists from the demo-library table in
   `ia.md`. Do not invent track data. When a task needs a case the demo library
   lacks, describe it generically ("a track whose title ends in `(Original
   Mix)`") and request a scene that shows the real equivalent.
4. **Write for one person trying to get something done.** Every page answers:
   what will I be able to do, how do I do it, what can go wrong, where do I go
   next.
5. **Do not overclaim.** AGENTS.md rules apply to prose too: analysis is offline
   and agreement with other tools is measured, not "accurate"; there is one
   whole-track energy rating, never energy per cue or per section; Mixed In Key,
   Lexicon, Beatport, Traxsource and rekordbox are not affiliated with CueRithm;
   no claim of parity with another product; compatibility claims are scoped to what
   `help/compatibility.md` and the code record (direct rekordbox writes: compatible 6/7
   builds on macOS, Windows and Wine/Proton; unrecognised database or asset formats stay
   read-only; no collection XML transport; Mixxx not tried on Windows; seven sync
   destinations). Re-read `help/compatibility.md`, `help/not-available-yet.md` and the roadmap in the repository README before any
   support statement. Decoding stops at about 24 minutes; Windows and Linux builds are
   unsigned; there is no Intel Mac build; there is no auto-update. Never publish private library paths other
   than `~/Music/Beatport`-style generic paths, credentials, or per-track research labels.
6. **Own only your files.** Edit only the pages assigned to you, plus your
   `_meta/figures/<tab>.json`. Do not edit `mkdocs.yml`, `redirects.json`,
   `_meta/ia.md`, `style.md`, other tabs' pages, `stylesheets/` or
   `_snippets/abbreviations.md`. Put requests (a new page, a nav change, a new
   abbreviation, a CSS class, a fact you found wrong elsewhere) in your final
   report.

## 2. Voice, spelling and terms

- **Friendly, plain, second person, active voice, present tense.** "Select the
  tracks you want to archive", not "The user selects…". Short sentences. Say the
  *why* in one sentence when a rule is surprising.
- **British spelling in prose** (colour, analyse, organise, favourite). **UI
  labels are quoted exactly as the app shows them**, even when the app spells
  them differently (**Organize files**, **Analyze again**, **Colour theme**).
- **No filler or hype.** Avoid: simply, just, easy, powerful, seamless, robust,
  leverage, utilise, "in order to", "it is worth noting", "comprehensive". If
  something were simple the reader would not be here.
- **Do not apologise for gaps.** If CueRithm does not do something, say so
  plainly, once, and give the workaround.
- Numbers: digits for anything on screen ("26 tracks"), words for one to nine in
  running prose.
- **First person is not used** in task pages. (The provenance page is the one
  exception, written by the owner.)

| Use | Not |
|---|---|
| track (a file in the library) | song, item, entry |
| library | collection, database (except in how-it-works) |
| Incoming, Tracks, Archive (the three library views) | inbox, queue |
| prepare (a track: decode, analyse, tag, clean up, add) | process, import |
| cue (a labelled position), loop, beatgrid | marker, hot cue (unless quoting another app) |
| smart playlist, ordinary playlist, folder | dynamic playlist, crate |
| sync (to DJ software) | export, transfer |
| watch folder | monitored folder, hot folder |
| catalog tagging / automatic tagging | scraping, lookup |

Write **UI labels in bold**: **Add tracks**, **Tools → Sync** is not valid (the
app has no Tools menu: Tools is a sidebar group), so write "choose **Sync** in
the sidebar". Menu bar paths use `→`: **File → …**. Field names, tokens, rule
syntax and anything the reader types go in code style.

## 3. Titles, headings and page skeletons

**Titles are goals in the reader's words.** This is the main change from the
old guide.

| Do not write | Write |
|---|---|
| Maintenance | Find and merge duplicate tracks |
| Player, waveform and cues | Preview a track with the player |
| Settings reference | Find the setting you need |
| Cleanup rules | Write cleanup rules *(page about the rules)* / Clean up messy titles, genres and artists *(tutorial)* |
| Under the hood | How it works |
| Limits, Details, Overview, Usage, Notes, Introduction | the specific thing: "What apply guarantees", "When a track needs attention" |

Rules:

- **H1 = what the reader wants to do or know**, usually a verb phrase or a
  question. Landing pages may use the tab name ("Tag and clean up your music").
  Reference pages may use the thing they list ("Smart playlist fields and
  operators").
- **H2/H3 name the step, the sub-goal or the symptom**, never a bare noun like
  "Settings". Tutorials use numbered steps: `## 1. Open Cleanup`.
- **Every heading another page links to has an explicit id**:
  `## Undo a cleanup { #undo }`. Do not rely on generated slugs across pages.
  Ids are lower-case words joined by hyphens, stable, and describe the content.
- Every page has one `#` heading, a `<p class="lead">` sentence under it saying
  what the reader gets and about how long it takes, and no front matter unless
  a skeleton below needs it.

Six page types. Pick one and follow its skeleton.

### Landing page (`<tab>/index.md`, `analysis/corrections/index.md`)
A map, not a manual. Lead paragraph; a screenshot of the feature in use; **Start
here** (the tutorial, with why); tables **Your goal → Page** grouped by task in
which **every page of the tab appears**; "Not what you wanted?" links to
neighbouring tabs; related Help pages.

### Tutorial
Learning by doing with a guaranteed result. Lead: what you will make and how
long it takes. **You need:** checklist with `- [ ]`. Numbered `##` steps, each
with *what to do*, *what you should see* (screenshot or exact description) and
*why*. One path only. Ends with `## What you learned` and `## Next steps`
(2 to 4 deep links). Use demo-library tracks and playlists.

### How-to (a task)
For a reader who knows the basics. Lead: the outcome. `**Before you start**`
only if there are prerequisites (link them). Numbered steps or one `##` per
sub-task, each self-contained. `## What can go wrong` with symptom → cause → fix,
each linking to the relevant Help page. `!!! warning` for anything that
changes files; say how to undo it. Ends with `## Related`.

### Recipe (`*/recipes/*.md`)
One problem, one solution. Start with a `<div class="recipe-meta" markdown>` box
(**Uses**, **Works on**, **Time**, **Undo**), then `## The problem` (before →
after table with real tracks), `## The solution` (numbered steps and, for rules,
a code block or table of exact settings), `## How it works`, `## Variations`,
`## Related`. Every rule or setting in a recipe is built from fields and
options that exist in the code (name the file you checked in the verification
comment).

### Concept
Builds understanding without a procedure. Opens with the question a confused
reader asks, answers in plain words with a small table or diagram, ends with
"What this means in CueRithm" and links to the task pages.

### Reference and symptom pages
**Reference**: opens with orientation and a `!!! tip "New to this?"` pointing at
the matching tutorial; complete tables (control, what it does, default, link to
the task page); ends with `## Related`. **Symptom** (`help/troubleshooting/*`):
H1 is what the reader sees ("A track shows a dash instead of a waveform"); then
**Why it happens**, **Fix it** (numbered, with deep links), **Check it worked**,
**Still stuck?**.

## 4. Links: deep links everywhere

The guide is navigated by links, so link densely and specifically.

- Link with relative paths to `.md` files and, wherever it exists, to the
  **section** (`../tagging/cleanup-rules.md#smart-fixes`). `mkdocs build --strict`
  fails on a broken page or anchor.
- **At least three contextual deep links in the body** of every page (not only
  in Related), pointing at the specific task or reference section the sentence
  mentions, not at a tab landing page.
- Every page ends with `## Related` (tutorials: `## Next steps`) with two to
  five links. Tutorials and how-tos link to reference; reference links back to
  the tutorial.
- Link a term the first time it appears on a page.
- A page that mentions a failure links to the matching `help/troubleshooting/…`
  page; a page that mentions a setting links to its `reference/settings/…`
  section.
- Link outside the guide only to stable sources (the CueRithm repository and
  releases, rekordbox/DJ-software vendor pages, Beatport). Never link to another
  product's documentation as the explanation.
- Link targets that do not exist yet are fine: the plan in `ia.md` and
  `mkdocs.yml` fixes every path. Use only paths from the plan.

## 5. Screenshots

Every page that describes something on screen has at least one screenshot;
tutorials have one per step that changes the screen. The lead captures them
from the real app (real tracks, real cover art, light and dark). **You request
them; you never write `<figure>` markup and never reference an image file.**

Add an entry per figure to `_meta/figures/<tab>.json`, then describe the state
so it can be reproduced. Read `figures/README.md` for the format. In short:

```json
{
  "scene": "cleanup-preview",
  "page": "tagging/cleanup-messy-titles.md",
  "after": "## 3. Scan the library",
  "alt": "Cleanup with ten proposed changes listed under Scan tracks",
  "caption": "Scanning previews every change before anything is written.",
  "state": "Sidebar: Cleanup, Smart Fixes tab, after Scan tracks on All ready tracks, proposals visible, nothing applied."
}
```

- `scene` is kebab-case, starts with the tab folder ("cleanup-…" is fine for
  `tagging/`), and describes the state. Reuse a scene id across pages when the
  picture is the same.
- `after` is the exact heading line the figure follows (`intro` = after the lead
  paragraph). Keep it in sync if you rename a heading.
- `alt` says what the reader needs to see, not "screenshot". `caption` makes the
  point in one sentence. Alt and caption use real values from the facts sheet.
- `state` is for the lead: where to click, what to select, what must be visible,
  which demo playlists or tracks. Be exact; say "dark/light don't matter".
- Show the smallest state that makes the point. Do not request a picture that
  only repeats the text. Do not request pictures of things the app cannot show.
- Do not describe a screenshot's contents in prose as if it were guaranteed to
  match pixel for pixel. Prose stands on its own.
- Aim for one to three figures per task page, three to six per tutorial, one per
  landing page.

## 6. Keyboard shortcuts and platforms

Use `pymdownx.keys`: `++cmd+f++ / ++ctrl+f++` renders ⌘F / Ctrl+F. Write the Mac form
first. Keys alone: `++enter++`, `++esc++`. The full list lives in
`reference/keyboard-shortcuts.md` (owned by the Reference writer); other pages may
repeat a shortcut but must not contradict it. Check real shortcuts in
`apps/desktop/src` and `apps/desktop/src-tauri/src/lib.rs` (menu bar), not in the
legacy page.

Where steps differ by platform use linked content tabs, in this order and with
these labels: `=== "macOS"`, `=== "Windows"`, `=== "Linux"`. For a one-word
difference, write it in the sentence.

## 7. Admonitions

| Type | Use |
|---|---|
| `!!! tip` | a faster or better way; "New to this?" pointers |
| `!!! note` | background that is not needed to finish the task |
| `!!! warning "Gotcha"` | something that silently gives the wrong result |
| `!!! danger` | can lose data and cannot be undone |
| `!!! example` | a worked example longer than a table row |
| `!!! info "Coming from another tool"` | where CueRithm behaves differently |
| `??? question` / `??? success` | collapsible exercise and answer in tutorials |

At most one admonition per screen of text.

## 8. Tables, code and data labels

- Tables for catalogues and *Your goal → Page* maps. Put every real value in a
  table cell with the units: `128 BPM`, `6A`, `7/10`.
- Key, BPM and energy have visible labels in the app, never colour alone. In
  prose use `<span class="cr-key">6A</span>`, `<span class="cr-bpm">128</span>`
  and `<span class="cr-energy">7/10</span>` for inline chips; cues use
  `cr-cue cr-cue--start|drop|breakdown|end` as in the legacy CSS.
- Rules, patterns and tokens go in fenced `text` blocks so the copy button
  copies exactly what to type.
- Write paths with `/`. Use `~/Music/Beatport` as the example watch folder.

## 9. Abbreviations and glossary

`_snippets/abbreviations.md` is appended to every page and gives hover tooltips
(BPM, FLAC, ADTS, …). List any abbreviation you introduce in your report; the lead
adds it. Terms for readers belong in `reference/glossary.md`; list new terms in
your report.

## 10. Verification comments

End every page with one HTML comment listing the code and tests you checked:

```html
<!-- verified: apps/desktop/src/views/Cleanup.tsx; crates/cuerithm-cleanup/src/lib.rs; legacy guides/cleanup-rules.md re-checked -->
```

Mark anything you could not verify with `<!-- UNVERIFIED: … -->` inline.

## 11. Working in parallel

- Work in the shared worktree you were given. Do not
  create branches, commit, push or stash.
- Build only into your own directory and without `--strict`, because other
  writers' pages are still missing:
  `python -m mkdocs build -q -d /tmp/<your-name>-site` in a Python environment with
  `docs/site-requirements.txt` installed.
  Warnings about missing pages that other writers own are expected; warnings
  about **your** pages (broken anchors, missing images, unknown links) are not.

## 12. Checklist before you report done

- [ ] Every page in your list exists, has one H1 that is a goal, a lead paragraph,
      and no heading from the "do not write" list.
- [ ] Every UI label is quoted exactly from the code; every default and limit is
      from the code or a test.
- [ ] Every example uses the private demo-library facts sheet data or is plainly generic.
- [ ] At least three contextual deep links per page, `## Related` present, every
      link target is a path from the plan, every cross-page `#anchor` has an
      explicit id on the target page (yours) or exists in the plan's owner's page
      (ask in your report if unsure).
- [ ] Figure requests are in `_meta/figures/<tab>.json`, each `after` matches an
      existing heading exactly, each `state` is reproducible.
- [ ] `mkdocs build` into your own directory shows no warning about your pages.
- [ ] Verification comment at the end of every page.
- [ ] Final report: pages written, pages merged or added versus the plan (and
      why), UNVERIFIED items, anchors other writers' pages must provide,
      abbreviations/terms to add, CSS needs, facts found wrong elsewhere, and
      figure requests that need new demo-library state.
