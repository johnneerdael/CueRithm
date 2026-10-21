# Sync to DJ software

Use **Sync** in the sidebar to send prepared tracks, metadata, cues, beatgrids and
playlists to a DJ library or USB drive. Each sync starts with a preview and saves a
recoverable backup.

<figure markdown>
  ![Sync destination, options and a preview of 24 tracks and three playlists](../assets/screens/85-sync-preview.webp){ .screen loading=lazy }
</figure>

## Choose a destination

The **DJ libraries and USB** tab contains the **Sync to** menu with five targets:

| Target | Choose |
| --- | --- |
| VirtualDJ | A current VirtualDJ 8.5 library folder |
| Engine DJ | A current Engine DJ library folder or target drive |
| djay Pro | A current djay Media Library folder |
| Serato DJ Pro 4 | A Serato Library folder |
| Pioneer USB | A mounted USB drive |

Initialize desktop libraries in their DJ application first. Choose a discovered
library, use the folder button to browse, or enter its location. The refresh button
checks for libraries again. Older library formats are not a separate export target.
Choose the **rekordbox 6 / 7** tab for a desktop rekordbox library. Pioneer USB
exports to a drive rather than a desktop rekordbox library.

Sync remembers the selected software and keeps the destination, scope and options
separately for each target. Returning restores these choices; create a fresh preview
before applying them.

## Sync directly to rekordbox 6 or 7

Native sync discovers installed rekordbox 6 and 7 libraries on macOS and Windows.
The installed version and database location appear under **Installed library**.
Linux does not support a native desktop rekordbox installation.

**Current acceptance status:** native writes remain unavailable for application
builds that have not passed isolated acceptance in rekordbox. A readable library
can appear as **Read only**. Review its availability notice; previewing a library
does not enable writes. Support is checked against the actual installed build.

1. Close rekordbox and **rekordbox Agent**. The Agent can remain running after
   closing the main application.
2. Open **Sync → rekordbox 6 / 7** and choose the installed library.
3. Choose the full library, a playlist or folder, or tracks modified since the last
   sync. Select the metadata, cues, beatgrids, artwork, playlists and My Tags to
   include. **Convert smart playlists to track lists** exports their saved members.
4. Choose **Preview rekordbox sync** and review the values from both libraries.
5. Choose which values to use. The first sync requires explicit choices, including
   **Accept equal values** when both libraries already agree. Bulk choices apply to
   unresolved fields with available values and preserve choices already made.
6. Preview again after changing choices. **Sync rekordbox** applies only this fresh
   preview, when the installed library supports writing.

**Use CueRithm** sends the local value to rekordbox. **Keep rekordbox** preserves
the native value; it does not rewrite source music tags. **Clear in rekordbox** is
an explicit removal. An **Empty** or **Unavailable** observation never requests a
removal automatically. Beatgrids can be retained or replaced; clearing a complete
beatgrid is unavailable.

**Create compatible WAV copies for unsupported audio** is off by default. Enable
it to sync formats that rekordbox cannot play directly. CueRithm creates separate
WAV copies for the native library and leaves source music unchanged. Review the
copy count and additional storage size before applying the preview. Without this
option, unsupported audio stops the preview with an explanation.

**Use nearest supported cue colors** is also off by default. Enable it to map
unsupported colors to the closest rekordbox color. The preview reports the cues
whose colors change; otherwise unsupported colors stop the preview for review.

For cues, **Replace listed rekordbox cues** and **Clear listed rekordbox cues**
explicitly replace or remove the recognized cues shown in the review. CueRithm
manages those cues after the sync commits. Unknown cue data stays protected.
The bulk **Use CueRithm for unresolved** action shows how many cue sets it includes.
Accepting equal cues alone does not transfer their management.

**Skip this field** leaves it unsynced and unacknowledged. Completed syncs report
the actual tracks, changed rows, acknowledged fields and skipped fields. A skipped
field stays available for review in a later preview.

Changing the destination, scope, options or local library invalidates the preview.
Later native edits also require another preview. Existing field choices remain
visible after a failed apply; **Reset choices** starts a new comparison without
those choices. **Cancel** waits for the active operation to stop before another
operation can begin. The refresh button rechecks availability after closing an app
or Agent.

Native previews and recovery information appear in **rekordbox sync history**,
including after restarting CueRithm. Discard an unused preview before preparing
another sync to the same database. Choose **Recover sync** for interrupted work.
**Restore backup** asks for confirmation and checks the saved library and file
identities before reverting that sync. Later destination edits prevent restoration.
If native changes were published but recovery cannot acknowledge later local
changes, a guarded restore may also be available. Restore does not overwrite
current local music files.

