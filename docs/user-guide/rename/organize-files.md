# Rename and sort files into folders

<p class="lead">In about fifteen minutes you will preview new names and folders for six tracks, copy them into a library folder sorted by artist, check the result and see how to reverse it.</p>

!!! warning "This changes files on disk"
    **Rename in place** and **Move to folder** change where your audio files live, and **Copy to folder** creates new files. Nothing happens until you press **Apply**. Existing files are never overwritten and CueRithm deletes nothing. Every batch can be reversed from **History and recovery** on the same page: see [Recover or undo a reorganisation](undo-organize.md). This tutorial uses **Copy to folder**, which leaves your originals exactly where they are.

**You need:**

- [ ] Six Ready tracks that have an artist tag. The screenshots use *Palm Of Your Hands* by Adriatique, *Knockout* by SOLANCE, *Dancin* by Jast, *When The Fire Starts To Burn* by Colyn, *Say Less* by SLVR and *Undercurrent* by Random Factor, all FLAC files from a Beatport purchase folder.
- [ ] An empty folder to receive the copies, outside CueRithm's own data folder, for example `~/Music/DJ Library`.
- [ ] Free disk space for the six files. The preview estimates it for you.

## 1. Select the tracks and open Organize files { #open-organize }

In **Tracks**, [select the six tracks](../analysis/select-many.md). Press **Organize files** in the table toolbar, or choose **Organize files** under **TOOLS** in the sidebar. The page is titled **Organize files** with the line "Review complete naming plans before changing your library."

The **Scope** menu reads **6 selected tracks**. Its other choices are **Current playlist**, **Folder** (press **Choose scope folder** and tick **Include subfolders** if you want them) and **All ready tracks**. Only Ready tracks that are not archived can be organised.

*Why:* a plan always covers a complete scope you can see. Start small, then widen the scope when the pattern is right.

## 2. Choose Copy to folder { #choose-copy }

Open the **Naming rules** section if it is collapsed. The row **When preparation finishes** is the mode menu in both places this panel appears; on this page it sets the mode for the plan you are building. Choose **Copy to folder**, press **Choose destination** and pick your empty folder.

The mode choices are:

| Mode | What happens to each file |
|---|---|
| **Leave files in place** | Nothing. The default, and **Preview complete plan** is disabled in this mode. |
| **Rename in place** | The file keeps its folder and gets a new name. |
| **Move to folder** | The file moves to the destination folder with its new name. |
| **Copy to folder** | A new file appears in the destination folder. The original stays and the library switches to the copy. |

