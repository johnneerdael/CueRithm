# Add tracks by dropping, picking or watching a folder

<p class="lead">There are three ways to put music into CueRithm, and all of them end in the same place: the Incoming view, where each track is prepared automatically. Adding files takes under a minute.</p>

| Way | Best for | Accepts folders? |
|---|---|---|
| [**Add tracks** button](#pick-files) | A few files you have in front of you | No, files only |
| [Drag and drop](#drop) | Files or whole folders from Finder, Explorer or your file manager | Yes |
| [Watch folder](watch-folders.md) | A folder that keeps receiving music, such as `~/Music/Beatport` | Yes, set once |

Your files are not copied or moved. CueRithm records where each one is and works from there. The only exception is a [file organization](../rename/organize-files.md) rule you have switched on, which can rename, move or copy a file after it is prepared.

## Pick files with Add tracks { #pick-files }

<!-- figure:prepare-add-tracks-button -->
<figure markdown>
![The CueRithm header with the Add tracks button at the top right, beside the colour theme button and the All up to date indicator](../assets/screens/prepare-add-tracks-button-light.webp#only-light){ .screen loading=lazy }
![The CueRithm header with the Add tracks button at the top right, beside the colour theme button and the All up to date indicator](../assets/screens/prepare-add-tracks-button-dark.webp#only-dark){ .screen loading=lazy }
<figcaption>The Add tracks button sits at the top right of every library view.</figcaption>
</figure>
<!-- /figure -->

1. Select **Add tracks** at the top right of the window.
2. In the system file picker, choose one or more files. The picker opens on the **Audio** filter. Choose **All files** to see a file whose extension is not in the list.
3. Confirm. CueRithm switches to **Incoming** and your files start to appear there.

The empty Tracks and Incoming views show an **Add audio files** button that does the same thing. The picker selects files, not folders. To add a folder, drop it on the window or [add it as a watch folder](watch-folders.md).

## Drop files or a folder on the window { #drop }

Drag audio files or folders from your file manager onto any CueRithm view. While you drag, an overlay reads **Drop to prepare** with *Full analysis. Fresh cues. Ready for your library.* Release the files and CueRithm switches to **Incoming**.

- A dropped **folder** is searched for audio files. Subfolders are included unless you switched off **Include subfolders** (Settings → Automatic tagging → Matching, on by default).
- Inside a folder, CueRithm picks up only the [supported extensions](supported-formats.md#how-files-are-recognised). Files whose names start with a dot are skipped, and links to other locations are not followed.
- Files you drop or pick **one by one** are not filtered by extension. If one cannot be decoded, it fails during preparation and waits in Incoming with the reason; see [Fix a track that needs attention](needs-attention.md).

## What happens next { #after-adding }

Each file becomes a row in **Incoming** and moves through the stages described in [Follow a track through Incoming](follow-incoming.md). The header shows *Preparing 26 tracks* while it works, and each finished track moves to **Tracks**.

CueRithm captures your preparation rules at the moment you add a file. If a [folder preset](folder-presets.md) is assigned to the folder the file sits in, that preset's rules apply; otherwise your global settings do.

## Adding a file CueRithm already has { #already-in-library }

CueRithm identifies a track by the location of its file, so adding the same file twice does not create two tracks.

| You add… | Result |
|---|---|
| The same file, unchanged, with unchanged settings | Nothing changes. The existing track stays where it is. |
| The same file after you changed your preparation settings | The track returns to Incoming and is prepared again with the new settings. |
| A file whose contents changed since it was prepared | The track is prepared again from the new contents. |
| A copy of the same song in another folder | A second track. [Find and merge duplicate tracks](library/find-duplicates.md) can find it later. |
| A file that is in the [Archive](archive.md) | Nothing happens until you restore it. |

## What can go wrong { #what-can-go-wrong }

| What you see | Why | What to do |
|---|---|---|
| A message at the bottom of the window ending in *Incoming file is missing* | The file was moved or its drive was disconnected before CueRithm read it | Reconnect the drive or add the file from its new location |
| You dropped a folder and nothing appeared | It holds no supported files, or the files are in subfolders and **Include subfolders** is off | Check [the supported extensions](supported-formats.md); switch on **Include subfolders** |
| The file picker greys out your file | Its extension is not in the **Audio** filter | Choose **All files**; the decoder recognises files by content, not by name |
| A row shows a warning triangle | A stage failed | [Fix a track that needs attention](needs-attention.md) |
| Rows stay in *Queued* | Other tracks are still being prepared | Wait; the header shows how many remain |

## Related

- [Keep a folder like Beatport in your library automatically](watch-folders.md)
- [Check that your files are supported](supported-formats.md)
- [Follow a track through Incoming](follow-incoming.md)
- [Prepare your first tracks](../get-started/first-tracks.md)

<!-- verified: apps/desktop/src/App.tsx (addPaths, onDrop overlay text, Add audio files, empty states); apps/desktop/src/api.ts (chooseFiles: files only, Audio and All files filters; onDrop); crates/cuerithm-library/src/prepare.rs (enqueue_paths: folders walk with include_subfolders, single files not filtered); crates/cuerithm-library/src/watch.rs (supported_path); crates/cuerithm-library/src/jobs.rs (enqueue_one: same path and hash with same revision returns the existing job; archived returns current job); crates/cuerithm-library/src/preparation_presets/resolution.rs (enqueue_with_current_preparation); legacy get-started/add-music.md re-checked -->
