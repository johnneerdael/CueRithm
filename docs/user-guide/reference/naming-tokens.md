# File naming tokens

<p class="lead">A filename pattern is ordinary text with tokens in braces, such as <code>{artist} - {title}</code>. This page lists all 19 tokens, the two filters, optional parts and the limits, each with a result you can check. Look up one token in about a minute.</p>

!!! tip "New to this?"
    Build your first pattern with [Build a naming pattern](../rename/naming-patterns.md), then try it on a few tracks in [Rename and sort files into folders](../rename/organize-files.md). The controls that use patterns are listed in [File organization settings](settings/file-organization.md).

## Every token { #tokens }

Tokens read the track's final tags and CueRithm's own measurements, after tagging and cleanup have been accepted. The examples use the demo track **Palm Of Your Hands (Extended Mix)** by Adriatique: genre `Melodic House & Techno`, key `11A`, 127 BPM, energy 6/10.

<!-- figure:reference-settings-token-buttons -->
<figure markdown>
![The Filename pattern box and the token buttons Artist, Title, Mix / version, Label, Year, Bpm, Key and Energy under it](../assets/screens/reference-settings-token-buttons-light.webp#only-light){ .screen loading=lazy }
![The Filename pattern box and the token buttons Artist, Title, Mix / version, Label, Year, Bpm, Key and Energy under it](../assets/screens/reference-settings-token-buttons-dark.webp#only-dark){ .screen loading=lazy }
<figcaption>Eight tokens have buttons; the other eleven are typed in braces.</figcaption>
</figure>
<!-- /figure -->

| Token | Gives | Where the value comes from | Example | Buttons in Settings |
|---|---|---|---|---|
| `{artist}` | The first artist | The first non-blank value of the Artist tag | `Adriatique` | **Artist** |
| `{artists}` | Every artist | All non-blank Artist values, joined with a comma and a space | A track with two artist values gives `95 South, SIDEPIECE` | |
| `{title}` | The title | The first Title value, exactly as tagged, including any text in brackets | `Palm Of Your Hands (Extended Mix)` | **Title** |
| `{album}` | The album | The first Album value | | |
| `{album_artist}` | The first album artist | The first non-blank Album artist value | | |
| `{album_artists}` | Every album artist | All non-blank Album artist values, joined with a comma and a space | | |
| `{genre}` | The first genre | The first non-blank Genre value | `Melodic House & Techno` | |
| `{genres}` | Every genre | All non-blank Genre values, joined with a comma and a space | | |
| `{label}` | The record label | The first Label value | | **Label** |
| `{version}` | The mix or version | The first value of the **Mix** tag. It does not read the title, so a title that already ends in "(Extended Mix)" leaves `{version}` empty unless the Mix tag is filled. | | **Mix / version** |
| `{release_date}` | The release date, as written | The first of these tags that is present: `releasedate`, `release_date`, `release date`, `release_time`, `releasetime`, `tdrl`, `date`, `year` | A tag reading `2026` gives `2026` | |
| `{year}` | The four-digit year | The part of the release date before the first `-`, when it is four digits and not `0000` | A release date of `2026` gives `2026` | **Year** |
| `{key}` | The musical key | CueRithm's measured key in Camelot notation | `11A` | **Key** |
| `{bpm}` | The tempo with two decimals | CueRithm's measured BPM, rounded to hundredths. A track with no BPM, or a BPM of 0, is an error. | A track listed at 127 BPM gives `127.00`; a tested value of 124.05 BPM gives `124.05` | **Bpm** |
| `{energy}` | The whole-track energy | The single energy rating, 1 to 10. A value outside that range is an error. | `6` | **Energy** |
| `{track}` | The track number | The number before any `/` in the track-number tag, written without padding | A tag `7/12` gives `7` | |
| `{disc}` | The disc number | The number before any `/` in the disc-number tag | | |
| `{original_name}` | The file's current name | The file name without its extension | | |
| `{parent}` | The folder it is in | The name of the single folder that holds the file, not a full path | A file in `~/Music/Beatport` gives `Beatport` | |

Eight tokens have buttons under **Filename pattern** in Settings and in **Organize files**. The other eleven are typed by hand. Key, BPM and energy come from CueRithm's analysis, not from file tags. The pattern fixes only the name and any subfolders; the file keeps its original extension, even if the title contains a dot.

## Filters { #filters }

Add a filter after a vertical bar inside the braces.

| Filter | Works on | Effect | Example |
|---|---|---|---|
| `upper` | Any token | Converts the value to upper case | `{artists|upper}` turns `Artist / DJ, Guest` into `ARTIST _ DJ, GUEST` |
| `lower` | Any token | Converts the value to lower case | `{key|lower}` turns `8A` into `8a` |
| `pad:1` to `pad:8` | `{track}` and `{disc}` only | Pads with leading zeros to at least that many digits | `{track|pad:3}` turns `7` into `007` |

Any other filter is an error: `{title|exec}`, `{track|pad:99}` and `{artists|pad:2}` are all refused.

## Optional parts { #optional }

Square brackets mark text that may be left out.

```text
{artist} - {title}[ ({version})]
```

With a Mix tag of `Club Mix`, this gives `Adriatique - Palm Of Your Hands (Club Mix)`. With no Mix tag, the bracketed part disappears and you get `Adriatique - Palm Of Your Hands`, but only when **Missing values** is **Omit optional part**. With **Needs attention** the missing `{version}` blocks the track.

| Rule | Detail |
|---|---|
| Nesting | One level only. `[[{title}]]` is refused. |
| Folders | An optional part cannot contain `/` |
| Required parts | A token outside brackets is required. If it is empty, the track needs attention, or is skipped when organization is optional and **Missing values** is **Keep original name (optional organization)**. |
| Empty result | A name that ends up empty is always blocked |

## Patterns that were tested { #tested }

These results come from the automated tests for the naming engine (`crates/cuerithm-organize/tests/templates.rs`). The tracks are made up to exercise the rules, not demo-library tracks.

| Pattern | Track values | Result (before the extension) |
|---|---|---|
| `{artists|upper}/{track|pad:3} - {title}[ ({version})]` | artists `Artist / DJ` and `Guest`, title `A: Title`, track 7, no version, **Omit optional part** | `ARTIST _ DJ, GUEST/007 - A_ Title` |
| `{year}/{bpm} - {key}[ {energy}] {title}` | release date `2026`, BPM 124.05, key `8A`, energy 7, title `A: Title` | `2026/124.05 - 8A 7 A_ Title` |
| `{artist} - {artists}` | artists: one blank value, `Guest`, one empty value | `Guest - Guest` (blank values are ignored) |

## What happens to unsafe characters { #characters }

Values from tags are cleaned, but text you type in the pattern is checked and never repaired.

| In a tag value | Result |
|---|---|
| `< > : " / \ | ? *` and control characters | Each becomes `_`, so `A: Title` gives `A_ Title` and `Artist / DJ` gives `Artist _ DJ` |
| Spaces or dots at the end of a value | Removed |
| A value that is empty after cleaning | Counts as missing |

Folder separators in values never create folders; only a `/` you type in the pattern does, and only for **Move to folder** and **Copy to folder**.

## Limits and error messages { #limits }

The pattern holds 1 to 2048 bytes. Each folder or file name is at most 255 characters, and the whole relative path at most 4096. Names that Windows reserves (`CON`, `PRN`, `AUX`, `NUL`, `COM1` to `COM9`, `LPT1` to `LPT9`) and names ending in a space or dot are always refused, so a pattern works on every system. The pattern cannot start with `/`, contain `//` or `..`, or name a drive.

| Message | Meaning |
|---|---|
| "Unknown naming field: *name*" | The word in braces is not one of the 19 tokens |
| "Unclosed field", "Unmatched field delimiter" | A `{` without `}` or a `}` without `{` |
| "Unmatched optional delimiter", "Unclosed optional segment" | Brackets that do not pair up |
| "Nested optional segments are unsupported", "Optional segments cannot contain directories" | See [Optional parts](#optional) |
| "Unsupported typed filter: *filter*" | See [Filters](#filters) |
| "Template must contain 1–2048 bytes" | The pattern is empty or too long |
| "Missing naming field: *token*" | The track has no value for a required token |
| "Invalid measured naming value" | BPM is 0 or energy is outside 1 to 10 |
| "Rename in place cannot create folders; choose Move instead" | **Rename in place** with a `/` in the pattern |

## Related

- [Build a naming pattern](../rename/naming-patterns.md)
- [Rename and sort files into folders](../rename/organize-files.md)
- [File organization settings](settings/file-organization.md)
- [Recover or undo a reorganisation](../rename/undo-organize.md)

<!-- verified: crates/cuerithm-organize/src/template.rs (TOKENS, field(), filters, parse errors); crates/cuerithm-organize/src/paths.rs (sanitize, validate_relative, reserved names); crates/cuerithm-organize/README.md; crates/cuerithm-organize/tests/templates.rs (run on this commit: passes); crates/cuerithm-library/src/organization/service/capture.rs (where each NamingContext field comes from); crates/cuerithm-library/src/organization/service/examples.rs (validate, 19 tokens); apps/desktop/src/views/OrganizationSettings.tsx (eight token buttons); legacy guides/file-organization.md re-checked. Demo-track results are derived from the private demo-library facts sheet values: the whole-number BPM 127 is shown as 127.00 on the assumption that the stored tempo is 127.00 to two decimals. -->
