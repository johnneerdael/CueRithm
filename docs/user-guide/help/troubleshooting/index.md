# Fix a problem

<p class="lead">Find what you see in the table, open the page for it, and follow the steps. Each page says why the problem happens, how to fix it, how to check the fix, and what to do if it still fails. Most fixes take under ten minutes.</p>

## What are you seeing? { #what-are-you-seeing }

| What you see | Most likely cause | Page |
|---|---|---|
| A dash in the **PREVIEW** column, or the player will not load a track | CueRithm cannot read the audio file | [A track shows a dash or will not play](no-waveform-or-playback.md) |
| More tracks than files, titles twice, tracks marked **Missing** | You moved or renamed files outside CueRithm | [I moved or renamed my files](files-moved.md) |
| A track will not leave Incoming, or the header says *N need attention* | A step failed, or the file is still changing | [A track is stuck in Incoming](incoming-stuck.md) |
| No tags from Beatport or Traxsource, or **Matching catalog** never ends | Tagging is off, the site blocks requests, or there is no close match | [Catalog tagging finds nothing](no-catalog-match.md) |
| Another app shows a different key or BPM | Notation, tags, half or double tempo, or a constant grid | [My key or BPM differs from another app](key-or-bpm-differs.md) |
| The sync finished, but rekordbox shows nothing, or Sync is **Read only** | rekordbox is running, the scope or options leave things out, or the format is not recognised | [My sync does not show up in rekordbox](rekordbox-sync-problems.md) |
| Windows or macOS warns about CueRithm, or it closes at start | Unsigned Windows build, damaged download, missing decoder files | [My computer blocks CueRithm at first launch](first-launch-blocked.md) |
| Titles, artists or genres changed that you wanted to keep | A cleanup rule or scope was wider than you meant | [Cleanup changed too much](cleanup-changed-too-much.md) |

## Before you start { #before-you-start }

Three habits make every page faster:

1. **Read the message.** CueRithm names the cause on screen: in the second line of an Incoming row, in a message at the bottom of the window, or in a status block in **Sync**. Copy it exactly if you report the problem.
2. **Check which view you are in.** Tracks that are still being prepared are in **Incoming**; tracks you archived are in **Archive**. Neither appears in **Tracks** or in a sync. [Find your way around the window](../../get-started/main-window.md) shows the three views.
3. **Know your version.** The file name of the download carries it, for example `Cuerithm-arm64-0.1.13.dmg`. [Report a problem or ask for a feature](../report-a-problem.md) shows where else to find it.

None of the fixes ask you to delete audio files. Anything that changes tags or the library has an undo or a recovery backup, and each page names it.

## Check the installation { #check-installation }

If CueRithm will not start, or a bundled component seems damaged, run the built-in check. It verifies the bundled decoder and the key model and prints a single line of JSON in which `resources_verified` and `built_in_analysis_verified` are `true` when all is well. [Download and install CueRithm](../../get-started/install.md) shows the command for each system, and [My computer blocks CueRithm at first launch](first-launch-blocked.md) explains what to do with a failure.

## If your problem is not here { #not-here }

- [Frequently asked questions](../faq.md) answer the common questions, grouped by goal.
- [What CueRithm does not do yet](../not-available-yet.md) lists the limits of this version, so you can tell a missing feature from a fault.
- [Check what works with your DJ software](../compatibility.md) records what was actually tested.
- [Report a problem or ask for a feature](../report-a-problem.md) explains what to send, and what to leave out.

## Related

- [Get help](../index.md)
- [Frequently asked questions](../faq.md)
- [Status icons and job states](../../reference/status-icons.md)
- [Where CueRithm keeps your data](../../files/library/where-data-lives.md)

<!-- verified: troubleshooting pages in this folder; apps/desktop/src/App.tsx (header text, views), Sidebar.tsx (LIBRARY views); release.yml (artifact names Cuerithm-arm64-<version>.dmg); src-tauri/src/lib.rs diagnostics() output fields -->
