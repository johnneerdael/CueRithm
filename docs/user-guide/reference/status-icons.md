# Status icons and job states

<p class="lead">Every icon, dash, word and counter CueRithm uses to say what is happening to a track, what each one means and what to do next. Find the symbol you are looking at and follow the link on its row.</p>

!!! tip "New to this?"
    To watch a track go through these states, follow [Follow a track through Incoming](../files/follow-incoming.md). If something looks wrong right now, start at [Fix a problem](../help/troubleshooting/index.md).

## Icons in the Status column { #table }

The narrow column between the row number and the title shows one icon per track. It is empty when there is nothing to report.

| Icon | Name (hover text) | Meaning | What to do |
|---|---|---|---|
| Warning triangle | **Needs attention** | A stage failed and the track is waiting in Incoming. Hover for the reason. In the Incoming view the same reason is the second line of the row. Selecting the icon opens **Review match** for that track. | Read the reason and follow [Fix a track that needs attention](../files/needs-attention.md). If it is a catalog problem, use [Review and fix a wrong match](../tagging/review-a-match.md). |
| Turning circle | The stage name: **Queued**, **Waiting for file**, **Decoding**, **Analyzing**, **Matching catalog**, **Preparing tags**, **Cleaning up tags**, **Writing tags**, **Organizing files**, **Applying cues**, or **Preparing** when the stage has no label | The track is being prepared. See [Preparation stages](#stages). | Wait. If a track sits on one stage for a long time, see [A track is stuck in Incoming](../help/troubleshooting/incoming-stuck.md). |
| Tick | **Tagged from catalog match** | A catalog provider matched the track and its tags were applied | Nothing. To check or change the match, choose **Review match** in the row menu ([Review and fix a wrong match](../tagging/review-a-match.md)). |
| Tick | **Already tagged** | The track carried a OneTagger or CueRithm tagging marker, so the catalog step was skipped | Nothing. Analysis and cues still ran. |
| Tag symbol | **No catalog match; existing tags kept** | No provider matched. The file's own tags stayed as they were. | [Identify a track with no match](../tagging/identify-unknown-tracks.md) or [Catalog tagging finds nothing](../help/troubleshooting/no-catalog-match.md) |
| (empty) | none | No tagging result is recorded: automatic tagging was off when the track was prepared | Turn it on in [Automatic tagging settings](settings/automatic-tagging.md) and choose **Analyze again** if you want it |

## Buttons at the ends of a row { #row-buttons }

| Control | Name | What it does |
|---|---|---|
| Triangle on the left | **Play** *title* | Loads the track in the player and plays it |
| Three moving bars in its place | Playing | This row is the track that is playing |
| **⋯** at the right | **Actions for** *title* | Opens **Edit metadata**, **Review match**, **Curate**, **Analyze again** and **Archive track** (**Restore to library** in the Archive) |
| Circular arrow at the right | **Retry** *title*, tooltip "Retry preparation" | Replaces **⋯** on a track that needs attention. It tries the track again with your current settings. |

## Preparation stages { #stages }

A track in Incoming moves through these stages in this order. Which ones appear depends on what you turned on. Each stage is shown as its label in the status tooltip and as the second line of the row in Incoming.

| Stage label | Meaning | Only when |
|---|---|---|
| **Queued** | Waiting for a free worker | Always |
| **Waiting for file** | CueRithm is checking that the file has stopped changing | Always |
| **Decoding** | The bundled decoder is turning the file into the audio CueRithm analyses | Always |
| **Analyzing** | Key, BPM, beatgrid, energy and cues are being measured | Always |
| **Matching catalog** | Looking the track up with your providers | Automatic tagging is on |
| **Preparing tags** | Building the plan for tag changes | Tagging or cleanup is on |
| **Cleaning up tags** | Applying your cleanup rules to the planned tags | Cleanup is on |
| **Writing tags** | Writing tags to a staged copy, checking it and swapping it in | Tags changed |
| **Applying cues** | Saving the result and the new cues | Always |
| **Organizing files** | Renaming, moving or copying the file by your pattern, then repairing known DJ-library links to it | A file organization mode is on |

When the last stage ends, the track leaves Incoming and appears in **Tracks**. A track becomes ready only with a key, a positive BPM and beatgrid, an energy rating and at least one cue, plus any tagging, cleanup and organization you turned on.

## Job states { #job-states }

Behind the stages each preparation is a job with a state. You see states through the header and the Status column rather than by name.

| State | What you see | Meaning | Next |
|---|---|---|---|
| queued | Turning circle labelled **Queued** | In line | Wait |
| running | Turning circle with a stage name | A worker is on it | Wait |
| attention | Warning triangle, reason on the second line | The job stopped on an error. Provider errors and errors you can fix land here. | [Fix a track that needs attention](../files/needs-attention.md). Saving settings, or the retry arrow, tries again. |
| completed | The track is in **Tracks** | Done | Nothing |
| superseded | Nothing | A newer job replaced this one, for example you chose **Analyze again** or the file changed | Nothing |

## The dash in a cell { #dash }

A dash (**—**) means CueRithm has no value to show.

| Where | What the dash means | What to do |
|---|---|---|
| **BPM**, **KEY**, **ENERGY** | The measurement is not available, usually because the track has not finished preparing | Wait for the track to finish, or see [Read a track's key, BPM and energy](../analysis/read-key-bpm-energy.md) |
| **ARTIST**, **GENRE** | The tag is empty | [Edit tags and artwork](../tagging/edit-tags-by-hand.md) |
| **PREVIEW** | No waveform could be drawn. Hover over the dash to read why: the file was not found, the audio could not be read, the track's details could not be loaded, or the track has no waveform data. [What the dash says](../help/troubleshooting/no-waveform-or-playback.md#dash-messages) lists the four texts | [A track shows a dash or will not play](../help/troubleshooting/no-waveform-or-playback.md). If the file moved, see [I moved or renamed my files](../help/troubleshooting/files-moved.md). |

When a preview works, it shows the waveform with a small triangle at each cue and faint vertical lines every 16 bars. A blank bar means it is still loading. Hover to read the saved cues: "A · Start", "B · Drop" and so on. After you correct a track's cues the preview shows the corrected ones. A screen reader hears the count, for example "5 cues", and reads the dash's explanation when there is no preview. See [Cue names, letters and colours](cue-types.md).

## The header and the sidebar { #header }

| What you see | Where | Meaning |
|---|---|---|
| **Preparing** *N* **tracks** (or **track**), with a turning circle | Header | *N* jobs are queued or running |
| *N* **need attention** | Header, and a link at the top of Tracks and Archive | *N* tracks are waiting in Incoming for you. The link opens Incoming. |
| **All up to date** with a dot | Header | Nothing is queued or needs attention |
| **Opening library…** | Header, at startup | CueRithm is starting |
| **Library unavailable** and **Try again** | Header and a red banner | The library could not be opened. Press **Try again**. |
| A number beside **Tracks**, **Incoming**, **Archive** | Sidebar | How many tracks are in that view. The Incoming number is highlighted when it is above zero. |
| **Offline audio analysis** | Footer of the library | Analysis and cue generation run on your computer; only catalog tagging uses the internet |
| A coloured dot beside a playlist | Sidebar | The smart playlist is not up to date: pending, stale or error. Hover for the message. See [Playlist status](#playlist-state). |

## Playlist status { #playlist-state }

| Message above the table | Meaning | What to do |
|---|---|---|
| **Evaluating playlist…** | A smart playlist is being worked out for the first time | Wait a moment |
| **Updating playlist…** | The library changed and the playlist is catching up | Wait |
| The error text, in red | The rule failed, for example a too-complex regular expression | Fix the rule: [Smart playlist fields and operators](smart-playlist-fields.md#cmp-regex). CueRithm keeps showing "the last successfully evaluated results" meanwhile. |
| "No saved results are available yet." | The playlist has not worked once yet | Fix the error above it |

## File organization history { #organize-states }

The Organize files tool lists each batch under **History and recovery** with one of these states.

| State | Meaning | Next |
|---|---|---|
| applying | Files are being renamed, moved or copied | Wait, or **Cancel operation** |
| complete | Every file was handled | Nothing, or **Undo** |
| partial | Some files were handled, others were blocked or failed | Open **Details**; see [Recover or undo a reorganisation](../rename/undo-organize.md) |
| cancelled | You stopped it | Review **Details** |
| recovery required | An interruption left work to finish safely | **Retry recovery** or **Retry in Incoming** |
| undone | The batch was reversed | Nothing |

## Related

- [Follow a track through Incoming](../files/follow-incoming.md)
- [Fix a track that needs attention](../files/needs-attention.md)
- [Fix a problem](../help/troubleshooting/index.md)
- [Find a track: search, sort and filter](../analysis/find-tracks.md)
- [Glossary](glossary.md)

<!-- verified: apps/desktop/src/components/TrackTable.tsx (status(), stages map, row buttons, cell dashes, menu items); apps/desktop/src/components/RowPreview.tsx (dash conditions, tooltip, aria-label, 16-bar grid); apps/desktop/src/App.tsx (header indicator text, attention link, view subtitles, playlist status banners, footer); apps/desktop/src/components/Sidebar.tsx (counts, playlist-state dot); crates/cuerithm-library/src/jobs.rs (states queued/running/attention/completed/superseded) and prepare.rs (stage order: setup, organizing, waiting, decoding, analysis, matching, tagging, cleanup, writing_tags, committing, organizing); apps/desktop/src/views/Organize.tsx and organization-types.ts (batch states, history actions); legacy guides/track-table.md and guides/incoming-archive.md re-checked. The "cleanup" stage has the label "Cleaning up tags" in the stages map. The labels "Repairing linked references" and the stage name "organization" exist in the UI map but no code path sets them, so they are not documented. UNVERIFIED in the running app: the exact moment each stage appears. -->
