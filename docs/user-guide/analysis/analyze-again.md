# Analyze a track again

<p class="lead">A finished track keeps its results until you ask for new ones. Analyze again measures key, BPM, beatgrid, energy and cues from scratch, using your current settings. It takes a few seconds per track plus any tagging you have switched on.</p>

**Before you start:** decide whether you want tagging, cleanup and file organization to run too. Analyze again runs whatever is switched on. Read [What also runs](#what-also-runs) first if you use automatic tagging.

## When to use it { #when }

| Situation | Why Analyze again helps |
|---|---|
| You changed [the cue rules](cue-rules.md) | Finished tracks keep their old cues until they are analysed again |
| You replaced the audio file with a better version at the same location | The new audio gets its own measurements |
| An update brought new analysis | Completed results change only on request |
| A track's genre changed | Cues are recalculated with the genre the file now carries |
| A result looks wrong and you want a clean second measurement | The saved analysis for the file is discarded, so nothing is reused |

## 1. Select the tracks { #select }

Open **Tracks**, a playlist or **Incoming**, and select one or more tracks. Press ++cmd+a++ / ++ctrl+a++ to select everything in the current list; see [Work on many tracks at once](select-many.md).

## 2. Choose Analyze again { #choose }

- With the selection made, a toolbar appears above the table. Select **Analyze again**.
- For a single track, select the **⋯** button at the end of its row and choose **Analyze again**.

There is no confirmation. The tracks go back to **Incoming**, the header reads *Preparing 3 tracks*, and each track follows the stages in [Follow a track through Incoming](../files/follow-incoming.md). When a track finishes, it returns to Tracks.

## 3. Check the result { #check }

Open the track in the player and compare. Key, BPM and energy are in the table; cues appear on the waveform. See [Read a track's key, BPM and energy](read-key-bpm-energy.md) and [Jump to the drop with cues](jump-to-cues.md).

## What changes and what stays { #changes }

| Replaced | Stays |
|---|---|
| Key, BPM and beatgrid | The track's place in your playlists |
| Whole-track energy rating | Categories, rating and notes from Curate |
| All cues, including any you placed or moved by hand, and the S.O.S. loop if enabled | The audio file, unless tagging, cleanup or file organization are on |
| Title, artist, album and genre shown in the library, re-read from the file | The track's date added |

The new cues replace the old ones only when the analysis succeeds. If it fails, the track stops with a message and keeps its previous results. See [Fix a track that needs attention](../files/needs-attention.md).

## What also runs { #what-also-runs }

Analyze again prepares the track like a new one, with the rules that apply today for its folder. That means:

- **Automatic tagging** looks the track up again and can write catalog tags to the file. Switch on **Skip previously tagged catalog lookups** (Settings → Automatic tagging → Matching) to skip tracks that CueRithm or OneTagger already tagged. Analysis and new cues still run.
- **Cleanup** applies your rules again if **Clean incoming tracks automatically** is on.
- **File organization** renames, moves or copies the file if a rule is set.

To measure without any of that, follow [Analyze tracks without tagging or cleanup](../files/recipes/analyze-only.md). To keep a different setup for one folder, use a [folder preset](../files/folder-presets.md); Analyze again then uses that folder's rules.

!!! warning "Hand-corrected cues are replaced"
    A track you corrected by hand gets fresh automatic cues. If the corrections matter, note them or save them as a [cue template](corrections/cue-templates.md) before you analyse again. A correction made earlier can no longer be undone once the track has changed.

## What can go wrong { #what-can-go-wrong }

| What you see | Why | What to do |
|---|---|---|
| A message *Restore archived tracks before preparing them* | An archived track was in the selection | [Restore it from the Archive](../files/archive.md#restore), then try again |
| Only some of many selected tracks went to Incoming | Analyze again stops at the first track it cannot queue and shows why | Fix that track, then select the rest again |
| The cues did not change | The file and the rules are the same as before, so the same cues are expected | Change [the cue rules](cue-rules.md) first |
| Tags changed unexpectedly | Automatic tagging was on | Read [What also runs](#what-also-runs); undo with [Undo a tag change](../tagging/tag-history.md) |
| A row stays in Incoming with a message | A stage failed | [Fix a track that needs attention](../files/needs-attention.md) |

## Related

- [Choose how CueRithm places cues](cue-rules.md)
- [Analyze tracks without tagging or cleanup](../files/recipes/analyze-only.md)
- [Use different rules for different folders](../files/folder-presets.md)
- [How does CueRithm analyse a track?](../how-it-works/pipeline.md)

<!-- verified: apps/desktop/src/App.tsx (selection toolbar Analyze again, loop over reprepareTrack, no confirmation, ctrl/cmd+A select all); apps/desktop/src/components/TrackTable.tsx (row menu Analyze again, Archive track / Restore to library); crates/cuerithm-library/src/prepare.rs (reprepare: clears saved analysis for the file, enqueues with current rules, retry); crates/cuerithm-library/src/jobs.rs (retry: Restore archived tracks before preparing them; store_analysis_tx replaces analysis, inserts a new cue revision, re-reads title/artist/album/genre; enqueue_one keeps the track id); crates/cuerithm-library/src/analysis_cache.rs (clear_track_analysis_cache); docs/automatic-tagging.md (Analyze again jobs use the tagging config); apps/desktop/src/views/TaggingSettings.tsx (Skip previously tagged catalog lookups note); crates/cuerithm-library/src/manual_publication.rs ('Track changed after this correction; Undo cannot replace newer work'). Code-read, not run: playlists, categories, ratings and notes staying with the track is inferred from the stable track id. -->