## Choose what to sync

Only prepared, active tracks are eligible. Incoming and archived tracks are excluded.
Finish preparation when a track has no saved analysis, and resolve smart playlists
that have no usable saved results before exporting.

| What to sync | Included |
| --- | --- |
| Full library | All eligible tracks and playlists |
| Playlist or folder | The selected playlist, or playlists inside the selected folder, with their eligible tracks |
| Modified since last sync | Eligible tracks and playlists changed since the last successful sync to this destination |

Smart playlists stay dynamic when the destination supports their entire definition.
Otherwise, they export their saved matching tracks as ordinary playlists. Enable
**All smartlists to playlists** to export ordinary playlists throughout. Pioneer USB
always uses ordinary playlists.

## Set the options

Options change with the destination. **Use nearest supported colors** adapts colors
to the target, while **Field mappings** chooses which local text value fills each
destination field. **Keep existing beatgrids** preserves destination beatgrids where
available. Engine DJ and Serato also offer **Beatgrid locks**.

Engine DJ can **Copy audio to the target drive**. Engine DJ and Pioneer USB can
**Add album art**. Serato and Pioneer USB offer **Convert keys** with original,
musical, Camelot and Open Key notation.
Engine DJ copies audio automatically for libraries at recognized mounted-drive
locations, including `E:\Engine Library` on Windows. Enable **Copy audio to the
target drive** for other folders when the export needs its own audio copies.

### Pioneer USB

<figure markdown>
  ![Pioneer USB options with key notation, cue destination and CDJ compatibility controls](../assets/screens/86-sync-pioneer-usb.webp){ .screen loading=lazy }
</figure>

Choose **Hot cues**, **Memory cues** or **Hot and memory cues** under
**Cue destination**. **Extra cue markers** adds the extra markers supported by this
export. Review any cue-limit or compatibility notices in the preview.

**CDJ compatibility** can leave audio unchanged, detect incompatible audio, or
convert incompatible audio. Conversion writes copies on the USB drive and leaves
source audio intact. **Clean up unused files** removes unreferenced files from the drive’s export
folders. Source files known to your library are protected; inspect removals in the
preview before syncing. If the drive contains two Pioneer library catalogs, disable
cleanup to preserve both.

## Preview and apply

1. Close the destination DJ application. For Pioneer USB, close rekordbox.
2. Choose **Preview sync**.
3. Review the track and playlist counts, compatibility notices, files to create,
   update or remove, and folders to create or remove.
4. Choose **Sync to** followed by the destination name to apply that preview.

Changing the library, destination, scope or options invalidates the preview. Create
a fresh preview before applying those changes. If the destination has changed since
preview, resolve the conflict and preview again.
Use **Discard preview** to remove unused prepared export files. Changing options,
replacing a preview or leaving Sync automatically cancels the unused preview.
Older prepared entries in **Sync history** also offer **Discard preview**; failed
cleanup stays available there for retry.
Reconnect a USB at its original mount location or drive letter before resuming its
history. If the filesystem cannot provide a persistent volume or catalog identity,
sync stops before publishing changes.

Full-library and modified syncs can remove playlists that CueRithm previously created
and that have since been removed locally. The preview lists these as owned playlists
to remove. A playlist matched to an existing destination playlist is retained when
removed locally; the preview reports matched playlists retained. Unrelated native
library entries are left alone. Edits in the destination that conflict with a sync
are reported for resolution.

## Backups and interrupted operations

**Sync history** lists completed syncs and operations with recovery information.
Choose **Restore backup** to reverse a sync. Restore checks for later destination
edits and stops on a conflict instead of overwriting them.
Archiving a synced track does not prevent restoring its backup; its source location
and file contents must still match the recorded sync.

Serato writes cues and grids into source music files, so its backup includes those
audio files as well as its native library files. Pioneer USB conversion backs up and
restores the affected USB copies; it does not replace the original source audio.

If an operation is interrupted, keep the destination available and return to
**Sync history**. Use **Finish sync** when offered to complete sync recovery, or
**Finish restore** to complete restore recovery. **Restore backup** is available
where the interrupted operation can be rolled back. Failed or interrupted work does
not advance the last successful sync until recovery completes.

This implementation remains under verification. The screenshots show the production
Sync interface with invented example data. Synthetic format checks and reference
parser agreement are separate from acceptance in the destination DJ applications
and on physical CDJ hardware; these images do not establish that acceptance.
