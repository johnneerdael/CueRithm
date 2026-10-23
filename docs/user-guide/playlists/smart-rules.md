# Write smart playlist rules

<p class="lead">Choose how conditions combine, pick the right field and operator, and avoid the few rules that behave in ways you would not guess. Read it once, then keep it open while you edit.</p>

!!! tip "New to this?"
    Start with [Build a smart playlist](build-a-smart-playlist.md), which walks through one complete playlist. This page is the task guide for everything the rule editor can do. The complete field and operator tables are in [Smart playlist fields and operators](../reference/smart-playlist-fields.md).

## Choose how conditions combine { #combine-conditions }

A smart playlist has one flat list of up to 128 conditions. There are no nested groups. The **Match** menu above the list decides how they combine:

| Setting | A track is included when |
|---|---|
| **all conditions** | every condition is true |
| **any condition** | at least one condition is true |

An empty list matches nothing. If any single condition cannot be evaluated, the whole playlist reports an error and keeps its last good result.

### Mix "any" and "all" with an OR group { #or-group }

Under **all conditions** you can still allow alternatives. Tick **OR group** on two or more conditions. Then at least one ticked condition must be true, and every unticked condition must still be true. A single ticked condition changes nothing.

For example, to collect high-energy house or techno:

```text
Match: all conditions
genre   contains   House     [OR group]
genre   contains   Techno    [OR group]
Energy  is at least   7
```

In the Beatport purchase folder this finds six tracks, among them *17 (Extended Mix)*, *Come To Life (Original Mix)* and *Man Like Me (Extended Mix)*. There is only one OR group per playlist. For a second group, put it in another smart playlist and use [it as a condition](#playlist-rules).

## Pick a field { #fields }

The field menu is sorted by name. Common fields have friendly names: **Title**, **Artist**, **Album**, **Album artist**, **BPM**, **Key**, **Energy**, **Date added**, **Library modified**, **Bitrate (kbps)**, **Size (bytes)**, **Cue count**, **Tempo marker count**, **Any text field** and **Every text field**. Others appear under their raw names, such as `genre`, `year`, `duration`, `label`, `remixer`, `archived`, `incoming` and `playlist`. Both kinds work the same way.

Key, BPM and energy are CueRithm's own measurements, never values copied from file tags. The cue count comes from the cues stored for the track. See [Read a track's key, BPM and energy](../analysis/read-key-bpm-energy.md). A field marked *· unavailable* in the menu, for example a streaming-service field, has no local source and cannot be used.

| To find… | Field | Operator | Value |
|---|---|---|---|
| a genre family | `genre` | contains | `Melodic House` |
| an exact key | **Key** | is | `7A` |
| tracks that mix with a key | **Key** | is compatible with key | `7A` |
| a BPM window | **BPM** | is between | `124` and `128` |
| high-energy tracks | **Energy** | is at least | `7` |
| new arrivals | **Date added** | is in the last | `7` `days` |
| a mix name in the title | **Title** | ends with | `(Original Mix)` |
| tracks with no genre | `genre` | has no value | none |
| tracks in a set | `playlist` | is in playlist | **Warm-up set** |
| tracks with a missing file | `CustomFileMissing` | is true | none |

The last row checks the disk once a minute; see [Find missing, moved or broken files](../files/library/find-missing-files.md).

## Know how each kind of rule compares { #operators }

### Text { #text-rules }

- Text is compared in lower case with spaces trimmed from the track's value. Spaces you type in the value count.
- **contains**, **starts with** and **ends with** need a value. With an empty value they match nothing, and their "does not" versions match everything.
- **is** compares the whole value. For keys, use **is**: `Key contains 1A` also matches `11A` and `12A`, because it matches any part of the text.
- **Any text field** is true when any text field matches. **Every text field** needs all of them to match.
- **Artist or remixer credit** and **Individual genre** split a tag into separate names, so **is** matches one name out of several.

### Numbers { #number-rules }

!!! warning "Gotcha"
    A track with no value for a number field counts as 0. **BPM is less than 126** also matches a track that has no BPM, and **Energy is at most 6** matches a track that has no energy rating. Add **Energy has a value**, or use **is between**, which ignores tracks with no value.

- **is between** includes both ends and accepts them in either order.
- BPM is rounded to two decimals before it is compared.
- Energy is one whole-track rating from 1 to 10 ([How is energy rated?](../how-it-works/energy.md)).
- **is a whole BPM** is true for a BPM such as 128, and **is not a whole BPM** for 127.4.

### Dates { #date-rules }

- Date rules compare calendar days in your library time zone. Set it under **Library preferences**; see [Library preferences](../reference/settings/library.md). While you pick a date the editor shows *Resolving calendar date…*.
- **is on or after** includes the chosen day. **is before** excludes it. **is between dates** includes both days.
- **is in the last** takes a number and **days**, **weeks** or **months**, counted back from now. CueRithm recalculates these rules at least once a minute.
- **is not on** also matches tracks that have no date.

### Keys { #key-rules }

