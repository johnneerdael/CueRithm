# Keep a folder like Beatport in your library automatically

<p class="lead">Add a folder once and CueRithm prepares every supported file that lands in it, now and in future. Setting up a watch folder takes about two minutes.</p>

<!-- figure:existing-10-settings-incoming-watch-folders -->
<figure markdown>
![Settings, Incoming page, listing a watch folder](../assets/screens/10-settings-incoming.webp){ .screen loading=lazy }
<figcaption>Watch folders are listed under Settings → Incoming.</figcaption>
</figure>
<!-- /figure -->

**Before you start:** know which folder you want to watch. The examples use `~/Music/Beatport`, a folder of purchases. A new library already watches `Music/Cuerithm/Incoming` inside your Music folder; CueRithm creates it for you.

## 1. Add the folder { #add-folder }

1. Choose **Settings** at the bottom of the sidebar. (The sliders button **Preparation settings**, at the top right of Tracks, Incoming and Archive, opens the same place.)
2. Stay on the **Incoming** page and select **Add watch folder**.
3. Pick `~/Music/Beatport` in the system folder picker. The path appears in the list.
4. Select **Save settings** at the top right. The button reads *Saving…* and then *Settings saved*.

The folder takes effect when you save, not when you pick it. Within seconds, **Incoming** fills with the files in the folder and the header reads *Preparing 26 tracks*.

## 2. Check that it is working { #check }

Open **Incoming** in the sidebar. Each file in the folder has a row with its current stage. When a track finishes, it leaves Incoming and the **Tracks** count grows. If nothing appears, see [What can go wrong](#what-can-go-wrong).

From now on, copy or download new files into the folder and they appear in Incoming by themselves. You do not need to open CueRithm's Add tracks button again for that folder.

## What counts as a new file { #what-is-detected }

CueRithm combines two ways of noticing files: live notifications from your operating system, and a rescan of every watch folder every five seconds. It also scans all watch folders when it starts.

| Question | Answer |
|---|---|
| Which files? | Files ending in `mp3`, `aac`, `m4a`, `alac`, `flac`, `wav`, `wave`, `aif`, `aiff`, `aifc`, `ogg`, `oga`, `opus` or `wma`, in any letter case. See [Check that your files are supported](supported-formats.md). |
| Subfolders? | Included by default. Switch off **Include subfolders** under Settings → Automatic tagging → Matching to watch only the top level. |
| Hidden files and links? | Files whose names start with a dot are skipped. Links to other places are not followed. |
| A file still being copied? | The row shows *Waiting for file* until its size and modified time stop changing. Files untouched for a few seconds start straight away. A file that keeps changing for five minutes fails with *File copy did not settle*; press **Retry** once the copy is done. |
| A file you change later? | A file whose contents change in a watch folder is prepared again from the new contents. |
| Files already prepared? | Left alone. CueRithm recognises a prepared file by its location and contents. |

The two-second wait between checks is fixed. The app has no setting for it.

## Watch several folders { #several-folders }

Select **Add watch folder** again for each folder. All watch folders use your global settings unless you assign a preset to them; see [Use different rules for different folders](folder-presets.md). A folder inside another watch folder is allowed, and the deeper folder's assignment wins.

## Stop watching a folder { #remove-folder }

Select the **×** beside the folder under Settings → Incoming, then **Save settings**. The tracks already in your library stay, and so do their files. CueRithm only stops looking for new files.

## When a watch folder is offline { #offline }

If a watch folder cannot be opened, for example because an external drive is unplugged, a banner above the Incoming list reads *Folder is offline* followed by the path. Nothing is removed from your library. CueRithm checks again every five seconds and resumes when the folder returns.

## What a watch folder does not do { #limits }

- It follows a file you move or rename yourself only inside the watch folder, and only when the file has exactly the content of one missing track (CueRithm 0.1.15 or later). The track then keeps its analysis, cues, playlists and tags, also when you moved the file while CueRithm was closed. A file that could belong to several tracks, an edited file and a move out of the folder look like a new file at the new location, and the old track keeps pointing at the old one. Read [Move or rename files without breaking the library](move-files.md) before you reorganise a watched folder.
- It does not delete tracks when you delete files. The track stays in your library and shows as missing in [Missing & broken](library/find-missing-files.md), where you can [remove the record](library/find-missing-files.md#remove) of a file that is gone for good.
- It does not retag or re-analyse tracks when you change settings. Use [Analyze again](../analysis/analyze-again.md).

## What can go wrong { #what-can-go-wrong }

| What you see | Why | What to do |
|---|---|---|
| Incoming stays empty after you picked the folder | **Save settings** was not selected | Select **Save settings** |
| Incoming stays empty and the folder holds music | The files use an extension that is not in the list, or sit in subfolders with **Include subfolders** off | Check [the supported extensions](supported-formats.md#how-files-are-recognised) and the setting |
| *Folder is offline* above the list | The folder or its drive is not reachable | Reconnect the drive; CueRithm retries by itself |
| A track stops with *File copy did not settle* | The file changed for five minutes, for example a cloud-sync folder still downloading | Wait until the file is complete, then press **Retry** |
| The same songs appear twice after you reorganised folders | The moved files were edited, could belong to several tracks, left the watch folder, or you use CueRithm 0.1.14 or earlier | [Move or rename files without breaking the library](move-files.md) |
| Tracks stay in Incoming with an error | A required stage failed | [Fix a track that needs attention](needs-attention.md) |

## Related

- [Add tracks by dropping, picking or watching a folder](add-tracks.md)
- [Prepare a new Beatport purchase folder](recipes/prepare-a-purchase-folder.md)
- [Follow a track through Incoming](follow-incoming.md)
- [Incoming settings reference](../reference/settings/incoming.md)

<!-- verified: apps/desktop/src/views/Settings.tsx (Incoming page, Add watch folder, Remove watch folder, Save settings states); apps/desktop/src/App.tsx (watch-warning banner shows service watch_errors in Incoming; Preparation settings button); apps/desktop/src-tauri/src/lib.rs (new library watches audio_dir/Cuerithm/Incoming); crates/cuerithm-library/src/prepare.rs (5 s rescan, notify watcher, Folder is offline, scan on start); crates/cuerithm-library/src/watch.rs (supported_path, wait_for_stable 300 s, File copy did not settle, Discovery); crates/cuerithm-library/src/settings.rs (stability_millis 2000, include_subfolders default true); crates/cuerithm-library/src/jobs.rs (changed contents create a new job); legacy get-started/add-music.md and guides/settings.md#incoming re-checked -->
