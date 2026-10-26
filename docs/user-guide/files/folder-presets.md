# Use different rules for different folders

<p class="lead">Assign a preparation preset to a watch folder and every file that arrives in it is prepared with that preset, while the rest of your music keeps using your global settings. Assigning a preset takes about a minute.</p>

**Before you start:** you need a [preparation preset](presets.md) and the folder must be one of your [watch folders](watch-folders.md). Only watch folders can be assigned a preset.

## 1. Find the folder list { #find }

Choose **Settings**, stay on the **Incoming** page and scroll below **Add watch folder** to **Folder preparation presets**. Every watch folder has a row with its path, a menu named *Preparation preset for* that path, and a status line.

## 2. Choose rules for a folder { #choose }

Open the menu on the folder's row and pick one of three kinds of choice.

<!-- figure:prepare-folder-assignments -->
<figure markdown>
![Settings, Incoming page, Folder preparation presets section: the row for a watch folder (its path blurred) with the preset menu set to Beatport purchases and the line Saved effective rules: Beatport purchases](../assets/screens/prepare-folder-assignments-light.webp#only-light){ .screen loading=lazy }
![Settings, Incoming page, Folder preparation presets section: the row for a watch folder (its path blurred) with the preset menu set to Beatport purchases and the line Saved effective rules: Beatport purchases](../assets/screens/prepare-folder-assignments-dark.webp#only-dark){ .screen loading=lazy }
<figcaption>Each watch folder has a menu for its preparation rules.</figcaption>
</figure>
<!-- /figure -->

| Choice | What it means |
|---|---|
| **Inherit parent assignment** | The default. Use the rules of the nearest watch folder above this one that has an assignment. If there is none, use your global settings. |
| **Global default** | Use your global settings, even if a folder above has a preset. This stops inheritance. |
| A preset name | Use that preset's rules for files in this folder. A preset that is not *ready* is greyed out. |

Select **Save settings** at the top right. Folder choices save together with the rest of Settings. Until you do, the status line reads *Assignment pending · save settings*. Afterwards it reads *Saved effective rules:* followed by the preset's name, or *Global default*.

## 3. Give a sub-folder its own rules { #nested }

Folders you can assign are the ones in the watch list. To give part of a folder different rules, add that part as a watch folder too and assign it separately.

| Watch folder | Choice | Files prepared with |
|---|---|---|
| `~/Music/Beatport` | Beatport purchases (a preset) | The `Beatport purchases` preset |
| `~/Music/Beatport/Hard dance` | **Inherit parent assignment** | The `Beatport purchases` preset |
| `~/Music/Beatport/Hard dance` | A preset named `Hard dance cues` | The `Hard dance cues` preset |
| `~/Music/Beatport/Hard dance` | **Global default** | Your global settings |

The deepest assigned folder that contains a file wins. A folder with a similar name is not a child: `~/Music/Beatport 2` does not inherit from `~/Music/Beatport`. You can keep at most 256 assignments.

## When the rules are applied { #when }

- **New files** use the folder's rules from the moment you save, whether a watch folder found them or you added them from a location inside that folder with [Add tracks or by dropping](add-tracks.md).
- **Files queued before you saved** keep the rules they were added with.
- **Finished tracks** are not changed. [Analyze again](../analysis/analyze-again.md) applies the folder's current rules.
- **Editing a preset later** does not rewrite tracks already added. Each save of a preset is a new revision, and a track keeps the revision it was added under.

## Reconnect a folder assignment { #reconnect }

CueRithm remembers which directory on your disk an assignment belongs to, so a folder that was deleted and recreated, or swapped for a different drive, never silently picks up old rules. When that happens the row shows a message and a **Reconnect folder assignment** button.

1. Read the message on the folder's row.
2. Select **Reconnect folder assignment**.
3. Select **Save settings**, even if you keep the same choice.

Until you do, CueRithm does not start new files from that folder. A file you add yourself shows a message such as *Reconnect this folder assignment before preparing files*, and a file already queued stops with *Captured preset folder changed; reconnect and explicitly retry current rules*. After you reconnect, files in the watch folder are picked up again on the next scan, and a stopped file can be retried with **Retry**.

To clear an assignment for a folder that is no longer watched, add the watch folder again, choose **Inherit parent assignment** and save.

## What can go wrong { #what-can-go-wrong }

| What you see | Why | What to do |
|---|---|---|
| The row says *Folder is unavailable or contains a link* | The folder or a parent folder is a link to another location | Add the real folder as the watch folder |
| A preset is greyed out in the menu | Its status is not *ready*, for example a Move rule without a destination | Fix it in [Save your setup as a preset](presets.md#change) |
| A message that *Assigned preset '…' is unavailable* when you add or retry a file | The preset needs reconnecting or was damaged | Repair the preset, then add or retry the file again |
| **Save settings** is refused with *Folder assignment changed; reload before saving* | Settings changed elsewhere while you edited | Reload Settings and choose again |
| *Remove folder assignments before deleting this preset* | You deleted a preset that a folder still uses | Set each such folder to **Inherit parent assignment**, save, then delete |
| The preset's rules did not apply to a file | The file is outside every assigned watch folder | Move it into one, or assign the folder it is in |

## Related

- [Save your setup as a preparation preset](presets.md)
- [Keep a folder like Beatport in your library automatically](watch-folders.md)
- [Fix a track that needs attention in Incoming](needs-attention.md)
- [Incoming settings reference](../reference/settings/incoming.md)

<!-- verified: apps/desktop/src/components/PreparationFolderAssignments.tsx (heading, menu label, options, status lines, Reconnect folder assignment); apps/desktop/src/views/Settings.tsx (assignments render on the Incoming page; changes saved with Save settings); crates/cuerithm-library/src/preparation_presets/assignments.rs (only watch folders listed; inactive folders; 256 limit; reconnect rules; messages); resolution.rs (deepest assigned folder wins; Inherit skipped; applies to any source below an assigned watch folder); crates/cuerithm-library/tests/preparation_presets.rs (deepest_folder_rules_and_global_barriers_capture_each_source_immutably: nested watch folders, Global barrier, Inherit, captured revisions unchanged by later preset edits, assigned preset cannot be deleted); legacy guides/preparation-presets.md#folder-assignments re-checked. Code-read, not run: the sibling-folder example relies on path-component matching (Path::starts_with). -->
