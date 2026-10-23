# Choose which folders CueRithm watches

<p class="lead">The Incoming section lists your watch folders and the preparation preset each one uses. The Preparation presets section, covered at the end of this page, holds the presets themselves. About five minutes to read; changing a folder takes one minute.</p>

!!! tip "New to this?"
    To watch a folder step by step, follow [Keep a folder like Beatport in your library](../../files/watch-folders.md). For a one-off set of files, [Add tracks: drop, pick or watch a folder](../../files/add-tracks.md) shows the quicker way.

## Watch folders { #watch-folders }

Files that appear in a watch folder are prepared automatically: decoded, analysed, tagged and cleaned up if you turned those on, given cues, and then moved into **Tracks**. CueRithm waits until a file has stopped changing before it starts, so a copy in progress is safe. Files already in a folder when you add it are picked up too.

<!-- figure:existing-10-settings-incoming-reference-settings-incoming -->
<figure markdown>
![Settings, Incoming page, listing a watch folder](../../assets/screens/10-settings-incoming.webp){ .screen loading=lazy }
<figcaption>Watch folders are listed under Settings → Incoming.</figcaption>
</figure>
<!-- /figure -->

| Control | What it does | Default | Valid values | Task page |
|---|---|---|---|---|
| **Add watch folder** | Opens your system's folder picker and adds the folder you choose to the list. Choosing a folder that is already listed does nothing. | A new library watches `Cuerithm/Incoming` inside your system Music folder, for example `~/Music/Cuerithm/Incoming`. CueRithm creates it the first time it runs. | Any folder you can read | [Keep a folder like Beatport in your library](../../files/watch-folders.md) |
| The folder list | One line per watch folder, with the full path. | The folder above | Any number of folders | [Keep a folder like Beatport in your library](../../files/watch-folders.md) |
| **×** beside a folder | Stops watching that folder. Tracks already prepared stay in your library, and so do the files on disk. | n/a | n/a | [Keep a folder like Beatport in your library](../../files/watch-folders.md) |
| The note under the list | States what happens to the audio files after preparation, following your [File organization](file-organization.md) mode: "Original audio files stay in place.", "Original files are renamed after preparation.", "Completed files move to the saved destination folder." or "Completed files are copied to the saved destination folder; originals are retained." | Original audio files stay in place. | Not editable | [Rename and sort files into folders](../../rename/organize-files.md) |

Changes apply when you press **Save settings**. Removing a folder in the list and saving does not touch any file.

### What CueRithm picks up { #picked-up }

