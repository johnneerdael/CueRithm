# Sync to rekordbox 6 or 7

<p class="lead">In about fifteen minutes you will send the Club nights folder, with its Warm-up set and Peak hour playlists, into your own rekordbox library. You will see exactly what changes before anything is written, check the result in rekordbox, and learn how to undo it.</p>

**You need:**

- [ ] rekordbox 6 or 7 installed on this computer and opened at least once, so its library exists
- [ ] Prepared tracks in the **Tracks** view of CueRithm, and a playlist folder to send. This tutorial uses **Club nights**; any playlist or folder works ([Create a playlist and add tracks](../playlists/create-a-playlist.md))
- [ ] rekordbox and rekordbox Agent closed (step 1 shows how)
- [ ] Optional but wise for a library you gig from: your own copy of rekordbox's library folder

!!! note "What direct sync means"
    CueRithm opens rekordbox's own library database and its analysis and artwork files and writes to them itself. There is no XML file to export or import. The database is encrypted, so CueRithm works out the key from your installed rekordbox when it needs it, keeps it in memory only, and ignores your rekordbox account details. A preview changes nothing in rekordbox: it reads the library and builds the whole change on a private copy.

## Which builds can be written to { #builds }

CueRithm decides from the library's database and asset formats, not from a list of build numbers. Compatible rekordbox 6 and 7 builds can be written to on macOS, Windows and Wine or Proton.

