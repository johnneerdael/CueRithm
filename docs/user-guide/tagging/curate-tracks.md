# Group tracks with categories and custom tags

<p class="lead">Make your own categories such as Mood or Set slot, tick choices on one track or fifty, add a star rating and a note, then filter the table by them. The first category takes about five minutes.</p>

<!-- figure:existing-89-curate-light-curate-tracks -->
<figure markdown>
![Curate with categories and choices for the selected tracks](../assets/screens/89-curate-light.png#only-light){ .screen loading=lazy }
![Curate with categories and choices for the selected tracks](../assets/screens/89-curate-dark.png#only-dark){ .screen loading=lazy }
<figcaption>Group tracks with categories you define.</figcaption>
</figure>
<!-- /figure -->

Categories are your labels. They live in the CueRithm library, next to the track, and by default they are never written into the audio file. Key, BPM and energy stay CueRithm's own measurements: Curate shows them read-only so you can see them while you decide, and your star rating is a separate number.

The examples below use two categories you create yourself: **Mood** (several choices allowed: Warm, Dark, Uplifting) and **Set slot** (one choice only: Warm-up, Peak, Closing).

## Open Curate { #open-curate }

Select one or more tracks in **Tracks** and press **Curate** in the table toolbar, or open a track's row menu and choose **Curate**. A **Curate** panel opens beside the table. It keeps the [player](../analysis/preview-a-track.md) available, so you can listen while you label.

With one track selected, the panel shows a **Measured analysis** block (key, BPM, energy; read-only), your categories, **Your rating** and **Notes**. With several tracks selected, the title reads "N selected tracks".

## Create categories and choices { #create-categories }

1. In the Curate panel, press **Manage categories and mappings**.
2. Under **New category**, type `Mood`, leave **Selection** on **Multiple choices** and press **Add category**.
3. With `Mood` chosen in **Category to edit**, type `Warm` in **New choice** and press **Add choice**. Repeat for `Dark` and `Uplifting`.
4. Add `Set slot` with **Selection** set to **One choice**, and give it the choices `Warm-up`, `Peak` and `Closing`.
5. Press **Back to curation**.

A **One choice** category replaces its previous choice when you pick another. A **Multiple choices** category lets you tick as many as you like.

[Cleanup](cleanup-rules.md#genres) can also create categories for you: its genre conversion puts a detail such as `Techno (Peak Time / Driving)` into a category called `Subgenres` (or any name you choose). Those categories appear here and can be used like your own.

## Tick choices, rate and add notes { #categories-and-selections }

Each category is a group of buttons labelled **Choose any** or **Choose one**. Click a choice to tick it and click again to clear it. In **Your rating**, click a star from 1 to 5, or **Clear rating**. Type a listening note in **Notes**, for example "Long blend, vocal after the break".

You can also work from the keyboard while the Curate panel has focus: the digits ++1++ to ++9++ toggle the first nine choices in the order the panel lists them (the number is shown on the button), and `[` and `]` move to the previous and next track in the table. Typing in the notes box never triggers these keys.

Your star rating is separate from the track's **Energy** and from any rating already stored in the file.

## Label many tracks at once { #many-tracks }

Select several tracks and open Curate. A choice that only some of them have shows a half-filled state ("Some selected tracks · apply to all"). Clicking it applies the choice to every selected track and leaves their other categories alone; clicking a ticked choice removes it from all. In a **One choice** category, ticking a choice sets it on all.

The **Notes action** menu protects existing notes: **Keep each track's notes** (the default), **Set notes for all selected** or **Clear notes for all selected**. With a single track selected, **Previous track** and **Next track** step through the table order, with a counter such as "3 / 26".

## Find curated tracks { #find-curated-tracks }

- **Category filters** appears in the table toolbar once at least one category exists. Choose **Match**: **All selected choices**, **Any selected choice** or **Exclude selected choices**. Tick choices, optionally tick **No choices in** a category, set a **Minimum rating** and **Maximum rating**, or type text in **Notes contain**. Press **Apply category filters**; **Clear category filters** removes them. The button shows how many filters are active.
- The columns **Categories**, **User rating** and **Curation notes** can be shown in the table: [Show the columns you need](../analysis/choose-columns.md).
- Smart playlist rules can test for a choice, for any of several choices, or for any choice in a category. See [Write smart playlist rules](../playlists/smart-rules.md).

The track you edited stays in the list, even if its new choices no longer match the filter, so you do not lose your place.

## Rename, merge or delete categories safely { #manage-categories }

Open **Manage categories and mappings** and choose the category in **Category to edit**.

- **Rename category** and **Rename choice** change the name only. Filters and smart playlists follow the new name because they use a stable ID.
- **Selection mode** switches between **Multiple choices** and **One choice**. Switching to **One choice** is refused while any track has more than one choice in that category.
- **Delete** a choice or **Delete category** opens a confirmation with a **Replacement** menu. Choose another choice in the same category (or **No replacement**) and press **Confirm deletion**. With a replacement, tracks that had the deleted choice receive the replacement. If a smart playlist rule uses the item, CueRithm refuses to delete it unless you choose a replacement, and swaps the replacement into the rule so the playlist does not silently widen.
- A category can only be replaced by another category once it has no assigned choices.
- Unsaved Curate changes on the affected tracks must finish or be discarded first.

After a change, the panel reports "N tracks affected · M require explicit re-export".

## Save, retry and undo { #autosave-pending-work-and-undo }

Curate saves as you click. The status line beside the title says what is happening:

| Status | Meaning |
|---|---|
| **Saving draft…** | Your last click is being stored locally. |
| **Saved locally · publication pending** | The choice is stored and waits to be published to the library (and to files, if you mapped it). |
| **Saved** | Everything is published. |
| **Draft not saved** | Storing failed. Use **Retry saving draft**, or **Discard unsaved changes** to drop only what was never stored. |
| **Publication needs attention** | Publishing a track's change failed. The track's card offers **Retry publication** or **Discard pending changes**. |

**Pending and history** (or **Drafts and history** in the table toolbar) lists unfinished curation and past publications. A past entry has **Details** and, when it is safe, **Undo curation**; an interrupted one offers **Recover curation**. Undo only works while the track still matches what that publication wrote, and it keeps later unrelated changes. Discarding pending changes is different from Undo: it abandons a revision that was never published.

## Write categories into the file's tags (optional) { #optional-native-mappings }

By default nothing you do in Curate changes a file. If you want a category to travel with the file, add a mapping.

1. Open **Manage categories and mappings** and expand **Optional file mappings**.
2. Choose a **Source**: a category, **User rating** or **Curation notes**.
3. Choose a **Target**: **Typed text field** (genre, style, grouping, label, version, composer or lyricist), **Named comment identity**, or **Rating owner** (for ratings).
4. Choose an **Encoding**: **Native values** or **Structured JSON**, tick the file formats (ID3, Vorbis, MP4, ASF) and tick **Write this mapping to files**.
5. Press **Save mapping**.

Saving a mapping does not rewrite your library. Existing tracks are marked stale, and each track card then offers **Re-export mapped tags**. **Allow mapping conversion to ID3v2.4** is off by default because it can affect older players. Two sources cannot claim the same target; CueRithm rejects the clash. Choose a field you do not already fill: a mapping to `genre` targets the file's Genre tag itself.

## What can go wrong { #what-can-go-wrong }

| Symptom | Cause | Fix |
|---|---|---|
| No **Category filters** button | No category exists yet | Create one under **Manage categories and mappings** |
| A choice cannot be deleted | A smart playlist uses it, or the track has unfinished Curate changes | Choose a **Replacement**; finish or discard pending changes |
| Cannot switch a category to **One choice** | Some tracks have several choices | Remove the extras first |
| "Category catalog changed while these names were being edited" | A change arrived from elsewhere while you typed a rename | Press **Discard category draft and reload** and retype |
| A Cleanup run stops with "multiple values for a single-select Curate category" | Genre conversion wants to add several choices to a **One choice** category | Switch the category to **Multiple choices** or convert fewer genres |

## Related

- [Merge similar genres into one](recipes/merge-similar-genres.md)
- [Keep Beatport's detailed genre as a category](recipes/keep-subgenres-as-tags.md)
- [Write smart playlist rules](../playlists/smart-rules.md)
- [Work on many tracks at once](../analysis/select-many.md)
- [Edit tags and artwork](edit-tags-by-hand.md)

<!-- verified: apps/desktop/src/components/CuratePanel.tsx, CategoryEditor.tsx, CategoryChoices.tsx, CurationFilters.tsx, CurationHistory.tsx; apps/desktop/src/App.tsx (toolbar Curate, Category filters, Drafts and history); apps/desktop/src/columns.ts; apps/desktop/src/curation-types.ts; apps/desktop/src/test/curate.test.tsx; crates/cuerithm-library/src/curation/taxonomy.rs (delete/replace rules, single-select checks); crates/cuerithm-library/src/curation/aliases.rs; crates/cuerithm-library/src/cleanup/journal.rs (single-select error); legacy guides/curating.md re-checked -->