CueRithm prepares files with these extensions: `mp3`, `aac`, `m4a`, `alac`, `flac`, `wav`, `wave`, `aif`, `aiff`, `aifc`, `ogg`, `oga`, `opus` and `wma`. It ignores files whose names start with a dot. Subfolders of a watch folder are included by default; the switch is **Include subfolders** under [Automatic tagging → Matching](automatic-tagging.md#matching). See [Check that your files are supported](../../files/supported-formats.md).

CueRithm checks a new file every 2 seconds and starts only when its size and modified time stop changing. That interval is fixed; it is not a setting. Symbolic links are not followed. A file that is still changing after five minutes is marked as needing attention.

### If a folder shows a warning { #watch-warnings }

When CueRithm cannot watch a folder, Incoming shows a warning line above the table with the reason, for example "Folder is offline:" followed by the path when a drive is unplugged or the folder moved. Reconnect the drive or remove the folder from the list, then save. If tracks do not appear at all, see [A track is stuck in Incoming](../../help/troubleshooting/incoming-stuck.md).

## Folder preparation presets { #folder-presets }

Below the folder list, **Folder preparation presets** shows one row for each watch folder with a menu. It decides which rules prepare files from that folder.

| Control | What it does | Default | Choices | Task page |
|---|---|---|---|---|
| **Preparation preset for** *folder* | Chooses the rules for files from that folder | **Inherit parent assignment** | **Inherit parent assignment** (use the nearest parent folder's rule); **Global default** (use the settings in this window, and stop inheriting from a broader folder); or any preset whose status is ready. A preset that is not ready is listed greyed out with its status, such as "needs connection". | [Use different rules for different folders](../../files/folder-presets.md) |
| **Reconnect folder assignment** | Appears when a folder's identity changed or its assignment needs reconnecting. Pair it with **Save settings**, even if the preset stays the same. | Hidden | n/a | [Use different rules for different folders](../../files/folder-presets.md) |
| The line under the menu | Shows "Saved effective rules:" and the rules in force (a preset name or "Global default", plus the first 12 characters of its revision), or "Assignment pending · save settings" until you save | n/a | n/a | n/a |

The most specific folder assignment wins: a preset on `~/Music/Beatport/Purchases` beats one on `~/Music/Beatport`. With no watch folders in the list, the section reads "Add a watch folder to assign a preparation preset."

Folder choices are saved together with the rest of the draft by **Save settings**. If the settings or an assignment changed since you opened the screen, the whole save is refused with a message, so nothing is half applied.

## Preparation presets { #preparation-presets }

A preparation preset is a named bundle of the rules from [Cue generation](cue-generation.md), [Automatic tagging](automatic-tagging.md), [Cleanup](cleanup.md) and [File organization](file-organization.md). Your global settings remain the default; a preset only matters for folders you assign it to. Creating or changing a preset never re-runs tracks you have already prepared.

<!-- figure:existing-90-preparation-presets-light-reference-settings-incoming -->
<figure markdown>
![Preparation presets in Settings](../../assets/screens/90-preparation-presets-light.png#only-light){ .screen loading=lazy }
![Preparation presets in Settings](../../assets/screens/90-preparation-presets-dark.png#only-dark){ .screen loading=lazy }
<figcaption>Preparation presets are saved from the same screen.</figcaption>
</figure>
<!-- /figure -->

| Control | What it does | Default | Valid values |
|---|---|---|---|
| **New preset name** | The name for a preset you are about to create | Empty | Up to 256 characters |
| **Create preset from current draft** | Saves the rules currently shown in Settings (including unsaved changes) as a new preset. Reads "Preset saved. Global settings have not been applied." | Disabled until a name is typed | Up to 100 presets in total |
| **Saved preset** | Picks the preset to work with. Presets that cannot run are listed with their status. | "Choose a preset" | Your presets |
| Status line | "Ready" or the problem, then how many folder assignments use the preset | n/a | n/a |
| **Preset name** | Edit a preset's name | The current name | Up to 256 characters |
| **Load into settings draft** | Copies the preset's rules into the Settings sections as unsaved changes. Reads "Rules loaded into the settings draft. Save settings to apply them." Watch folders, the Cloudflare solver and other global settings are not replaced. | Enabled only for a ready preset | n/a |
| **Update from current draft** | Replaces the preset's rules with the rules currently shown | n/a | n/a |
| **Rename preset** | Saves the new name from **Preset name** | Disabled until the name differs | n/a |
| **Duplicate preset** | Makes a copy called "*name* copy" | n/a | n/a |
| **Reconnect destination** | Opens the folder picker so a Move or Copy rule has a destination on this computer | n/a | A folder on this computer |
| **Export preset**, then **Save preset file** | Prepares a portable file, shows any warnings, then saves it through the system save dialog. Reads "Preset file saved." | n/a | n/a |
| **Delete preset**, then **Confirm delete preset** or **Keep preset** | Deletes after confirmation. If folders use the preset, the confirmation warns that they need the deletion to be resolved. Otherwise it says that existing captured jobs keep their saved rules. | n/a | n/a |
| **Import preset file**, then **Import reviewed preset** | Opens a file, shows its name, status and any issues, and imports it only after you confirm. Importing never changes your global settings or assigns folders. Reads "Preset imported. Reconnect any required destination before use." | n/a | A preset file from CueRithm |

With no presets yet, the section says "No presets yet. Create one from the current settings draft."

### What a preset contains { #preset-contents }

| In the preset | Stays global |
|---|---|
| Cue generation: minimum breakdown length, drop at start, emergency loop | Watch folders and the 2-second file-settle wait |
| Whether automatic tagging is on, the providers and their order, fields to write and overwrite, matching, genre and style routing, artwork, file tag format, the Beatport options, what to do when nothing matches, the source of key and BPM, writing energy | Tagging concurrency, **Include subfolders**, the Cloudflare solver and detected access restrictions, fingerprint consent, provider keys and accounts |
| Whether cleanup is on, plus its Smart Fixes, genre rules and artist rules | Analysis models |
| File organization: mode, pattern, missing-value and collision choices, and whether organization is optional | The destination folder grant: a Move or Copy preset needs **Reconnect destination** on each computer |

A preparation that has started keeps the rules it started with. New files and explicit retries use the folder's current rule.

## What can go wrong { #problems }

| Symptom | Cause | Fix |
|---|---|---|
| A preset is greyed out in a folder's menu | It is not ready: a Move or Copy destination is missing or offline, or this build cannot run tagging or cleanup | Select it in **Saved preset**, read the status line, then use **Reconnect destination** |
| Files in a folder use the wrong rules | A closer folder has its own assignment, or **Global default** stops inheritance | Check each row of **Folder preparation presets**; see [Use different rules for different folders](../../files/folder-presets.md) |
| "Reconnect folder assignment" appears | The folder was moved or replaced | Press it, then **Save settings** |
| Nothing new appears in Incoming | The folder is not listed, the file type is not supported, or the drive is offline | [A track is stuck in Incoming](../../help/troubleshooting/incoming-stuck.md) |

## Related

- [Keep a folder like Beatport in your library](../../files/watch-folders.md)
- [Save your setup as a preset](../../files/presets.md)
- [Use different rules for different folders](../../files/folder-presets.md)
- [Look up a setting](index.md)

<!-- verified: apps/desktop/src/views/Settings.tsx (Incoming section, note text per organization mode); apps/desktop/src/components/PreparationFolderAssignments.tsx; apps/desktop/src/views/PreparationPresetsSettings.tsx; apps/desktop/src/preset-types.ts (PresetTaggingOptions pick list); crates/cuerithm-library/src/preparation_presets/{service,assignments}.rs (status values, Inherit default, 100 preset limit); crates/cuerithm-library/src/watch.rs (extensions, 300 s settle limit); crates/cuerithm-library/src/settings.rs (stability 2000 ms); apps/desktop/src-tauri/src/host.rs + lib.rs (initial folder audio_dir/Cuerithm/Incoming); App.tsx watch_errors banner; legacy guides/preparation-presets.md and guides/settings.md re-checked -->