| Situation | What Sync does |
|---|---|
| macOS, rekordbox **6.8.7.0303** or **7.2.19.0342** | Writes. These two builds were checked in rekordbox itself: it reopened the synced library and showed the metadata, artwork, grids, cues and loops, the created FLAC tracks played and looped, and repeat-sync, restore and conflict checks passed |
| Any other rekordbox 6 or 7 whose database and asset formats CueRithm recognises, on macOS or Windows | Writes. These builds have not been opened in rekordbox by the checks above, so make your own backup first |
| Windows rekordbox through Wine or Proton on Linux | Same rule, after a [manual setup](windows-libraries-on-linux.md#rekordbox-on-wine). Windows and Wine playback in rekordbox has not been checked |
| A database or asset format CueRithm does not recognise, such as a newer release that changed it | **Read only**: you can preview, but **Sync rekordbox** stays disabled |

The Mac and Windows apps look for the library automatically. Linux has no native rekordbox, so a library there always needs the manual setup. [Check what works with your DJ software](../help/compatibility.md) lists the checks and their scope.

## 1. Close rekordbox and rekordbox Agent { #close-rekordbox }

CueRithm refuses to preview or apply while either program is running, and it never closes them for you. rekordbox Agent is a background helper that can keep running after you quit rekordbox, and it holds the library open.

=== "macOS"

    Quit rekordbox. Open Activity Monitor, search for `rekordbox`, and quit every `rekordbox` and `rekordboxAgent` process.

=== "Windows"

    Close rekordbox. Open Task Manager, find `rekordbox` and `rekordboxAgent`, and end each one.

**What you should see:** neither program in the process list.

## 2. Choose rekordbox 6 / 7 in Sync { #open-sync }

Choose **Sync** in the sidebar (under **TOOLS**), then open the **Sync to** menu and choose **rekordbox 6 / 7**. The menu also lists the six other destinations, and CueRithm remembers your choice for next time.

**What you should see:** a heading **Sync**, an **Installed library** menu, a scope menu and a list headed **Include in sync**.

## 3. Check that the library is ready { #check-library }

CueRithm looks for the library when the view opens. The **Installed library** menu shows `rekordbox` followed by the version and the location of `master.db`. Under it a status block tells you what CueRithm found:

| Status | Meaning |
|---|---|
| **Ready for preview** | The library can be read and written |
| **Read only** | It can be read but not written. The line below says why: rekordbox or its Agent is running, an earlier preview or interrupted sync is waiting, or the format is not recognised |
| **Library unavailable** | CueRithm could not open the library. The line below says why |

If you closed rekordbox after opening this view, press the refresh button next to the menu so CueRithm checks again.

**What you should see:** **Ready for preview** and the version of your rekordbox.

## 4. Choose what to send { #choose-scope }

Open **What to sync** and choose **Playlist or folder**. In the **Playlist** menu choose **Club nights**.

This tutorial avoids **Full library** on purpose. The demo library contains smart playlists, and rekordbox cannot hold a CueRithm smart playlist unless you convert it to a plain track list. [Choose what to sync](choose-what-to-sync.md#smart-playlists) explains the options.

**What you should see:** the **Playlist** menu showing **Club nights**. Both playlists inside the folder are included, with every track they hold.

## 5. Choose what to include { #choose-contents }

Leave **Include in sync** as it is. Every item is ticked except the last three, which stay off.

- **Cues and loops**, **Beatgrids**, **Artwork**, **Playlists and folders** and **My Tags** are on.
- **Convert smart playlists to track lists** is off, which is right for this folder.
- **Create compatible WAV copies for unsupported audio** is off. The demo tracks are FLAC, which rekordbox plays as they are.
- **Use nearest supported cue colors** is off.
- **Metadata fields** lists 18 fields, all ticked. Leave **Title** ticked: a track without a title cannot be added.

[Set the sync options](sync-options.md#rekordbox-options) describes each control.

## 6. Preview the sync { #preview }

Choose **Preview rekordbox sync**. CueRithm reads your whole rekordbox library, checks each audio file in the scope, and builds the complete change on a private copy. A few tracks take seconds; a large scope takes longer. **Cancel** stops it.

**What you should see:**

- A summary line such as `16 tracks · 52 rows to change · 0 fields skipped`, and below it the number of compatible audio copies (none here). A row is one entry in rekordbox's database.
- A panel headed **N new tracks** with a **Tracks to add** list. None of the demo tracks are in rekordbox yet, so every track is new. New tracks need no choices; CueRithm sends the values you ticked and leaves out empty ones ([Add only new tracks to rekordbox](rekordbox-new-tracks.md)).
- A table headed **Playlists and My Tags**. It shows **Create · Folder** for Club nights and **Create · Playlist** for Warm-up set and Peak hour, each with its position and track list under **Track membership**.
- No **Review fields** table. That table appears only for tracks rekordbox already has ([Resolve rekordbox sync conflicts](rekordbox-conflicts.md)).
- **Sync rekordbox** is enabled.

Read any red or amber lines. Changing anything on screen, or anything in your CueRithm library, throws the preview away and you preview again.

## 7. Apply the sync { #apply }

Choose **Sync rekordbox**. A progress line reads **Syncing rekordbox** while CueRithm writes. Leave rekordbox closed.

**What you should see:** a result block headed **Sync completed** with counts such as `16 tracks · 52 rows changed · 140 fields acknowledged · 0 fields skipped`, and a new entry in **rekordbox sync history** with the status `applied`.

CueRithm changed three places, and only these:

- **rekordbox's library database** (`master.db`): the new tracks with the metadata you ticked, their cues and loops, and the folder, playlists and My Tags.
- **rekordbox's `share` folder**: new analysis files (beat grid, cues and waveforms) and cover images in three sizes. They are written as new versions; existing files are not overwritten.
- **CueRithm's own data folder**: a verified copy of the database as it was before the sync, which **Restore backup** uses, and a record of the sync ([Where CueRithm keeps your data](../files/library/where-data-lives.md)).

The backup is made before the first change, and the database is changed last, in one transaction. Your music files and the rest of the rekordbox library are untouched.

## 8. Check the result in rekordbox { #check-in-rekordbox }

Open rekordbox. In the playlist tree you should find the **Club nights** folder with **Warm-up set** and **Peak hour** inside it, in the order you set. Load a track from Warm-up set on a deck. rekordbox shows:

- the title, artist, album, genre and cover art CueRithm sent
- a beat grid at the track's BPM. For <span class="cr-bpm">128</span> BPM tracks such as 17 (Extended Mix) the bars line up with the kick
- CueRithm's named cues on the waveform, such as `Drop` and `Breakdown`, as hot cues or memory cues according to your [cue layout](../analysis/cue-rules.md). 17 (Extended Mix) has its Drop at 1:15

Your music files were not changed. CueRithm added or updated only the tracks, playlists and files in this sync; the other tracks, playlists and settings in your rekordbox library were preserved. Re-analysing a synced track inside rekordbox replaces CueRithm's grid with rekordbox's own; that is a separate edit made in rekordbox.

## 9. Undo the sync if you need to { #undo }

Close rekordbox and rekordbox Agent again. In Sync, find the `applied` entry under **rekordbox sync history** and choose **Restore backup**. A panel asks **Restore this backup?**; choose **Confirm restore**, or **Keep current library** to back out.

**What you should see:** **Backup restored**. The Club nights folder and the new tracks are gone from rekordbox, and CueRithm's records of the sync are rewound to match.

Restore works only while rekordbox's library and files still match the result of that sync. Edits you make in rekordbox afterwards, including a tag reload or a re-analysis, make CueRithm refuse rather than overwrite them. [Preview, apply and restore a sync](preview-apply-restore.md#restore) lists the exact checks.

## If something goes wrong { #what-can-go-wrong }

| What you see | Why | What to do |
|---|---|---|
| **Read only** and "Close the target DJ software before syncing" | rekordbox or rekordbox Agent is still running | Quit both, then press the refresh button next to **Installed library** |
| "No rekordbox library found. Select a native installation or configure its Windows library for Wine / Proton below." | rekordbox is not installed, was never started, or runs under Wine | Start rekordbox once, then refresh. For Wine use **Set up another library** ([setup guide](windows-libraries-on-linux.md#rekordbox-on-wine)) |
| "Complete smart semantics require explicit whole-list materialization" | The scope contains a smart playlist | Pick a folder without smart playlists, or tick **Convert smart playlists to track lists** |
| "Library changed; create a new rekordbox preview" or "Native database changed after preview" | Something changed after the preview | Choose **Preview rekordbox sync** again |
| "Discard the existing preview or recover the interrupted rekordbox operation first" | An earlier preview or interrupted sync is still open | Use **Discard preview** or **Recover sync** in the history |
| "New rekordbox tracks need a title. Include title in Metadata fields without skipping it, then preview again." | **Title** is unticked | Tick **Title** under **Metadata fields** |
| "Audio source changed after preparation; prepare the track again before native sync" | A file changed after CueRithm analysed it | [Analyze the track again](../analysis/analyze-again.md) |
| The sync completed but rekordbox shows nothing | Wrong library, or rekordbox was open while you checked | See [My sync does not show up in rekordbox](../help/troubleshooting/rekordbox-sync-problems.md) |

Cancelling during **Syncing rekordbox** is safe. If it stopped partway, the history shows an entry with **Recover sync**. CueRithm writes the new analysis and artwork files first and changes rekordbox's database last, so a sync that stopped before the database changed is rolled back by **Recover sync**, and one that stopped after is completed by it. See [Recover an interrupted sync](preview-apply-restore.md#recover).

## What you learned

- Direct sync writes to rekordbox's own library, needs rekordbox and its Agent closed, and shows a **Ready for preview**, **Read only** or **Library unavailable** status first.
- You choose a scope and what to include, preview, then apply that exact preview. Tracks rekordbox does not have are added without per-field choices.
- A verified copy of the database is saved before each sync, and **Restore backup** returns to it while rekordbox is unchanged.

## Next steps

- [Add only new tracks to rekordbox](rekordbox-new-tracks.md): send this week's purchases and nothing else.
- [Resolve rekordbox sync conflicts](rekordbox-conflicts.md): what to do when a track is already in rekordbox.
- [Preview, apply and restore a sync](preview-apply-restore.md): what each safeguard guarantees if a sync is interrupted.
- [Save a destination for next time](saved-destinations.md): name a library you sync to often.

<!-- verified: apps/desktop/src/views/RekordboxSync.tsx; components/RekordboxConflicts.tsx; components/RekordboxEntities.tsx; src/test/rekordbox-sync.test.tsx; crates/cuerithm-rekordbox/src/write_profile.rs (protocol gate, compatible_patch_builds test); crates/cuerithm-library/src/rekordbox_sync_service.rs (preflight, ensure_closed); rekordbox_sync_service/planning.rs, publication.rs; crates/cuerithm-library/src/native_playlist_sync.rs (smart list message); crates/cuerithm-dj-sync/src/discovery.rs (process names); docs/research/rekordbox-direct-contract.md (accepted profiles); legacy guides/sync.md re-checked -->
<!-- UNVERIFIED: the sample counts in step 6 and 7 are placeholders for what the app prints; the exact tree position of Club nights in rekordbox; that Activity Monitor and Task Manager list the process names exactly as shown -->