**is compatible with key** takes a Camelot key from `1A` to `12B`. Which neighbours count depends on **Key compatibility** in Library preferences:

| Mode | Keys that match `7A` |
|---|---|
| **Harmonic** (default) | `7A`, its relative `7B`, and the same-letter neighbours `6A` and `8A` |
| **Fuzzy** | the same or neighbouring number on either letter: `6A`, `6B`, `7A`, `7B`, `8A`, `8B` |

The wheel wraps, so `12A` neighbours `1A`. See [Understand Camelot keys](../analysis/camelot-keys.md). Changing the mode re-evaluates every key rule.

### Other playlists { #playlist-rules }

The `playlist` field has six operators: **is in playlist**, **is not in playlist**, **is in playlist folder**, **is not in playlist folder**, **is in smart playlist** and **is not in smart playlist**. Pick the playlist from the menu that follows. A folder counts every track in the playlists inside it.

A smart playlist cannot depend on itself, directly or through a folder that contains it: CueRithm refuses it with *Smart playlist dependency cycle*. Keep a rule that refers to a folder outside that folder. If you remove a playlist a rule uses, the smart playlist reports an error until you restore it.

### Tags and cues { #tags-and-cues }

- **Tags** and **categories** come from [Group tracks with categories and custom tags](../tagging/curate-tracks.md). Tick the tags or categories to use. Choosing none matches nothing.
- Cue rules read the cues on the track: **any cue label contains**, **no cue label contains**, **any cue has color**, **has a loop**, **has an active loop**, **has a memory cue** and their opposites. Cue names are listed in [Cue names, letters and colours](../reference/cue-types.md).

## Match with a regular expression { #regular-expressions }

Text fields offer **matches regular expression** and **does not match regular expression**. Enter `/pattern/flags` or a bare pattern. The flavour is JavaScript. It is case-sensitive unless you add the `i` flag.

```text
/\(original mix\)$/i
```

That finds titles ending in *(Original Mix)* in any case: five tracks in the purchase folder, including *Desert (Original Mix)* and *Undercurrent (Original Mix)*. The allowed flags are `d g i m s u v y`; repeats are errors, as is `u` together with `v`. A pattern holds at most 100 characters and a tested value at most 1 MiB. A pattern that takes too long to run is an error, never a silent non-match.

## Include archived or Incoming tracks { #archived-and-incoming }

A smart playlist leaves out archived tracks unless one condition is `archived` **is true**, and leaves out tracks still in Incoming unless one is `incoming` **is true**. Everything else comes from **Tracks**.

## Preview, save and refresh { #preview-and-refresh }

**Preview matches** evaluates the rules and shows the count; **Create** and **Save changes** stay unavailable until it succeeds. A preview for a rule that depends on the clock or on files expires after about a minute with *Preview expired. Preview matches again before saving.* Click **Preview matches** again. After saving, the list refreshes itself; [Build a smart playlist](build-a-smart-playlist.md#it-stays-current) shows what the dot and the banner mean.

## What can go wrong

| What you see | Why | What to do |
|---|---|---|
| The playlist is empty | A text value is empty, a key or tag does not exist, or the tracks are archived or in Incoming | Preview again and check each value; see [Include archived or Incoming tracks](#archived-and-incoming) |
| Tracks with no BPM or energy appear | A missing number counts as 0 | Add **has a value**, or use **is between** |
| `Key contains 1A` also finds `11A` | **contains** matches part of the text | Use **is** |
| A date rule is a day off | The library time zone differs from yours | Set the zone in [Library preferences](../reference/settings/library.md) |
| *Smart playlist dependency cycle* | A rule depends on a playlist that depends on this one | Move the playlist outside the folder it refers to |
| A field is greyed out | It has no local source | Choose another field |

## Related

- [Build a smart playlist](build-a-smart-playlist.md)
- [Playlist recipes](recipes/index.md)
- [Smart playlist fields and operators](../reference/smart-playlist-fields.md)
- [How do smart playlists stay current?](../how-it-works/smart-engine.md)

<!-- verified: apps/desktop/src/smart-rules.ts (operator labels, allowedOperators, initialRuleValue); components/SmartRuleEditor.tsx (Match, OR group, 128 limit, hint text, key select 1A-12B); views/PlaylistEditor.tsx; crates/cuerithm-smartlists/src/lib.rs (matches: empty list, archived, OR semantics, validate_shape), text.rs (trim, lower case, empty-query rules, credit fields), numeric.rs (missing=0, between needs a value, BPM two decimals, whole BPM), dates.rs, keys.rs (harmonic/fuzzy), membership.rs, tags.rs, cues.rs, model.rs (field ids); crates/cuerithm-regex/src/lib.rs (flags, 100 units, 1 MiB, budget); crates/cuerithm-library/src/playlists/snapshot.rs (field labels, unavailable fields, dynamic rules), tree.rs (dependency cycle), evaluate.rs (allows_archive, allows_incoming); counts computed from _meta/the private demo-library facts sheet; legacy guides/smart-rules.md re-checked (its "Energy 0 to 10" corrected to 1 to 10) -->
