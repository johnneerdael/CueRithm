# Save your setup as a preparation preset

<p class="lead">A preparation preset is a named copy of your cue, tagging, cleanup and file organization rules. Save one once, reuse it for a folder, share it as a file. Creating a preset takes about two minutes and changes nothing until you use it.</p>

**Before you start:** set Settings the way you want the preset to be. You can leave edits unsaved; the preset is built from the Settings page as it is on screen.

## What a preset contains { #contents }

| Included | Not included (stays global) |
|---|---|
| Cue generation: minimum breakdown length, drop at start, emergency loop | Watch folders |
| Automatic tagging on or off, [providers](../tagging/choose-providers.md) and their order, matching, fields to write, artwork and file tag format | Include subfolders, the Cloudflare solver address and its timeout |
| Cleanup on or off, and its [rules](../tagging/cleanup-rules.md): Smart Fixes, genre and artist mappings | Provider accounts and credentials |
| File organization mode, filename pattern, missing-value and collision rules, and the destination folder link | Analysis models and local fingerprint consent |

Creating or editing a preset does not rerun existing tracks and does not change your global settings. The page says so: *Global settings remain the default.*

## 1. Create a preset { #create }

<!-- figure:existing-90-preparation-presets-light-presets -->
<figure markdown>
![Preparation presets in Settings](../assets/screens/90-preparation-presets-light.png#only-light){ .screen loading=lazy }
![Preparation presets in Settings](../assets/screens/90-preparation-presets-dark.png#only-dark){ .screen loading=lazy }
<figcaption>Save a tagging, cleanup and cue setup under a name.</figcaption>
</figure>
<!-- /figure -->

1. Choose **Settings**, then adjust the pages you care about, for example [Cue generation](../analysis/cue-rules.md) and Automatic tagging.
2. Open **Preparation presets**.
3. Type a name in **New preset name**, for example `Beatport purchases`.
4. Select **Create preset from current draft**.

You should see *Preset saved. Global settings have not been applied.* and the preset appears in the **Saved preset** list. Names must be 1 to 80 characters and unique, ignoring capital letters. You can keep up to 100 presets.

## 2. Use a preset { #use }

A preset does nothing by itself. There are two ways to put it to work:

- **For one folder:** assign it to a watch folder so every new file in that folder is prepared with it. See [Use different rules for different folders](folder-presets.md).
- **As your global settings:** choose it under **Saved preset**, select **Load into settings draft**, then **Save settings**. The page says *Rules loaded into the settings draft. Save settings to apply them.* Loading keeps your credentials, solver address, watch folders and consent settings.

**Load into settings draft** is disabled while a preset's status is not *Ready*.

## 3. Change a preset { #change }

Choose the preset under **Saved preset**, then:

| Button | What it does |
|---|---|
| **Update from current draft** | Replaces the preset's rules with the Settings page as it is now |
| **Rename preset** | Applies the name in the **Preset name** field |
| **Duplicate preset** | Makes a copy named *original name* followed by `copy` |
| **Reconnect destination** | Picks the destination folder again for a Move or Copy rule |
| **Export preset** | Prepares a file to share; see below |
| **Delete preset** | Asks you to confirm with **Confirm delete preset** or cancel with **Keep preset** |

Each save creates a new, unchangeable revision. A track that was queued under an earlier revision keeps the rules it captured, so editing a preset never rewrites work in progress or finished tracks. To apply a new revision to a finished track, use [Analyze again](../analysis/analyze-again.md).

A preset that a folder uses cannot be deleted. Remove its folder assignments first; the message reads *Remove folder assignments before deleting this preset*.

## 4. Share a preset as a file { #share }

1. Select **Export preset**. A **Portable export** panel lists any warnings.
2. Select **Save preset file** and choose where to save. The default name is `Preparation preset.json`; the page confirms *Preset file saved.*
3. On another computer or library, select **Import preset file** and pick the file.
4. Read the import summary. Select **Import reviewed preset**.

An export never contains credentials, the solver address, folder assignments, model paths or permission to write to your folders. A preset that moves or copies files needs a destination on the new computer: you see *Preset imported. Reconnect any required destination before use.* Use **Reconnect destination**. An import creates a preset only; it does not apply it anywhere.

## What can go wrong { #what-can-go-wrong }

| What you see | Why | What to do |
|---|---|---|
| *needs connection* beside a preset's name | Its rule moves or copies files and has no destination on this computer | **Reconnect destination** |
| *Preset name is already in use* | Another preset has that name | Pick a different name |
| *Keep at most 100 active presets* | You reached the limit | Delete one you no longer use |
| *Selected file is not a supported preparation preset…* | The file is not a CueRithm preset, or is from an unknown version | Export it again from CueRithm |
| *Settings draft changed while loading the preset. Load again…* | You edited Settings during loading | Select **Load into settings draft** again |

## Related

- [Use different rules for different folders](folder-presets.md)
- [Choose how CueRithm places cues](../analysis/cue-rules.md)
- [Tag new tracks from Beatport and Traxsource](../tagging/tag-new-tracks-automatically.md)
- [Find the setting you need](../customize/find-a-setting.md)

<!-- verified: apps/desktop/src/views/PreparationPresetsSettings.tsx (all labels, messages and buttons); apps/desktop/src/preset-types.ts and crates/cuerithm-library/src/preparation_presets/model.rs (rules contained); portable.rs (valid_name 1 to 80 characters, project/overlay exclusions, includeSubfolders and solver left global); service.rs (100 preset limit, unique name_key, delete refused while assigned, duplicate adds ' copy', status values); apps/desktop/src-tauri/src/preparation_preset_commands.rs (Preparation preset.json, error text); crates/cuerithm-library/tests/preparation_presets.rs (captured revisions immutable; delete refused while assigned); legacy guides/preparation-presets.md re-checked -->
