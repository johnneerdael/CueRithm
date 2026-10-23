# Sort files into artist folders

<div class="recipe-meta" markdown>
**Uses:** Organize files with the pattern `{artist}/{artist} - {title}` and **Copy to folder** (or **Move to folder**).  
**Works on:** Ready tracks you select, a playlist, a folder or all ready tracks.  
**Time:** About 10 minutes.  
**Undo:** **History and recovery** on the same page: [Recover or undo a reorganisation](../undo-organize.md).
</div>

!!! warning "This changes files on disk"
    **Copy to folder** creates new files and leaves your originals alone. **Move to folder** moves them and keeps a hidden recovery copy of each original. Preview first, and read [Rename and sort files into folders](../organize-files.md) before you move a whole library.

## The problem

A purchase folder is a flat pile of files. You want one folder per artist, named the way the tags say, with files that still read `Artist - Title`. The demo library gives these results (the extension `.flac` stays):

| Tag artist | Tag title | New path under the destination |
|---|---|---|
| Adriatique | Palm Of Your Hands (Extended Mix) | `Adriatique/Adriatique - Palm Of Your Hands (Extended Mix).flac` |
| SOLANCE | Knockout (Extended Mix) | `SOLANCE/SOLANCE - Knockout (Extended Mix).flac` |
| Jast | Dancin (Extended Mix) | `Jast/Jast - Dancin (Extended Mix).flac` |
| Colyn | When The Fire Starts To Burn (Extended Mix) | `Colyn/Colyn - When The Fire Starts To Burn (Extended Mix).flac` |
| SLVR | Say Less (Extended Mix) | `SLVR/SLVR - Say Less (Extended Mix).flac` |
| Random Factor | Undercurrent (Original Mix) | `Random Factor/Random Factor - Undercurrent (Original Mix).flac` |

## The solution

1. Select the tracks in **Tracks** and press **Organize files** (or choose it under **TOOLS**). Check that **Scope** shows what you expect.
2. In **Naming rules**, set the mode menu to **Copy to folder** and press **Choose destination**. Pick a folder outside CueRithm's own data, for example `~/Music/DJ Library`.
3. Replace **Filename pattern** with the line below. Check **Example filenames**.
4. Press **Preview complete plan**. Read the paths, open any **blocked** row and fix its cause.
5. Press **Apply N eligible**.

```text
Mode: Copy to folder
Destination: ~/Music/DJ Library
Filename pattern: {artist}/{artist} - {title}
Missing values: Needs attention
If a filename already exists: Needs attention
```

## How it works

`{artist}` is the track's first artist value; the `/` starts a folder; the extension is added for you. Folders are created as needed. Characters that file systems refuse are replaced by `_` in the folder name as well: an artist called `AC/DC` goes into `AC_DC`. A track with no artist tag is blocked with "Missing naming field: artist" and stays where it is. Existing files are never overwritten.

Tracks with several artists land in the folder of the first one when the file stores each artist as its own value. If a file stores the whole credit as one value, that text becomes the folder name, for example `Final Request, Skuro (IT)`; the preview shows exactly what will happen. To control this, fix the credit first with [Rename an artist everywhere](../../tagging/recipes/rename-an-artist-everywhere.md) or edit the tag by hand.

## Variations

- **Move instead of copy.** Choose **Move to folder**. The library follows the file, linked DJ libraries that CueRithm knows about are repaired, and the old file stays as a hidden `.cuerithm-…original` in its old folder until you remove it.
- **Rename only.** Use **Rename in place** with `{artist} - {title}`. A pattern with `/` is refused in this mode.
- **Sort by label or genre.** Use `{label}/{artist} - {title}` or `{genre}/{artist} - {title}`. Genres such as `140 / Deep Dubstep / Grime` become `140 _ Deep Dubstep _ Grime`, so [merge similar genres](../../tagging/recipes/merge-similar-genres.md) first.
- **Do it for every new track.** Save the same rules in **Settings → File organization**: [Rename and sort files into folders](../organize-files.md#make-it-automatic).

## Related

- [Build a naming pattern](../naming-patterns.md)
- [Recover or undo a reorganisation](../undo-organize.md)
- [Put key and energy in file names](put-key-and-energy-in-file-names.md)
- [Tag and clean-up recipes](../../tagging/recipes/index.md)

<!-- verified: crates/cuerithm-organize/src/template.rs and plan.rs (rendering, missing policy, rename-in-place with slash); example paths run through the template engine with demo tags from the private demo-library facts sheet; apps/desktop/src/views/Organize.tsx and OrganizationSettings.tsx (labels); crates/cuerithm-library/src/organization/coordinator.rs (hidden .original names). UNVERIFIED: whether the demo files store multi-artist credits as separate values or one joined value. -->
