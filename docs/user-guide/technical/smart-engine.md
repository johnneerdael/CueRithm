# Smart playlist engine

Smart playlists are evaluated by a separate engine over a snapshot of your library.

## Model

A smart playlist is a flat list of up to 128 conditions, each with a field, an operator and a value, and a combinator (*all* or *any*, optionally with an OR group). See [Smart playlist rules](../guides/smart-rules.md) for the catalogue.

## Evaluation

- Every condition is evaluated for every candidate track. An error in any condition fails the whole evaluation. It is not masked by an earlier true result.
- A track is a candidate if it is active. Archived tracks join only if the playlist explicitly asks with *Archived is true*, and incoming tracks only with *Incoming is true*.
- **Missing values**: numbers compare as 0 (except *is between*). Dates that are missing do not match. File-availability fields distinguish *present*, *missing* and *unavailable* (for example when a volume cannot be read).
- **Measured fields.** Key, BPM and energy come from CueRithm's analysis. BPM compares after rounding to two decimals.
- **Text.** Lower-cased and trimmed on the track value, with the query's spaces kept.
- **Dates.** Calendar days in the library time zone. Relative cutoffs use calendar subtraction with clamped month ends.

## Keys

*Is compatible with key* uses the **Harmonic** or **Fuzzy** setting. Both use the standard Camelot adjacency, including 6A with 7A and 7A with 7B. Key text accepts Camelot, Open Key and musical names.

## Regular expressions

The engine uses `regress`, a Rust implementation of ECMAScript regular-expression syntax adapted to enforce a work budget. It does not need a JavaScript runtime. Patterns are at most 100 characters, tested values at most 1 MiB, and the budget is one million steps. Exceeding the budget is an error, even inside a negative assertion, never a silent "no match".

## Refresh

| Trigger | When |
| --- | --- |
| Preparation, metadata, cleanup or playlist change | Immediately |
| Background service | Every second |
| Relative-date and file-dependent rules (missing file, waveform, rating, embedded art) | At least every minute |

If an evaluation fails, the last good membership stays and an error shows. Nothing is partially published. Opening a playlist never triggers tagging or analysis.

## Automatic generation

Opt-in [Smart Playlist settings](../guides/settings.md#smart-playlists) reconcile generated nodes before membership refresh. Discovery reads effective prepared metadata, including cleanup's artist/remixer/genre values. It adds missing artist, genre and release-month groups, configured BPM ranges and the 24 compatible-key lists.

Preferences and node creation commit atomically. Managed identities are separate from names, so manual lists with matching names are untouched. Enabled categories restore their generated rules while preserving user names and folder positions. Rule repair keeps user edit identities intact, so renames and moves remain undoable and newer edits still prevent an older Undo. Removed nodes remain suppressed; disabled categories and removed BPM ranges retain their existing live lists.

Release grouping accepts valid year-month or full calendar dates, preferring explicit release tags over generic DATE. It never derives a month from a year alone. Individual artist/remixer and genre equality rules compare complete credits rather than substrings.

## Membership references

A rule can refer to another playlist, a folder or a smart playlist. Cycles, including through a folder's descendants, are rejected when you save.

## Previews are pinned

A preview is tied to the playlist revision, library data, time zone and file observations it ran on. Saving a smart playlist requires a current successful preview, so a stale window cannot publish an old result.

## Limits

Up to 128 conditions, values up to 1,048,576 characters, and 5,000 playlist nodes per generator batch.
