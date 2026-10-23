# Put key and energy in file names

<div class="recipe-meta" markdown>
**Uses:** Organize files with `{key}`, `{energy}` and **Rename in place**.  
**Works on:** Ready tracks you select, a playlist, a folder or all ready tracks.  
**Time:** About 5 minutes.  
**Undo:** **History and recovery** on the same page: [Recover or undo a reorganisation](../undo-organize.md).
</div>

!!! warning "This renames files on disk"
    **Rename in place** changes file names and keeps a hidden recovery copy of each original beside it. Preview first, and try a few tracks before a whole library.

## The problem

In a file manager, a USB stick or a player that shows only file names, you cannot see a track's key or energy. You would like the name to start with them so a sort by name groups tracks that mix: `11A - 6 - Adriatique - Palm Of Your Hands (Extended Mix).flac`.

The key and energy come from CueRithm's analysis, which is why they exist even if no tag holds them:

| Track | Key | Energy | New file name |
|---|---|---|---|
| Palm Of Your Hands (Extended Mix) by Adriatique | 11A | 6/10 | `11A - 6 - Adriatique - Palm Of Your Hands (Extended Mix).flac` |
| Undercurrent (Original Mix) by Random Factor | 9A | 5/10 | `9A - 5 - Random Factor - Undercurrent (Original Mix).flac` |
| When The Fire Starts To Burn (Extended Mix) by Colyn | 7A | 6/10 | `7A - 6 - Colyn - When The Fire Starts To Burn (Extended Mix).flac` |

## The solution

1. Select the tracks in **Tracks** and press **Organize files**.
2. In **Naming rules**, set the mode menu to **Rename in place**.
3. Replace **Filename pattern** with the line below, then check **Example filenames**.
4. Press **Preview complete plan** and read the new names.
5. Press **Apply N eligible**.

```text
Mode: Rename in place
Filename pattern: {key} - {energy} - {artist} - {title}
Missing values: Needs attention
```

## How it works

`{key}` is the Camelot key CueRithm measured, `{energy}` is its single whole-track energy rating from 1 to 10, and `{artist}` and `{title}` come from the tags. Energy is one number for the whole track; there is no per-section energy. `{bpm}` is also available and always has two decimals, such as `127.00`.

Because key and energy are measurements, they follow the analysis. If you [analyse a track again](../../analysis/analyze-again.md) and the result changes, organise it again to rename it. The numbers in the file name never feed back into the analysis.

A track that lacks a value the pattern needs is blocked with a reason such as "Missing naming field: key" rather than named without it.

## Variations

- **Make the key optional.** Use `[{key} - ]{artist} - {title}` and set **Missing values** to **Omit optional part**. Tracks with no key then keep the plain name. Brackets only work with that setting; with **Needs attention** a missing tag inside brackets still blocks the track.
- **Add the tempo.** `{key} - {bpm} - {artist} - {title}` writes the saved BPM with two decimals, so a track saved at exactly 127 BPM becomes `11A - 127.00 - Adriatique - Palm Of Your Hands (Extended Mix).flac`.
- **Folders by key.** Use **Move to folder** or **Copy to folder** with `{key}/{artist} - {title}` to get a folder per key.
- **Change case.** `{artist|upper}` writes the artist in capitals; [Build a naming pattern](../naming-patterns.md#filters) lists the filters.

## Related

- [Build a naming pattern](../naming-patterns.md#tokens)
- [Understand Camelot keys](../../analysis/camelot-keys.md)
- [Sort files into artist folders](sort-files-into-artist-folders.md)
- [Tag and clean-up recipes](../../tagging/recipes/index.md)

<!-- verified: crates/cuerithm-organize/src/template.rs; crates/cuerithm-library/src/organization/service/capture.rs (key = tracks.musical_key Camelot, energy, bpm_hundredths); crates/cuerithm-library/src/jobs.rs (musical_key = key.camelot); example names rendered by the template engine with key and energy from library-facts.md (#17 11A 6/10, #22 9A 5/10, #26 7A 6/10); the 127.00 shape is the token format, not a claim about the stored BPM of that track -->
