# Rename an artist everywhere

<div class="recipe-meta" markdown>
**Uses:** Cleanup, **Artists** tab: mappings and roles.  
**Works on:** The artist, remixer, producer, composer and lyricist tags of Ready tracks.  
**Time:** About 5 minutes.  
**Undo:** **Undo history** in Cleanup restores the batch: [Undo a cleanup](../undo-cleanup.md).
</div>

## The problem

One act appears under two spellings, or with a disambiguation suffix you do not want. Beatport writes the producer Anyma as `Anyma (ofc)`, and four tracks in the demo library carry it: *Atoma*, *No Good*, *Other Dimension* and *Prophecy*. You want plain `Anyma`, so that sorting, searching and artist folders put the tracks together.

| Track | Artist column before | After |
|---|---|---|
| Atoma (Extended Mix) | `Anyma (ofc)` | `Anyma` |
| No Good (Extended Mix) | `Stylo, Anyma (ofc)` | `Stylo, Anyma` |
| Other Dimension (Extended Mix) | `Volkoder, Anyma (ofc)` | `Volkoder, Anyma` |
| Prophecy (Extended Mix) | `Baset, Anyma (ofc), 19:26` | `Baset, Anyma, 19:26` |

The last three rows hold only when the files store each artist as a value of its own, which the **Artists** tab shows you in step 2.

## The solution

<!-- figure:cleanup-artists-search-anyma -->
<figure markdown>
![Artists tab with the search box showing Anyma and the row for the value Anyma (ofc) with its track count and role counts, and the Replace box holding Anyma](../../assets/screens/cleanup-artists-search-anyma-light.webp#only-light){ .screen loading=lazy }
![Artists tab with the search box showing Anyma and the row for the value Anyma (ofc) with its track count and role counts, and the Replace box holding Anyma](../../assets/screens/cleanup-artists-search-anyma-dark.webp#only-dark){ .screen loading=lazy }
<figcaption>The list shows each stored value with how many tracks carry it.</figcaption>
</figure>
<!-- /figure -->

1. Open **Cleanup** and the **Artists** tab. Tick **Enable artist cleanup**. Leave **artist** ticked; also tick **remixer** or **producer** if the name appears there.
2. In **Search artists**, type `Anyma`. The list shows each stored value with its track count and role counts, such as "artist: 4". If you see a row `Anyma (ofc)`, each artist is its own value. If you only see joined rows such as `Stylo, Anyma (ofc)`, the credit is stored as one value; see the note below.
3. On the `Anyma (ofc)` row, type `Anyma` in the box that says **Keep existing**.
4. Press **Save cleanup rules**, set the **Scope** and press **Scan tracks**.
5. Check each card: only the artist line changes, and other artists on the track stay. Press **Apply N changes**.

```text
Enable artist cleanup: on
Roles: artist
Anyma (ofc)  ->  Anyma
```

## How it works

An artist mapping replaces a whole value in the roles you ticked, and leaves other values on the same track alone: `Stylo` and `19:26` stay as they are. The match is exact and case sensitive. If two values on one track end up the same, one is dropped.

- **Joined credits.** If a file stores `Stylo, Anyma (ofc)` as a single value, mapping `Anyma (ofc)` does not touch it, because the value is longer than the mapping's source. Map the whole text instead, for example `Stylo, Anyma (ofc)` to `Stylo, Anyma`. CueRithm cannot split a joined value into separate artists.
- **Roles.** Only the roles you ticked are changed. Album artist is not one of them; correct it with [Edit tags and artwork](../edit-tags-by-hand.md).
- **Protected artists.** A name on **Protected artists, one per line** is never changed by a mapping, even if a mapping matches.

## Variations

- **Delete a credit.** Tick **Remove** on a mapping to delete the value from the ticked roles. `Various Artists` removed from a track that also lists `Jast` leaves `Jast`.
- **Rename in files too.** The artist tag is rewritten in each file. If you later sort files by artist, preview again with [Sort files into artist folders](../../rename/recipes/sort-files-into-artist-folders.md); `{artist}` uses the new name.
- **Keep the old spelling for one track.** Edit that track by hand afterwards, or add the old spelling to the protected list before you run the cleanup.

## Related

- [Write cleanup rules](../cleanup-rules.md#artists)
- [Fix capital letters without breaking artist names](fix-capital-letters.md)
- [Merge similar genres into one](merge-similar-genres.md)
- [Tag and clean-up recipes](index.md)

<!-- verified: crates/cuerithm-cleanup/src/mappings.rs (artists branch); crates/cuerithm-cleanup/tests/mappings.rs (aliases_preserve_multiple_contributors_and_only_selected_roles); apps/desktop/src/views/CleanupSettings.tsx (CanonicalEditor: roles, Search artists, Replace ... with, Protect); apps/desktop/src/test/cleanup.test.tsx (role counts); every row run through the engine with the demo artist values. UNVERIFIED: whether the demo FLAC files store the artists as separate values or as one joined value; the recipe tells the reader how to check. -->
