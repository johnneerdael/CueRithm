# Set up automatic cleanup

<p class="lead">The Cleanup section holds the rules that tidy titles, genres and artists, and the switch that runs them on every new track. The same rules are used by the Cleanup tool in the sidebar. About ten minutes to read; most libraries need two or three rules.</p>

<!-- figure:existing-13-settings-cleanup-reference-settings-cleanup -->
<figure markdown>
![Cleanup settings](../../assets/screens/13-settings-cleanup.webp){ .screen loading=lazy }
<figcaption>The Cleanup section of Settings.</figcaption>
</figure>
<!-- /figure -->

!!! tip "New to this?"
    Try the rules on a few tracks first with [Clean up messy titles, genres and artists](../../tagging/cleanup-messy-titles.md). It previews every change before anything is written. Then come back and switch on automatic cleanup with [Run cleanup on every new track](../../tagging/cleanup-automatically.md).

Cleanup edits human-readable tags such as title, artist, album, genre and comment. It never changes the audio, key, BPM, beatgrid, energy, file paths or cues. No rules exist in a new library: **Smart Fixes** is empty and the genre and artist editors are off.

Open **Settings → Cleanup** and press **Save settings**. Rules saved here change tracks prepared afterwards; they never rewrite tracks that are already finished. Use the Cleanup tool for those ([Clean up messy titles, genres and artists](../../tagging/cleanup-messy-titles.md)).

## Run cleanup on every new track { #automatic }

| Control | What it does | Default | Valid values | Task page |
|---|---|---|---|---|
| **Clean incoming tracks automatically** | Applies the saved rules to each new track after catalog tags are applied and before the track enters **Tracks**. Cleanup never contacts a provider. | Off | On or off | [Run cleanup on every new track](../../tagging/cleanup-automatically.md) |

## Smart Fixes { #smart-fixes }

Smart Fixes run as an ordered list, from the top. The section says: "Rules run in this order, before genre and artist mappings. Only enabled rules are applied."

Every rule in the list has the same controls:

| Control | What it does |
|---|---|
| **Enabled** | Rules that are off are skipped but kept |
| The number | The rule's place in the order |
| **Move up** / **Move down** | Changes the order. The first rule cannot move up and the last cannot move down. |
| **Remove rule** | Deletes the rule |
| The rule's own options | See the table below |
| **Ignored values, one per line** | Whole values this rule must leave alone. Each must match the field exactly. |

To add a rule, choose a type in **New Smart Fix** and press **Add Smart Fix**. The button stops working at 64 rules.

| Rule (as listed in **New Smart Fix**) | What it does | Options and defaults when added |
|---|---|---|
| **Casing** | Turns values that are entirely upper case or entirely lower case into title case. Mixed-case values are left alone, and acronyms are kept. | **Field**: title. **Uppercase values** on, **Lowercase values** on. |
| **Characters to spaces** | Replaces each listed character in the title with a space and trims the ends | **Characters**: `_` (up to 64) |
| **Encoded characters** | Decodes HTML-style entities in the title, such as `&amp;` | None |
| **Extract artist** | When Artist is empty, takes one part of the title as the artist and shortens the title | **Separator**: ` - ` (1 to 64 characters). **Result number**: `1` (whole number, 1 to 256). |
| **Extract remixer** | When Remixer is empty, takes the name that comes before a suffix such as "remix" | **Remixer suffix**: `remix` (1 to 64 characters). **Remove remixer from title** on. |
| **Garbage** | Removes repeated spaces, stray punctuation at the edges and empty brackets | **Field**: title |
| **Number prefix** | Removes a leading track number from the title | None |
| **URLs and email** | Removes web addresses and complete email addresses | **Field**: title. **Clear entire matching field** off. |
| **Mix parentheses** | Wraps a trailing mix or version in parentheses | None |
| **Common title text** | Removes one kind of text a store or tool added to the title | **Pattern**: **Original Mix suffix**. Other choices: **Camelot key**, **Camelot key and energy**, **BPM suffix**. |
| **Promotional text** | Removes bracketed promos, "premiere" prefixes and promotional tails | **Field**: title |

Rules with a **Field** menu can act on: title, artist, album, album artist, genre, comment, grouping, label, remixer, mix, producer, composer or lyricist. Rules without one act on the title. A rule that would empty a value leaves it alone.

