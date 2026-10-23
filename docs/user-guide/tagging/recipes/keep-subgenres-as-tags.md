# Keep Beatport's detailed genre as a category

<div class="recipe-meta" markdown>
**Uses:** Cleanup, **Genres** tab: mappings and **Convert genres into categorized tags**.  
**Works on:** The Genre tag of Ready tracks, plus a category in your library.  
**Time:** About 10 minutes.  
**Undo:** **Undo history** in Cleanup restores the genre tag and takes the converted choices off the tracks again; the category itself stays in Curate: [Undo a cleanup](../undo-cleanup.md).
</div>

## The problem

You want a short, consistent Genre tag such as `Techno`, but Beatport's detailed name, `Techno (Peak Time / Driving)`, is useful when you pick music. Replacing the genre would lose it. Converting it into a category keeps both: the short name stays in the file, and the detail becomes a choice you can filter and use in smart playlists.

| Track | Genre before | Genre after | New category choice |
|---|---|---|---|
| Dancin (Extended Mix) | `Techno (Peak Time / Driving)` | `Techno` | **Subgenres**: `Techno (Peak Time / Driving)` |
| Undercurrent (Original Mix) | `Electro (Classic / Detroit / Modern)` | `Electro` | **Subgenres**: `Electro (Classic / Detroit / Modern)` |
| Palm Of Your Hands (Extended Mix) | `Melodic House & Techno` | unchanged | none |

## The solution

1. Open **Cleanup** and the **Genres** tab. Tick **Enable genre cleanup**.
2. Add the two mappings from the table. For each, type the short name in the box on the genre's row, or use **Add mapping**.
3. Tick **Convert genres into categorized tags**. Leave **Category** as `Subgenres`, or type another name. Under **Genres to convert, one per line**, enter the two long names. Leave **Remove converted values from Genre** unticked.
4. Press **Save cleanup rules**, then **Scan tracks**.
5. In each card, check the genre line and a **Custom tags** line that goes from **Empty** to `Subgenres: Techno (Peak Time / Driving)`. Press **Apply N changes**.

The settings, in full:

```text
Enable genre cleanup: on
Mappings:
  Techno (Peak Time / Driving)         ->  Techno
  Electro (Classic / Detroit / Modern) ->  Electro
Convert genres into categorized tags: on
  Category: Subgenres
  Genres to convert, one per line:
    Techno (Peak Time / Driving)
    Electro (Classic / Detroit / Modern)
  Remove converted values from Genre: off
```

## How it works

For every genre value on the list, CueRithm adds a choice with the same text to the category, then applies the mappings to the genre as usual. The category and the choice are created on the first run if they do not exist yet, as a category that allows **Multiple choices**. Categories live in your library and are not written into the file's tags, so a run that only adds choices does not rewrite any file.

Open [Curate](../curate-tracks.md) on a converted track and you find **Subgenres** with the choice ticked. From there you can use **Category filters** in the table, build [smart playlist](../../playlists/smart-rules.md) rules on the category, or [map the category into a file tag](../curate-tracks.md#optional-native-mappings) when you do want it in the file.

## Variations

- **Drop the long genre from the tag instead of mapping it.** Tick **Remove converted values from Genre** and remove the mapping. A track whose only genre was converted then has an empty Genre.
- **Convert without mapping.** Leave the mappings out: the genre stays as it is and the detail is also stored as a choice.
- **Choose the category name.** Any name up to 256 bytes works. Use a name that is not a **One choice** category, or a track that gets several converted genres stops the run with "multiple values for a single-select Curate category".
- **Do it on arrival.** [Run cleanup on every new track](../cleanup-automatically.md) applies conversions to each new track as well.

## Related

- [Merge similar genres into one](merge-similar-genres.md)
- [Write cleanup rules](../cleanup-rules.md#genres)
- [Group tracks with categories and custom tags](../curate-tracks.md)
- [Tag and clean-up recipes](index.md)

<!-- verified: crates/cuerithm-cleanup/src/mappings.rs (conversion before mapping); crates/cuerithm-cleanup/tests/mappings.rs (genres_map_deduplicate_and_keep_selected_original_detail_as_tags, conversion_can_remove_a_genre_without_losing_its_custom_tag); crates/cuerithm-library/src/cleanup/tests.rs (custom_tag_only_conversion_keeps_file_bytes_and_undo_preserves_prior_categories); crates/cuerithm-library/src/curation/aliases.rs (category created on demand); apps/desktop/src/views/Cleanup.tsx (Custom tags row); examples run through the engine -->
