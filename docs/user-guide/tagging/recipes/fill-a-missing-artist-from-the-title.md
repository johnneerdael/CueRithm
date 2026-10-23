# Fill in a missing artist from the title

<div class="recipe-meta" markdown>
**Uses:** Cleanup, Smart Fixes, **Extract artist** (and optionally **Characters to spaces**).  
**Works on:** Tracks whose Artist tag is empty and whose title reads `Artist - Title`.  
**Time:** About 5 minutes.  
**Undo:** **Undo history** in Cleanup restores the batch: [Undo a cleanup](../undo-cleanup.md).
</div>

## The problem

Files from some sources have the artist inside the title and nothing in the Artist tag. That leaves a blank Artist column, no artist for [file names and folders](../../rename/recipes/sort-files-into-artist-folders.md), and a title that repeats the artist. The demo library is fully tagged, so the table uses placeholder names.

| Title before | Artist before | Title after | Artist after |
|---|---|---|---|
| `Example Artist - Example Title (Extended Mix)` | (empty) | `Example Title (Extended Mix)` | `Example Artist` |
| `Example Artist – Example Title` (en dash) | (empty) | `Example Title` | `Example Artist` |
| `Example Artist - Example Title` | `Existing Artist` | unchanged | unchanged |
| `Example Title (Extended Mix)` | (empty) | unchanged | unchanged |

## The solution

1. In **Cleanup**, on **Smart Fixes**, add **Extract artist**.
2. Leave **Separator** as ` - ` (a space, a hyphen, a space) and **Result number** as `1`.
3. Press **Save cleanup rules**, set the **Scope** and press **Scan tracks**.
4. Each card should show two lines, `title` and `artist`. Check the split, then press **Apply N changes**.

```text
Smart Fix: Extract artist
  Separator:     " - "
  Result number: 1
```

## How it works

The fix acts only on a track that has no artist at all. That protects your tags: the third row above is not touched, and neither is a track such as the demo library's *PLV* by Rabteu and Nivk, which already has its artists. The fix splits the title at every separator. With the default ` - `, a spaced en dash ( – ) counts as a separator too; an em dash does not.

**Result number** says which piece is the artist. The remaining pieces form the title: those after the artist, or those before it when the artist is the last piece. A title with fewer pieces than the number is left alone.

| Title | Result number | Artist | Title after |
|---|---|---|---|
| `Example Artist - Example Title` | 1 | `Example Artist` | `Example Title` |
| `Example Title - Example Artist` | 2 | `Example Artist` | `Example Title` |
| `Prefix - Example Artist - Example Title` | 2 | `Example Artist` | `Example Title` |
| `A - B - C` | 1 | `A` | `B - C` |

## Variations

- **Underscored file names.** Titles like `Example_Artist_-_Example_Title` need **Characters to spaces** (`_`) placed above **Extract artist**; the order matters. See [Write cleanup rules](../cleanup-rules.md#order-of-application).
- **A different separator.** Type it in **Separator**, for example ` ~ `. It can be up to 64 characters.
- **Several artists in the title.** The piece becomes one artist value, for example `Nivk, Rabteu`. The Artists tab cannot split it.
- **The remixer too.** Add **Extract remixer** to move `(Clara Remix)` into Remixer; see [Write cleanup rules](../cleanup-rules.md#fix-extract-remixer).

## Related

- [Write cleanup rules](../cleanup-rules.md#fix-extract-artist)
- [Edit tags and artwork](../edit-tags-by-hand.md)
- [Sort files into artist folders](../../rename/recipes/sort-files-into-artist-folders.md)
- [Tag and clean-up recipes](index.md)

<!-- verified: crates/cuerithm-cleanup/src/fixes/extract.rs (artist), fixes/mod.rs (acts only when Artist empty); crates/cuerithm-cleanup/tests/fixtures/lexicon-v1.json extract_artist; every table row run through the engine, including the order with Characters to spaces -->
