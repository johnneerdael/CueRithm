# Merge similar genres into one

<div class="recipe-meta" markdown>
**Uses:** Cleanup, **Genres** tab: mappings.  
**Works on:** The Genre tag of Ready tracks.  
**Time:** About 5 minutes.  
**Undo:** **Undo history** in Cleanup restores the batch: [Undo a cleanup](../undo-cleanup.md).
</div>

## The problem

A Beatport purchase folder arrives with long sub-genre names that you would write differently, and two spellings of the same style can sit side by side. In the demo library, one track is tagged `140 / Deep Dubstep / Grime` and another `Dubstep`. A genre column, a genre filter and a `{genre}` folder name all treat them as different genres.

| Genre before | Genre after |
|---|---|
| `140 / Deep Dubstep / Grime` | `Dubstep` |
| `Dubstep` | `Dubstep` (unchanged) |
| `Techno (Peak Time / Driving)` | `Techno` |
| `Electro (Classic / Detroit / Modern)` | `Electro` |
| `Melodic House & Techno` | unchanged |

## The solution

1. Open **Cleanup** and choose the **Genres** tab. Tick **Enable genre cleanup**.
2. In **Search genres**, type part of a name. Your library's genres are listed with their track counts.
3. On each row you want to change, type the new spelling in the box that says **Keep existing** (**Replace … with**). Do this for the three rows below. For a value that is not in your library yet, press **Add mapping** and fill **Exact source value** and **Canonical value**.
4. Press **Save cleanup rules**, set the **Scope**, and press **Scan tracks**.
5. Check each card. Only `genre` should change. Press **Apply N changes**.

The mappings, in full:

```text
Enable genre cleanup: on
140 / Deep Dubstep / Grime          ->  Dubstep
Techno (Peak Time / Driving)        ->  Techno
Electro (Classic / Detroit / Modern) ->  Electro
```

## How it works

A mapping matches the whole genre value, exactly and case sensitively. `dubstep` (lower case) is a different value from `Dubstep`, so it needs a mapping of its own. Everything that matches is replaced by the canonical text.

- **Chains are followed.** If `Deep Dubstep` maps to `Dubstep` and `Dubstep` maps to `Bass Music`, a track tagged `Deep Dubstep` ends with `Bass Music`. A loop is refused when you save.
- **Duplicates collapse.** A track tagged with both `Dubstep` and `140 / Deep Dubstep / Grime` ends with one `Dubstep`.
- **Other genres are left alone**, and a genre on the **Protected genres** list is never changed, even if a mapping matches it.

The library list only shows genres from Ready tracks, so scan again after new music arrives.

!!! warning "Gotcha"
    Anything that tests the old text stops matching. A [smart playlist](../../playlists/smart-rules.md) whose rule says the genre contains `Deep Dubstep` will be empty after this run. The demo playlist *Melodic house 128+* is not affected here, because `Melodic House & Techno` is not mapped.

## Variations

- **Keep the detail.** If you still want `Techno (Peak Time / Driving)` somewhere, turn it into a category as you merge: [Keep Beatport's detailed genre as a category](keep-subgenres-as-tags.md).
- **Delete a genre.** Tick **Remove** on a mapping to delete the value instead of renaming it. A track whose only genre was removed ends up with an empty Genre.
- **Do it on arrival.** Follow [Run cleanup on every new track](../cleanup-automatically.md). The genre that cleanup produces is also the genre CueRithm uses when it places cues on that new track.
- **Clean folder names.** A genre containing `/` becomes `_` in a folder name. After this recipe, [Sort files into artist folders](../../rename/recipes/sort-files-into-artist-folders.md) works with `{genre}` too.

## Related

- [Write cleanup rules](../cleanup-rules.md#genres)
- [Keep Beatport's detailed genre as a category](keep-subgenres-as-tags.md)
- [Group tracks with categories and custom tags](../curate-tracks.md)
- [Tag and clean-up recipes](index.md)

<!-- verified: crates/cuerithm-cleanup/src/mappings.rs; crates/cuerithm-cleanup/tests/mappings.rs; examples run through the engine with demo genres from library-facts.md; crates/cuerithm-library/tests/cue_genre.rs (cleaned genre used for cues in preparation); apps/desktop/src/views/CleanupSettings.tsx (CanonicalEditor labels) -->
