# Tag and clean-up recipes

<p class="lead">Ten worked fixes for the problems a Beatport or record-pool folder really has. Each recipe lists the exact settings, shows a before and after on real or clearly marked placeholder tracks, and says how to undo it.</p>

Every cleanup recipe works the same way: you add or change a rule in **Cleanup**, press **Save cleanup rules**, scan, read the preview and only then press **Apply**. Nothing is written until you apply, and the **Undo history** can reverse the whole batch. The two file recipes use **Organize files**, which previews a complete plan and keeps your originals. If this is your first time, do [Clean up messy titles, genres and artists](../cleanup-messy-titles.md) or [Rename and sort files into folders](../../rename/organize-files.md) first.

!!! tip "Try a recipe on a few tracks first"
    Select three or four tracks before you open **Cleanup** or **Organize files**, so **Scope** reads **N selected tracks**. Switch to **All ready tracks** only when the preview looks right.

## Tidy titles { #titles }

| Recipe | Before → after |
|---|---|
| [Remove "(Original Mix)" from titles](remove-original-mix-suffix.md) | `Brackish (Original Mix)` → `Brackish` |
| [Fix capital letters without breaking artist names](fix-capital-letters.md) | `SIGNAL AND SOUND` → `Signal And Sound`, and `SOLANCE` stays `SOLANCE` |
| [Strip the key and BPM from titles](strip-key-and-bpm-from-titles.md) | `8A - 7 - Signal 128` → `Signal` |
| [Remove promo text and web addresses](remove-promo-text-and-urls.md) | `Signal [FREE DOWNLOAD]` → `Signal` |

## Fix genres { #genres }

| Recipe | Before → after |
|---|---|
| [Merge similar genres into one](merge-similar-genres.md) | `140 / Deep Dubstep / Grime` and `Dubstep` → `Dubstep` |
| [Keep Beatport's detailed genre as a category](keep-subgenres-as-tags.md) | `Techno (Peak Time / Driving)` → Genre `Techno` plus the category choice **Subgenres**: `Techno (Peak Time / Driving)` |

## Fix artists { #artists }

| Recipe | Before → after |
|---|---|
| [Rename an artist everywhere](rename-an-artist-everywhere.md) | `Anyma (ofc)` → `Anyma` |
| [Fill in a missing artist from the title](fill-a-missing-artist-from-the-title.md) | Title `Example Artist - Example Title`, Artist empty → Artist `Example Artist`, Title `Example Title` |

## Name and sort files { #files }

| Recipe | Before → after |
|---|---|
| [Sort files into artist folders](../../rename/recipes/sort-files-into-artist-folders.md) | A flat folder → `Adriatique/Adriatique - Palm Of Your Hands (Extended Mix).flac` |
| [Put key and energy in file names](../../rename/recipes/put-key-and-energy-in-file-names.md) | `Palm Of Your Hands (Extended Mix)` → `11A - 6 - Adriatique - Palm Of Your Hands (Extended Mix).flac` |

## Not covered by a built-in rule

Some fixes people ask for cannot be done with cleanup, because the rule does not exist:

- **Removing "(Extended Mix)".** **Common title text** has four patterns only: Original Mix suffix, Camelot key, Camelot key and energy and BPM suffix.
- **Splitting a joined artist credit** such as `Stylo, Anyma (ofc)` into two artists. A mapping replaces a whole value; it cannot split one. [Rename an artist everywhere](rename-an-artist-everywhere.md) explains the workaround for joined values.
- **Changing album artist with a mapping.** Artist cleanup covers artist, remixer, producer, composer and lyricist. A Smart Fix with a **Field** menu can still change the album artist's text.

## Related

- [Write cleanup rules](../cleanup-rules.md)
- [Build a naming pattern](../../rename/naming-patterns.md)
- [Run cleanup on every new track](../cleanup-automatically.md)
- [Tag and clean up your music](../index.md)

<!-- verified: each linked recipe, run through crates/cuerithm-cleanup and crates/cuerithm-organize with the examples shown; crates/cuerithm-cleanup/src/model.rs (CommonTextPattern has four variants; ContributorRole has five) -->