Rules set on this page apply to this plan only. Rules for future Incoming tracks are saved in **Settings → File organization** ([see below](#make-it-automatic)).

## 3. Write the pattern { #write-the-pattern }

In **Filename pattern**, replace `{artist} - {title}` with:

<!-- figure:existing-17-settings-file-organization-organize-files -->
<figure markdown>
![File organization settings with example tracks](../assets/screens/17-settings-file-organization.jpg){ .screen loading=lazy }
<figcaption>The naming pattern and its live examples.</figcaption>
</figure>
<!-- /figure -->

```text
{artist}/{artist} - {title}
```

Braces insert a tag, and the slash makes a folder. CueRithm adds the audio extension itself, so you never type `.flac`. Below the box, **Example filenames** shows the pattern on invented metadata:

- **Complete title**: `example.flac` becomes `Example Artist/Example Artist - Example (Club Mix).flac`.
- **Missing artist**: the arrow points to "Missing naming field: artist". A track with no artist tag would be blocked.

Leave **Missing values** on **Needs attention** and **If a filename already exists** on **Needs attention**. Both mean "stop and tell me" rather than guess. [Build a naming pattern](naming-patterns.md) lists every tag you can use.

## 4. Preview the complete plan { #preview }

Press **Preview complete plan**. CueRithm checks every track in the scope, then shows **Complete plan · 6 tracks** with the totals "6 eligible · 0 unchanged · 0 skipped · 0 blocked", the **Estimated additional space** and one row per track: its title, the current path, and "→" the new path.

<!-- figure:existing-87-organization-preview-organize-files -->
<figure markdown>
![The complete organization preview with eligible and blocked counts](../assets/screens/87-organization-preview.jpg){ .screen loading=lazy }
<figcaption>Read the complete plan before you apply it.</figcaption>
</figure>
<!-- /figure -->

| Track | New path inside the destination folder |
|---|---|
| Palm Of Your Hands (Extended Mix) | `Adriatique/Adriatique - Palm Of Your Hands (Extended Mix).flac` |
| Knockout (Extended Mix) | `SOLANCE/SOLANCE - Knockout (Extended Mix).flac` |
| Dancin (Extended Mix) | `Jast/Jast - Dancin (Extended Mix).flac` |
| When The Fire Starts To Burn (Extended Mix) | `Colyn/Colyn - When The Fire Starts To Burn (Extended Mix).flac` |
| Say Less (Extended Mix) | `SLVR/SLVR - Say Less (Extended Mix).flac` |
| Undercurrent (Original Mix) | `Random Factor/Random Factor - Undercurrent (Original Mix).flac` |

A track that cannot be organised shows **blocked** with its reason, for example "Missing naming field: artist" or "Destination already exists". It stays where it is, and the rest can still go ahead. Rows are listed 50 to a page; use **Previous** and **Next**.

Nothing has happened on disk yet. **Discard preview** throws the plan away. If you change the scope or any rule, the preview is discarded and you preview again.

## 5. Apply the plan { #apply }

Press **Apply 6 eligible**. The page shows "Applying organization…" and you can leave it; the work continues and the result is kept in **History and recovery**. When it finishes you see **Organization · complete** with "6 completed · 0 unchanged · 0 skipped · 0 blocked · 0 failed · 0 need recovery · 0 cancelled / 6 total".

For each file CueRithm writes a complete copy under a hidden temporary name in the destination, checks its bytes against the original, and only then moves it to its final name without replacing anything. Then the library switches the track to the new path. Press **Cancel operation** while it runs to stop before the remaining files; completed files stay.

## 6. Check the result { #check }

Open the destination folder: six artist folders, each holding one file. In CueRithm, show the **Location** column in **Tracks** to see the new paths. Playlist membership, analysis, cues and the loaded [player](../analysis/preview-a-track.md) position are unchanged; the track is the same track at a new path.

Your originals are still in the Beatport folder. CueRithm remembers their exact bytes so a watch folder does not import them again as new tracks. If an original later changes, the page shows **Changed original files** with **Keep managed copy** and **Add changed original as Incoming**; see [Recover or undo a reorganisation](undo-organize.md#changed-originals).

## 7. Undo it { #undo }

Scroll to **History and recovery**. The batch reads "Organization · complete" with its date and "6 of 6 completed". Press **Undo** to point the six tracks back at their original files and set the copies aside as hidden recovery files. Undo checks every file again first and stops for a track whose files changed, and says why. The full story is in [Recover or undo a reorganisation](undo-organize.md).

## Make it automatic { #make-it-automatic }

To name every future Incoming track the same way, open **Settings → File organization**, set the mode, destination and pattern, and press **Save settings**. The rules run last in preparation, after tagging, cleanup and analysis, and before the track enters **Tracks**. Every track is checked against its final metadata, so a title that [cleanup](../tagging/cleanup-automatically.md) tidies is named from the tidied title. Different watch folders can use different rules through [preparation presets](../files/folder-presets.md).

## Safety rules in one place { #safety-rules }

- **Preview first.** Apply uses the frozen plan for the complete scope, not only the page you looked at. If the library changed since, CueRithm says "Library changed. Preview the complete plan again before applying." and discards the plan.
- **Existing files are never overwritten.** A name that is already taken blocks the track, or skips it if you made organization optional.
- **Originals are retained.** **Copy to folder** leaves the original untouched. **Move to folder** and **Rename in place** keep the old file as a hidden recovery copy next to where it was, named like `.cuerithm-<id>-0.original`. CueRithm never deletes these, so a moved or renamed library needs its space twice until you remove them yourself. The **Estimated additional space** includes them, and Apply is blocked if the disk is too small.
- **Linked DJ libraries.** CueRithm repairs the file paths of DJ libraries it knows about from earlier [syncs](../sync/index.md) before it retires a moved file, for the destinations that have a path-repair adapter. A track that is linked to a rekordbox library is blocked instead, with the reason *Imported Rekordbox reference … has no registered database path/write adapter*, because CueRithm cannot repair rekordbox's file paths. A library it cannot repair, such as one that is open in its application, blocks that track with a reason. CueRithm cannot see other programs, playlists or projects that point at your files, so check those yourself.
- **Move files here, not in Finder.** A file you move yourself breaks its track's stored path. See [Move or rename files without breaking the library](../files/move-files.md) and [I moved or renamed my files](../help/troubleshooting/files-moved.md).

## What you learned

- **Organize files** previews a complete plan of new names and folders before it touches a file.
- The pattern uses `{tags}` for values, `/` for folders and keeps the extension.
- **Copy to folder** keeps your originals; **History and recovery** reverses a batch.
- The same rules can run automatically on every new track from **Settings → File organization**.

## Next steps

- [Build a naming pattern](naming-patterns.md): every tag, optional parts and what happens to unusual characters.
- [Sort files into artist folders](recipes/sort-files-into-artist-folders.md): the pattern from this tutorial as a recipe.
- [Put key and energy in file names](recipes/put-key-and-energy-in-file-names.md)
- [Recover or undo a reorganisation](undo-organize.md)

<!-- verified: crates/cuerithm-library/src/organization_references.rs (rekordbox blocker); apps/desktop/src/views/Organize.tsx; apps/desktop/src/views/OrganizationSettings.tsx; apps/desktop/src/organization-types.ts; apps/desktop/src/App.tsx (Organize files button and sidebar item, selection capture); apps/desktop/src/columns.ts (Location); apps/desktop/src/test/organization.test.tsx and file-organization-settings.test.tsx; crates/cuerithm-organize/src/{template,plan,paths}.rs (all example paths run through the template engine); crates/cuerithm-library/src/organization/README.md and service/{planning,capture,examples,storage}.rs, coordinator.rs (quarantine name, copy ownership, undo); legacy guides/file-organization.md re-checked. README.md of the organization module is stale (says no UI invokes it); the code and UI are the authority. -->
