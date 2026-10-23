# My sync does not show up in rekordbox

<p class="lead">A Sync that ends without tracks, cues or playlists in rekordbox has one of a handful of causes, and the Sync screen names most of them. Work down the table in about ten minutes.</p>

## Why it happens { #why }

Sync writes only after you have a fresh preview and choose **Sync rekordbox**. It can stop, or send less than you expected, at these points:

| What you see | Cause |
|---|---|
| **Sync rekordbox** is greyed out, or the status reads **Read only** with `Close the target DJ software before syncing` | rekordbox or **rekordbox Agent** is running. The Agent can keep running after you quit rekordbox. |
| **Read only** with `This rekordbox library format is not supported for syncing changes` | CueRithm does not recognise this version's database or asset format. You can preview but not write. |
| **Read only** with `Discard the existing preview or recover the interrupted rekordbox operation first` | An earlier preview is still open, or a sync was interrupted. |
| `No rekordbox library found. Select a native installation or configure its Windows library for Wine / Proton below.` | rekordbox has never created its library on this computer, or the library is somewhere CueRithm does not look. |
| **Library unavailable** | CueRithm could not open the library. The line under it says why. |
| The sync completed, but a track is missing | It was not eligible. Only prepared tracks in **Tracks** are synced. Tracks in Incoming or Archive are left out, and **Modified since last sync** sends only what changed. |
| The tracks arrived, but playlists, My Tags or cues did not | The matching box in **Include in sync** was off: **Cues and loops**, **Beatgrids**, **Artwork**, **Playlists and folders** or **My Tags**. |
| The preview warns `Playlist and My Tag changes await missing tracks…` | The playlists need tracks that are not in the sync scope yet. All playlist changes are withheld from that preview. |
| The preview warns `New rekordbox tracks need a title…` | **title** is switched off or skipped under **Metadata fields**. |
| Values in rekordbox did not change | Those fields were set to **Keep rekordbox**, **Skip this field**, or left unresolved. A skipped field stays unsynced. |
| `Library changed; create a new rekordbox preview` (or a similar message ending the same way) | You changed the library, a playlist, settings or a file after the preview. |
| A smart playlist is missing | Smart playlists may need **Convert smart playlists to track lists** before rekordbox can hold them. The preview says which ones become track lists. |

If you are unsure which rekordbox builds can be written to, [Sync to rekordbox 6 or 7](../../sync/rekordbox-direct.md) lists them, and [Check what works with your DJ software](../compatibility.md) records what was actually tested. In short, CueRithm writes by database and asset format, not by build number, and has tested playback on macOS builds 6.8.7.0303 and 7.2.19.0342.

## Fix it { #fix }

1. **Close rekordbox and rekordbox Agent.** Quit rekordbox, then end any `rekordboxAgent` process in Activity Monitor (macOS) or Task Manager (Windows). Press the refresh button beside **Installed library** so CueRithm checks again.
2. **Choose the right library.** In **Sync**, set **Sync to** to **rekordbox 6 / 7** and pick the entry under **Installed library**, which shows the rekordbox version and the path of its database. Open rekordbox once if the list is empty. For a library CueRithm does not find, including a Windows rekordbox under Wine or Proton on Linux, use **Set up another library** ([Use a Windows DJ library on Linux](../../sync/windows-libraries-on-linux.md)).
3. **Clear an old preview.** Open **rekordbox sync history**. Choose **Discard preview** for an unused preview, or **Recover sync** for an interrupted one.
4. **Check the scope and options.** Under **What to sync** choose **Full library**, or the playlist or folder you want. Tick the items under **Include in sync** that you expect to see in rekordbox ([Choose what to sync](../../sync/choose-what-to-sync.md), [Set the sync options](../../sync/sync-options.md)). Finish preparing any track that is still in Incoming.
5. **Choose Preview rekordbox sync again** and read the warnings above the table. Work through **Review fields**: choose **Use CueRithm**, **Keep rekordbox** or **Accept equal values** for each field, or use the bulk buttons. On a first sync every field needs an explicit choice ([Resolve rekordbox sync conflicts](../../sync/rekordbox-conflicts.md)).
6. **Choose Sync rekordbox.** When it finishes, the heading **Sync completed** appears with a line that reads *N tracks · N rows changed · N fields acknowledged · N fields skipped*, with your own counts. Wait for it before you open rekordbox.
7. **Open rekordbox** and look in the collection and the playlists. CueRithm does not sync while rekordbox is running, so open it only after the sync has finished.

If a sync went wrong, **Restore backup** in **rekordbox sync history** reverts it, provided you have not changed the rekordbox library since ([Preview, apply and restore a sync](../../sync/preview-apply-restore.md)).

## Check it worked { #check }

- **rekordbox sync history** shows your sync with the status *applied*.
- The result line counts the tracks you expected.
- rekordbox lists the tracks, playlists and My Tags you included, and shows the cues and grids from CueRithm.

## Still stuck? { #still-stuck }

[Report the problem](../report-a-problem.md) with the exact text of the status block and any warning, your rekordbox version, and your operating system. Do not attach the rekordbox database or its Agent options file. The same preview, apply and restore steps apply to the other destinations; [Send your library to DJ software](../../sync/index.md) lists them.

## Related

- [Sync to rekordbox 6 or 7](../../sync/rekordbox-direct.md)
- [Add only new tracks to rekordbox](../../sync/rekordbox-new-tracks.md)
- [Resolve rekordbox sync conflicts](../../sync/rekordbox-conflicts.md)
- [Check what works with your DJ software](../compatibility.md)

<!-- verified: apps/desktop/src/views/RekordboxSync.tsx (labels: Installed library, Ready for preview/Read only/Library unavailable, "No rekordbox library found…", What to sync options, Include in sync items, Preview rekordbox sync, Sync rekordbox, Discard preview, Recover sync, Restore backup, rekordbox sync history, outcomeText); RekordboxConflicts.tsx (Use CueRithm, Keep rekordbox, Accept equal values, Skip this field, First sync: choose explicitly); ManualRekordboxSetup.tsx (Set up another library); crates/cuerithm-library/src/rekordbox_sync_service.rs (preflight order, ensure_closed, ensure_settled, require_capability messages, verify_* "create a new rekordbox preview" messages), rekordbox_sync_service/planning.rs (warnings, can_apply), crates/cuerithm-dj-sync/src/discovery.rs (process names rekordbox, rekordboxagent), crates/cuerithm-rekordbox/src/write_profile.rs (family 6/7 + recognised schema, no build list), dj_sync verify_local_rows (ready and not archived); README Compatibility and validation; docs/research/rekordbox-direct-contract.md; legacy guides/sync.md re-checked. UNVERIFIED: the exact smart-playlist behaviour when Convert smart playlists to track lists is off -->
