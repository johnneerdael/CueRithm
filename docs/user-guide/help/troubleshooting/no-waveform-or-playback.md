# A track shows a dash instead of a waveform, or will not play

<p class="lead">A dash in the PREVIEW column and a player that refuses to load almost always have one cause: CueRithm cannot read the track's audio file. This page finds the cause from the message on screen and fixes it, usually in about five minutes.</p>

<!-- figure:existing-help-preview-dash-light-help-troubleshooting-no-waveform-or-playback -->
<figure markdown>
![The Tracks table with a dash in the PREVIEW column for tracks whose files are missing, next to their working duplicates](../../assets/screens/help-preview-dash-light.webp#only-light){ .screen loading=lazy }
![The Tracks table with a dash in the PREVIEW column for tracks whose files are missing, next to their working duplicates](../../assets/screens/help-preview-dash-dark.webp#only-dark){ .screen loading=lazy }
<figcaption>A dash in PREVIEW means CueRithm cannot load the file at its stored location.</figcaption>
</figure>
<!-- /figure -->

## Why it happens { #why }

The **PREVIEW** column draws a small waveform with cue markers for each row. To draw it, CueRithm decodes the audio file at the location stored in the library (see [where CueRithm keeps your data](../../files/library/where-data-lives.md)). It does this only for rows that are on screen, after a short pause, two at a time. A grey bar while a row waits is normal. The dash appears when the row has nothing to draw. Hover over it, or let a screen reader read it, for the reason.

### What the dash says { #dash-messages }

| Hover text | What it means | What to do |
|---|---|---|
| *File not found - move it back or run Maintenance* | CueRithm looked at the stored location and the file is not there. It does not try to decode it. | [Fix it](#fix), step 2 |
| *No preview: the audio file could not be read. It may be missing, moved or in an unsupported format.* | The file was found, but building the waveform from the audio failed. Double-click the row to see the exact reason. | [Fix it](#fix), steps 2 to 5 |
| *No preview: it could not be loaded.* | The library did not return this track's details, so nothing is known about the file. This is not a sign that the file is gone. | Scroll a screenful away and back, or restart CueRithm. If it stays, [report the problem](../report-a-problem.md). |
| *No preview: this track has no waveform data.* | The audio loaded but held nothing to draw. | Try the file in another player, then see step 3 of [Fix it](#fix). |

Pressing Play runs the same decoding step, so whatever stops the preview also stops playback. The message tells you which cause you have:

| Message | Cause | Where you see it |
|---|---|---|
| `File not found - move it back or run Maintenance` | CueRithm checked the stored location and the file is not there. You moved, renamed or deleted it, or its drive is disconnected. | Message at the bottom of the window, and the hover text of the dash |
| `Audio file is missing` | CueRithm could not open the file at the stored location, but could not confirm that it is gone either: for example a drive that is reachable but unreadable, or a file that vanished a moment ago. | Message at the bottom of the window |
| `Audio decoder failed: …` | The bundled decoder cannot read the file: a damaged download, a protected file, or an extension that does not match the content. | Message at the bottom of the window |
| `Source changed during decode` or `Source changed during capture` | Another program was still writing to the file. | Message at the bottom of the window |
| `Media scan exceeded its output limit` | The track is longer than about 24 minutes. | Message at the bottom of the window |
| `No audio stream found` or `Unsupported audio rate/channel layout` | The file has no audio CueRithm can use. | Message at the bottom of the window |
| `Audio output unavailable: …` or `Audio output failed: …` | The waveform is fine, but the computer's default output device is missing or stopped. | Red text at the right end of the cue strip under the waveform |

The messages continue with the system's own explanation after the colon. Copy it if you [report the problem](../report-a-problem.md).

A dash in the **KEY**, **BPM** or **ENERGY** columns is a different symptom: the track has no saved analysis yet. Look in Incoming and read [A track is stuck in Incoming](incoming-stuck.md).

## Fix it { #fix }

Start by reading the message for your track.

1. Double-click the row. The player shows **Loading track…**, then either plays or leaves a message at the bottom of the window. Dismiss the message with its close button once you have read it.
2. **`File not found - move it back or run Maintenance` or `Audio file is missing`.** Show the **Location** column ([choose the columns you need](../../analysis/choose-columns.md)) and open that folder in Finder, Explorer or your file manager.
    - If a drive is disconnected, reconnect it.
    - If you moved or renamed the file, [review Relocate](../../files/library/find-missing-files.md#relocate) to reconnect it, or move it back to exactly that location. [I moved or renamed my files](files-moved.md) covers the whole situation, including tracks that were added twice.
    - To see every affected track at once, choose **Maintenance** in the sidebar, open **Missing & broken** and choose **Scan library files**. [Find missing, moved or broken files](../../files/library/find-missing-files.md) explains the report.
    - Do not use **Analyze again** on a missing file. It fails with `Incoming file is missing`.
3. **`Audio decoder failed: …` or `No audio stream found`.** Try the file in another player. If that fails too, the file is damaged: put a good copy at the same location, then choose **Analyze again** from the row's actions menu ([Analyze a track again](../../analysis/analyze-again.md)). Check the format against [Check that your files are supported](../../files/supported-formats.md).
4. **`Source changed …`.** Wait until the copy or edit has finished, then double-click the row again.
5. **`Media scan exceeded its output limit`.** Long mixes do not decode. There is no setting to raise the limit. Split the mix into shorter files ([what CueRithm does not do yet](../not-available-yet.md#long-tracks)).
6. **`Audio output …`.** CueRithm plays through the system's default output and has no device picker ([what CueRithm does not do yet](../not-available-yet.md#no-device-picker)). Choose the right output in your system's sound settings, reconnect the headphones or interface, then press Play again.

## Check it worked { #check }

- Scroll the row into view. After a moment, **PREVIEW** shows a waveform with small cue markers. Hover over it to see the cue list.
- Double-click the row. The title area shows the track, the clock counts up and the waveform area reads **BEATGRID & CUES**. [Preview a track with the player](../../analysis/preview-a-track.md) shows what each part does.
- If you scanned for missing files, the report reads **0 files need attention** and *No files need attention in this report.*

## Still stuck? { #still-stuck }

If the file plays in other apps and the message is not in the table, [report the problem](../report-a-problem.md) with the exact message and the file type. Do not attach the audio file itself.

## Related

- [I moved or renamed my files](files-moved.md)
- [A track is stuck in Incoming](incoming-stuck.md)
- [Preview a track with the player](../../analysis/preview-a-track.md)
- [Find missing, moved or broken files](../../files/library/find-missing-files.md)

<!-- verified: apps/desktop/src/components/RowPreview.tsx (dash texts: FILE_NOT_FOUND, audio, request, no waveform data; 160 ms settle, two-at-a-time queue); apps/desktop/src/App.tsx (previewLoader: track lookup first, file_available false -> FileNotFoundError, then the waveform); apps/desktop/src-tauri host.rs (file_available, source_path FILE_NOT_FOUND) and crates/cuerithm-library/src/store.rs (track_file_missing: try_exists is Ok(false)); TrackTable.tsx (default PREVIEW column, Location label via columns.ts, row menu Analyze again); App.tsx (load -> notify toast, playerLoad, "Loading track…"); PlayerPanel.tsx (player-error in cue strip, BEATGRID & CUES); crates/cuerithm-media/src/decode.rs ("Audio file is missing", "Source changed during decode/capture"), probe.rs ("No audio stream found", "Unsupported audio rate/channel layout"), runtime.rs/cancel.rs ("Audio decoder failed", "Media scan exceeded its output limit", 64_000_000*8 byte cap); crates/cuerithm-library/src/playback.rs ("Audio output unavailable/failed", default device only); prepare.rs enqueue_paths ("Incoming file is missing" from jobs.rs canonicalize); MaintenanceBroken.tsx (labels, "No files need attention in this report."); legacy technical/decoding.md re-checked -->
