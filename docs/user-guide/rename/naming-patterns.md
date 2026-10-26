# Build a naming pattern

<p class="lead">Write the pattern that decides what every file is called and which folder it goes in: the tags you can use, optional parts, what happens when a value is missing, and which characters CueRithm changes for you. Allow ten minutes to get a pattern right with the live examples.</p>

!!! tip "New to this?"
    Follow [Rename and sort files into folders](organize-files.md) first. It uses a pattern from preview to undo. This page is the reference you come back to when you change the pattern.

A pattern is a short line of text. CueRithm fills it in for each track and keeps the file's own extension, so you never type `.flac` or `.mp3`. You write it in **Filename pattern**, in **Organize files** for a one-off plan and in **Settings → File organization** for new tracks. The panel under it, **Example filenames**, re-checks the pattern as you type, and **Preview complete plan** shows the real result for every track before anything is changed.

## How a pattern is built { #how-patterns-work }

| You write | Meaning |
|---|---|
| `{artist}` | Insert a tag. The name goes in braces. |
| `[ ({version})]` | An optional part. It disappears when its tag is empty and **Missing values** allows it. |
| `/` | Start a new folder. Only for **Move to folder** and **Copy to folder**. |
| Any other text | Copied as it is: spaces, dashes, brackets, digits. |

The pattern may be 1 to 2,048 bytes long. Press a button under the box (**Artist**, **Title**, **Mix / version**, **Label**, **Year**, **Bpm**, **Key**, **Energy**) to insert that tag at the cursor.

## Tags you can use { #tokens }

