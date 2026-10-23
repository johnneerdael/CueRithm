# Save a destination for next time

<p class="lead">Give a DJ library, USB stick or Wine setup a name so you can pick it again in one click instead of browsing for it. Saving takes under a minute and never starts a sync.</p>

## What CueRithm remembers without being asked { #remembered-choices }

Before you save anything, Sync already remembers some choices on this computer:

| Remembered | For which destinations |
|---|---|
| The destination in the **Sync to** menu, including **rekordbox 6 / 7** | All; it opens on your last choice |
| The library location, the scope, the playlist, every option ([Set the sync options](sync-options.md)), the field mappings and any Windows setup | VirtualDJ, Engine DJ, djay Pro, Serato DJ Pro 4, Pioneer USB and Mixxx, each kept separately. Mixxx never restores a Windows setup |
| Nothing but the destination | rekordbox: its library is found again each time, and its scope and options start from their defaults |

A preview is never remembered. After you return, choose **Preview sync** or **Preview rekordbox sync** again.

A saved setup is for something different: it keeps a *location* under a name, so you can switch between several, for example two USB sticks ([Prepare a Pioneer USB stick](pioneer-usb.md)) or a Mixxx library kept with `--settings-path` ([Sync to Mixxx](mixxx.md)).

## What a saved setup keeps { #what-a-setup-keeps }

A saved setup holds where the library is: its folder or drive, and for a Windows library under Wine or Proton the prefix, the installation and the drive mappings. For a rekordbox library it also holds the paths to the executable, the database, the Agent options and the asset folder. It does not hold the scope, the options or any key or credential. Names are 1 to 100 characters with no control characters. Setups are stored in CueRithm's own library on this computer.

## Save a setup { #save-a-setup }

1. In **Sync**, choose the destination in **Sync to** and set the library: browse to it, pick it from **Found libraries**, or enter its location. For rekordbox, choose the library in **Installed library** or fill in [**Set up another library**](windows-libraries-on-linux.md#rekordbox-on-wine).
2. In **Setup name**, type a name such as `Booth stick` or `Studio laptop`.
3. Choose **Save setup**.

**What you should see:** the name appears in **Saved library setup** and stays selected. **Save setup** is unavailable until a library is set and a name is entered. The note beneath reads "Saving keeps this setup for later. Sync starts only after you preview and apply changes."

Saved setups are listed per destination: the **Saved library setup** menu in Engine DJ shows only Engine DJ setups, and the one in the rekordbox view shows only rekordbox setups.

## Use a setup { #use-a-setup }

Open **Saved library setup** and choose the name. The location, drive root and Windows setup fill in, and **Unsaved setup** at the top of the menu goes back to entering a location by hand. Then preview as usual.

A selected setup is part of the preview. If you edit it, replace it or delete it afterwards, the open preview is discarded, and applying a preview built from a setup that has since been removed is refused ("Saved target setup was removed; create a new preview").

## Change or delete a setup { #delete-a-setup }

To change a setup, select it, change the library or the **Setup name**, and choose **Save setup**; the same setup is updated. If it was changed elsewhere in the meantime you see "Saved target setup changed; reload it before saving". Select it again and redo the change.

To remove one, select it and choose **Delete saved setup**. Deleting a setup does not touch the library it points to, and syncs you have already made keep their backups.

## What can go wrong { #what-can-go-wrong }

| What you see | Why | What to do |
|---|---|---|
| **Save setup** is unavailable | No library is selected, or the name is empty | Set the library and type a name |
| "DJ target was not found; initialize it in the DJ app first" | The location does not exist yet. Mixxx is the exception: it can be created by the sync | Start the DJ program once, or correct the location |
| "This saved setup uses a path that cannot be edited as text. Choose the library again." | The saved location contains characters that cannot be shown as text | Choose the library again and save it under a new name |
| A saved USB stick is not found | The stick is mounted somewhere else | Mount it at the same place or drive letter, or save a new setup |

## Related

- [Sync to rekordbox 6 or 7](rekordbox-direct.md)
- [Use a Windows DJ library on Linux](windows-libraries-on-linux.md)
- [Set the sync options](sync-options.md)
- [Prepare a Pioneer USB stick](pioneer-usb.md)

<!-- verified: apps/desktop/src/components/SavedSyncTargets.tsx; components/ManualRekordboxSetup.tsx; views/Sync.tsx; views/RekordboxSync.tsx; sync-preferences.ts (what is persisted; Mixxx environment reset); sync-target-types.ts; src/test/saved-sync-targets.test.tsx; crates/cuerithm-library/src/sync_target_configurations.rs (name rules, revisions, no keys); dj_sync_service.rs (canonical_target); rekordbox_sync_service.rs (saved setup binding) -->
