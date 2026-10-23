# Smart playlist fields and operators

<p class="lead">Every field a smart playlist condition can test, every operator it can use, and exactly how text, numbers, dates, keys and regular expressions are compared. Use it to settle "will this rule match that track?" in a couple of minutes.</p>

!!! tip "New to this?"
    Build a first rule with [Build a smart playlist](../playlists/build-a-smart-playlist.md). The rule editor itself, with previews and saving, is explained in [Write smart playlist rules](../playlists/smart-rules.md). This page is the lookup table behind both.

## How this page was produced { #provenance }

The catalogue is read from the code, not from memory. Re-check it after each release against these files:

| Source file | What it provides |
|---|---|
| `crates/cuerithm-smartlists/src/model.rs` | The 58 operators (`Operator::ALL`) and the built-in field list (`FieldCatalog::standard`) |
| `crates/cuerithm-library/src/playlists/snapshot.rs` | Fields added at run time, their picker labels, which are unavailable, and where every value comes from |
| `apps/desktop/src/smart-rules.ts` | The operator wording shown in the editor and which fields offer which operators |
| `crates/cuerithm-smartlists/src/{text,numeric,dates,keys,tags,cues,membership}.rs` and `crates/cuerithm-regex/src/lib.rs` | How each comparison behaves |

`cargo test -p cuerithm-smartlists -p cuerithm-regex` replays the recorded reference cases, including a check that all 58 operators are covered with a matching and a non-matching track each. The worked examples near the end of this page come from those tests.

## Combine conditions { #combining }

