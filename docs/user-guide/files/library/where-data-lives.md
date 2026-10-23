# Where CueRithm keeps your data

<p class="lead">Where is my library, what are all those backups taking space, and what is safe to leave alone? This page answers all three and shows what is stored on your computer outside the library.</p>

## Your music stays where it is { #music }

CueRithm never keeps a copy of your music. The library stores a track identity, its original file reference and an optional verified local binding, plus accepted metadata and analysis. A checksum can be unknown for an unresolved imported track. Your files live wherever you put them, for example `~/Music/Beatport`.

Analysis alone never changes a music file. CueRithm writes to one only when you switch something on or ask for it: [automatic tagging](../../tagging/tag-new-tracks-automatically.md), [cleanup](../../tagging/cleanup-messy-titles.md), saving a change in [Edit tags and artwork](../../tagging/edit-tags-by-hand.md), copying from a donor in a [duplicate merge](find-duplicates.md#donor), or [Organize files](../../rename/organize-files.md) renaming or moving it. Before it rewrites a file, it keeps the original. Moving a file yourself in Finder or Explorer breaks the stored location; see [Find missing, moved or broken files](find-missing-files.md#moved-files).

## Find the data folder { #data-folder }

Everything else lives in one folder, named after CueRithm's application identifier `com.cuerithm.app`:

| System | Data folder |
|---|---|
| macOS | `~/Library/Application Support/com.cuerithm.app` |
| Windows | `%APPDATA%\com.cuerithm.app` |
| Linux | `~/.local/share/com.cuerithm.app`, or `$XDG_DATA_HOME/com.cuerithm.app` when that variable is set |

On macOS the `Library` folder is hidden. In Finder choose **Go → Go to Folder…** (++cmd+shift+g++) and paste the path. Your audio files are never inside this folder.

## What is in the folder { #what-is-in-the-folder }

| Item | What it holds | Leave it alone? |
|---|---|---|
| `library.sqlite`, or `libraries/<id>/library.sqlite`, with the matching WAL, SHM and lock files | The library: tracks, analysis, cues, playlists, settings, presets and history. It is one SQLite database. The `.lock` file keeps two copies of CueRithm from opening it together | Yes. Never edit or delete it. Use [Library backup](back-up-library.md) to protect it |
| `active-library.json`, activation journals and `libraries/` | Select the active library and retain replaced libraries for recovery | Yes. Use [Retained libraries](restore-library.md#go-back), not manual file replacement |
| `.library-installation-id` | Identifies a library to same-installation recovery snapshots | Yes. Those snapshots need it |
| `analysis-components/` | The built-in analysis parameters, installed and verified when CueRithm starts | CueRithm recreates it |
| `cache/` (`media`, `artwork`, `waveforms`, `fingerprints`, `dj-timing`) | Working copies and derived data: decode scratch folders, extracted cover art, waveforms, decoded-audio fingerprints and DJ timing. `cache/media` is cleaned at every start | These are caches. Do not touch them while CueRithm is open |
| `tagging/` | The original of every file CueRithm tagged, so a tag write can be undone | Keep |
| `cleanup/` | Originals kept by cleanup | Keep |
| `metadata-edits/` | Staging and artwork for tag and artwork edits you make by hand, and their originals | Keep |
| `maintenance/` | `removal/` holds the originals of files you removed as unused; `staging/` holds working files for duplicate merges and their Undo | Keep until you [dispose of them](undo-maintenance.md#dispose) |
| `organization-references/` | Recovery records for file reorganisation and for DJ-library references | Keep |
| `sync/` | Previews, journals and backups of DJ-software syncs, for [restoring a sync](../../sync/preview-apply-restore.md) | Keep |
| `library-backups/` | A restore waiting for the next start, and the pre-restore backups of earlier restores | Keep |

The folders marked *Keep* are what file-level **Undo** and **Restore** use. Delete them and the matching recovery can disappear. These resources are separate from a portable archive, which excludes original audio and executable recovery journals. Same-installation snapshots still check them in place.

A library generation has its own database directory. Library-owned caches and recovery subfolders can sit beside that database; older libraries can still use the top-level location. Keep the whole data folder intact when preserving local recovery. A portable archive is the supported way to transfer library data without copying these machine bindings.

## See how much space the originals use { #space }

Open **Settings** at the bottom of the sidebar, choose the **Automatic tagging** page and look for the row **Recovery backups**. It shows the size and file count of the original-file backups from tagging, cleanup and manual edits. CueRithm keeps these without a limit and has no setting to trim them yet. Originals from unused-file removals have their own disposal step in [History & recovery](undo-maintenance.md#dispose).

## What is stored outside the data folder { #outside }

| What | Where | In a library backup? |
|---|---|---|
| Your music | Where you keep it | No |
| Portable `.cuerithm` files and same-installation snapshot folders | The destination you chose | They are separate backup formats |
| Provider keys and sign-ins | Your system's credential store: Keychain on macOS, Credential Manager on Windows, the Secret Service on Linux | No |
| Theme, waveform style, sidebar and player sizes, table columns | The app's own browser storage on this computer, not the library. See [Switch between light and dark](../../customize/theme.md) and [Resize the sidebar and player](../../customize/layout.md) | No |
| The default watch folder | `Music/Cuerithm/Incoming` inside your Music folder, created for a new library | No (the folder's files are music) |

## Remove everything { #uninstall }

Removing the application does not remove its data folder. To wipe CueRithm completely, quit it, uninstall it and delete the data folder above. That deletes the library and every retained original. Your music stays. Make a [library backup](back-up-library.md) first if you might want the work back.

Looking for how the library is built? See [How is the library stored?](../../how-it-works/library.md).

## What this means in CueRithm { #summary }

- Your music and the library are separate. Back up both: music with your usual backup, the library with [Library backup](back-up-library.md).
- The retained originals are what make Undo possible. They take space on purpose.
- Leave the data folder to CueRithm. Use the screens in this tab to change what is inside it.

## Related

- [Back up your library](back-up-library.md)
- [Restore your library from a backup](restore-library.md)
- [Undo a merge or restore removed files](undo-maintenance.md)
- [Find the setting you need](../../customize/find-a-setting.md)

<!-- verified: apps/desktop/src-tauri/src/lib.rs (app_data_dir, audio_dir()/Cuerithm/Incoming), host.rs (library.sqlite, cache/media cleaned at start, cache/artwork, waveforms), builtin.rs (analysis-components); crates/cuerithm-library/src/library_backup.rs (library-backups, .library-installation-id, RESOURCE_DIRS tagging/cleanup/maintenance/sync/organization-references/metadata-edits, .lock file); maintenance/scan.rs (cache/fingerprints), removal.rs (maintenance/removal), donors.rs (maintenance/staging), dj_sync_service.rs (cache/dj-timing, sync); tagging.rs tagging_storage (what Recovery backups counts); crates/cuerithm-credentials/src/native.rs; theme.ts, waveform-style.ts, App.tsx, TrackTable.tsx (localStorage); Tauri path docs via ctx7 (app_data_dir = data dir + identifier on macOS, Windows Roaming AppData, Linux XDG_DATA_HOME or ~/.local/share); legacy technical/library.md and get-started/install.md re-checked (legacy marked the Windows and Linux paths unverified; confirmed by Tauri docs; legacy schema 22 is outdated, the code is at 29). UNVERIFIED: that the caches can be rebuilt after deletion, and the exact location of the web storage on each system. -->

<!-- verified: portable storage and retained generations re-checked against apps/desktop/src-tauri/src/host.rs; crates/cuerithm-library/src/generations.rs; backup/spool.rs; source locators and optional local bindings in portable.rs. -->
