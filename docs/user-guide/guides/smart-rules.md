# Smart playlist rules

A smart playlist is a list of **conditions** evaluated against every active track. Build one in the [playlist editor](../learn/smart-playlist.md).

<figure markdown>
  ![The smart playlist rule editor](../assets/screens/55-smart-playlist-editor.webp){ .screen loading=lazy }
</figure>

## Combining conditions

- Up to **128 conditions** in one flat list. There is no nesting.
- **all conditions**: every condition must be true.
- **any condition**: at least one must be true.
- **OR group**: tick *OR group* on two or more conditions under *all conditions*. The flagged conditions then form one alternative group: at least one flagged condition must be true, and every unflagged condition must still be true. A single flagged condition has no effect.
- An empty list matches nothing.
- Every rule is evaluated. An error in any rule fails the whole list. It is never hidden by an earlier match.

## Fields

!!! note "Friendly names and raw ids"
    The field picker lists friendly labels for the common fields and raw identifiers for the rest. Both are valid.

**Text:** Title, Artist, Album, Album artist, `genre`, `comment`, `grouping`, `label`, `remixer`, `mix`, `producer`, `composer`, `lyricist`, Key, `location`, `fileType`, `isrc`, `catalogNumber`, `extra1`, `extra2`.

**Number:** BPM, `duration`, `year`, **Energy** (0 to 10), `rating` (0 to 5), Bitrate (kbps), Size (bytes), `sampleRate`, `trackNumber`, Cue count, Tempo marker count.

**Date:** Date added, Library modified, `releaseDate` (only complete `YYYY-MM-DD` tags count; year-only values stay unknown).

**Yes/no:** `archived`, `incoming`, `waveform`, `IsAnalyzed`, `CustomFileMissing`, Embedded artwork, and whether the genre is a protected cleanup genre.

**Other:** `color`, `tags` and `categories` (custom tags), `cues`, `tempos`, `playlist` (membership), *Any text field*, *Every text field*, native audio fields such as `audio.codec`, `audio.sample_rate`, `audio.channels`, `audio.bits_per_sample`, and any other embedded metadata CueRithm discovers.

**Unavailable:** `popularity`, `happiness`, `streamingService` and cloud-upload fields are greyed out with *No local source is available for this service field*. Plays, last played and danceability are deliberately not offered anywhere.

Key, BPM and energy come from CueRithm's own analysis, never from file tags.

## Operators

| Kind | Operators |
| --- | --- |
| **Text** | contains; does not contain; is; is not; starts with; does not start with; ends with; does not end with; matches regular expression; does not match regular expression |
| **Number** | equals; does not equal; is greater than; is at least; is less than; is at most; is between |
| **Date** | is on; is not on; is on or after; is before; is in the last (days, weeks, months); is not in the last; is between dates |
| **Colour** | is color; is not color |
| **Yes/no** | is true; is false |
| **Tags** | includes every selected tag; includes none of the selected tags; includes any selected tag; has exactly the selected tags |
| **Categories** | has no tags in selected categories; has a tag in any selected category; has a tag in every selected category |
| **Existence** | has a value; has no value |
| **Tempo** | all tempo markers have the same BPM; tempo markers have different BPM; is a whole BPM; is not a whole BPM |
| **Membership** | is in playlist; is not in playlist; is in playlist folder; is not in playlist folder; is in smart playlist; is not in smart playlist |
| **Cues** | any cue label contains; any cue has color; no cue label contains; no cue has color; has a loop; has no loop; has an active loop; has no active loop; has a memory cue; has no memory cue |
| **Key** | is compatible with key (1A to 12B) |

### How comparisons behave

- **Text** is compared lower-cased and trimmed on the track value. Spaces you type in the query are significant. The maximum value is 1,048,576 characters.
- **Numbers**: a missing value compares as 0, except for *is between*. BPM compares after rounding to two decimals.
- **Dates** are calendar days in your **library time zone**, set in *Settings > Library preferences*. *Is on or after* includes the chosen day and *is before* excludes it. Relative rules such as *in the last 14 days* refresh at least once a minute.
- **Key compatibility** has two modes, **Harmonic** (same number, or one step either way on the same letter, or the relative major or minor) and **Fuzzy** (same number or one step either way, whichever letter), chosen in Library preferences. Key text accepts Camelot, Open Key and musical names.
- **Cues**: loops, active loops and memory cues are explicit facts, never inferred from names. CueRithm's generated cues are hot cues and never invent memory cues.
- **Membership** rules may not form cycles. CueRithm rejects them on save.

### Regular expressions

Text rules can use `matches regular expression`. Enter `/pattern/flags` or a bare pattern. The flavour is ECMAScript (JavaScript). Valid flags are `d g i m s u v y`; duplicates, or `u` with `v`, are errors; `y` requires a match at the start. Use `i` for case-insensitive matching.

Limits: the pattern is at most 100 characters, the tested value at most 1 MiB, and a work budget of one million steps. Exceeding the budget is an **error**, never a silent non-match. An invalid pattern is reported when you validate.

## Preview, evaluation and refresh

- You must **Preview matches** successfully before **Create** or **Save changes**. An expired preview says *Preview expired. Preview matches again before saving.*
- Smart playlists refresh automatically after preparation, metadata or cleanup changes and playlist edits. A background service checks every second, and relative-date and file-dependent rules re-evaluate at least every minute.
- If evaluation fails, the playlist keeps its last good membership and shows an error banner.
- Archived tracks are excluded unless a rule says *Archived is true*; incoming tracks unless *Incoming is true*.
