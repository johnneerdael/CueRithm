# Analyze tracks without tagging or cleanup

<p class="lead">Get key, BPM, beatgrid, energy and cues for your tracks while CueRithm leaves every file and tag untouched. Switch three settings off and add your files; it takes about two minutes.</p>

<div class="recipe-meta" markdown>
**Uses:** Settings → Automatic tagging, Cleanup and File organization switched off.
**Works on:** Any supported audio files, with no internet connection.
**Time:** About two minutes to set up.
**Undo:** Nothing to undo: your files and their tags are never written. Switch tagging back on and use [Analyze again](../../analysis/analyze-again.md) when you want it.
</div>

## The problem

You want key, BPM, beatgrid, energy and cues for your tracks, but you do not want CueRithm to look anything up online, change a tag, tidy a title or rename a file. Your files should stay byte for byte as they are.

| | Before | After |
|---|---|---|
| File | `Man Like Me (Extended Mix)`, FLAC | The same file, untouched |
| Tags | As they were | As they were |
| Key, BPM, energy | None in CueRithm | <span class="cr-key">4A</span> <span class="cr-bpm">128</span> <span class="cr-energy">7/10</span> |
| Cues | None | Start 0:00, Drop 1:00, Breakdown 1:53, Second Drop 3:15, Last beat 5:15 |
| Network use | | No catalog lookups |

## The solution

1. Choose **Settings**, then **Automatic tagging**. Switch **Automatic tagging** off.
2. Open **Cleanup**. Switch **Clean incoming tracks automatically** off.
3. Open **File organization**. Set **When preparation finishes** to *Leave files in place*.
4. Select **Save settings** and wait for *Settings saved*.
5. Add your files with [Add tracks, by dropping, or with a watch folder](../add-tracks.md).

```text
Settings → Automatic tagging
  Automatic tagging:          off
Settings → Cleanup
  Clean incoming tracks automatically:   off
Settings → File organization
  When preparation finishes:  Leave files in place
```

Open **Incoming** to watch. Each row goes Queued, Waiting for file, Decoding, Analyzing, Applying cues. There is no *Matching catalog*, *Preparing tags* or *Writing tags* stage. When a track finishes it appears in Tracks with its key, BPM, energy and cues, and its title, artist and genre as they already were in the file.

To check that nothing was written, look at the file's modified date in your file manager. It is the date you last changed the file yourself.

## How it works

Preparation takes one of two paths. If tagging, cleanup or file organization is on, CueRithm builds a tag plan, writes it to a copy and publishes the result. If all three are off, it only reads the file: the bundled decoder makes a private working copy, the analysis runs on that, and the results are saved in CueRithm's library. The existing title, artist, album and genre are read from the file for display and for the genre-aware part of [cue placement](../../analysis/cue-rules.md#genre).

Analysis and cue generation work offline. A catalog lookup is the only step that needs the internet, and it is off.

## Variations

- **Analysis only for one folder, tagging for the rest.** Folder assignments are saved together with the Settings draft, so do this in order: (1) switch the three settings off but do not save; (2) under **Preparation presets**, create a preset named `Analyze only` ([Save your setup as a preparation preset](../presets.md)); (3) select **Library** at the top left of Settings to leave without saving, then reopen Settings, which restores your saved global settings; (4) on the **Incoming** page, choose `Analyze only` for the folder and select **Save settings** ([Use different rules for different folders](../folder-presets.md)).
- **Measure tracks you already have.** Select them and choose [Analyze again](../../analysis/analyze-again.md) after switching the three settings off. Analyze again uses today's settings.
- **Cue rules only.** Leave tagging as it is and change only [the cue rules](../../analysis/cue-rules.md).
- **Add tags later.** Switch **Automatic tagging** on, save, and run [Tag new tracks from Beatport and Traxsource](../../tagging/tag-new-tracks-automatically.md); finished tracks are not retagged automatically.

## Related

- [Choose how CueRithm places cues](../../analysis/cue-rules.md)
- [Follow a track through Incoming](../follow-incoming.md)
- [Read a track's key, BPM and energy](../../analysis/read-key-bpm-energy.md)
- [Prepare a new Beatport purchase folder](prepare-a-purchase-folder.md)

<!-- verified: apps/desktop/src/views/TaggingSettings.tsx (Automatic tagging switch and its note: analysis and cue generation work offline); apps/desktop/src/views/CleanupSettings.tsx (Clean incoming tracks automatically); apps/desktop/src/views/OrganizationSettings.tsx (Leave files in place); crates/cuerithm-library/src/prepare.rs (process: without autotag_enabled, cleanup_enabled, editor override or review the committing branch only reads native metadata and stores results; organization stage only when enabled); crates/cuerithm-library/src/settings.rs (defaults: tagging, cleanup off); apps/desktop/src/components/TrackTable.tsx (stage labels); docs/user-guide/_meta/the private demo-library facts sheet for Man Like Me (Extended Mix). Code-read, not run: the modified-date check. -->
