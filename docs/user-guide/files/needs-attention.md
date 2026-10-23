# Fix a track that needs attention in Incoming

<p class="lead">A track that needs attention stopped at one stage and waited for you, instead of reaching Tracks with a guess. Read the message under its title, apply the matching fix, and press Retry. Most fixes take a minute.</p>

## What you see { #what-you-see }

- The row shows a **warning triangle**, and the message replaces the artist line under the title.
- A circular **Retry** arrow sits at the end of the row.
- The header reads *1 need attention* (or *3 need attention*) once nothing else is running. In Tracks and Archive, a button with the same text takes you to Incoming.
- Your earlier results, if the track had any, are kept. A failed preparation never replaces cues.

## Why it happens { #why }

The message tells you which stage failed. Match yours to the closest line.

| Message (start of the text) | Stage | Cause | Fix |
|---|---|---|---|
| *File copy did not settle* | Waiting for file | The file kept changing for five minutes, for example a download or cloud sync that has not finished | Wait for the file to finish, then press **Retry** |
| *Source changed during capture*, *Source changed during decode*, *Source changed before analysis* | Decoding | Another program modified the file while CueRithm read it | Let that program finish, then press **Retry** |
| *Audio decoder failed: …* | Decoding | The file is damaged or truncated, is not audio, or is protected by digital rights management | Play it in another player. Replace it with a good copy. See [Check that your files are supported](supported-formats.md) |
| *No audio stream found*, *Unsupported audio rate/channel layout* | Decoding | The file has no usable audio | Replace it or [archive](archive.md) the track |
| *Media scan exceeded its output limit* or *…its time limit* | Decoding | The track is longer than about 24 minutes, or decoding took longer than five minutes | Long mixes cannot be prepared. See [the length limit](supported-formats.md#limits) |
| *Key is unresolved*, *BPM and grid are unresolved*, *Energy is unresolved*, *Invalid replacement cues* | Analyzing | The audio did not give CueRithm a required result, for example it is silent or extremely short | Try [Analyze again](../analysis/analyze-again.md). If it fails again, this file cannot be prepared |
| *Configure automatic tagging in Settings*, *Configure automatic cleanup in Settings* | Setup | A switch is on but its rules were never saved | Open Settings → Automatic tagging or Cleanup, complete the page, **Save settings** |
| *Catalog identity or availability requires review* | Matching catalog | Two releases fit equally well, or a provider could not be used | Select the warning triangle to open **Review match**. See [Review and fix a wrong match](../tagging/review-a-match.md) |
| *No catalog match; review required* | Matching catalog | No provider found the track and **No catalog match** is set to *Keep in Incoming for attention* | Review it by hand, or change the setting to *Keep existing tags and finish* |
| A message about a rejected key, a rate limit or an unreachable provider | Matching catalog | The provider failed. This is not the same as "no match" | [Connect provider accounts](../tagging/provider-accounts.md); for Cloudflare blocks see [Get past a Cloudflare block](../tagging/cloudflare-solver.md) |
| *Captured preset folder changed; reconnect and explicitly retry current rules*, *Source left its captured preset folder* | Setup | The folder assignment this track was added under needs reconnecting, or the file left that folder | Follow [Use different rules for different folders](folder-presets.md#reconnect), then press **Retry** |
| *Source unavailable: …* | Waiting for file | You restored an archived track whose file is no longer there | Reconnect the drive or put the file back |
| *Models changed since settings were saved; save Settings again* | Waiting for file | The built-in analysis models changed after an update | Open Settings and select **Save settings** |

## Fix it { #fix-it }

1. **Read the message.** Hover the warning triangle if the text is cut off.
2. **Fix the cause** using the table. If it names a setting, change it in Settings and keep the page open.
3. **Retry.** Press the circular arrow at the end of the row. Retry uses your current settings. You can also select **Save settings**, which retries every track that needs attention at once. If the track already has tag or rename work in progress, CueRithm finishes that work with the rules it captured earlier.
4. **Retry many tracks.** After one fix that applies to all (a provider key, for example), **Save settings** is quicker than pressing Retry on every row.
5. **Give up on a track.** Select it and choose **Archive**. It leaves Incoming and stops being prepared, and nothing is deleted. Restore it later from the [Archive](archive.md).

## Check it worked { #check }

The row's text changes to a stage name such as *Decoding*, then the track leaves Incoming and appears in Tracks with a key, BPM and energy. If the header now reads *All up to date*, nothing is waiting.

## Still stuck? { #still-stuck }

- A track that never fails but never finishes: [A track is stuck in Incoming](../help/troubleshooting/incoming-stuck.md).
- Catalog tagging that finds nothing for tracks you know exist: [Catalog tagging finds nothing](../help/troubleshooting/no-catalog-match.md).
- Report the exact message with CueRithm's version: [Report a problem or ask for a feature](../help/report-a-problem.md).

## Related

- [Follow a track through Incoming](follow-incoming.md)
- [Status icons and job states](../reference/status-icons.md)
- [Check that your files are supported](supported-formats.md)

<!-- verified: apps/desktop/src/components/TrackTable.tsx (status(): attention triangle and Review match button; Retry preparation button; row message replaces artist line); apps/desktop/src/App.tsx (attention-link, header counts); crates/cuerithm-library/src/watch.rs (File copy did not settle); crates/cuerithm-media/src/decode.rs, probe.rs, runtime.rs, cancel.rs (messages and limits); crates/cuerithm-library/src/jobs.rs (validate_analysis messages; retry); crates/cuerithm-library/src/settings.rs (ready(): Configure automatic ...); crates/cuerithm-library/src/prepare.rs (Models changed...; save_settings retry_attention in src-tauri commands.rs); crates/cuerithm-autotag/src/write.rs (catalog messages); crates/cuerithm-library/src/preparation_presets/resolution.rs and assignments.rs (preset messages); crates/cuerithm-library/src/store.rs (set_archived: Source unavailable); apps/desktop/src/views/TaggingSettings.tsx (No catalog match options); legacy guides/incoming-archive.md#when-a-track-needs-attention re-checked -->
