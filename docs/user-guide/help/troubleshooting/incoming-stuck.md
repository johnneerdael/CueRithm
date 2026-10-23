# A track is stuck in Incoming

<p class="lead">A track leaves Incoming only when every step you configured has finished. If one stays, it is either still working or it stopped with an error that the row spells out. This page tells the two apart and clears the common errors in a few minutes.</p>

## Why it happens { #why }

Look at the header next to the search box first:

- **Preparing N tracks** with a spinner means work is running or queued. Nothing is wrong. CueRithm prepares several tracks at once, and the first run on a big batch takes time.
- **N need attention** means the remaining tracks stopped. Each has an error in the coloured second line of its row, and a warning icon.
- **All up to date** with tracks still in Incoming should not happen. [Report it](../report-a-problem.md).

[Follow a track through Incoming](../../files/follow-incoming.md) lists the stages. The row text tells you the cause:

| Row text | Cause | What to do |
|---|---|---|
| **Waiting for file** for a long time, then `File copy did not settle` | Another program keeps changing the file, for example a download or a sync client. CueRithm waits up to five minutes for it to stop. | Let the copy finish, then press the retry icon. |
| `Audio file is missing` or `Incoming file is missing` | The file was moved or deleted before CueRithm reached it. | Put it back, or add it again from its new location. See [I moved or renamed my files](files-moved.md). |
| `Audio decoder failed: …`, `No audio stream found`, `Unsupported audio rate/channel layout` | The file is damaged, protected or not audio CueRithm supports. | Try it in another player. See [Check that your files are supported](../../files/supported-formats.md). |
| `Media scan exceeded its output limit` or `Media scan exceeded its time limit` | The track is longer than about 24 minutes, or decoding took more than five minutes. | Long mixes do not prepare. See [what CueRithm does not do yet](../not-available-yet.md#long-tracks). |
| `Key is unresolved`, `BPM and grid are unresolved`, `Energy is unresolved`, `Cue analysis has not run` | The audio is silent or too short to measure. CueRithm needs about 10 seconds for timing and 3 seconds for the key. It never guesses a value to fill the gap. | Check the file in a player. A track that cannot yield a key, BPM, energy and cues stays in Incoming. |
| `Configure automatic tagging in Settings` or `Configure automatic cleanup in Settings` | A switch is on but its settings were never saved. | Open **Settings**, review the page, and choose **Save settings**. |
| `Models changed since settings were saved; save Settings again` | The built-in analysis model changed after you saved Settings. | Choose **Save settings** again. |
| `No catalog match; review required`, `Catalog identity or availability requires review`, `Source requires a FlareSolverr-compatible solver; configure it in Settings → Automatic tagging` | Catalog tagging could not finish. | [Catalog tagging finds nothing](no-catalog-match.md). |
| A warning line above the table, `Folder is offline: …` | A watch folder's drive is not connected. | Reconnect the drive or fix the folder in **Settings** → **Incoming**. |

Other errors, such as those from organising files, are described in [Fix a track that needs attention](../../files/needs-attention.md).

## Fix it { #fix }

1. Open **Incoming** and read the second line of the stuck row. Find it in the table above.
2. Fix the cause. For a file problem, fix the file. For a settings problem, change the setting.
3. Press the retry icon at the right of the row (its tooltip reads **Retry preparation**). It runs the track again from the start.
4. To retry every track at once, choose **Save settings** in **Settings**. Saving re-queues every track that needs attention, with the settings you just saved.
5. For a catalog problem, click the warning icon on the row. It opens **Review match**, where you can compare the proposed fields and choose a different result ([Review and fix a wrong match](../../tagging/review-a-match.md)).
6. If a file will never prepare and you no longer want it in the library, select its row and choose **Archive** in the toolbar. The track leaves Incoming and the file on disk is not touched.

## Check it worked { #check }

- The row disappears from Incoming and the sidebar count for **Incoming** drops by one.
- The track appears in **Tracks** with a key, a BPM and an energy rating. [Read a track's key, BPM and energy](../../analysis/read-key-bpm-energy.md) shows how to read them.
- The header reads **All up to date**.

## Still stuck? { #still-stuck }

Copy the row's error text exactly and [report the problem](../report-a-problem.md) with the file type and CueRithm version. Check [Frequently asked questions](../faq.md) for the list of formats and limits first.

## Related

- [Follow a track through Incoming](../../files/follow-incoming.md)
- [Fix a track that needs attention](../../files/needs-attention.md)
- [Status icons and job states](../../reference/status-icons.md)
- [Catalog tagging finds nothing](no-catalog-match.md)

<!-- verified: apps/desktop/src/App.tsx (header "Preparing N tracks"/"N need attention"/"All up to date", watch-warning line); TrackTable.tsx (stages labels, attention row with Review match button and "Retry preparation" button, row text); crates/cuerithm-library/src/prepare.rs (process(), fail_for_run -> attention, retry_attention on save via commands.rs save_settings), watch.rs (wait_for_stable 300 s "File copy did not settle", "Folder is offline"), settings.rs ready() errors, jobs.rs validate_analysis ("Key is unresolved", "BPM and grid are unresolved", "Energy is unresolved", "Cue analysis has not run", "Invalid replacement cues"), lexicon_timing/mod.rs (10 s minimum), docs legacy technical/key.md (3 s key guard) re-checked, cuerithm-media cancel.rs/runtime.rs messages, cuerithm-autotag write.rs and vendor/onetagger/providers/http.rs messages; App.tsx toolbar Archive; store.rs set_archived; playlists/queries.rs (Incoming view = incoming and not archived) -->
