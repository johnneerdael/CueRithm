# How do smart playlists stay current?

<p class="lead">Smart playlists are evaluated by a separate engine over a snapshot of your library. A rule is checked against every track, and the result refreshes in the background, so a playlist follows your library without you rebuilding it.</p>

<!-- figure:existing-55-smart-playlist-editor-how-it-works-smart-engine -->
<figure markdown>
![The smart playlist rule editor](../assets/screens/55-smart-playlist-editor.webp){ .screen loading=lazy }
<figcaption>The rules the smart playlist engine evaluates.</figcaption>
</figure>
<!-- /figure -->

!!! abstract "What this means for you"
    - **Membership updates itself, usually within a second.** When tracks are prepared or edited, or a playlist changes, the playlists that depend on them are re-evaluated. Rules about dates or files re-check at least once a minute.
    - **Missing values have fixed meanings.** A track with no BPM counts as 0, and a track with no date never matches "recent". Read [Missing values](#missing-values) before you build a rule on a field some tracks lack.
    - **Opening a playlist never starts tagging or analysis.** The engine only reads what is already in the library.
    - **A broken refresh keeps the last good list** and shows an error. Nothing is partly published.
    - To build one, follow [Build a smart playlist](../playlists/build-a-smart-playlist.md).

## What a smart playlist is made of { #model }

A smart playlist is a flat list of up to 128 conditions, each with a field, an operator and a value, plus a choice to **Match** *all conditions* or *any condition*. With *all*, you can flag two or more conditions as an OR group; then at least one flagged condition must match, along with every unflagged one. The field and operator catalogue is in [Smart playlist fields and operators](../reference/smart-playlist-fields.md).

## How a rule is checked { #evaluation }

- Every condition is evaluated for every candidate track. An error in any condition fails the whole evaluation; a later or earlier true result does not hide it.
- A track is a candidate if it is active. Archived tracks join only if a rule asks for them (the `archived` field is true), and Incoming tracks only if a rule on the `incoming` field is true. <!-- UNVERIFIED: the on-screen labels of the archived and incoming fields; the rule semantics are verified in evaluate.rs and lib.rs --> A smart playlist with no conditions matches nothing.
- **Measured fields.** Key, BPM and energy come from CueRithm's analysis. BPM compares after rounding to two decimals.
- **Text.** The track's value is trimmed and lower-cased; the query is lower-cased with its spaces kept. An empty query never matches "contains", "starts with" or "ends with", and always matches the "does not" forms.
- **Dates.** Calendar days in the library time zone, which you set in **Settings → Library preferences** (it defaults to your system's). Relative cutoffs ("in the last 7 days") use calendar subtraction, and month ends are clamped.

## Missing values { #missing-values }

| Field is missing | What the rule sees |
|---|---|
| A number (BPM, energy, rating) | It counts as 0. Only **is between** needs a real value |
| A date | It never matches *is*, *after*, *before*, *between* or *recent*, but it does match *is not* and *not recent* |
| A file | File-availability fields distinguish *present*, *missing* and *unavailable*. If a volume cannot be read, the result is *unavailable* and the evaluation reports it instead of guessing |

## How key rules match { #keys }

*Is compatible with key* uses the **Harmonic** or **Fuzzy** setting from **Settings → Library preferences**.

| Mode | Matches |
|---|---|
| **Harmonic** (default) | The same number in either mode (6A with 6B), and the neighbouring numbers in the same mode (6A with 5A and 7A, including 12 with 1) |
| **Fuzzy** | Both modes of the same and neighbouring numbers (6A with 5B, 6B and 7B as well) |

In the demo library, "Mixes with 7A" is a harmonic list: 6A, 7A, 8A and 7B. Key text in a rule accepts Camelot, Open Key and musical names. See [Understand Camelot keys](../analysis/camelot-keys.md).

## Which regular expressions work { #regular-expressions }

The engine uses `regress`, a Rust implementation of ECMAScript regular-expression syntax, adapted to enforce a work budget. It needs no JavaScript runtime. A pattern is at most 100 UTF-16 code units, a tested value at most 1 MiB, and the budget is one million steps. Exceeding the budget is an error, even inside a negative assertion, never a silent "no match".

## When a playlist refreshes { #refresh }

| Trigger | When |
|---|---|
| Preparation, metadata, cleanup or playlist change | The background service checks about once a second and re-evaluates affected playlists |
| Relative-date rules and file-dependent rules (missing file, waveform, rating, embedded art) | At least every minute |

If an evaluation fails, the last good membership stays and an error shows. Nothing is partly published. Refreshing never decodes audio, writes tags or analyses cues.

## Automatic generation { #automatic-generation }

The opt-in options under **Settings → Smart Playlists** create lists for you, under **Automatic Smart Playlists**:

| Option | Lists created |
|---|---|
| **By Artist** | One list per individual artist or remixer, including collaboration credits |
| **By BPM** | Up to 10 inclusive ranges that you define |
| **By release date** | Year and month folders with all tracks, *Warm-Up* (energy 1 to 5), *Dancefloor* (6 to 7) and *Anthems* (8 to 10) |
| **By Genre** | One list per genre in your library |
| **By Key** | 24 Camelot lists of compatible tracks |

Generated lists have their own identity, separate from their name, so a list you made yourself with the same name is left alone. Turning an option off, or removing a BPM range, keeps the lists that already exist. Release grouping accepts valid year-month or full dates, prefers explicit release tags over the generic date, and never derives a month from a year alone. Artist, remixer and genre rules compare whole credits rather than substrings. See [Keep smart playlists up to date automatically](../playlists/automatic-smart-playlists.md).

## Playlists that refer to other playlists { #membership-references }

A rule can refer to another playlist, a folder or a smart playlist. Cycles, including through a folder's descendants, are rejected when you save.

## Previews are pinned { #previews-are-pinned }

**Preview matches** is tied to the playlist revision, library data, time zone and file observations it ran on. Saving a smart playlist requires a current successful preview, and a preview expires if it depends on the clock, so a stale window cannot publish an old result.

## How big a smart playlist can be { #limits }

Up to 128 conditions per playlist, text values up to 1,048,576 characters, and 10 BPM ranges for automatic lists. The playlist generator creates at most 5,000 playlists per batch.

## Where you see it in the app { #where-you-see-it }

| Place | What shows |
|---|---|
| Playlist editor | **Match** all or any, the condition list with its `n/128` count, **Preview matches** and the number of matching tracks |
| **Playlists** in the sidebar | Smart playlists, and the **Automatic Smart Playlists** folder |
| **Settings → Smart Playlists** | The automatic options |

## Related

- [Build a smart playlist](../playlists/build-a-smart-playlist.md)
- [Write smart playlist rules](../playlists/smart-rules.md)
- [Smart playlist fields and operators](../reference/smart-playlist-fields.md)
- [Find tracks that mix with the one playing](../playlists/find-compatible-tracks.md)

<!-- verified: crates/cuerithm-smartlists/src/lib.rs (128 rules, match_all and OR group, all rules evaluated, archived rule), numeric.rs (missing = 0, between, BPM two decimals), text.rs (trim, lower-case, empty query), dates.rs (calendar days, relative cutoffs, missing dates), keys.rs (Harmonic and Fuzzy, Open Key aliases), crates/cuerithm-regex/src/lib.rs and tests/budget.rs (100 UTF-16 units, 1 MiB, 1,000,000 steps), crates/cuerithm-library/src/playlists/service.rs (1 s loop), evaluate.rs (allows_incoming and archive, dynamic fields and 60 s, dependency deadlines), snapshot.rs (dynamic()), tree.rs (cycle rejection), generator.rs (5000 nodes), model.rs (AutomaticPlaylistSettings, 10 ranges), apps/desktop PlaylistEditor.tsx (Preview matches, save requires preview), SmartRuleEditor.tsx (Match all conditions or any condition, OR group, n/128), AutomaticPlaylistOptions.tsx (options, Warm-Up, Dancefloor, Anthems), docs/research/automatic-smart-playlists.md; legacy technical/smart-engine.md re-checked. Changed from legacy: dates with no value match the negated operators -->