| Choice | Meaning |
|---|---|
| **Match** **all conditions** | Every condition must be true |
| **Match** **any condition** | At least one condition must be true |
| **OR group** (a box on each condition) | Under **all conditions**, tick it on two or more conditions. At least one ticked condition must be true, and every unticked condition must still be true. A single ticked condition changes nothing. |
| An empty list | Matches nothing |
| Up to 128 conditions | The counter next to **Match** shows how many you use. There is no nesting. |
| Archived tracks | Never match, unless a condition says **archived** *is true* |
| Tracks still in Incoming | Not considered until a condition says **incoming** *is true* |
| An error in any condition | The whole playlist fails instead of guessing; see [When a smart playlist updates](#refresh) |

## Fields { #fields }

In the editor, **Field** lists every field by its picker label, in alphabetical order. Many fields have no friendly label and show their identifier instead, such as `genre` or `fileType`. Both are valid. CueRithm has 63 built-in fields; 5 are listed but cannot be used. Every other property your files report adds one more field.

Key, BPM, energy, duration and cue data are CueRithm's own measurements, never values read from file tags.

### Text { #fields-text }

| Picker label | ID | What it holds | In Any/Every text field |
|---|---|---|---|
| Title | `title` | Title tag | Yes |
| Artist | `artist` | Artist tag; several values are joined with `; ` | Yes |
| Album | `album` | Album tag | Yes |
| Album artist | `albumArtist` | Album artist tag | Yes |
| genre | `genre` | Genre tag; several values joined with `; ` | Yes |
| comment, grouping, label, remixer, mix, producer, composer, lyricist | same as the label | The tag of that name (`mix` is the mix or version) | Yes |
| Key | `key` | CueRithm's measured key, in Camelot notation | Yes |
| location | `location` | The full path of the audio file | No |
| fileType | `fileType` | The audio codec, such as `FLAC` | No |
| extra1, extra2 | same as the label | Tags named `extra1` and `extra2` | Yes |
| isrc, catalogNumber | same as the label | The tags of that name | Yes |
| Artist or remixer credit | `artistCredit` | One entry per individual artist or remixer, split at commas, semicolons, `feat.`, `ft.`, `vs`, `x`, `&` and similar, plus a remixer named in the title | Yes |
| Individual genre | `genreCredit` | One entry per genre in the Genre tag | Yes |
| Curate notes | `curation_notes` | Your notes from Curate | Yes |
| audio.codec | `audio.codec` | The codec name as the file reports it | No |
| Any text field / Every text field | `AnyTextField`, `EveryTextField` | Not a field of its own: runs the same text test on all the "Yes" fields above and on every `tag.*` field. **Any** needs one to pass; **Every** needs all to pass. | n/a |

### Numbers { #fields-number }

| Picker label | ID | What it holds |
|---|---|---|
| BPM | `bpm` | CueRithm's measured tempo |
| duration | `duration` | Length in seconds |
| year | `year` | Year from the year or date tag |
| Energy | `energy` | The whole-track energy rating, 1 to 10 |
| rating | `rating` | The star rating stored in the file's own tags; 0 when there is none. Read from the file, so re-checked about once a minute. |
| Bitrate (kbps) | `bitrate` | Bitrate in kilobits per second |
| Size (bytes) | `sizeBytes` | File size |
| sampleRate | `sampleRate` | Sample rate in Hz |
| trackNumber | `trackNumber` | The number before any `/` in the track-number tag |
| Cue count | `cuepoints` | How many cues the track has |
| Tempo marker count | `tempomarkers` | How many tempo markers; CueRithm saves one grid per track |
| User rating | `user_rating` | Your 1 to 5 rating from Curate |
| Release month (YYYYMM) | `releaseMonth` | The release month as one number, for example `202603`. Needs a complete year-month or date in a release-date tag; a year alone gives nothing. |
| audio.sample_rate, audio.channels, audio.bits_per_sample | same as the label | The file's own properties |

### Dates { #fields-date }

| Picker label | ID | What it holds |
|---|---|---|
| Date added | `dateAdded` | When the track entered your library |
| Library modified | `dateModified` | When CueRithm last updated the track's record. This is not the audio file's modified date. |
| releaseDate | `releaseDate` | A complete calendar day from a release-date tag. A year or a year-month alone leaves it unknown. |

### Yes or no { #fields-boolean }

| Picker label | ID | True when |
|---|---|---|
| archived | `archived` | The track is archived |
| incoming | `incoming` | The track is still in Incoming |
| IsAnalyzed | `IsAnalyzed` | The track has a tempo from analysis |
| waveform | `waveform` | A waveform has been stored for the track. Re-checked about once a minute. |
| CustomFileMissing | `CustomFileMissing` | The audio file is not at its stored path. Re-checked about once a minute. If CueRithm cannot tell, the playlist shows an error rather than a wrong answer. |
| CustomHasLockedGenreCleanupGenre | `CustomHasLockedGenreCleanupGenre` | One of the track's genres is on the protected list in [Cleanup settings](settings/cleanup.md#genres) |
| Embedded artwork | `coverartPreview` | The file has artwork embedded in it. Re-checked about once a minute. |

### Colour, tags, cues and playlists { #fields-other }

| Picker label | ID | What it holds |
|---|---|---|
| color | `color` | The colour stored in the file's `color` tag |
| tags | `tags` | The custom tags you applied in [Curate](../tagging/curate-tracks.md) |
| categories | `categories` | The Curate categories those tags belong to |
| cues | `cues` | The track's cues and loops |
| tempos | `tempos` | The track's tempo markers |
| playlist | `playlist` | Membership of playlists, folders and other smart playlists |
| `tag.*`, `format.*` and the other `audio.*` | the name the file reports | Every embedded tag as `tag.<name>`, and every other property of the file. The type (text, number, yes/no) is taken from the values found; a mix of types is treated as text. `tag.*` fields join the Any/Every text field tests. |

### Listed but unavailable { #fields-unavailable }

These appear greyed out, marked "unavailable", with the message "No local source is available for this service field". CueRithm has no local data for them, so a rule can never use them.

| Picker label | ID |
|---|---|
| Popularity | `popularity` |
| Happiness | `happiness` |
| Streaming service | `streamingService` |
| Uploaded to cloud storage | `CustomUploadedCloudStorage` |
| Cloud upload | `cloudUpload` |

Plays, last played and danceability are not offered at all.

## Operators { #operators }

There are 58. The wording is exactly what the editor shows after **Operator**.

### Text { #op-text }

You type the value. Comparisons ignore capitals and spaces at the ends of the track's value; see [Text](#cmp-text).

| Operator | Matches when the field… |
|---|---|
| contains | contains your text (never matches an empty query) |
| does not contain | does not contain your text (always matches an empty query) |
| is | equals your text |
| is not | does not equal your text |
| starts with | starts with your text (never matches an empty query) |
| does not start with | does not start with it (always matches an empty query) |
| ends with | ends with it (never matches an empty query) |
| does not end with | does not end with it (always matches an empty query) |
| matches regular expression | matches your pattern; see [Regular expressions](#cmp-regex) |
| does not match regular expression | does not match the pattern, or is empty |

### Numbers { #op-number }

| Operator | Value | Matches when the field is… |
|---|---|---|
| equals | one number | equal to it |
| does not equal | one number | different from it |
| is greater than | one number | above it |
| is at least | one number | it or above |
| is less than | one number | below it |
| is at most | one number | it or below |
| is between | two numbers | between them, both ends included, in either order |

### Dates { #op-date }

| Operator | Value | Matches when the date is… |
|---|---|---|
| is on | a calendar day | on that day |
| is not on | a calendar day | on any other day, or unknown |
| is on or after | a calendar day | on that day or later |
| is before | a calendar day | on an earlier day |
| is in the last | a whole number and days, weeks or months | within that span back from now, or in the future |
| is not in the last | the same | older than that span, or unknown |
| is between dates | two calendar days | between them, both days included |

### Yes or no, colour and existence { #op-simple }

| Operator | Value | Matches when… |
|---|---|---|
| is true | none | the field is true |
| is false | none | the field is false |
| is color | a colour | the colour equals it, or both are empty |
| is not color | a colour | the colour differs |
| has a value | none | the field is not empty |
| has no value | none | the field is empty |

### Tags and categories { #op-tags }

| Operator | Value | Matches when the track… |
|---|---|---|
| includes every selected tag | one or more tags | has all of them |
| includes none of the selected tags | one or more tags | has none of them |
| includes any selected tag | one or more tags | has at least one |
| has exactly the selected tags | one or more tags | has those tags and no others |
| has no tags in selected categories | one or more categories | has no tag from any selected category |
| has a tag in any selected category | one or more categories | has a tag from at least one |
| has a tag in every selected category | one or more categories | has a tag from each |

With nothing selected, every tag and category operator matches nothing.

### Tempo and whole BPM { #op-tempo }

| Operator | Value | Matches when… |
|---|---|---|
| all tempo markers have the same BPM | none | every marker has the same tempo. CueRithm saves one tempo per track, so this is true for any analysed track. |
| tempo markers have different BPM | none | there are two or more markers with different tempos. This does not happen with CueRithm's own analysis. |
| is a whole BPM | none | the BPM has no decimals once rounded to three places, for example `128` or `128.0004` |
| is not a whole BPM | none | the BPM is set and has decimals. A track with no BPM matches neither whole-BPM operator. |

### Playlists { #op-membership }

| Operator | Value | Matches when the track… |
|---|---|---|
| is in playlist | an ordinary playlist | is in it |
| is not in playlist | an ordinary playlist | is not in it |
| is in playlist folder | a folder | is in any playlist inside that folder |
| is not in playlist folder | a folder | is in none of them |
| is in smart playlist | a smart playlist | is among its current results |
| is not in smart playlist | a smart playlist | is not |

A smart playlist cannot depend on itself, directly or through others; CueRithm refuses to save a cycle.

### Cues { #op-cues }

| Operator | Value | Matches when… |
|---|---|---|
| any cue label contains | text | some cue's name contains it (an empty query finds cues with no name) |
| no cue label contains | text | no cue's name does |
| any cue has color | a colour | some cue has exactly that colour |
| no cue has color | a colour | no cue does |
| has a loop | none | the track has a saved loop |
| has no loop | none | it has none |
| has an active loop | none | a loop is set as active, like the S.O.S. loop |
| has no active loop | none | none is |
| has a memory cue | none | a memory cue exists. CueRithm's own cues are never memory cues. |
| has no memory cue | none | none exists |

### Key { #op-key }

| Operator | Value | Matches when… |
|---|---|---|
| is compatible with key | a key from `1A` to `12B` (default `8A`) | the track's key is compatible under your [Key compatibility](settings/library.md#key-modes) mode |

## Which operators each field offers { #matrix }

| Field type | Operators offered |
|---|---|
| Text | Text, Existence. `key` also offers **is compatible with key**. |
| Any text field, Every text field | Text only |
| Numbers | Numbers, Existence. `bpm` adds the whole-BPM pair; `cuepoints` adds the Cues group; `tempomarkers` adds the tempo pair. |
| Dates | Dates, Existence |
| Yes or no | Yes or no, Existence |
| `color` | Colour, Existence |
| `tags` | Tags, Existence |
| `categories` | Categories, Existence |
| `cues` | Cues, Existence |
| `tempos` | Tempo pair, Existence |
| `playlist` | Playlists only |

## How comparisons behave { #comparisons }

### Text { #cmp-text }

- Both sides are compared in lower case, with spaces and similar blanks trimmed from the track's value. Spaces you type in the query count. A track title of `" YES "` matches **is** `yes`.
- **is** compares the whole value. **contains**, **starts with** and **ends with** look for your text inside it.
- `Key` is compared as its Camelot form, so **is** `8a` finds a track stored as `Am`. `fileType` is compared as upper-case, such as `FLAC`.
- **is** on `artistCredit` and `genreCredit` matches any single credit, not the whole combined value.
- A missing value counts as an empty value: **does not contain**, **is not** and **does not match regular expression** all match tracks where it is missing.
- A value can be up to 1,048,576 characters.

### Numbers { #cmp-number }

- BPM is rounded to two decimals before it is compared, halves rounding up: a measured 123.455 equals 123.46, and 124.995 equals 125.
- **A missing number counts as 0**, except in **is between**, which also needs the value to exist. So **BPM is less than 125** matches a track with no BPM, and **BPM is between 119 and 121** does not. Add **has a value** when you do not want unknowns.
- Cue count and Tempo marker count are real numbers, so a track with no cues has a count of 0.
- Reversed limits are fine: between 124 and 123 means between 123 and 124.

### Dates { #cmp-date }

- Dates are compared as calendar days in your **Time zone** from [Library preferences](settings/library.md#controls). The time of day does not matter. **Date added is on** 18 October matches a track added at 09:00 that day and not one added on the 20th.
- **is on or after** includes the chosen day. **is before** does not.
- **is in the last 7 days** counts back seven calendar days from now; **weeks** counts seven days each, and **months** counts calendar months. A date in the future also counts as recent. A count of 0 matches nothing. The amount can be from -1,000,000 to 1,000,000.
- A track with no date matches **is not on** and **is not in the last** and fails the other date operators.
- A day you pick in the editor becomes midnight at the start of that day in the library time zone: 25 October 2026 is 22:00 UTC on 24 October in `Europe/Amsterdam`. A day in a skipped hour moves forward by the length of the gap. Use `YYYY-MM-DD`; `2026-02-30` and `2026-1-2` are refused.

### Key { #cmp-key }

- The track's key and the key you choose are read as Camelot (`8A`), Open Key (`1m`, `2d`) or a musical name (`Am`, `A minor`, `C major`, `Db`); anything else is "unknown" and never matches.
- **Harmonic** accepts the same key, one step either way on the same letter, and the same number with the other letter. **Fuzzy** accepts the same number and one step either way with either letter. Numbers wrap, so 12 sits next to 1. A table is on [Library preferences](settings/library.md#key-modes).

### Regular expressions { #cmp-regex }

- The flavour is ECMAScript (JavaScript). Enter `/pattern/flags` or a bare pattern. The editor suggests `/pattern/flags` and limits the box to 100 characters.
- Flags: `d`, `g`, `i`, `m`, `s`, `u`, `v`, `y`. A repeated flag, or `u` with `v`, is an error. `y` requires the match to start at the beginning. Use `i` for case-insensitive matching; without it the match respects capitals.
- The pattern runs on the raw value, not the trimmed, lower-cased one, so `/^Am$/` matches a stored key `Am` exactly. It runs only when the field has a value.
- An invalid pattern, such as `[`, is refused when you check the rule.
- A pattern that needs too much work (more than one million steps) on a value is an error, never a quiet "no match": the pattern `(a+)+$` on a title of one hundred `a` characters followed by `!` fails the playlist. Simplify the pattern. The tested value can be up to 1 MiB.

### Tags, cues and playlists { #cmp-other }

- Colour comparisons are exact and case-sensitive, and stored colour names matter. Cues that CueRithm creates use lower-case names such as `red`, `yellow` and `violet` ([Cue names, letters and colours](cue-types.md#colours)). Type `red`, not `Red`; the suggestion list in the editor offers capitalised names.
- Cue label tests ignore capitals and surrounding spaces.
- Playlist tests need the other list's membership to be known; if it is not, the playlist shows an error.

## Tested examples { #examples }

These cases are asserted by `crates/cuerithm-smartlists/tests/contract.rs` and the recorded reference cases in `tests/reference.rs`. The tracks are test fixtures, not demo-library tracks.

| Rule | Track | Result |
|---|---|---|
| Title **is** `yes` | Title `" YES "` | Matches |
| Title **is** `yes` **or** Title **is** `no`, with **any condition** | Title `" YES "` | Matches. With **all conditions**: does not match. |
| **all conditions**. Not in the OR group: Title **is** `yes`. In the OR group: Title **is** `yes`, Title **is** `no` | Title `" YES "` | Matches: the unticked condition is true and one OR-group condition is true |
| **all conditions**. Not in the OR group: Title **is** `no`. In the OR group: Title **is** `yes` only | Title `" YES "` | Does not match: a single ticked condition is an ordinary condition, and `no` is false |
| Title **is** `yes` | The same track, archived | Does not match |
| Title **is** `yes` and **archived** **is true** | The same track, archived | Matches |
| Key **is** `8a` | Stored key `Am` | Matches |
| Key **matches regular expression** `/^Am$/` | Stored key `Am` | Matches |
| Key **is compatible with key** `8A` | Key `9B`, Harmonic | Does not match |
| The same rule | Key `9B`, Fuzzy | Matches |
| BPM **equals** `123.46` | BPM 123.455 | Matches |
| BPM **equals** `125` | BPM 124.995 | Matches |
| BPM **is less than** `125` | No BPM | Matches |
| BPM **is between** `119` and `121` | No BPM | Does not match |
| BPM **is between** `124` and `123` | BPM 123.455 | Matches |
| Title **does not match regular expression** `[` | any | Refused when the rule is checked |
| Title **does not match regular expression** `(a+)+$` | Title: 100 times `a`, then `!` | Error: the work limit is exceeded |
| Date added **is on** 2026-10-18 (UTC) | Added 2026-10-18 09:00 | Matches |
| Date added **is on** 2026-10-18 (UTC) | Added 2026-10-20 | Does not match |
| Date added **is in the last** 1 **days**, now 2026-10-18 12:00 UTC | Added 2026-10-18 09:00 | Matches |
| Date added **is not in the last** 1 **days** | No date | Matches |
| **CustomFileMissing** **is false** | File availability not checked | Error, not a guess |

## When a smart playlist updates { #refresh }

- CueRithm checks every second for work to do. A playlist is re-evaluated after preparation, tag changes, cleanup, playlist edits and changes to [Library preferences](settings/library.md).
- Rules that depend on the clock or on files (**is in the last**, **is not in the last**, `CustomFileMissing`, `waveform`, `rating`, Embedded artwork) are also re-evaluated about once a minute.
- If an evaluation fails, the playlist keeps its last good results and shows the error message, plus "Showing the last successfully evaluated results." More: [How do smart playlists stay current?](../how-it-works/smart-engine.md).

## Limits { #limits }

| Limit | Value |
|---|---|
| Conditions per playlist | 128 |
| Text in a condition | 1,048,576 characters |
| Regular expression | 100 characters; 1,000,000 steps of work per value; value up to 1 MiB |
| Tags or categories in one condition | 10,000 |
| "In the last" amount | -1,000,000 to 1,000,000 |
| Playlist path in a membership condition | 64 levels |

## Related

- [Write smart playlist rules](../playlists/smart-rules.md)
- [Build a smart playlist](../playlists/build-a-smart-playlist.md)
- [Smart Playlists settings](settings/smart-playlists.md), the lists CueRithm builds for you
- [Library preferences](settings/library.md), for time zone and key mode
- [Understand Camelot keys](../analysis/camelot-keys.md)

<!-- verified: crates/cuerithm-smartlists/src/{lib,model,text,numeric,dates,keys,tags,cues,membership}.rs; crates/cuerithm-regex/src/lib.rs (flags dgimsuvy, 100 UTF-16 units, 1,000,000 work, 1 MiB); crates/cuerithm-library/src/playlists/snapshot.rs (field additions, labels, unavailable reasons, value sources) and evaluate.rs (60000 ms refresh for dynamic rules), service.rs (1 s tick), tree.rs (cycle rejection); crates/cuerithm-library/src/playlists/credits.rs; apps/desktop/src/smart-rules.ts (operator labels, allowedOperators); apps/desktop/src/components/SmartRuleEditor.tsx (value inputs, 128 counter, regex maxLength 100, colour suggestions); crates/cuerithm-smartlists/tests/{contract,reference,calendar_input}.rs and fixtures/lexicon-expanded-v1.json (examples; cargo test -p cuerithm-smartlists -p cuerithm-regex -p cuerithm-organize passed on this commit); legacy guides/smart-rules.md re-checked. UNVERIFIED in the running app: the capitalised colour suggestions not matching CueRithm's lower-case cue colours (read from code: exact comparison in cues.rs, suggestions in SmartRuleEditor.tsx). -->