<!-- figure:organize-token-buttons -->
<figure markdown>
![The Filename pattern box with the eight token buttons Artist, Title, Mix / version, Label, Year, Bpm, Key and Energy under it](../assets/screens/organize-token-buttons-light.webp#only-light){ .screen loading=lazy }
![The Filename pattern box with the eight token buttons Artist, Title, Mix / version, Label, Year, Bpm, Key and Energy under it](../assets/screens/organize-token-buttons-dark.webp#only-dark){ .screen loading=lazy }
<figcaption>Eight buttons insert the most-used tags at the cursor.</figcaption>
</figure>
<!-- /figure -->

| Tag | Gives you | Notes |
|---|---|---|
| `{artist}` | The first artist | `Adriatique` |
| `{artists}` | Every artist, joined with `, ` | `Final Request, Skuro (IT)` |
| `{title}` | The title tag | Beatport titles already end in the mix name, as in `Palm Of Your Hands (Extended Mix)` |
| `{album}` | The album | |
| `{album_artist}`, `{album_artists}` | First, or all, album artists | |
| `{genre}`, `{genres}` | First, or all, genres | A genre that contains `/` is changed to `_`; see [below](#illegal-characters) |
| `{label}` | The record label | |
| `{version}` | The file's **Mix** tag (the button reads **Mix / version**) | Often already inside the title; adding it again repeats it |
| `{release_date}` | The release date from the file's tags | `2024-05-17` |
| `{year}` | The four-digit year of the release date | Missing when the date has no valid year |
| `{key}` | CueRithm's measured Camelot key | `11A` |
| `{bpm}` | CueRithm's measured BPM, always with two decimals | `127.00` |
| `{energy}` | CueRithm's whole-track energy rating, 1 to 10 | `6` |
| `{track}`, `{disc}` | Track or disc number, digits only | `4` from a tag of `4/12` |
| `{original_name}` | The current file name without its extension | |
| `{parent}` | The name of the folder the file is in now | |

`{key}`, `{bpm}` and `{energy}` come from CueRithm's analysis of the audio, not from tags, so they exist for every Ready track. If you analyse a track again and the result changes, its name changes the next time you organise it. The demo track *Palm Of Your Hands (Extended Mix)* has key `11A` and energy `6`.

A tag with no value counts as missing. A tag whose value is only spaces counts as missing too.

## Change the case or pad a number { #filters }

Add a filter after a vertical bar inside the braces. The app does not list these, but the pattern accepts them:

| Filter | Example | Result |
|---|---|---|
| `upper` | `{artist|upper}` | `ADRIATIQUE` |
| `lower` | `{title|lower}` | `palm of your hands (extended mix)` |
| `pad:N` (1 to 8; `{track}` and `{disc}` only) | `{track|pad:2}` | `04` |

## Make a part optional { #optional-parts }

Wrap text and tags in square brackets, such as `[{key} - ]`. If any tag inside is missing, the whole bracketed part is left out. For that to happen, set **Missing values** to **Omit optional part**; with the default, a missing tag inside brackets still stops the track.

Brackets cannot be nested and cannot contain a `/`.

## Choose what happens when a value is missing { #missing-values }

| **Missing values** | What happens to a track with a missing tag |
|---|---|
| **Needs attention** (default) | The track is blocked with the reason "Missing naming field: key". It stays where it is. |
| **Omit optional part** | Bracketed parts that lack a tag are dropped. A missing tag outside brackets still blocks the track. |
| **Keep original name (optional organization)** | The track is skipped and keeps its name: "Optional organization skipped: missing key". Needs **Organization is optional**. |

Tick **Organization is optional** to allow the skip choices. Unticking it resets a skip choice in either menu to **Needs attention**. A title that is empty after the clean-up below is always blocked.

## Characters CueRithm changes for you { #illegal-characters }

Tag values can hold characters that file systems refuse. CueRithm replaces these in a **tag value** with `_`: `<` `>` `:` `"` `/` `\` `|` `?` `*` and control characters. Spaces and dots at the end of a name are trimmed. What you type in the pattern itself is never repaired; an unsafe pattern is rejected.

| Tag value | In the file name |
|---|---|
| `F*ck The Club (Extended Mix)` | `F_ck The Club (Extended Mix)` |
| `Baset, Anyma (ofc), 19:26` | `Baset, Anyma (ofc), 19_26` |
| `Electro (Classic / Detroit / Modern)` | `Electro (Classic _ Detroit _ Modern)` |
| `140 / Deep Dubstep / Grime` | `140 _ Deep Dubstep _ Grime` |
| `AC/DC` | `AC_DC` |

The same rules keep names portable between macOS, Windows and Linux: no name may be longer than 255 characters, and Windows device names such as `CON` are blocked.

Genres written like `140 / Deep Dubstep / Grime` make poor folder names. Tidy them first with [Merge similar genres into one](../tagging/recipes/merge-similar-genres.md) and the genre folders come out clean.

## Folders and renaming { #folders }

`/` in a pattern creates folders under the destination you chose, and only works with **Move to folder** and **Copy to folder**. **Rename in place** keeps the file in its folder, so a pattern with a `/` is refused: "Rename in place cannot create folders; choose Move instead".

## When two files want the same name { #collisions }

Existing files are never overwritten. If the destination name is taken, the track is blocked. Names that differ only in capital letters, or in how an accented letter is encoded, count as the same name. If two tracks in one plan would get the same name, both are blocked with "Multiple selected files reserve the same destination", so add `{label}`, `{album}` or `{track}` to tell them apart. Set **If a filename already exists** to **Skip track (optional organization)** to skip such tracks instead of blocking them.

## Patterns that work { #example-patterns }

Results use tracks from the demo library; `.flac` is added to each.

| Pattern | Mode | Example result |
|---|---|---|
| `{artist} - {title}` (the default) | any | `Adriatique - Palm Of Your Hands (Extended Mix).flac` |
| `{artists} - {title}` | any | `Final Request, Skuro (IT) - F_ck The Club (Extended Mix).flac` |
| `{artist}/{artist} - {title}` | move, copy | `Colyn/Colyn - When The Fire Starts To Burn (Extended Mix).flac` |
| `{genre}/{artist} - {title}` | move, copy | `Electro (Classic _ Detroit _ Modern)/Random Factor - Undercurrent (Original Mix).flac` |
| `{key} - {energy} - {artist} - {title}` | any | `11A - 6 - Adriatique - Palm Of Your Hands (Extended Mix).flac` |
| `[{key} - ]{artist} - {title}` with **Omit optional part** | any | `Adriatique - Palm Of Your Hands (Extended Mix).flac` when the track has no key |
| `{artist|upper} - {title|lower}` | any | `ADRIATIQUE - palm of your hands (extended mix).flac` |
| `{track|pad:2} - {title}` | any | `04 - Palm Of Your Hands (Extended Mix).flac` for track number 4 |

Two of these have recipes: [Sort files into artist folders](recipes/sort-files-into-artist-folders.md) and [Put key and energy in file names](recipes/put-key-and-energy-in-file-names.md).

## What can go wrong { #what-can-go-wrong }

The **Example filenames** panel shows these messages as you type, and **Preview complete plan** and **Save settings** are blocked until the pattern is valid.

<!-- figure:organize-pattern-error -->
<figure markdown>
![Example filenames showing the error Unknown naming field: artsit for the pattern {artsit} - {title}, with Preview complete plan disabled](../assets/screens/organize-pattern-error-light.webp#only-light){ .screen loading=lazy }
![Example filenames showing the error Unknown naming field: artsit for the pattern {artsit} - {title}, with Preview complete plan disabled](../assets/screens/organize-pattern-error-dark.webp#only-dark){ .screen loading=lazy }
<figcaption>A misspelled tag is caught as you type.</figcaption>
</figure>
<!-- /figure -->

| Message | Cause and fix |
|---|---|
| "Unknown naming field: artsit" | A tag name is misspelled. Use a name from the table above. |
| "Unsupported typed filter: pad:2" | `pad` works only on `{track}` and `{disc}`, with 1 to 8. `upper` and `lower` work on every tag. |
| "Nested optional segments are unsupported" | Brackets inside brackets. Use one level. |
| "Optional segments cannot contain directories" | A `/` inside `[...]`. Move the slash outside the brackets. |
| "Unclosed field", "Unclosed optional segment", "Unmatched field delimiter" | A missing `}` or `]`, or an extra one. |
| "Nonportable path component" | A name that ends in a space or dot, for example a pattern ending in `.`. |
| "Missing naming field: title" on a real track | The tag is empty. Edit the tag with [Edit tags and artwork](../tagging/edit-tags-by-hand.md), or use brackets and **Omit optional part**. |

## Related

- [Rename and sort files into folders](organize-files.md)
- [Recover or undo a reorganisation](undo-organize.md)
- [Naming tokens reference](../reference/naming-tokens.md)
- [Recipes for tagging and cleanup](../tagging/recipes/index.md)

<!-- verified: crates/cuerithm-organize/src/template.rs (tokens, filters, optional segments, missing policies, 2048-byte limit); crates/cuerithm-organize/src/paths.rs (sanitise, portable rules, collision keys); crates/cuerithm-organize/src/plan.rs; crates/cuerithm-organize/tests/templates.rs; crates/cuerithm-library/src/organization/service/{capture,examples}.rs (context: version = Mix tag, key = Camelot, bpm two decimals, original_name, parent); apps/desktop/src/views/OrganizationSettings.tsx (buttons, menus, messages); all example results run through the template engine with demo-library values; legacy guides/file-organization.md re-checked -->
