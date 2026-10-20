# Packaged-app walkthrough of the Maintenance screens, 2026-10-20

The macOS `CueRithm.app` built with `pnpm tauri build --bundles app` (release profile, bundled media runtime) was started
against a generated library of 10,000 tracks and driven through the Maintenance screens with the accessibility API.
`CUERITHM_DATA_DIR` pointed at a throwaway data folder, so nothing touched a real library. The library came from
`build_library_fixture_for_the_desktop_app` in `crates/cuerithm-library/tests/scale_benchmark.rs`, which writes the
files and the library with no analysis jobs pending (10,000 one-second WAV files, 20 duplicate groups).

## What was walked through

| Screen and step | Result |
| --- | --- |
| Library | "Tracks 10000", 200 of 10,000 rows shown |
| Duplicates: scan the whole library | 63 s; "20 duplicate groups among 10,000 tracks"; progress counted up to 10,000 |
| Open the review, select all mergeable | Reviewing 20 groups, then "20 selected to merge" |
| Preview the merge | "20 tracks will be archived and 0 tag/cue copies made. No music files are deleted." |
| Apply (confirmation dialog) | "Merge applied: 20 tracks archived"; library database shows 20 archived of 10,000 |
| Undo from the merge history | "Merge undone. 20 tracks restored"; database shows 0 archived; source files untouched |
| Duplicates with 500 groups | The scan and the review opened; with that many groups the rendered page is too long to reach the preview panel by accessibility alone, so the merge steps were walked through with 20 groups |
| Unused files: scan a folder of 30 stray audio files | "30 unused files · 470 KB" |
| Select all, Review removal, Remove 30 files (confirmation, Cancel focused by default) | "Removed 30 files"; the folder is empty |
| Restore from the removal history | "Restored 30 files to their original locations" |
| Remove again, Retained backups, select the removal, Review disposal | A second dialog needs a ticked "I understand this cannot be undone" box before the button enables; Cancel is focused by default |
| Dispose of backups | "Disposed of 30 backup files"; the retained copies are gone from application storage |

The library-level steps match the measurements in [the scale write-up](large-library-scale-2026-10-20.md).

## A bug the walkthrough found and the fix

After Restore, a removal's recovery copies stay in application storage (the backend keeps them until they are disposed
of, and a test says so), but the Retained backups tab only listed removals in the `committed` state. A restored
removal is `undone`, so its copies were never listed and could not be disposed of from the app: a space leak after every
Restore. The opposite problem sat next to it: a removal whose copies had already been disposed of stayed in the list as
"Unavailable", and selecting it could only fail.

The history now carries `backups_disposed`, and the tab lists a removal while its copies exist, whether it was restored
or not, and drops it once they are disposed of. Tests cover the backend history before and after Restore and disposal
and the tab's listing. After rebuilding the packaged app, the same library showed the leaked removal as "Restored",
disposing of it deleted its 30 files, and the table said "No removal backups are being retained."

## What this does not cover

- The Missing files tab, the Cleanup, Playlist tools and Rekordbox screens, and Windows and Linux windows.
- A real user library: the audio is generated and every file is one second long.
- The saved review for 500 groups cannot be reached by accessibility alone; a person scrolling could.
- It is an unsigned local build of the release profile run from its bundle, not the signed, notarized download.
