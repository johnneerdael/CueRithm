# Use a Windows DJ library on Linux

<p class="lead">If your DJ program runs in Wine or Proton, tell CueRithm where its prefix is and which Windows drives hold your music, and Sync writes the library as that program expects. Setting it up takes about ten minutes and you save it once.</p>

**Before you start:**

- [ ] The DJ program is installed in a Wine or Proton prefix on this computer and has been started once, so its library exists.
- [ ] You know the folder of that prefix, and which Windows drives or network shares your music sits on (`D:\Music`, say).
- [ ] The program is closed. CueRithm never starts Wine or the DJ program; choosing a library does not launch anything.

This applies to rekordbox, VirtualDJ, Engine DJ, djay Pro and Serato DJ Pro 4. Mixxx runs natively on Linux and always uses the native setting ([Sync to Mixxx](mixxx.md)). On Windows itself you do not need any of this: leave **Application environment** on **Native installation**.

## 1. Choose Windows through Wine / Proton { #pick-environment }

In **Sync**, choose the destination in **Sync to**. Under the library location, open **Application environment** and choose **Windows through Wine / Proton**. New fields appear:

| Field | What to enter |
|---|---|
| **Wine or Proton prefix** | The prefix folder on this computer. The folder button opens a picker |
| **Windows installation folder** | Optional. The folder, or the program file, of the installed program inside the prefix. For rekordbox it must contain the executable you choose below, otherwise you see "Selected Windows installation and executable do not match" |
| **Windows root** and **Host folder** | One pair for each Windows drive or share, added with **Add drive mapping** |

## 2. Map the Windows drives to folders on this computer { #map-drives }

A DJ library stores Windows paths such as `D:\Music\Warm-up set\track.flac`, while CueRithm reads the same file at a Linux path. A drive mapping connects the two. Choose **Add drive mapping** for each drive or share the library uses.

| Windows root | Host folder |
|---|---|
| `C:\` | the prefix's `drive_c` folder |
| `D:\Music` | `/mnt/music` |

CueRithm reads your music from the host folder and writes the Windows reference into the library. The rules:

- You need at least one mapping, and every Windows root must be different.
- Every host folder must exist, be an absolute path without `..`, and be used by only one mapping.
- If a track's file is not under any mapping, or a path could mean two things, the preview stops and says which path ("Source has no selected Windows drive mapping", "Windows reference has an ambiguous reverse alias"). CueRithm never guesses a replacement.

Keep the mappings the same for later syncs. A changed mapping, installation or library invalidates a preview you have already made, and a later repair of moved files relies on the original mappings.

## 3. Choose the library { #choose-library }

Enter the library's location as a folder on this computer, or use the folder button. For VirtualDJ, Engine DJ, djay Pro and Serato DJ Pro 4 this is the folder inside the prefix that holds the library, as the guide for that program describes: [VirtualDJ](virtualdj.md), [Engine DJ](engine-dj.md), [djay Pro](djay-pro.md), [Serato DJ Pro](serato-dj-pro.md).

Three details are specific to Windows libraries:

- **VirtualDJ** keeps a separate database on each drive. A track on `D:` is written to `D:\VirtualDJ\database.xml`, which must already exist: "VirtualDJ drive database is missing; initialize this drive in VirtualDJ first".
- **Engine DJ** desktop libraries can refer to music on another Windows drive. Copying audio stays a separate option.
- A smart playlist rule that depends on a file location is sent as its saved tracks when the mapping cannot keep its meaning. The preview says so.

## rekordbox in Wine or Proton { #rekordbox-on-wine }

Linux has no native rekordbox, so a library there always needs a manual setup.

1. Choose **rekordbox 6 / 7** in **Sync to**, then open **Set up another library**.
2. Type a **Library setup name**.
3. Set **Application environment** to **Windows through Wine / Proton**, then fill in the prefix, installation and drive mappings as above.
4. Fill in the four rekordbox locations, each with its folder button:

    | Field | What it is |
    |---|---|
    | **rekordbox executable** | `rekordbox.exe`. Other files are refused: "Select the Windows rekordbox executable for Wine/Proton" |
    | **rekordbox database** | `master.db` |
    | **rekordbox Agent options** | `options.json`, in the Agent's `storage` folder |
    | **rekordbox asset folder** | the `share` folder beside the database |

5. Leave **Version hint (optional)** empty unless the executable has no readable version. Then type the version, for example `7.2.19.0342`.
6. Choose **Save library setup**.

**What you should see:** the library appears in **Installed library** and the status block reads **Ready for preview** or **Read only** with a reason. Saving does not start a sync. Continue with [Sync to rekordbox 6 or 7](rekordbox-direct.md#close-rekordbox).

rekordbox keeps its data in the Windows user's `AppData\Roaming\Pioneer` folder: the database and `share` in `rekordbox`, the Agent options in `rekordboxAgent\storage`. Under Wine that folder sits inside the prefix's `drive_c\users` tree. Wine layouts vary, so look inside your prefix.

CueRithm writes to the same database and asset formats as on Windows. A format it does not recognise stays **Read only**. rekordbox's own playback under Wine has not been checked ([Check what works with your DJ software](../help/compatibility.md)).

## 4. Save the setup { #save-the-setup }

Name the setup and choose **Save setup** so you can pick it again ([Save a destination for next time](saved-destinations.md)). Changing a saved setup or its mapped folders discards any open preview. Restore and recovery keep the locations of the original sync and check them again before writing.

## What can go wrong { #what-can-go-wrong }

| What you see | Why | What to do |
|---|---|---|
| "Choose explicit Windows drive mappings" | No drive mapping was added | Choose **Add drive mapping** |
| "Duplicate Windows mapping root", "Ambiguous physical mapping roots" | Two mappings share a Windows root or a host folder | Remove or correct one |
| "Source has no selected Windows drive mapping" | A synced track sits on a drive you did not map | Add a mapping for its drive |
| "Select the Windows rekordbox executable for Wine/Proton" | The executable field points at something other than `rekordbox.exe` | Pick the executable inside the prefix |
| "Target environment, mapped root, or installation changed since preview" | A mapping or the prefix changed after the preview | Preview again |
| "Case-colliding host entries cannot share a Windows reference" | Two files differ only in letter case on this case-sensitive system | Rename one of them |

## Related

- [Sync to rekordbox 6 or 7](rekordbox-direct.md)
- [Save a destination for next time](saved-destinations.md)
- [Preview, apply and restore a sync](preview-apply-restore.md)
- [Install on Linux](../get-started/platforms/linux.md)

<!-- verified: apps/desktop/src/components/SyncTargetSetup.tsx; components/ManualRekordboxSetup.tsx; views/Sync.tsx (SyncTargetSetup hidden for Mixxx); src/test/sync-target-setup.test.tsx; crates/cuerithm-rekordbox/src/target_context.rs (validate, mapping errors); rekordbox_sync_service.rs (configure_rekordbox_sync_target: rekordbox.exe, database/options regular files); discovery.rs (Pioneer/rekordboxAgent/storage/options.json, Roaming); crates/cuerithm-dj-sync/src/sync.rs (VirtualDJ drive database message); docs/research/windows-linux-dj-sync-contract.md; legacy guides/sync.md re-checked -->
<!-- UNVERIFIED: Wine or Proton layouts under drive_c/users; CueRithm and the DJ programs were not run together under Wine -->