How each rule behaves, with before and after examples: [Write cleanup rules](../../tagging/cleanup-rules.md).

## Genres { #genres }

| Control | What it does | Default | Valid values |
|---|---|---|---|
| **Enable genre cleanup** | Turns the genre editor on | Off | On or off |
| **Exact source value** → **Canonical value** | One mapping per row: an exact existing genre in the first box (placeholder "Exact source value") and the spelling you want in the second. Chains are followed to the end. | No mappings | Source: required, exact. Canonical: not blank. |
| **Remove** (in a mapping row) | Deletes the value instead of renaming it | Off | On or off |
| **Add mapping** | Adds a row | n/a | Up to 4096 mappings, no duplicate sources, no cycles |
| **Protected genres, one per line** | Genres that must stay exactly as they are | None | Up to 4096 values, each up to 4096 bytes |
| **Convert genres into categorized tags** | Turns chosen genres into category tags (a custom category in your library) instead of leaving them as genres | Off | On or off |
| **Category** | The category that receives the converted genres | `Subgenres` | Not blank, up to 256 bytes |
| **Genres to convert, one per line** | The genres to convert | None | Up to 4096 values |
| **Remove converted values from Genre** | Takes the converted values out of the Genre tag | Off | On or off |

The Cleanup tool also shows every genre in your library with its track count, a search box, a sort menu (**Name**, **Track count**), a **Replace** box and a **Protect** box per value. Those appear only in the tool, not in Settings. See [Group tracks with categories and custom tags](../../tagging/curate-tracks.md) for categories.

## Artists { #artists }

| Control | What it does | Default | Valid values |
|---|---|---|---|
| **Enable artist cleanup** | Turns the artist editor on | Off | On or off |
| **artist**, **remixer**, **producer**, **composer**, **lyricist** | The roles cleanup covers | **artist** only | At least one when enabled |
| **Exact source value**, **Canonical value**, **Remove**, **Add mapping** | The same mapping editor as Genres | No mappings | As for genres |
| **Protected artists, one per line** | Names that must stay exactly as they are | None | As for genres |

Names with a comma stay one value when the file stores them that way. Similar spellings are never merged on their own.

## Limits { #limits }

If a rule breaks a limit, a red banner above the section names it, "Review Cleanup settings." follows, and **Save settings** stays unavailable until it is fixed.

| Limit | Value |
|---|---|
| Smart Fixes | 64 |
| **Characters**, **Separator**, **Remixer suffix** | 64 characters each; the separator and suffix cannot be empty |
| **Result number** | A whole number from 1 to 256 |
| Ignored values (per rule) and protected values | 4096 unique values; each non-blank, up to 4096 bytes |
| Mappings (genres, artists) | 4096; sources unique and non-empty; targets non-blank or **Remove** |
| Genres to convert | 4096 unique values |
| Artist cleanup | At least one role selected |

## In what order cleanup runs { #order }

Enabled Smart Fixes run in list order. Genre conversion and mapping follow, then artist mapping. Values that appear as the source or target of a mapping are protected from the text rules in the roles they cover.

## Related

- [Clean up messy titles, genres and artists](../../tagging/cleanup-messy-titles.md)
- [Write cleanup rules](../../tagging/cleanup-rules.md)
- [Undo a cleanup](../../tagging/undo-cleanup.md)
- [Cleanup changed too much](../../help/troubleshooting/cleanup-changed-too-much.md)
- [Look up a setting](index.md)

<!-- verified: apps/desktop/src/views/CleanupSettings.tsx (templates, defaults, labels, 64-rule cap, Source/Canonical placeholders, role list); apps/desktop/src/cleanup-validation.ts (all limits); crates/cuerithm-cleanup/src/model.rs (SmartFix::validate, CommonTextPattern default OriginalMixFromTitle, 64 fixes, 4096 limits); crates/cuerithm-library/src/settings.rs (cleanup_enabled false, cleanup None); apps/desktop/src/test/settings-pages.test.tsx; legacy guides/cleanup-rules.md re-checked. UNVERIFIED: "Convert genres into categorized tags" wording on custom category creation is taken from the UI label and legacy text; the text-rule protection of mapping values is from legacy guides/cleanup-rules.md. -->
